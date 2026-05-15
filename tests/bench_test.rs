use tempfile::TempDir;
use tokensave::bench::{run_bench, BenchOptions, OutputFormat};
use tokensave::tokensave::TokenSave;

#[tokio::test]
async fn bench_runs_and_returns_report() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("a.rs"),
        "pub fn hello() {}\npub fn world() {}\n",
    )
    .unwrap();
    let cg = TokenSave::init(tmp.path()).await.unwrap();

    let queries_path = tmp.path().join("q.toml");
    std::fs::write(
        &queries_path,
        r#"[[query]]
task = "where is hello defined"

[[query]]
task = "what does world do"
"#,
    )
    .unwrap();

    let report = run_bench(
        &cg,
        &queries_path,
        BenchOptions {
            format: OutputFormat::Json,
            max_nodes: 20,
        },
    )
    .await
    .expect("bench run");

    assert_eq!(report.results.len(), 2);
    assert_eq!(report.aggregate.queries, 2);
    for r in &report.results {
        assert!(r.baseline_tokens > 0, "baseline must be > 0 for query: {}", r.task);
    }
}
