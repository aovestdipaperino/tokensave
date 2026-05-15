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
    assert_eq!(hits[0].reason.as_deref(), Some("session tokens flagged by legal"));
    assert_eq!(hits[0].files, vec!["src/auth.rs"]);
    assert_eq!(hits[0].tags, vec!["security", "decision"]);
}

#[tokio::test]
async fn session_recall_orders_newest_first_when_no_query() {
    let (_tmp, cg) = make_project().await;

    cg.record_decision("first", None, &[], &[]).await.unwrap();
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

    cg.record_code_area("src/auth.rs", Some("OAuth provider")).await.unwrap();
    cg.record_code_area("src/auth.rs", None).await.unwrap();
    cg.record_code_area("src/auth.rs", None).await.unwrap();

    let areas = cg.list_code_areas(10).await.unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0].path, "src/auth.rs");
    assert_eq!(areas[0].touch_count, 3);
    assert_eq!(areas[0].description.as_deref(), Some("OAuth provider"));
}
