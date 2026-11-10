//! `edges.resolved_by` is persisted and read back (#544).

use std::collections::HashMap;
use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{Edge, EdgeKind, Node, ResolvedBy};

const UTILS_RS: &str = "pub fn compute_total(x: i32) -> i32 {
    x + 1
}

pub struct Counter { n: i32 }

impl Counter {
    pub fn new() -> Self { Counter { n: 0 } }
}
";

/// Every edge joined to its endpoint nodes, through the storage API so the
/// test runs against every backend. Edges with a missing endpoint are
/// dropped, as an inner join drops them.
async fn joined_edges(cg: &TokenSave) -> Vec<(Node, Node, Edge)> {
    let edges = cg
        .db()
        .get_all_edges()
        .await
        .expect("reading every edge succeeds");
    let ids: Vec<String> = edges
        .iter()
        .flat_map(|e| [e.source.clone(), e.target.clone()])
        .collect();
    let nodes: HashMap<String, Node> = cg
        .db()
        .get_nodes_by_ids(&ids)
        .await
        .expect("reading the edge endpoints succeeds")
        .into_iter()
        .map(|n| (n.id.clone(), n))
        .collect();
    edges
        .into_iter()
        .filter_map(|e| {
            let source = nodes.get(&e.source)?.clone();
            let target = nodes.get(&e.target)?.clone();
            Some((source, target, e))
        })
        .collect()
}

/// `(source name, target name, resolved_by code)` for every `calls` edge,
/// ordered by source name, target name and line.
async fn call_provenance(cg: &TokenSave) -> Vec<(String, String, Option<i64>)> {
    let mut calls: Vec<(String, String, Option<u32>, Option<i64>)> = joined_edges(cg)
        .await
        .into_iter()
        .filter(|(_, _, e)| e.kind == EdgeKind::Calls)
        .map(|(s, t, e)| (s.name, t.name, e.line, e.resolved_by.map(ResolvedBy::code)))
        .collect();
    calls.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    calls
        .into_iter()
        .map(|(source, target, _, code)| (source, target, code))
        .collect()
}

#[test]
fn codes_round_trip_and_never_collide() {
    let mut codes = std::collections::HashSet::new();
    for r in ResolvedBy::ALL {
        assert!(codes.insert(r.code()), "duplicate code {}", r.code());
        assert!((1..=127).contains(&r.code()), "code must stay one byte");
        assert_eq!(ResolvedBy::from_code(r.code()), Some(r));
        assert_eq!(ResolvedBy::from_name(r.as_str()), Some(r));
    }
    assert_eq!(ResolvedBy::from_code(0), None);
    assert_eq!(ResolvedBy::from_name("direct"), None);
    assert!(ResolvedBy::QualifiedMatch.is_exact());
    assert!(ResolvedBy::ExactMatch.is_exact());
    assert!(!ResolvedBy::SimpleNameMatch.is_exact());
    assert!(!ResolvedBy::ExactMatchScored.is_exact());
}

#[tokio::test]
async fn full_index_persists_resolved_by() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src").join("utils.rs"), UTILS_RS).unwrap();
    fs::write(
        dir.path().join("src").join("main.rs"),
        "mod utils;\nuse crate::utils::Counter;\nfn main() {\n    let _ = utils::compute_total(1);\n    let _ = Counter::new();\n}\n",
    )
    .unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();

    let calls = call_provenance(&cg).await;
    let to_new = calls
        .iter()
        .find(|(s, t, _)| s == "main" && t == "new")
        .unwrap_or_else(|| panic!("{calls:?}"));
    assert_eq!(
        to_new.2,
        Some(ResolvedBy::QualifiedMatch.code()),
        "{calls:?}"
    );
    let to_total = calls
        .iter()
        .find(|(s, t, _)| s == "main" && t == "compute_total")
        .unwrap_or_else(|| panic!("{calls:?}"));
    assert_eq!(
        to_total.2,
        Some(ResolvedBy::PathTailMatch.code()),
        "{calls:?}"
    );

    // Read back through the edge API, not only through SQL.
    let total = cg.get_nodes_by_name("compute_total").await.unwrap();
    let incoming = cg.get_incoming_edges(&total[0].id).await.unwrap();
    let call = incoming.iter().find(|e| e.kind == EdgeKind::Calls).unwrap();
    assert_eq!(call.resolved_by, Some(ResolvedBy::PathTailMatch));
}

