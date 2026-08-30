//! #481: the build-variant pass no longer reads the whole graph.
//!
//! It used to load every `annotates` and `calls` edge — while the resolver's
//! node slice was still alive, so two graph-sized allocations were resident at
//! once and the sync's peak RSS landed there. It did that to find a set that is
//! tiny by construction: on tokensave's own tree the grouping keeps 3 groups
//! out of 19,331 nodes, and the pass emitted 0 edges.
//!
//! The bounded path asks SQL for the groups, then for only the `calls` edges
//! targeting a member. This file's job is to prove the two paths agree, on a
//! real indexed graph rather than on hand-built fixtures — the risk in a
//! rewrite like this is not that the emitter is wrong, it is that the SQL
//! selects a different candidate set than the Rust filter did.

use std::path::Path;
use tokensave::resolution::{
    emit_variant_edges, propagate_variant_edges, variant_groups_from_candidates,
};
use tokensave::tokensave::TokenSave;
use tokensave::types::{Edge, EdgeKind};

/// A project with three shapes the pass must tell apart: a genuine Rust cfg
/// pair, two same-named functions that are *not* cfg-gated (which must not be
/// fused), and a Go package with a build-constrained pair.
async fn indexed(dir: &Path) -> TokenSave {
    std::fs::create_dir_all(dir.join("src")).expect("src");
    std::fs::create_dir_all(dir.join("gopkg")).expect("gopkg");

    // The real thing: one name, two cfg-gated definitions, called from a third.
    std::fs::write(
        dir.join("src/platform.rs"),
        r#"
#[cfg(unix)]
pub fn symlink_dir(a: &str) -> bool { a.is_empty() }

#[cfg(windows)]
pub fn symlink_dir(a: &str) -> bool { !a.is_empty() }

pub fn caller() -> bool { symlink_dir("x") }
"#,
    )
    .expect("platform.rs");

    // The trap: same short name, no cfg gate. Fusing these would be wrong.
    std::fs::write(
        dir.join("src/plain.rs"),
        r#"
pub struct A;
pub struct B;
impl A { pub fn from(x: u8) -> u8 { x } }
impl B { pub fn from(x: u8) -> u8 { x + 1 } }
pub fn use_them() -> u8 { A::from(1) + B::from(2) }
"#,
    )
    .expect("plain.rs");

    // The caller lives beside one twin, so resolution has a same-file
    // preference to break the tie on — without that the call binds to neither
    // and there is nothing to propagate.
    std::fs::write(
        dir.join("gopkg/impl_linux.go"),
        "package gopkg\n\nfunc platformName() string { return \"linux\" }\n\nfunc Describe() string { return platformName() }\n",
    )
    .expect("go linux");
    std::fs::write(
        dir.join("gopkg/impl_darwin.go"),
        "package gopkg\n\nfunc platformName() string { return \"darwin\" }\n",
    )
    .expect("go darwin");

    let cg = TokenSave::init(dir).await.expect("init");
    cg.index_all().await.expect("index");
    cg
}

fn sorted(mut edges: Vec<Edge>) -> Vec<(String, String)> {
    edges.retain(|e| e.kind == EdgeKind::Calls);
    let mut pairs: Vec<(String, String)> = edges
        .into_iter()
        .map(|e| (e.source, e.target))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    pairs.sort();
    pairs
}

/// The whole point of #481: same answer, without reading the graph.
///
/// Run against the state a sync is actually in — a call bound to one twin and
/// not yet to its sibling — because the steady state after indexing already has
/// both edges, where "same answer" would be two empty vectors and prove
/// nothing. The propagated edge is withdrawn from the input, and both paths
/// must put it back.
#[tokio::test]
async fn the_bounded_path_agrees_with_the_whole_graph_pass() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let nodes = db.get_all_nodes_for_resolution().await.expect("nodes");
    let all_edges = db
        .get_edges_by_kinds(&[EdgeKind::Annotates, EdgeKind::Calls])
        .await
        .expect("edges");

    let rust = db
        .variant_group_candidates()
        .await
        .expect("rust candidates");
    let go = db.go_variant_candidates().await.expect("go candidates");
    let groups = variant_groups_from_candidates(&rust, &go);
    let members: Vec<String> = groups
        .values()
        .flatten()
        .map(|id| (*id).to_string())
        .collect();
    assert!(
        !members.is_empty(),
        "the fixture must produce variant groups or this test proves nothing"
    );

    let dropped = withdraw_one_propagated_edge(&all_edges, &members);
    let pre: Vec<Edge> = all_edges
        .iter()
        .filter(|e| !(e.source == dropped.0 && e.target == dropped.1))
        .cloned()
        .collect();

    let whole_graph = propagate_variant_edges(&nodes, &pre);
    assert!(
        !whole_graph.is_empty(),
        "the whole-graph pass found nothing to propagate, so the equality \
         assertion below would be vacuous"
    );

    let member_edges: Vec<Edge> = pre
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && members.contains(&e.target))
        .cloned()
        .collect();
    let bounded = emit_variant_edges(&groups, &member_edges);

    assert_eq!(
        sorted(whole_graph),
        sorted(bounded),
        "the SQL candidate set must select exactly what the Rust filter did"
    );
}

