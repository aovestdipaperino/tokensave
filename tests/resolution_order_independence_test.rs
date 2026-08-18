//! The graph must be a function of the source, not of the filesystem (#412).
//!
//! Every multi-candidate path in the resolver now scores its candidates and
//! refuses a tie instead of taking whichever one the file scan reached first.
//! The tests here pin that property directly rather than pinning one fixture's
//! expected edges: the same source, arranged so the scan meets the candidates
//! in the opposite order, must produce the same edges.
//!
//! The variable is *which file holds which definition*, with the file names
//! held constant. Renaming a file would also change the qualified names, which
//! is a legitimate difference and would make the comparison fail for a reason
//! unrelated to ordering — the trap the earlier fixture in
//! `ts_phantom_call_edges_test.rs` fell into.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;
use tokensave::types::EdgeKind;

/// Indexes `files` (name, contents) in a fresh project and returns every
/// `calls` edge as `Class::method -> Class::method`.
///
/// The last two `::` segments of a qualified name are the enclosing type and
/// the symbol, which is what the assertion is about; the leading path segments
/// are exactly the part that legitimately differs between the two arrangements.
async fn call_edges_by_symbol(dir: &str, files: &[(&str, &str)]) -> Vec<String> {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join(dir)).unwrap();
    for (name, contents) in files {
        std::fs::write(root.join(dir).join(name), contents).unwrap();
    }

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let nodes = cg.get_all_nodes().await.unwrap();
    let by_id: std::collections::HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.qualified_name.as_str()))
        .collect();
    let tail = |q: &str| -> String {
        let mut parts: Vec<&str> = q.rsplit("::").take(2).collect();
        parts.reverse();
        parts.join("::")
    };

    let mut edges: Vec<String> = cg
        .get_all_edges()
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .filter_map(|e| {
            let source = by_id.get(e.source.as_str())?;
            let target = by_id.get(e.target.as_str())?;
            Some(format!("{} -> {}", tail(source), tail(target)))
        })
        .collect();
    edges.sort();
    edges
}

const STATUS_BAR: &str =
    "export class StatusBar {\n    dispose(): void {\n        console.log('bar');\n    }\n}\n";
const PROVIDER_MANAGER: &str = "export class ProviderManager {\n    dispose(): void {\n        console.log('mgr');\n    }\n}\n";
/// An untyped parameter: no annotation and no initializer, so the receiver's
/// type genuinely cannot be recovered and both `dispose` methods tie.
const CALLER_TS: &str =
    "export class Extension {\n    deactivate(thing): void {\n        thing.dispose();\n    }\n}\n";

/// Two indistinguishable candidates, met in one order and then the other.
/// Whatever the resolver decides, it must decide the same thing both times.
///
/// This one already passed before `ab9ff29`: `e8a8a8a` had converted the
/// bare-name path, so the tie here was already refused rather than resolved by
/// arrival order. It is a regression guard on that path, not evidence for the
/// fix — the fixture below is the one that reproduces.
#[tokio::test]
async fn tied_candidates_resolve_the_same_way_in_either_scan_order() {
    let forward = call_edges_by_symbol(
        "src",
        &[
            ("a.ts", STATUS_BAR),
            ("b.ts", PROVIDER_MANAGER),
            ("extension.ts", CALLER_TS),
        ],
    )
    .await;
    let reversed = call_edges_by_symbol(
        "src",
        &[
            ("a.ts", PROVIDER_MANAGER),
            ("b.ts", STATUS_BAR),
            ("extension.ts", CALLER_TS),
        ],
    )
    .await;

    assert_eq!(
        forward, reversed,
        "swapping which file holds which candidate must not change the graph"
    );
}

/// The same property on the path the kind filter takes. When some nodes
/// sharing the name are kind-incompatible with a `Calls` reference — here a
/// struct named `process` — resolution goes through
/// `resolve_from_filtered_named`, which until `ab9ff29` picked the first
/// candidate in the reference's own file and otherwise the first overall.
///
/// This fixture does reproduce: against the previous resolver it fails with
/// `run -> Handler::process` one way round and `run -> Worker::process` the
/// other, which is the effect @vianbas demonstrated by renaming a file.
#[tokio::test]
async fn kind_filtered_candidates_resolve_the_same_way_in_either_scan_order() {
    const HANDLER: &str =
        "pub struct Handler;\n\nimpl Handler {\n    pub fn process(&self) {}\n}\n";
    const WORKER: &str = "pub struct Worker;\n\nimpl Worker {\n    pub fn process(&self) {}\n}\n";
    // A non-callable node sharing the name, so the kind filter shrinks the
    // candidate list and the filtered path is the one taken.
    const CALLER_RS: &str =
        "pub struct process;\n\npub fn run(thing: Unknown) {\n    thing.process();\n}\n";

    let forward = call_edges_by_symbol(
        "src",
        &[
            ("a.rs", HANDLER),
            ("b.rs", WORKER),
            ("caller.rs", CALLER_RS),
        ],
    )
    .await;
    let reversed = call_edges_by_symbol(
        "src",
        &[
            ("a.rs", WORKER),
            ("b.rs", HANDLER),
            ("caller.rs", CALLER_RS),
        ],
    )
    .await;

    assert_eq!(
        forward, reversed,
        "swapping which file holds which candidate must not change the graph"
    );
}

/// The control. Order-independence is trivially satisfiable by resolving
/// nothing, so at least one arrangement must still produce the ordinary edge
/// for an unambiguous call.
#[tokio::test]
async fn an_unambiguous_call_still_resolves_in_this_harness() {
    let edges = call_edges_by_symbol(
        "src",
        &[
            ("widget.ts", "export class Widget {\n    render(): void {\n        console.log('w');\n    }\n}\n"),
            (
                "app.ts",
                "import { Widget } from './widget';\n\nexport class App {\n    private widget = new Widget();\n\n    draw(): void {\n        this.widget.render();\n    }\n}\n",
            ),
        ],
    )
    .await;

    assert!(
        edges.iter().any(|e| e == "App::draw -> Widget::render"),
        "the unambiguous call must resolve, got {edges:?}"
    );
}
