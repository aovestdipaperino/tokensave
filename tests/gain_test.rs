use tempfile::TempDir;
use tokensave::global_db::GlobalDb;

async fn open_isolated_db(tmp: &TempDir) -> GlobalDb {
    let db_path = tokensave::config::get_tokensave_dir(tmp.path()).join("global.db");
    GlobalDb::open_at(&db_path).await.expect("global db open")
}

#[tokio::test]
async fn record_and_query_savings_round_trip() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let now: i64 = 1_715_000_000;
    db.record_savings("/proj/a", "tokensave_context", 10_000, 500, now)
        .await;
    db.record_savings("/proj/a", "tokensave_search", 2_000, 100, now + 60)
        .await;
    db.record_savings("/proj/b", "tokensave_context", 5_000, 250, now + 120)
        .await;

    let total_a = db.sum_savings(Some("/proj/a"), 0).await;
    assert_eq!(total_a.saved_tokens, 11_400);
    assert_eq!(total_a.calls, 2);

    let total_all = db.sum_savings(None, 0).await;
    assert_eq!(total_all.saved_tokens, 16_150);
    assert_eq!(total_all.calls, 3);

    // Range filter: only entries after now+90 -> only the third one
    let recent = db.sum_savings(None, now + 90).await;
    assert_eq!(recent.calls, 1);
    assert_eq!(recent.saved_tokens, 4_750);
}

#[tokio::test]
async fn savings_history_buckets_by_day() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    // day1 = arbitrary epoch second; day2 = day1 + 86400 + 60s (crosses a UTC midnight)
    let day1 = 1_715_000_000;
    let day2 = day1 + 86_400 + 60;
    db.record_savings("/proj/a", "tokensave_context", 1000, 100, day1)
        .await;
    db.record_savings("/proj/a", "tokensave_context", 500, 50, day1 + 3600)
        .await;
    db.record_savings("/proj/a", "tokensave_context", 800, 80, day2)
        .await;

    let history = db.savings_history(None, 0).await;
    assert_eq!(history.len(), 2);
    // Newest first
    assert_eq!(history[0].saved_tokens, 720); // day2: 800 - 80
    assert_eq!(history[1].saved_tokens, 1350); // day1: (1000-100) + (500-50)
}

#[tokio::test]
async fn savings_by_tool_groups_counts_and_sums() {
    let tmp = TempDir::new().unwrap();
    let db = open_isolated_db(&tmp).await;

    let now: i64 = 1_715_000_000;
    db.record_savings("/proj/a", "tokensave_context", 10_000, 500, now)
        .await;
    db.record_savings("/proj/a", "tokensave_context", 4_000, 1_000, now + 10)
        .await;
    db.record_savings("/proj/a", "tokensave_search", 2_000, 100, now + 20)
        .await;
    // after > before counts as a call that saved nothing, as in `sum_savings`.
    db.record_savings("/proj/a", "tokensave_status", 100, 300, now + 30)
        .await;
    db.record_savings("/proj/b", "tokensave_search", 5_000, 250, now + 40)
        .await;

    let a = db.savings_by_tool(Some("/proj/a"), 0).await;
    let a: Vec<_> = a
        .iter()
        .map(|t| (t.tool.as_str(), t.calls, t.saved_tokens))
        .collect();
    // Most-called first, ties by name.
    assert_eq!(
        a,
        [
            ("tokensave_context", 2, 12_500),
            ("tokensave_search", 1, 1_900),
            ("tokensave_status", 1, 0),
        ]
    );

    let all = db.savings_by_tool(None, 0).await;
    let all: Vec<_> = all
        .iter()
        .map(|t| (t.tool.as_str(), t.calls, t.saved_tokens))
        .collect();
    assert_eq!(
        all,
        [
            ("tokensave_context", 2, 12_500),
            ("tokensave_search", 2, 6_650),
            ("tokensave_status", 1, 0),
        ]
    );

    // The per-tool rows add up to the total.
    let total = db.sum_savings(None, 0).await;
    let calls: u64 = db
        .savings_by_tool(None, 0)
        .await
        .iter()
        .map(|t| t.calls)
        .sum();
    assert_eq!(calls, total.calls);

    let recent = db.savings_by_tool(None, now + 25).await;
    assert_eq!(recent.len(), 2);
    assert!(db.savings_by_tool(Some("/proj/none"), 0).await.is_empty());
}

/// A home whose global database holds a fixed ledger, and pricing that is
/// never refreshed, so `tokensave gain` reads only what the test wrote.
async fn seeded_home() -> TempDir {
    let home = TempDir::new().expect("temp home");
    let db_path = tokensave::config::get_tokensave_dir(home.path()).join("global.db");
    let db = GlobalDb::open_at(&db_path).await.expect("global db open");
    let now = tokensave::tokensave::current_timestamp();
    db.record_savings("/proj/a", "tokensave_context", 10_000, 500, now)
        .await;
    db.record_savings("/proj/a", "tokensave_context", 4_000, 1_000, now)
        .await;
    db.record_savings("/proj/b", "tokensave_search", 2_000, 100, now)
        .await;
    drop(db);
    std::fs::write(
        home.path().join(".tokensave").join("config.toml"),
        "last_pricing_fetch_at = 4102444800\n",
    )
    .expect("config");
    home
}

fn gain(home: &TempDir, args: &[&str]) -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("TOKENSAVE_UPDATE_CHECK", "off")
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .arg("gain")
        .args(args)
        .output()
        .expect("run tokensave gain");
    assert!(
        output.status.success(),
        "gain {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8 stdout")
}

#[tokio::test]
async fn gain_by_tool_json_lists_each_tool() {
    let home = seeded_home().await;
    let out = gain(&home, &["--all", "--range", "all", "--by-tool", "--json"]);
    let rows: serde_json::Value = serde_json::from_str(&out).expect("json");
    assert_eq!(
        rows,
        serde_json::json!([
            { "tool": "tokensave_context", "calls": 2, "saved_tokens": 12_500 },
            { "tool": "tokensave_search", "calls": 1, "saved_tokens": 1_900 },
        ])
    );
}

/// #918: `--by-tool` is a new view; the existing `gain --json` and
/// `gain --history --json` output keeps its fields and their order.
#[tokio::test]
async fn gain_json_without_by_tool_keeps_its_shape() {
    let home = seeded_home().await;

    let out = gain(&home, &["--all", "--range", "all", "--json"]);
    let total: serde_json::Value = serde_json::from_str(&out).expect("json");
    let usd = total["usd"].as_f64().expect("usd");
    let expected = serde_json::to_string_pretty(&serde_json::json!({
        "range": "all",
        "project": "ALL",
        "saved_tokens": 14_400,
        "calls": 3,
        "usd": usd,
    }))
    .expect("serialize");
    assert_eq!(out, format!("{expected}\n"));

    let out = gain(&home, &["--all", "--range", "all", "--history", "--json"]);
    let days: serde_json::Value = serde_json::from_str(&out).expect("json");
    let day = &days[0];
    let expected = serde_json::to_string_pretty(&serde_json::json!([{
        "day": day["day"],
        "saved_tokens": 14_400,
        "calls": 3,
        "usd": day["usd"],
    }]))
    .expect("serialize");
    assert_eq!(out, format!("{expected}\n"));
}