/// One `calls` edge into a variant member, chosen deterministically. Removing
/// it leaves the graph in the pre-propagation state.
fn withdraw_one_propagated_edge(edges: &[Edge], members: &[String]) -> (String, String) {
    let mut candidates: Vec<(String, String)> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && members.contains(&e.target))
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect();
    candidates.sort();
    candidates
        .pop()
        .expect("the fixture must have a call into a variant member")
}

/// The steady state, which is what almost every real sync hits: everything
/// already propagated, so a re-run must add nothing rather than duplicate.
#[tokio::test]
async fn a_second_pass_over_a_propagated_graph_adds_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let rust = db
        .variant_group_candidates()
        .await
        .expect("rust candidates");
    let go = db.go_variant_candidates().await.expect("go candidates");
    let groups = variant_groups_from_candidates(&rust, &go);
    let members: Vec<String> = groups
        .values()
        .flatten()
        .map(|id| (*id).to_string())
        .collect();
    let member_edges = db.calls_edges_into(&members).await.expect("member edges");

    assert!(
        emit_variant_edges(&groups, &member_edges).is_empty(),
        "re-propagating an already-propagated graph must emit nothing"
    );
}

/// The bounded path must read far fewer edges than the table holds, or it has
/// not actually fixed anything.
#[tokio::test]
async fn the_bounded_path_reads_a_fraction_of_the_edges() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let all_edges = db
        .get_edges_by_kinds(&[EdgeKind::Annotates, EdgeKind::Calls])
        .await
        .expect("edges");
    let rust = db
        .variant_group_candidates()
        .await
        .expect("rust candidates");
    let go = db.go_variant_candidates().await.expect("go candidates");
    let groups = variant_groups_from_candidates(&rust, &go);
    let members: Vec<String> = groups
        .values()
        .flatten()
        .map(|id| (*id).to_string())
        .collect();
    let member_edges = db.calls_edges_into(&members).await.expect("member edges");

    assert!(
        member_edges.len() < all_edges.len(),
        "bounded read {} of {} edges — no saving",
        member_edges.len(),
        all_edges.len()
    );
}

/// A cfg gate is a correctness condition, not an optimisation: two impls that
/// merely share a name must never be fused into one variant group.
#[tokio::test]
async fn same_named_functions_without_a_cfg_gate_are_not_a_variant_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let rust = db
        .variant_group_candidates()
        .await
        .expect("rust candidates");
    let go = db.go_variant_candidates().await.expect("go candidates");
    let groups = variant_groups_from_candidates(&rust, &go);

    for (key, members) in &groups {
        assert!(
            !key.contains("::from"),
            "ungated same-named impls were grouped: {key} -> {members:?}"
        );
    }
}

/// The common case — nothing cfg-gated anywhere — must not reach the edges
/// table at all. That is where the old pass spent the sync's peak.
#[tokio::test]
async fn a_project_with_no_variants_produces_no_groups() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn a() -> u8 { 1 }\npub fn b() -> u8 { a() }\n",
    )
    .expect("lib.rs");
    let cg = TokenSave::init(dir.path()).await.expect("init");
    cg.index_all().await.expect("index");

    let rust = cg
        .db()
        .variant_group_candidates()
        .await
        .expect("rust candidates");
    let go = cg
        .db()
        .go_variant_candidates()
        .await
        .expect("go candidates");
    let groups = variant_groups_from_candidates(&rust, &go);

    assert!(
        groups.is_empty(),
        "no cfg gates and no Go: nothing should group, got {groups:?}"
    );
}
