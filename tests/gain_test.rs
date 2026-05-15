use tempfile::TempDir;
use tokensave::global_db::GlobalDb;

async fn open_isolated_db(tmp: &TempDir) -> GlobalDb {
    let db_path = tmp.path().join(".tokensave").join("global.db");
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
