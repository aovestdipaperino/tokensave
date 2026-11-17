use crate::common::stability::Project;
use serde_json::Value;
use tokensave::graphify::export_file;

async fn fixture() -> Project {
    Project::new(&[
        (
            "a.rs",
            "/// private docs\npub fn alpha() {\n beta();\n beta();\n}\nfn beta() {}\n".into(),
        ),
        ("b.rs", "fn other() {}\n".into()),
    ])
    .await
}

fn read(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[tokio::test]
async fn snapshot_preserves_internal_graph_and_refuses_overwrite() {
    let p = fixture().await;
    let out = p.dir.path().join("snapshot.json");
    export_file(p.dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .unwrap();
    let graph = read(&out);
    assert_eq!(graph["directed"], true);
    assert_eq!(graph["multigraph"], true);
    assert_eq!(graph["graph"]["truncated"], false);
    let nodes = graph["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());
    assert!(nodes.iter().all(|n| n["source_file"] == "a.rs"));
    assert!(!std::fs::read_to_string(&out)
        .unwrap()
        .contains("private docs"));
    let links = graph["links"].as_array().unwrap();
    assert!(links.iter().any(|e| e["relation"] == "calls"));
    for e in links {
        assert!(nodes.iter().any(|n| n["id"] == e["source"]));
        assert!(nodes.iter().any(|n| n["id"] == e["target"]));
        assert!(e.get("tokensave_resolved_by").is_some());
        assert_ne!(e["confidence"], "EXTRACTED");
    }
    let expected_edges =
        p.cg.get_all_edges()
            .await
            .unwrap()
            .into_iter()
            .filter(|e| {
                nodes.iter().any(|n| {
                    n["id"]
                        .as_str()
                        .unwrap()
                        .ends_with(&format!(":{}", e.source))
                }) && nodes.iter().any(|n| {
                    n["id"]
                        .as_str()
                        .unwrap()
                        .ends_with(&format!(":{}", e.target))
                })
            })
            .count();
    assert_eq!(links.len(), expected_edges);
    let bytes = std::fs::read(&out).unwrap();
    assert!(export_file(p.dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .is_err());
    assert_eq!(bytes, std::fs::read(&out).unwrap());
    let second = p.dir.path().join("second.json");
    export_file(p.dir.path(), "a.rs", &second, 1000, 5000)
        .await
        .unwrap();
    assert_eq!(bytes, std::fs::read(&second).unwrap());
}

#[tokio::test]
async fn limits_and_stale_inputs_publish_nothing() {
    let p = fixture().await;
    let out = p.dir.path().join("snapshot.json");
    assert!(export_file(p.dir.path(), "a.rs", &out, 1, 5000)
        .await
        .is_err());
    assert!(!out.exists());
    assert!(export_file(p.dir.path(), "README.md", &out, 1000, 5000)
        .await
        .unwrap_err()
        .to_string()
        .contains("code-only"));
    assert!(export_file(p.dir.path(), "a.rs", &out, 0, 5000)
        .await
        .is_err());
    assert!(export_file(p.dir.path(), "../a.rs", &out, 1000, 5000)
        .await
        .is_err());
    std::fs::write(p.dir.path().join("a.rs"), "fn changed() {}\n").unwrap();
    assert!(export_file(p.dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .unwrap_err()
        .to_string()
        .contains("stale"));
    assert!(!out.exists());
    p.cg.sync().await.unwrap();
    export_file(p.dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .unwrap();
    assert!(read(&out)["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["label"].as_str().unwrap().contains("changed")));
}

#[tokio::test]
async fn separate_roots_never_share_snapshot_ids_and_deleted_files_fail() {
    let a = fixture().await;
    let b = fixture().await;
    let oa = a.dir.path().join("out.json");
    let ob = b.dir.path().join("out.json");
    export_file(a.dir.path(), "a.rs", &oa, 1000, 5000)
        .await
        .unwrap();
    export_file(b.dir.path(), "a.rs", &ob, 1000, 5000)
        .await
        .unwrap();
    assert_ne!(
        read(&oa)["graph"]["generation"],
        read(&ob)["graph"]["generation"]
    );
    std::fs::remove_file(a.dir.path().join("a.rs")).unwrap();
    assert!(export_file(
        a.dir.path(),
        "a.rs",
        &a.dir.path().join("deleted.json"),
        1000,
        5000
    )
    .await
    .is_err());
}

#[tokio::test]
async fn edge_budget_and_missing_parent_leave_no_partial_artifacts() {
    let p = fixture().await;
    let out = p.dir.path().join("out.json");
    assert!(export_file(p.dir.path(), "a.rs", &out, 1000, 1)
        .await
        .is_err());
    assert!(!out.exists());
    let missing = p.dir.path().join("missing").join("out.json");
    assert!(export_file(p.dir.path(), "a.rs", &missing, 1000, 5000)
        .await
        .is_err());
    assert!(!missing.exists());
    assert!(!std::fs::read_dir(p.dir.path()).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".tmp")));
}

#[tokio::test]
async fn untracked_checkout_refuses_default_branch_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .status()
            .unwrap()
            .success());
    };
    git(&["init", "-b", "main"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "--allow-empty",
        "-m",
        "fixture",
    ]);
    std::fs::write(dir.path().join("a.rs"), "fn alpha() {}\n").unwrap();
    let cg = tokensave::tokensave::TokenSave::init(dir.path())
        .await
        .unwrap();
    cg.index_all().await.unwrap();
    git(&["checkout", "-b", "untracked"]);
    let out = dir.path().join("out.json");
    assert!(export_file(dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .unwrap_err()
        .to_string()
        .contains("refusing fallback"));
    assert!(!out.exists());
}

#[tokio::test]
async fn oversized_database_text_is_refused_without_truncating_identity() {
    let p = fixture().await;
    p.cg.db()
        .conn()
        .execute(
            "UPDATE nodes SET qualified_name=?1 WHERE file_path='a.rs'",
            ["x".repeat(4097)],
        )
        .await
        .unwrap();
    let out = p.dir.path().join("oversized.json");
    let error = export_file(p.dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("4096"));
    assert!(!out.exists());
}

#[tokio::test]
async fn cancellation_isolated_in_child_publishes_nothing() {
    if std::env::var_os("TOKENSAVE_GRAPHIFY_CANCEL_TEST").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(crate::common::qualified_test_name(
                module_path!(),
                "cancellation_isolated_in_child_publishes_nothing",
            ))
            .env("TOKENSAVE_GRAPHIFY_CANCEL_TEST", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let p = fixture().await;
    let out = p.dir.path().join("cancelled.json");
    tokensave::cancel::request();
    assert!(export_file(p.dir.path(), "a.rs", &out, 1000, 5000)
        .await
        .unwrap_err()
        .to_string()
        .contains("cancelled"));
    assert!(!out.exists());
}
