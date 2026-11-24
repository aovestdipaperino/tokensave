//! SCIP interoperability prototype (#1100): read-only comparison of the
//! committed rust-analyzer index with a freshly indexed copy of its fixture.
use crate::common::stability::Project;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokensave::scip::{compare, Options};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("scip")
        .join("ambiguous")
}

fn source(name: &str) -> String {
    std::fs::read_to_string(fixture_dir().join("src").join(name)).unwrap()
}

async fn project() -> Project {
    Project::new(&[
        ("src/lib.rs", source("lib.rs")),
        ("src/alpha.rs", source("alpha.rs")),
        ("src/beta.rs", source("beta.rs")),
    ])
    .await
}

fn options(root: &Path, index: PathBuf) -> Options {
    Options {
        root: root.to_path_buf(),
        index,
        rebase_root: true,
        max_index_bytes: 1024 * 1024,
        samples: 100,
    }
}

fn committed_index() -> PathBuf {
    fixture_dir().join("index.scip")
}

fn db_bytes(root: &Path) -> Vec<u8> {
    std::fs::read(root.join(".tokensave").join("tokensave.db")).unwrap()
}

fn sample<'a>(report: &'a Value, category: &str) -> &'a [Value] {
    report["samples"][category]
        .as_array()
        .map_or(&[], Vec::as_slice)
}

#[tokio::test]
async fn compares_fixture_and_maps_ambiguous_references() {
    let p = project().await;
    let before = db_bytes(p.dir.path());
    let report = compare(&options(p.dir.path(), committed_index()))
        .await
        .unwrap();
    assert_eq!(before, db_bytes(p.dir.path()), "comparison must not write");

    assert_eq!(report["mode"], "read_only_comparison");
    assert_eq!(report["index"]["tool"], "rust-analyzer");
    assert_eq!(report["index"]["project_root"], "rebased");
    assert_eq!(report["documents"]["compared"], 3);
    assert_eq!(report["proposed_import"]["applied"], false);
    // Only module namespaces lack a node of their own.
    assert_eq!(
        report["definitions"]["unmapped_by_descriptor"],
        serde_json::json!({"namespace": 3})
    );

    let edges = &report["tokensave_edges"]["classified"];
    let classified = ["confirmed", "contradicted", "unverifiable"]
        .iter()
        .map(|k| edges[k].as_u64().unwrap())
        .sum::<u64>();
    let all_edges = p.cg.get_all_edges().await.unwrap();
    assert_eq!(
        classified,
        all_edges
            .iter()
            .filter(|e| e.kind.as_str() != "contains" && e.line.is_some())
            .count() as u64
    );

    // `crate::alpha::helper()` in beta.rs: SCIP binds it to alpha::helper.
    // tokensave either agrees (confirmed), binds another `helper`
    // (contradicted), or lacks the edge (proposed for import).
    let refers_to_alpha_helper = |v: &Value| {
        v["file"] == "src/beta.rs"
            && v["line"] == 15
            && (v["target"] == "src/alpha.rs::helper"
                || v["scip_targets"]
                    .as_array()
                    .is_some_and(|t| t.iter().any(|t| t == "src/alpha.rs::helper")))
    };
    let confirmed_alpha_helper = all_edges.iter().any(|e| {
        e.line == Some(14) && e.target.contains("alpha.rs") && e.target.contains("helper")
    });
    assert!(
        confirmed_alpha_helper
            || sample(&report, "proposed_import")
                .iter()
                .any(refers_to_alpha_helper),
        "{report:#}"
    );
    for contradiction in sample(&report, "contradicted_edges") {
        assert_ne!(contradiction["resolved_by"], "extractor");
    }
}

#[tokio::test]
async fn refuses_foreign_root_unless_rebased() {
    let p = project().await;
    let mut opts = options(p.dir.path(), committed_index());
    opts.rebase_root = false;
    let err = compare(&opts).await.unwrap_err().to_string();
    assert!(err.contains("--rebase-root"), "{err}");
}

#[tokio::test]
async fn refuses_malformed_and_oversized_indexes() {
    let p = project().await;
    let bytes = std::fs::read(committed_index()).unwrap();
    let truncated = p.dir.path().join("truncated.scip");
    std::fs::write(&truncated, &bytes[..bytes.len() - 7]).unwrap();
    let err = compare(&options(p.dir.path(), truncated))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("malformed SCIP index"), "{err}");

    let mut small = options(p.dir.path(), committed_index());
    small.max_index_bytes = 100;
    let err = compare(&small).await.unwrap_err().to_string();
    assert!(err.contains("limit"), "{err}");
}

