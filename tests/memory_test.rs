use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

async fn make_project() -> (TempDir, TokenSave) {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "pub fn hello() {}").unwrap();
    let cg = TokenSave::init(tmp.path()).await.unwrap();
    (tmp, cg)
}

#[tokio::test]
async fn record_decision_persists_and_recalls() {
    let (_tmp, cg) = make_project().await;

    let id = cg
        .record_decision(
            "use JWT for auth",
            Some("session tokens flagged by legal"),
            &["src/auth.rs".to_string()],
            &["security".to_string(), "decision".to_string()],
        )
        .await
        .unwrap();
    assert!(id > 0);

    let hits = cg.session_recall(Some("JWT"), None, 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].text, "use JWT for auth");
    assert_eq!(
        hits[0].reason.as_deref(),
        Some("session tokens flagged by legal")
    );
    assert_eq!(hits[0].files, vec!["src/auth.rs"]);
    assert_eq!(hits[0].tags, vec!["security", "decision"]);
}

#[tokio::test]
async fn session_recall_orders_newest_first_when_no_query() {
    let (_tmp, cg) = make_project().await;

    cg.record_decision("first", None, &[], &[]).await.unwrap();
    // current_timestamp() is second-granularity, so we need a >1s gap to guarantee
    // the two decisions have distinct created_at values for a deterministic ordering.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    cg.record_decision("second", None, &[], &[]).await.unwrap();

    let hits = cg.session_recall(None, None, 10).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].text, "second");
    assert_eq!(hits[1].text, "first");
}

#[tokio::test]
async fn record_code_area_upserts_touch_count() {
    let (_tmp, cg) = make_project().await;

    cg.record_code_area("src/auth.rs", Some("OAuth provider"))
        .await
        .unwrap();
    cg.record_code_area("src/auth.rs", None).await.unwrap();
    cg.record_code_area("src/auth.rs", None).await.unwrap();

    let areas = cg.list_code_areas(10).await.unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0].path, "src/auth.rs");
    assert_eq!(areas[0].touch_count, 3);
    assert_eq!(areas[0].description.as_deref(), Some("OAuth provider"));
}

/// Insert a decision with an explicit `created_at`, bypassing
/// `record_decision`'s `now()` so tests can simulate old vs. recent memories.
async fn insert_decision_at(cg: &TokenSave, text: &str, created_at: i64) {
    cg.db()
        .conn()
        .execute(
            "INSERT INTO memory_decisions (text, reason, created_at, files, tags) \
             VALUES (?1, NULL, ?2, '[]', '[]')",
            libsql::params![text, created_at],
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn session_recall_ranks_by_recency_decay_without_dropping_old() {
    let (_tmp, cg) = make_project().await;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // ~6 months old: heavily decayed but must still surface (no TTL).
    let very_old = now - 180 * 24 * 60 * 60;
    let recent = now - 60; // a minute ago

    insert_decision_at(&cg, "ancient decision", very_old).await;
    insert_decision_at(&cg, "fresh decision", recent).await;

    let hits = cg.session_recall(None, None, 10).await.unwrap();
    // Both are returned — the old one is NOT dropped.
    assert_eq!(hits.len(), 2);
    let texts: Vec<&str> = hits.iter().map(|d| d.text.as_str()).collect();
    assert!(texts.contains(&"ancient decision"));
    assert!(texts.contains(&"fresh decision"));
    // Recent ranks above old by decay weight.
    assert_eq!(hits[0].text, "fresh decision");
    assert_eq!(hits[1].text, "ancient decision");
}

#[tokio::test]
async fn session_delta_is_compact_and_budget_bounded() {
    let (_tmp, cg) = make_project().await;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Record more decisions than the delta budget allows.
    for i in 0..12 {
        insert_decision_at(&cg, &format!("decision number {i}"), now - i64::from(i)).await;
    }
    // And several touched code areas.
    for i in 0..8 {
        cg.record_code_area(&format!("src/area_{i}.rs"), None)
            .await
            .unwrap();
    }

    let delta = cg.session_delta().await.unwrap();

    // Entry counts are capped to the budget.
    assert!(
        delta.recent_decisions.len() <= 5,
        "decisions capped, got {}",
        delta.recent_decisions.len()
    );
    assert!(
        delta.recent_code_areas.len() <= 5,
        "code areas capped, got {}",
        delta.recent_code_areas.len()
    );
    // Content is present and non-empty.
    assert!(!delta.recent_decisions.is_empty());
    assert!(!delta.recent_code_areas.is_empty());
    assert!(!delta.recent_decisions[0].summary.is_empty());
    // The most recent decision (created_at == now) ranks first.
    assert_eq!(delta.recent_decisions[0].summary, "decision number 0");
}

#[tokio::test]
async fn session_delta_truncates_long_text() {
    let (_tmp, cg) = make_project().await;

    let long = "x".repeat(500);
    cg.record_decision(&long, None, &[], &[]).await.unwrap();

    let delta = cg.session_delta().await.unwrap();
    assert_eq!(delta.recent_decisions.len(), 1);
    // Truncated to the 120-char budget plus an ellipsis.
    let summary = &delta.recent_decisions[0].summary;
    assert!(summary.ends_with('…'));
    assert!(
        summary.chars().count() <= 121,
        "summary too long: {} chars",
        summary.chars().count()
    );
}

#[tokio::test]
async fn session_recall_filters_by_since() {
    let (_tmp, cg) = make_project().await;
    cg.record_decision("old decision", None, &[], &[])
        .await
        .unwrap();
    // Force a > 1s gap so created_at values differ deterministically.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // Force a > 1s gap on the new side too, otherwise the new record could share
    // its created_at with `cutoff` (second-granularity).
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    cg.record_decision("new decision", None, &[], &[])
        .await
        .unwrap();

    let hits = cg.session_recall(None, Some(cutoff), 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].text, "new decision");
}
