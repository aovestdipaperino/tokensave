//! Phantom `calls` edges from the bare-name fallback — #378.
//!
//! `resolve_one`'s Strategy 1/1b fall back to `try_exact_name_match_simple`
//! when the qualified or typed match fails, matching *any* node with that bare
//! method name anywhere in the graph. Go has a gate for this
//! (`try_go_selector_match`, conditioned on the qualifier being a known
//! import); TS/JS never got the equivalent, so `this.map.clear()` — where
//! `map` is a JS builtin — could bind to a local `LruCache.clear`.
//!
//! The reporter's follow-up found the more serious half. With two candidates
//! tied on every scoring dimension in `find_best_match`, the tie-break was
//! `if score > best_score`: strictly first-seen wins, and "first" is file
//! enumeration order. So the same source tree produced *different edges*
//! depending on the order the filesystem handed files back — renaming
//! `statusBar.ts` to `zStatusBar.ts` flipped which class absorbed the call,
//! and flipped whether `dead_code` reported a false positive.
//!
//! What these tests pin is the second half: an unresolvable ambiguity must
//! produce no edge rather than a coin-flip edge. The tie-break itself is
//! deliberately *not* changed here — measured on this repository, ties decide
//! roughly 12.7% of all call edges, so making them deterministic re-points
//! that many at once. That is too large to fold into a bug fix, and is
//! tracked separately.

use std::path::Path;
use tempfile::tempdir;
use tokensave::tokensave::TokenSave;
use tokensave::types::EdgeKind;

/// Two unrelated classes each defining `dispose()`, with the call made through
/// a field — the shape from the report. `collect_var_types` tracks bare `this`,
/// typed parameters and typed bindings, but not `this.<field>`, so the call
/// reaches the bare-name fallback with two indistinguishable candidates.
///
/// `first` decides which file the scan reaches first, which is the variable the
/// reporter isolated by renaming a file.
async fn project_with_two_dispose(
    first_file: &str,
    second_file: &str,
) -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src").join(first_file),
        "export class StatusBar {\n    dispose(): void {\n        console.log('bar');\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src").join(second_file),
        "export class ProviderManager {\n    dispose(): void {\n        console.log('mgr');\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/extension.ts"),
        "import { StatusBar } from './a';\nimport { ProviderManager } from './b';\n\
         export class Extension {\n    private statusBar = new StatusBar();\n\
         \n    deactivate(): void {\n        this.statusBar.dispose();\n    }\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

/// Every `calls` edge, as `(source_id, target_id)`, sorted for comparison.
async fn call_edges(cg: &TokenSave) -> Vec<(String, String)> {
    let mut edges: Vec<(String, String)> = cg
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .map(|e| (e.source, e.target))
        .collect();
    edges.sort();
    edges
}

/// Resolves the qualified names either side of each `calls` edge, so the
/// assertion reads in terms of the source rather than opaque ids.
async fn named_call_edges(cg: &TokenSave) -> Vec<(String, String)> {
    let nodes = cg.get_all_nodes().await.unwrap();
    let by_id: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.qualified_name.as_str()))
        .collect();
    let mut named: Vec<(String, String)> = call_edges(cg)
        .await
        .into_iter()
        .filter_map(|(s, t)| {
            Some((
                (*by_id.get(s.as_str())?).to_string(),
                (*by_id.get(t.as_str())?).to_string(),
            ))
        })
        .collect();
    named.sort();
    named
}

/// The same source should produce the same graph regardless of the order
/// files are enumerated in. The reporter demonstrated the opposite by renaming
/// a file, which is the only variable changed here.
///
/// **This fixture does not reproduce their case** — it passed before the fix
/// as well as after, because the tie here is resolved by the ambiguity refusal
/// rather than by the scan order. It is kept as a regression guard, not as
/// evidence that the graph is order-independent in general: it is not.
/// `find_best_match` still breaks ties on arrival order, which decides roughly
/// 12.7% of this repository's call edges. Fixing that re-points all of them at
/// once and is tracked separately.
#[tokio::test]
async fn call_edges_do_not_depend_on_file_enumeration_order() {
    let (_a, cg_a) = project_with_two_dispose("a.ts", "b.ts").await;
    let (_b, cg_b) = project_with_two_dispose("z_a.ts", "b.ts").await;

    let edges_a = named_call_edges(&cg_a).await;
    let edges_b = named_call_edges(&cg_b).await;

    assert_eq!(
        edges_a, edges_b,
        "renaming a file must not change which calls resolve"
    );
}

/// An ambiguous receiver call must produce no edge rather than an arbitrary
/// one. Binding `this.statusBar.dispose()` to whichever `dispose` the scan
/// reached first is a coin flip, and a wrong edge both fabricates a caller for
/// dead code and hides the real one.
#[tokio::test]
async fn an_ambiguous_bare_name_call_resolves_to_nothing() {
    let (_tmp, cg) = project_with_two_dispose("a.ts", "b.ts").await;

    let dispose_targets: Vec<String> = named_call_edges(&cg)
        .await
        .into_iter()
        .filter(|(_, target)| target.ends_with("dispose"))
        .map(|(source, target)| format!("{source} -> {target}"))
        .collect();

    assert!(
        dispose_targets.is_empty(),
        "two equally-plausible `dispose` candidates must yield no edge, got {dispose_targets:?}"
    );
}

/// The control that stops the fix from being "resolve nothing". With exactly
/// one candidate there is no ambiguity, and the edge must still be created —
/// otherwise every ordinary method call in TypeScript would stop resolving.
#[tokio::test]
async fn an_unambiguous_receiver_call_still_resolves() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src/widget.ts"),
        "export class Widget {\n    render(): void {\n        console.log('w');\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.ts"),
        "import { Widget } from './widget';\n\
         export class App {\n    private widget = new Widget();\n\
         \n    start(): void {\n        this.widget.render();\n    }\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let named = named_call_edges(&cg).await;
    assert!(
        named.iter().any(|(_, target)| target.ends_with("render")),
        "a call with exactly one candidate must still resolve, got {named:?}"
    );
}

/// Indexing the same tree twice must produce the same edges. A weaker version
/// of the ordering test that also catches non-determinism introduced anywhere
/// else in the pipeline.
#[tokio::test]
async fn reindexing_the_same_tree_produces_the_same_edges() {
    let (tmp, cg) = project_with_two_dispose("a.ts", "b.ts").await;
    let first = call_edges(&cg).await;
    drop(cg);

    let root: &Path = tmp.path();
    std::fs::remove_dir_all(root.join(".tokensave")).unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    assert_eq!(first, call_edges(&cg).await, "reindexing must be stable");
}
