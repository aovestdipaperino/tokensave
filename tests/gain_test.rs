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
async fn savings_compound_index_upgrades_existing_ledger_and_preserves_boundaries() {
    let tmp = TempDir::new().unwrap();
    let path = tokensave::config::get_tokensave_dir(tmp.path()).join("global.db");
    {
        let db = open_isolated_db(&tmp).await;
        for (project, before, after, ts) in [
            ("/a", 100, 10, 99),
            ("/a", 200, 20, 100),
            ("/a", 10, 20, 101),
            ("/b", 1000, 0, 100),
        ] {
            db.record_savings(project, "test", before, after, ts).await;
        }
    }
    let raw = libsql::Builder::new_local(&path).build().await.unwrap();
    let conn = raw.connect().unwrap();
    conn.execute("DROP INDEX idx_savings_ledger_project_ts", ())
        .await
        .unwrap();
    drop(conn);
    drop(raw);
    let db = open_isolated_db(&tmp).await;
    let total = db.sum_savings(Some("/a"), 100).await;
    assert_eq!((total.saved_tokens, total.calls), (180, 2));
    let history = db.savings_history(Some("/a"), 100).await;
    assert_eq!(history.len(), 1);
    assert_eq!((history[0].saved_tokens, history[0].calls), (180, 2));
    assert_eq!(db.sum_savings(None, 100).await.saved_tokens, 1180);
    let raw = libsql::Builder::new_local(&path).build().await.unwrap();
    let conn = raw.connect().unwrap();
    let mut rows = conn.query("EXPLAIN QUERY PLAN SELECT SUM(before_tokens-after_tokens) FROM savings_ledger WHERE project_path=?1 AND ts>=?2", libsql::params!["/a", 100]).await.unwrap();
    let mut indexed = false;
    while let Some(row) = rows.next().await.unwrap() {
        indexed |= row
            .get::<String>(3)
            .unwrap()
            .contains("idx_savings_ledger_project_ts");
    }
    assert!(indexed);
}