#[tokio::test]
async fn refuses_while_a_sync_holds_the_lock() {
    let p = project().await;
    let lock_path = p.dir.path().join(".tokensave").join("sync.lock");

    // A running sync holds the lock: the comparison must not read a
    // partially committed graph.
    let guard = tokensave::tokensave::try_acquire_sync_lock(p.dir.path()).unwrap();
    let err = compare(&options(p.dir.path(), committed_index()))
        .await
        .expect_err("comparison must refuse while a sync holds the lock");
    assert!(
        err.to_string().contains("sync is already in progress"),
        "{err}"
    );
    drop(guard);
    assert!(!lock_path.exists());

    // Once the sync finishes the comparison runs, and releases the lock.
    compare(&options(p.dir.path(), committed_index()))
        .await
        .unwrap();
    assert!(!lock_path.exists(), "comparison must release the sync lock");
}

#[tokio::test]
async fn skips_documents_when_either_index_is_stale() {
    let p = project().await;
    let alpha = p.dir.path().join("src").join("alpha.rs");
    std::fs::write(&alpha, format!("// shifted\n{}", source("alpha.rs"))).unwrap();

    // The tokensave graph predates the edit: the document is not compared.
    let report = compare(&options(p.dir.path(), committed_index()))
        .await
        .unwrap();
    assert_eq!(report["documents"]["compared"], 2);
    assert_eq!(report["documents"]["skipped"]["tokensave_index_stale"], 1);

    // After a sync the graph is fresh but the SCIP ranges are one line off.
    p.cg.sync().await.unwrap();
    let report = compare(&options(p.dir.path(), committed_index()))
        .await
        .unwrap();
    assert_eq!(report["documents"]["compared"], 2);
    // Whichever check trips first (a range past a line's end, or a
    // definition no longer spelling its name), the document is SCIP-stale.
    let stale: u64 = report["documents"]["skipped"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| k.starts_with("scip_stale_"))
        .map(|(_, v)| v.as_u64().unwrap())
        .sum();
    assert_eq!(stale, 1, "{report:#}");
    let skipped = sample(&report, "skipped_documents");
    assert_eq!(skipped[0]["path"], "src/alpha.rs");
}

#[tokio::test]
async fn skips_embedded_source_that_differs_from_the_graph_snapshot() {
    let p = project().await;
    let index = p.dir.path().join("embedded-source.scip");
    // Index.metadata.project_root and one document with a path and stale text.
    let bytes = b"\x0a\x0b\x1a\x09file:///p\x12\x15\x0a\x0csrc/alpha.rs\x2a\x05stale";
    std::fs::write(&index, bytes).unwrap();
    let report = compare(&options(p.dir.path(), index)).await.unwrap();
    assert_eq!(report["documents"]["compared"], 0);
    assert_eq!(
        report["documents"]["skipped"]["scip_stale_source_mismatch"],
        1
    );
}

fn typescript_fixture() -> PathBuf {
    fixture_dir().parent().unwrap().join("typescript")
}

fn typescript_source(name: &str) -> String {
    std::fs::read_to_string(typescript_fixture().join("src").join(name)).unwrap()
}

#[tokio::test]
async fn compares_scip_typescript_index() {
    // scip-typescript 0.6.1 emits zero-width module definitions and
    // `<constructor>` symbols; neither may mark a document as stale.
    let p = Project::new(&[
        ("src/validate.ts", typescript_source("validate.ts")),
        ("src/alpha.ts", typescript_source("alpha.ts")),
        ("src/beta.ts", typescript_source("beta.ts")),
        ("src/index.ts", typescript_source("index.ts")),
    ])
    .await;
    let report = compare(&options(
        p.dir.path(),
        typescript_fixture().join("index.scip"),
    ))
    .await
    .unwrap();
    assert_eq!(report["index"]["tool"], "scip-typescript");
    assert_eq!(report["documents"]["compared"], 4, "{report:#}");

    // `alpha.helper()` inside beta.ts is a known wrong binding (#1109).
    let contradicted = sample(&report, "contradicted_edges");
    assert!(
        contradicted.iter().any(|e| e["file"] == "src/beta.ts"
            && e["scip_targets"][0]
                .as_str()
                .is_some_and(|t| t.ends_with("alpha.ts::helper"))),
        "{report:#}"
    );
}
