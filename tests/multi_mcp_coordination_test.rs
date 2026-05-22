use std::time::Duration;
use tempfile::tempdir;
use tokensave::mcp::McpServer;
use tokensave::tokensave::TokenSave;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_mcps_on_same_project_coordinate_via_sync_lock() {
    let tmp = tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    std::fs::write(project.join("a.rs"), "fn a() {}").unwrap();

    // Initial sync so both MCPs start with the same DB state.
    let cg_init = TokenSave::init(&project).await.unwrap();
    cg_init.sync().await.unwrap();
    drop(cg_init);

    // Spin up two MCP servers on the same project.
    let cg1 = TokenSave::open(&project).await.unwrap();
    let cg2 = TokenSave::open(&project).await.unwrap();
    let server1 = McpServer::new(cg1, None).await;
    let server2 = McpServer::new(cg2, None).await;

    let count_before = server1.file_token_map_snapshot().len();

    // Trigger a change that both watchers will pick up.
    std::fs::write(project.join("b.rs"), "fn b() {}").unwrap();

    // Wait for both servers' maps to grow. Bounded poll loop with 15s ceiling.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while (server1.file_token_map_snapshot().len() <= count_before
        || server2.file_token_map_snapshot().len() <= count_before)
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let count_after_1 = server1.file_token_map_snapshot().len();
    let count_after_2 = server2.file_token_map_snapshot().len();

    assert!(count_after_1 > count_before, "server1 saw new file");
    assert!(count_after_2 > count_before, "server2 saw new file");

    // Both maps should converge to the same state — the sync lock ensures
    // exactly one MCP wrote the DB, and both refresh from the same DB.
    assert_eq!(count_after_1, count_after_2);

    server1.shutdown().await;
    server2.shutdown().await;
}
