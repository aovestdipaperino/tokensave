//! Phantom `calls` edges from the single-candidate bare-name fallback — #503.
//!
//! #378 taught the resolver to refuse when the bare-name fallback is
//! *ambiguous*: two candidates tied on every scoring dimension produce no edge
//! rather than a coin-flip one. It left the case where the ambiguity set has
//! exactly one member, and that is the case this covers.
//!
//! When a Python call's receiver is a type the index does not track — a
//! stdlib object, a third-party instance, a `logging.Logger` — the fallback
//! matches the bare method name against every indexed symbol. If exactly one
//! carries that name, the call binds to it, no matter which module or
//! directory it lives in. Being the only candidate is not evidence: nothing
//! checked that the call site can reach it.
//!
//! The failure is quiet and it has a direction. Test doubles are deliberately
//! named after the API they stand in for, so production code binds into the
//! test tree. Measured on a 992-file Python project, the guard removes 2,606
//! call edges, of which `append` accounts for 1,654, `debug` 346 and `execute`
//! 172 — `list.append`, `logger.debug` and `cursor.execute`, every one of them
//! a call on a receiver the index never typed.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;
use tokensave::types::EdgeKind;

/// Resolves the qualified names either side of each `calls` edge.
async fn named_call_edges(cg: &TokenSave) -> Vec<(String, String)> {
    let nodes = cg.get_all_nodes().await.unwrap();
    let by_id: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.qualified_name.as_str()))
        .collect();
    let mut named: Vec<(String, String)> = cg
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .filter_map(|e| {
            Some((
                (*by_id.get(e.source.as_str())?).to_string(),
                (*by_id.get(e.target.as_str())?).to_string(),
            ))
        })
        .collect();
    named.sort();
    named
}

/// The reporter's project, exactly: production code logging through a
/// `logging.Logger`, and a test double that happens to define `info`. Nothing
/// in `src/` imports `tests/`, and nothing can — the edge asserts a call that
/// the language would not permit.
#[tokio::test]
async fn a_lone_candidate_in_an_unreachable_module_is_not_a_match() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();

    std::fs::write(
        root.join("src/a.py"),
        "import logging\n\nlog = logging.getLogger(__name__)\n\n\n\
         def do_work(payload):\n    log.info(\"starting %s\", payload)\n    return payload\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tests/helper.py"),
        "def info(msg, *args):\n    return (msg, args)\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let edges = named_call_edges(&cg).await;
    assert!(
        !edges.iter().any(|(_, target)| target.contains("info")),
        "do_work does not call the test double: {edges:?}"
    );
}

/// The guard must not cost the edges that are real. An import of the callee's
/// module is evidence, and so is a call within one package directory.
#[tokio::test]
async fn an_imported_or_sibling_candidate_still_resolves() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::create_dir_all(root.join("other")).unwrap();

    // Imported across directories: `pkg.helpers` is named in the import.
    std::fs::write(
        root.join("other/caller.py"),
        "import pkg.helpers\n\n\ndef run(x):\n    return pkg.helpers.transmogrify(x)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("pkg/helpers.py"),
        "def transmogrify(x):\n    return x + 1\n",
    )
    .unwrap();
    // Siblings in one directory, called through an untracked receiver.
    std::fs::write(
        root.join("pkg/neighbour.py"),
        "def embiggen(x):\n    return x * 2\n",
    )
    .unwrap();
    std::fs::write(
        root.join("pkg/user.py"),
        "def use(thing):\n    return thing.embiggen(3)\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let edges = named_call_edges(&cg).await;
    assert!(
        edges
            .iter()
            .any(|(_, target)| target.contains("transmogrify")),
        "an imported module's function should still resolve: {edges:?}"
    );
    assert!(
        edges.iter().any(|(_, target)| target.contains("embiggen")),
        "a sibling in the same package should still resolve: {edges:?}"
    );
}
