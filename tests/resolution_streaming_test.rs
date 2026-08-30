//! #482: resolution reads the reference table a page at a time.
//!
//! #409 concluded the resolution pass could not be chunked, and for the *nodes*
//! that is right — binding a cross-file reference means looking up a name that
//! may be defined anywhere, so a chunked name index silently loses targets and
//! drops edges. The references are the other input and have no such property:
//! `resolve_batch` is a `par_iter().map(resolve_one)`, so each one is answered
//! against the whole index independently.
//!
//! What has to be proved is that paging the input changes nothing, including at
//! the page boundary — so these run the same references through one batch and
//! through many, and compare.

use std::collections::BTreeSet;
use tokensave::resolution::ReferenceResolver;
use tokensave::tokensave::TokenSave;
use tokensave::types::ResolvedRef;

/// A project with enough cross-file references that paging splits them.
async fn indexed(dir: &std::path::Path) -> TokenSave {
    std::fs::create_dir_all(dir.join("src")).expect("src");
    let mut lib = String::from("pub mod helpers;\n");
    for i in 0..40 {
        lib.push_str(&format!(
            "pub fn caller_{i}() -> u32 {{ crate::helpers::target_{i}() + crate::helpers::shared() }}\n"
        ));
    }
    std::fs::write(dir.join("src/lib.rs"), lib).expect("lib.rs");

    let mut helpers = String::from("pub fn shared() -> u32 { 1 }\n");
    for i in 0..40 {
        helpers.push_str(&format!("pub fn target_{i}() -> u32 {{ {i} }}\n"));
    }
    std::fs::write(dir.join("src/helpers.rs"), helpers).expect("helpers.rs");

    let cg = TokenSave::init(dir).await.expect("init");
    cg.index_all().await.expect("index");
    cg
}

fn edge_set(resolved: &[ResolvedRef]) -> BTreeSet<(String, String)> {
    resolved
        .iter()
        .map(|r| (r.original.from_node_id.clone(), r.target_node_id.clone()))
        .collect()
}

/// Paging the input must not change the answer, at any page size — including
/// sizes that split the reference list at every boundary.
#[tokio::test]
async fn paging_the_references_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let nodes = db.get_all_nodes_for_resolution().await.expect("nodes");
    let refs = db.get_unresolved_refs().await.expect("refs");
    assert!(
        refs.len() > 8,
        "the fixture must produce enough references to page, got {}",
        refs.len()
    );
    let resolver = ReferenceResolver::from_nodes(db, &nodes);

    let whole = resolver.resolve_all(&refs);
    assert!(
        !whole.resolved.is_empty(),
        "nothing resolved, so the comparison below would be vacuous"
    );

    for page in [1usize, 2, 3, 7, refs.len() - 1, refs.len(), refs.len() + 1] {
        let mut resolved = Vec::new();
        let mut ambiguous = Vec::new();
        for chunk in refs.chunks(page) {
            let (r, a) = resolver.resolve_batch(chunk);
            resolved.extend(r);
            ambiguous.extend(a);
        }
        resolver.finalize_resolved(&mut resolved);

        assert_eq!(
            edge_set(&whole.resolved),
            edge_set(&resolved),
            "page size {page} produced a different edge set"
        );
        assert_eq!(
            whole.ambiguous.len(),
            ambiguous.len(),
            "page size {page} produced a different number of ambiguity records"
        );
    }
}

/// A page size of one is the strongest form of the above: every reference is
/// its own batch, so anything that depended on seeing its neighbours breaks.
#[tokio::test]
async fn one_reference_per_page_still_resolves_everything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let nodes = db.get_all_nodes_for_resolution().await.expect("nodes");
    let refs = db.get_unresolved_refs().await.expect("refs");
    let resolver = ReferenceResolver::from_nodes(db, &nodes);

    let whole = resolver.resolve_all(&refs);
    let mut one_at_a_time = Vec::new();
    for r in &refs {
        let (resolved, _) = resolver.resolve_batch(std::slice::from_ref(r));
        one_at_a_time.extend(resolved);
    }
    resolver.finalize_resolved(&mut one_at_a_time);

    assert_eq!(edge_set(&whole.resolved), edge_set(&one_at_a_time));
}

/// The keyset cursor must walk the whole table exactly once — no row visited
/// twice, none skipped, whatever the page size.
#[tokio::test]
async fn the_cursor_visits_every_reference_exactly_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cg = indexed(dir.path()).await;
    let db = cg.db();

    let expected = db.count_unresolved_refs().await.expect("count");
    assert!(expected > 0, "the fixture must leave unresolved references");

    for page in [1usize, 3, 10, 10_000] {
        let mut seen_ids = BTreeSet::new();
        let mut cursor = 0i64;
        loop {
            let batch = db
                .get_unresolved_refs_after(cursor, page)
                .await
                .expect("page");
            let Some((last, _)) = batch.last() else { break };
            cursor = *last;
            for (id, _) in &batch {
                assert!(
                    seen_ids.insert(*id),
                    "page size {page} returned id {id} twice"
                );
            }
        }
        assert_eq!(
            seen_ids.len(),
            expected,
            "page size {page} visited {} of {expected} references",
            seen_ids.len()
        );
    }
}