#[tokio::test]
async fn incremental_sync_persists_resolved_by() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src").join("utils.rs"), UTILS_RS).unwrap();
    fs::write(
        dir.path().join("src").join("main.rs"),
        "mod utils;\nfn main() {}\n",
    )
    .unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    assert!(call_provenance(&cg)
        .await
        .iter()
        .all(|(s, _, _)| s != "main"));

    // Make sure the edit is seen as a change even on coarse mtime clocks.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(
        dir.path().join("src").join("main.rs"),
        "mod utils;\nuse crate::utils::Counter;\nfn main() {\n    let _ = compute_total(1);\n    let _ = Counter::new();\n}\n",
    )
    .unwrap();
    cg.sync().await.unwrap();

    let calls = call_provenance(&cg).await;
    let to_total = calls
        .iter()
        .find(|(s, t, _)| s == "main" && t == "compute_total")
        .unwrap_or_else(|| panic!("{calls:?}"));
    assert_eq!(to_total.2, Some(ResolvedBy::ExactMatch.code()), "{calls:?}");
    let to_new = calls
        .iter()
        .find(|(s, t, _)| s == "main" && t == "new")
        .unwrap_or_else(|| panic!("{calls:?}"));
    assert_eq!(
        to_new.2,
        Some(ResolvedBy::QualifiedMatch.code()),
        "{calls:?}"
    );
}

/// Every edge as `(source qname, target qname, kind, line, code)`, in that
/// column order.
async fn labels(cg: &TokenSave) -> Vec<(String, String, String, Option<i64>, Option<i64>)> {
    let mut out: Vec<_> = joined_edges(cg)
        .await
        .into_iter()
        .map(|(s, t, e)| {
            (
                s.qualified_name,
                t.qualified_name,
                e.kind.as_str().to_string(),
                e.line.map(i64::from),
                e.resolved_by.map(ResolvedBy::code),
            )
        })
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn full_index_and_incremental_sync_agree_on_labels() {
    let main = "mod utils;\nuse crate::utils::Counter;\nfn main() {\n    let mut c = Counter::new();\n    c.bump(); Counter::bump(&mut c);\n    let _ = utils::compute_total(1);\n}\n";
    let utils = "pub fn compute_total(x: i32) -> i32 { x + 1 }\npub struct Counter { n: i32 }\nimpl Counter {\n    pub fn new() -> Self { Counter { n: 0 } }\n    pub fn bump(&mut self) { self.n += 1; }\n}\n";
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src").join("utils.rs"), utils).unwrap();
    fs::write(dir.path().join("src").join("main.rs"), main).unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    let full = labels(&cg).await;
    assert!(full.iter().any(|l| l.4.is_some()), "{full:?}");

    // Edit both files without moving a line, so every edge keeps its key.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(
        dir.path().join("src").join("main.rs"),
        format!("{main}// edited\n"),
    )
    .unwrap();
    fs::write(
        dir.path().join("src").join("utils.rs"),
        format!("{utils}// edited\n"),
    )
    .unwrap();
    cg.sync().await.unwrap();
    assert_eq!(labels(&cg).await, full);
}

#[tokio::test]
async fn a_labelled_duplicate_fills_in_an_unlabelled_edge() {
    use tokensave::types::Edge;
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn a() {}\npub fn b() {}\n").unwrap();
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    let a = cg.get_nodes_by_name("a").await.unwrap()[0].id.clone();
    let b = cg.get_nodes_by_name("b").await.unwrap()[0].id.clone();
    let edge = |resolved_by| Edge {
        source: a.clone(),
        target: b.clone(),
        kind: EdgeKind::Calls,
        line: Some(7),
        resolved_by,
    };
    cg.db().insert_edges(&[edge(None)]).await.unwrap();
    cg.db()
        .insert_edges(&[edge(Some(ResolvedBy::QualifiedMatch))])
        .await
        .unwrap();
    // A later, weaker label does not overwrite one already recorded.
    cg.db()
        .insert_edges(&[edge(Some(ResolvedBy::SimpleNameMatch))])
        .await
        .unwrap();
    let incoming = cg.get_incoming_edges(&b).await.unwrap();
    let calls: Vec<_> = incoming
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].resolved_by, Some(ResolvedBy::QualifiedMatch));
}
