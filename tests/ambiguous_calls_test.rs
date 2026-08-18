//! Recording ambiguous calls instead of discarding them — #412.
//!
//! When a `recv.method()` call has several equally-plausible targets, #378
//! stopped fabricating an edge. That was right — an edge is an assertion, and
//! "one of these three" is not one — but it threw the information away: the
//! resolver knew exactly which candidates it could not separate, and said
//! nothing.
//!
//! The candidates are now recorded, so the ambiguity becomes data. A model
//! reading the code is far better placed to decide which `dispose` was meant
//! than a scoring heuristic that cannot see the receiver's type, and it can
//! only do that if it is told there was a choice.
//!
//! Two properties are load-bearing:
//!
//! - **Still no edge.** Recording a choice must not quietly reintroduce the
//!   guess the refusal removed.
//! - **`dead_code` must not call an ambiguously-referenced symbol dead.**
//!   Refusing the edge and then reporting the target as uncalled would trade
//!   one false positive for another, which is the trap #346 measured at a 97%
//!   false-positive rate from the opposite direction.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;
use tokensave::types::EdgeKind;

/// Two classes defining `dispose`, called through a receiver whose type
/// genuinely cannot be inferred — an untyped parameter. A typed field would be
/// resolved precisely now and would not exercise this path.
async fn ambiguous_project() -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src/a.ts"),
        "export class StatusBar {\n    dispose(): void {\n        console.log('bar');\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/b.ts"),
        "export class ProviderManager {\n    dispose(): void {\n        console.log('mgr');\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/extension.ts"),
        "export class Extension {\n    deactivate(thing): void {\n        thing.dispose();\n    }\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

/// The candidates the resolver could not separate are kept, not discarded.
#[tokio::test]
async fn an_ambiguous_call_records_its_candidates() {
    let (_tmp, cg) = ambiguous_project().await;

    let ambiguous = cg.db().get_ambiguous_calls(None, 50).await.unwrap();
    let dispose: Vec<_> = ambiguous
        .iter()
        .filter(|a| a.reference_name.ends_with("dispose"))
        .collect();

    assert_eq!(
        dispose.len(),
        1,
        "the unresolvable `dispose` call must be recorded once, got {ambiguous:?}"
    );
    assert!(
        dispose[0].candidate_node_ids.len() >= 2,
        "an ambiguity is only interesting if it names the alternatives, got {:?}",
        dispose[0]
    );
    assert_eq!(
        dispose[0].file_path, "src/extension.ts",
        "the record must point at the call site, not a candidate"
    );
}

/// Recording the choice must not reintroduce the guess. The whole point of
/// #378 was that no edge is better than an arbitrary one.
#[tokio::test]
async fn recording_an_ambiguity_still_creates_no_edge() {
    let (_tmp, cg) = ambiguous_project().await;

    let nodes = cg.get_all_nodes().await.unwrap();
    let dispose_ids: Vec<&str> = nodes
        .iter()
        .filter(|n| n.name == "dispose")
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(dispose_ids.len(), 2, "fixture must have two candidates");

    let calls_into_dispose = cg
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls && dispose_ids.contains(&e.target.as_str()))
        .count();

    assert_eq!(
        calls_into_dispose, 0,
        "an ambiguous call must still produce no edge"
    );
}

/// The correctness half. Having refused the edge, `dead_code` must not then
/// report the candidates as uncalled — that would trade a fabricated edge for
/// a fabricated finding, which is worse because it reads as actionable.
#[tokio::test]
async fn dead_code_does_not_report_an_ambiguously_referenced_symbol() {
    let (_tmp, cg) = ambiguous_project().await;

    let dead = cg
        .find_dead_code(&[tokensave::types::NodeKind::Method], true, true)
        .await
        .unwrap();
    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();

    assert!(
        !dead_names.contains(&"dispose"),
        "a symbol named by an unresolved ambiguity is not known to be dead, got {dead_names:?}"
    );
}

/// A project with nothing ambiguous records nothing, so the table does not
/// become noise on codebases that resolve cleanly.
#[tokio::test]
async fn an_unambiguous_project_records_no_ambiguity() {
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

    assert!(
        cg.db()
            .get_ambiguous_calls(None, 50)
            .await
            .unwrap()
            .is_empty(),
        "a project that resolves cleanly must record no ambiguity"
    );
}
