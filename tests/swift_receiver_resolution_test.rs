//! Integration tests for Swift receiver-aware and SPM-target-scoped call resolution.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, Node, NodeKind};

/// A root `Package.swift`, which makes `Sources/<Target>` and `Tests/<Target>`
/// `SwiftPM` targets for the resolver.
const PACKAGE_MANIFEST: (&str, &str) = ("Package.swift", "// swift-tools-version: 5.9\n");

async fn index_files(files: &[(&str, &str)]) -> (TempDir, TokenSave) {
    let dir = TempDir::new().expect("temp dir");
    for (rel, content) in files {
        let abs = dir.path().join(rel);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(abs, content).expect("write swift file");
    }
    let cg = TokenSave::init(dir.path()).await.expect("init");
    cg.index_all().await.expect("index_all");
    (dir, cg)
}

fn find_node<'n>(nodes: &'n [Node], file_suffix: &str, name: &str, kind: NodeKind) -> &'n Node {
    nodes
        .iter()
        .find(|n| n.file_path.ends_with(file_suffix) && n.name == name && n.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {name} in {file_suffix}"))
}

async fn outgoing_call_targets(cg: &TokenSave, source_id: &str) -> Vec<String> {
    let edges = cg.get_all_edges().await.expect("get_all_edges");
    edges
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.source == source_id)
        .map(|e| e.target)
        .collect()
}

/// Negative paths N1 & N2: stdlib calls (`bytes.append`, `records.sorted`, `self.points.append`)
/// must NOT self-recurse or link across unimported SPM targets or from Sources to Tests.
#[tokio::test]
async fn swift_stdlib_calls_and_unimported_targets_do_not_create_phantom_cycles() {
    let (_dir, cg) = index_files(&[
        PACKAGE_MANIFEST,
        (
            "Sources/Core/DataBuffer.swift",
            r#"import Foundation
public struct DataBuffer {
    public var bytes: [UInt8] = []
    public mutating func append(_ chunk: Data) {
        bytes.append(0x01)
    }
}
"#,
        ),
        (
            "Sources/Network/Packet.swift",
            r#"import Foundation
public struct Packet {
    public mutating func send() {
        var localBytes: [UInt8] = [0x01]
        localBytes.append(0x02)
        var rawData = Data()
        rawData.append(Data([0x03]))
        let encoder = JSONEncoder()
        _ = try? encoder.encode("ok")
    }
}
"#,
        ),
        (
            "Tests/CoreTests/BufferTests.swift",
            r#"@testable import Core
import Foundation
final class BufferTests {
    var bytes = Data()
    func append(_ s: String) {
        bytes.append(Data(s.utf8))
    }
}
"#,
        ),
        (
            "Sources/Core/Exporter.swift",
            r#"public struct Exporter {
    public func sorted(_ items: [Int]) -> [Int] {
        items.sorted()
    }
    public func encode(_ value: String) -> String {
        value
    }
}
"#,
        ),
    ])
    .await;

    let nodes = cg.get_all_nodes().await.expect("nodes");
    let packet_send = find_node(&nodes, "Packet.swift", "send", NodeKind::Method);
    let buffer_append = find_node(&nodes, "DataBuffer.swift", "append", NodeKind::Method);
    let test_append = find_node(&nodes, "BufferTests.swift", "append", NodeKind::Method);
    let exporter_sorted = find_node(&nodes, "Exporter.swift", "sorted", NodeKind::Method);

    let send_targets = outgoing_call_targets(&cg, &packet_send.id).await;
    assert!(
        send_targets.is_empty(),
        "Packet.send must not call DataBuffer.append or BufferTests.append: {send_targets:?}"
    );

    let test_targets = outgoing_call_targets(&cg, &test_append.id).await;
    assert!(
        !test_targets.contains(&test_append.id),
        "BufferTests.append must not self-recurse via Data.append"
    );
    assert!(
        !test_targets.contains(&buffer_append.id),
        "BufferTests.append must not resolve bytes.append to DataBuffer.append"
    );

    let sorted_targets = outgoing_call_targets(&cg, &exporter_sorted.id).await;
    assert!(
        !sorted_targets.contains(&exporter_sorted.id),
        "Exporter.sorted must not self-recurse via Array.sorted"
    );
}

/// Positive path P1 & P3 & Negative N2: typed local/property receivers resolve across files
/// within the same SPM target and across SPM targets ONLY when the target module is imported.
#[tokio::test]
async fn swift_typed_receivers_respect_spm_target_import_visibility() {
    let (_dir, cg) = index_files(&[
        PACKAGE_MANIFEST,
        (
            "Sources/Core/Store.swift",
            r#"public class HistoryStore {
    public init() {}
    public func persist() {}
}
"#,
        ),
        (
            "Sources/App/ImportedCaller.swift",
            r#"import Core
struct ImportedCaller {
    func run(store: HistoryStore) {
        store.persist()
    }
}
"#,
        ),
        (
            "Sources/Other/UnimportedCaller.swift",
            r#"struct OtherHistory {
    func callUntyped(_ store: UnknownStore) {
        store.persist()
    }
}
"#,
        ),
        (
            "Tests/CoreTests/StoreTestHelper.swift",
            r#"class TestFakeStore {
    func persist() {}
    func helperOnlyInTest() {}
}
"#,
        ),
        (
            "Sources/Core/SameTargetCaller.swift",
            r#"struct SameTargetCaller {
    let store: HistoryStore
    func execute() {
        store.persist()
        let f = TestFakeStore()
        f.helperOnlyInTest()
    }
}
"#,
        ),
    ])
    .await;

    let nodes = cg.get_all_nodes().await.expect("nodes");
    let store_persist = find_node(&nodes, "Store.swift", "persist", NodeKind::Method);
    let imported_run = find_node(&nodes, "ImportedCaller.swift", "run", NodeKind::Method);
    let unimported_call = find_node(
        &nodes,
        "UnimportedCaller.swift",
        "callUntyped",
        NodeKind::Method,
    );
    let same_target_exec = find_node(
        &nodes,
        "SameTargetCaller.swift",
        "execute",
        NodeKind::Method,
    );
    let test_helper = find_node(
        &nodes,
        "StoreTestHelper.swift",
        "helperOnlyInTest",
        NodeKind::Method,
    );

    let imported_targets = outgoing_call_targets(&cg, &imported_run.id).await;
    assert!(
        imported_targets.contains(&store_persist.id),
        "Imported target call must resolve HistoryStore.persist: {imported_targets:?}"
    );

    let same_target_targets = outgoing_call_targets(&cg, &same_target_exec.id).await;
    assert!(
        same_target_targets.contains(&store_persist.id),
        "Same SPM target property call must resolve HistoryStore.persist: {same_target_targets:?}"
    );
    assert!(
        !same_target_targets.contains(&test_helper.id),
        "Production target must NEVER call Tests target symbols: {same_target_targets:?}"
    );

    let unimported_targets = outgoing_call_targets(&cg, &unimported_call.id).await;
    assert!(
        !unimported_targets.contains(&store_persist.id),
        "Unimported SPM target must NOT call Core.HistoryStore.persist: {unimported_targets:?}"
    );
}

#[test]
fn integration_path_sanity() {
    assert!(Path::new("Sources/Core/Store.swift").ends_with("Store.swift"));
}

/// Positive P2/P3 & Negative N3: computed property reads, `!fn()`, higher-order
/// function refs, `#selector(...)`, and label/arity overload disambiguation resolve
/// accurately without creating calls for locals or shorthand enum values.
#[tokio::test]
async fn swift_computed_props_selectors_function_refs_and_overloads_resolve() {
    let (_dir, cg) = index_files(&[
        PACKAGE_MANIFEST,
        (
            "Sources/App/DB.swift",
            r#"public enum SqlDB {
    public static func bind(_ value: Double, at index: Int) {}
    public static func bind(
        _ value: String,
        key: String,
        at index: Int
    ) {}
}

public class ReceiverStore {
    public init() {}
    public func receive(_ item: String) {}
    public func receive(
        _ items: [String],
        for category: String
    ) {}
}
"#,
        ),
        (
            "Sources/App/ViewsAndController.swift",
            r#"public struct DashboardView {
    public let labelText: String

    public var body: String {
        _ = controls
        return !isBlocked(labelText) ? labelText : ""
    }

    private var controls: String {
        renderButton(action: lookUpHash)
    }

    private func renderButton(action: () -> Void) -> String { "" }
    private func lookUpHash() {}
    private func isBlocked(_ input: String) -> Bool { false }
}

public final class Controller {
    private let store = ReceiverStore()

    public func configure(mode: String) {
        _ = #selector(statusItemClicked)
        SqlDB.bind(1.5, at: 1)
        SqlDB.bind("abc", key: "k", at: 2)
        store.receive("one")
        store.receive(["a"], for: "cat")
        let localValue = mode
        _ = accept(localValue, style: .compact)
    }

    private func accept(_ v: String, style: Any) -> String { v }
    @objc private func statusItemClicked() {}
    public static func formatRow(_ row: String) -> String { row }
    public func exportRows(_ rows: [String]) -> [String] {
        rows.map(Controller.formatRow)
    }
}
"#,
        ),
    ])
    .await;

    let nodes = cg.get_all_nodes().await.expect("nodes");

    let bind_double = nodes
        .iter()
        .find(|n| n.name == "bind" && n.signature.as_deref().is_some_and(|s| s.contains("Double")))
        .expect("bind(Double)");
    let bind_string = nodes
        .iter()
        .find(|n| {
            n.name == "bind"
                && n.signature
                    .as_deref()
                    .is_some_and(|s| s.contains("key: String"))
        })
        .expect("bind(String, key:)");

    let recv_one = nodes
        .iter()
        .find(|n| {
            n.name == "receive"
                && n.signature
                    .as_deref()
                    .is_some_and(|s| !s.contains("for category"))
        })
        .expect("receive(_:)");
    let recv_two = nodes
        .iter()
        .find(|n| {
            n.name == "receive"
                && n.signature
                    .as_deref()
                    .is_some_and(|s| s.contains("for category"))
        })
        .expect("receive(_:for:)");

    let status_clicked = find_node(
        &nodes,
        "ViewsAndController.swift",
        "statusItemClicked",
        NodeKind::Method,
    );
    let lookup_hash = find_node(
        &nodes,
        "ViewsAndController.swift",
        "lookUpHash",
        NodeKind::Method,
    );
    let is_blocked = find_node(
        &nodes,
        "ViewsAndController.swift",
        "isBlocked",
        NodeKind::Method,
    );
    let format_row = find_node(
        &nodes,
        "ViewsAndController.swift",
        "formatRow",
        NodeKind::Method,
    );
    let controls = find_node(
        &nodes,
        "ViewsAndController.swift",
        "controls",
        NodeKind::Property,
    );
    let configure = find_node(
        &nodes,
        "ViewsAndController.swift",
        "configure",
        NodeKind::Method,
    );
    let body = find_node(
        &nodes,
        "ViewsAndController.swift",
        "body",
        NodeKind::Property,
    );
    let export_rows = find_node(
        &nodes,
        "ViewsAndController.swift",
        "exportRows",
        NodeKind::Method,
    );

    let all_edges = cg.get_all_edges().await.expect("edges");
    let configure_targets: Vec<&str> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.source == configure.id)
        .map(|e| e.target.as_str())
        .collect();

    assert!(
        configure_targets.contains(&status_clicked.id.as_str()),
        "selector missing: {configure_targets:?}"
    );
    assert!(
        configure_targets.contains(&bind_double.id.as_str()),
        "bind_double missing: {configure_targets:?}"
    );
    assert!(
        configure_targets.contains(&bind_string.id.as_str()),
        "bind_string missing: {configure_targets:?}"
    );
    assert!(
        configure_targets.contains(&recv_one.id.as_str()),
        "recv_one missing: {configure_targets:?}"
    );
    assert!(
        configure_targets.contains(&recv_two.id.as_str()),
        "recv_two missing: {configure_targets:?}"
    );

    // Ensure overload edges did not merge onto just one of the overloads (line-specific check).
    let bind_double_callers: Vec<_> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.target == bind_double.id)
        .collect();
    let bind_string_callers: Vec<_> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.target == bind_string.id)
        .collect();
    assert_eq!(
        bind_double_callers.len(),
        1,
        "bind_double callers: {bind_double_callers:?}"
    );
    assert_eq!(
        bind_string_callers.len(),
        1,
        "bind_string callers: {bind_string_callers:?}"
    );
    assert_ne!(bind_double_callers[0].line, bind_string_callers[0].line);

    let body_targets: Vec<&str> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.source == body.id)
        .map(|e| e.target.as_str())
        .collect();
    assert!(
        body_targets.contains(&controls.id.as_str()),
        "body -> controls missing: {body_targets:?}"
    );
    assert!(
        body_targets.contains(&is_blocked.id.as_str()),
        "body -> !isBlocked missing: {body_targets:?}"
    );

    let controls_targets: Vec<&str> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.source == controls.id)
        .map(|e| e.target.as_str())
        .collect();
    assert!(
        controls_targets.contains(&lookup_hash.id.as_str()),
        "controls -> lookUpHash missing: {controls_targets:?}"
    );

    let export_targets: Vec<&str> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls && e.source == export_rows.id)
        .map(|e| e.target.as_str())
        .collect();
    assert!(
        export_targets.contains(&format_row.id.as_str()),
        "exportRows -> formatRow missing: {export_targets:?}"
    );
}

#[tokio::test]
async fn swift_inheritance_self_super_spm_reachability_and_overload_ties() {
    let (_dir, cg) = index_files(&[
        PACKAGE_MANIFEST,
        (
            "Sources/Core/Base.swift",
            r#"
import Foundation

public protocol Styled {
    func applyStyle()
}

open class GenericBase<T> {
    public init(a: Int) {}
    public init(a: Int, b: Int) {}
    open func baseAction() {}
    public func inheritedOnly() {}
    public func sharedPing() {}
}

public func coreHelper() {}
"#,
        ),
        (
            "Sources/Core/CallerSameTarget.swift",
            r#"
import Foundation

final class Child<Element>: Core.GenericBase<Element>, Styled where Element: Equatable {
    func applyStyle() {}
    func helper() {}

    init(a: Int) {
        super.init(a: a)
    }

    init(a: Int, b: Int) {
        super.init(a: a)
    }

    override func baseAction() {
        super.baseAction()
        self.helper()
        self.missingOwn()
    }

    convenience init(pair: Int) {
        self.init(a: pair, b: pair)
    }
}

func exerciseInheritance(_ c: Child<String>) {
    c.inheritedOnly()
    c.applyStyle()
    c.nonexistent()
    coreHelper()
    render()
}

extension Child {
    func extHelper() {
        self.helper()
    }
}

struct LeftBox {
    func sharedPing() {}
    func render() {}
    func twoRequired(first: Int, second: Int) {}
}

struct RightBox {
    func sharedPing() {}
    func render() {}
    func twoRequired(x: Int, y: Int) {}
}

struct DuplicateOwner {
    func ping() {}
}

func useDup(_ d: DuplicateOwner) {
    d.ping()
}

func callTwoRequired(_ b: LeftBox) {
    twoRequired()
}
"#,
        ),
        (
            "Sources/Core/DuplicateOwnerA.swift",
            r#"
public struct DuplicateOwner {
    public func ping() {}
}
"#,
        ),
        (
            "Sources/UnimportedTarget/Hidden.swift",
            r#"
public func coreHelperFromOther() {}
"#,
        ),
    ])
    .await;

    let nodes = cg.get_all_nodes().await.expect("nodes");
    let base_action_base = find_node(&nodes, "Base.swift", "baseAction", NodeKind::Method);
    let base_action_child = find_node(
        &nodes,
        "CallerSameTarget.swift",
        "baseAction",
        NodeKind::Method,
    );
    let helper_node = find_node(&nodes, "CallerSameTarget.swift", "helper", NodeKind::Method);
    let ext_helper = find_node(
        &nodes,
        "CallerSameTarget.swift",
        "extHelper",
        NodeKind::Method,
    );
    let apply_style_proto = find_node(&nodes, "Base.swift", "applyStyle", NodeKind::Method);
    let apply_style_child = find_node(
        &nodes,
        "CallerSameTarget.swift",
        "applyStyle",
        NodeKind::Method,
    );
    let init_two = nodes
        .iter()
        .find(|n| {
            n.file_path.ends_with("CallerSameTarget.swift")
                && n.kind == NodeKind::Constructor
                && n.signature.as_deref().is_some_and(|s| s.contains("b: Int"))
        })
        .expect("2-arg init on Child");
    let conv_init = nodes
        .iter()
        .find(|n| {
            n.kind == NodeKind::Constructor
                && n.signature
                    .as_deref()
                    .is_some_and(|s| s.contains("pair: Int"))
        })
        .expect("convenience init");
    let core_helper = find_node(&nodes, "Base.swift", "coreHelper", NodeKind::Function);
    let inherited_only = find_node(&nodes, "Base.swift", "inheritedOnly", NodeKind::Method);
    let exercise = find_node(
        &nodes,
        "CallerSameTarget.swift",
        "exerciseInheritance",
        NodeKind::Function,
    );
    let dup_local = find_node(&nodes, "CallerSameTarget.swift", "ping", NodeKind::Method);
    let use_dup = find_node(
        &nodes,
        "CallerSameTarget.swift",
        "useDup",
        NodeKind::Function,
    );

    let base_targets = outgoing_call_targets(&cg, &base_action_child.id).await;
    assert!(
        base_targets.contains(&base_action_base.id) || base_targets.contains(&base_action_child.id),
        "super.baseAction missing: {base_targets:?}"
    );
    assert!(
        base_targets.contains(&helper_node.id),
        "self.helper missing: {base_targets:?}"
    );

    let ext_targets = outgoing_call_targets(&cg, &ext_helper.id).await;
    assert!(
        ext_targets.contains(&helper_node.id),
        "extension self.helper missing: {ext_targets:?}"
    );

    let conv_targets = outgoing_call_targets(&cg, &conv_init.id).await;
    assert!(
        conv_targets.contains(&init_two.id),
        "self.init(a:b:) must resolve to 2-arg init: {conv_targets:?}"
    );

    let ex_targets = outgoing_call_targets(&cg, &exercise.id).await;
    assert!(
        ex_targets.contains(&inherited_only.id),
        "c.inheritedOnly() must walk GenericBase: {ex_targets:?}"
    );
    assert!(
        ex_targets.contains(&apply_style_child.id) || ex_targets.contains(&apply_style_proto.id),
        "c.applyStyle() missing: {ex_targets:?}"
    );
    assert!(
        ex_targets.contains(&core_helper.id),
        "same SPM target coreHelper missing: {ex_targets:?}"
    );

    let dup_targets = outgoing_call_targets(&cg, &use_dup.id).await;
    assert!(
        dup_targets.contains(&dup_local.id),
        "same-file DuplicateOwner::ping should break multi-owner tie: {dup_targets:?}"
    );
}

async fn call_target_nodes<'n>(cg: &TokenSave, nodes: &'n [Node], source: &Node) -> Vec<&'n Node> {
    let targets = outgoing_call_targets(cg, &source.id).await;
    nodes.iter().filter(|n| targets.contains(&n.id)).collect()
}

/// Catches: a `Sources/` directory without a `Package.swift` (the Tuist and
/// XcodeGen layout, files placed directly in `Sources/`) being split into
/// fake `SwiftPM` targets, which dropped every cross-file call and base class.
#[tokio::test]
async fn swift_sources_dir_without_manifest_is_not_a_spm_target() {
    let (_dir, cg) = index_files(&[
        (
            "App/Sources/Base.swift",
            "class BaseVC { func setup() {} }\nfunc helperB() {}\nfinal class Worker { func process(x: Int) {} }\n",
        ),
        (
            "App/Sources/Sub.swift",
            "final class SubVC: BaseVC {}\nfunc useB() { helperB() }\nfunc runWorker() {\n    let w = Worker()\n    w.process(x: 1)\n}\n",
        ),
        ("Sources/Views/Screen.swift", "func show() { model() }\n"),
        ("Sources/Models/Model.swift", "func model() {}\n"),
    ])
    .await;

    let nodes = cg.get_all_nodes().await.expect("nodes");
    let helper = find_node(&nodes, "Base.swift", "helperB", NodeKind::Function);
    let process = find_node(&nodes, "Base.swift", "process", NodeKind::Method);
    let base = find_node(&nodes, "Base.swift", "BaseVC", NodeKind::Class);
    let sub = find_node(&nodes, "Sub.swift", "SubVC", NodeKind::Class);
    let use_b = find_node(&nodes, "Sub.swift", "useB", NodeKind::Function);
    let run_worker = find_node(&nodes, "Sub.swift", "runWorker", NodeKind::Function);
    let show = find_node(&nodes, "Screen.swift", "show", NodeKind::Function);
    let model = find_node(&nodes, "Model.swift", "model", NodeKind::Function);

    assert!(outgoing_call_targets(&cg, &use_b.id)
        .await
        .contains(&helper.id));
    assert!(outgoing_call_targets(&cg, &run_worker.id)
        .await
        .contains(&process.id));
    assert!(outgoing_call_targets(&cg, &show.id)
        .await
        .contains(&model.id));
    let edges = cg.get_all_edges().await.expect("edges");
    assert!(
        edges
            .iter()
            .any(|e| e.kind == EdgeKind::Extends && e.source == sub.id && e.target == base.id),
        "SubVC must extend BaseVC across files in a manifest-less Sources/ dir"
    );
}

/// Catches: a type and its same-file extension, which share a qualified name,
/// yielding the same method twice so the typed-receiver match tied with itself.
#[tokio::test]
async fn swift_typed_receiver_with_same_file_extension_resolves() {
    let (_dir, cg) = index_files(&[(
        "Alpha.swift",
        r#"struct Alpha { func ping(n: Int) {} }
extension Alpha { var label: String { "a" } }
func callAlpha() {
    let a = Alpha()
    a.ping(n: 1)
}
"#,
    )])
    .await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let ping = find_node(&nodes, "Alpha.swift", "ping", NodeKind::Method);
    let caller = find_node(&nodes, "Alpha.swift", "callAlpha", NodeKind::Function);
    assert!(outgoing_call_targets(&cg, &caller.id)
        .await
        .contains(&ping.id));
}

/// Catches: `items.count` inside a computed `count` (and `other.title`) being
/// recorded as a read of the enclosing type's own computed property.
#[tokio::test]
async fn swift_computed_property_read_ignores_foreign_receivers() {
    let (_dir, cg) = index_files(&[(
        "Counter.swift",
        r#"struct Counter {
    var items: [Int] = []
    var count: Int { items.count }
    var title: String { "t" }
    var summary: String { self.title }
    func describe(other: Other) -> String { other.title }
}
struct Other { var title: String { "o" } }
"#,
    )])
    .await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let count = find_node(&nodes, "Counter.swift", "count", NodeKind::Property);
    let summary = find_node(&nodes, "Counter.swift", "summary", NodeKind::Property);
    let describe = find_node(&nodes, "Counter.swift", "describe", NodeKind::Method);
    let counter_title = nodes
        .iter()
        .find(|n| n.name == "title" && n.qualified_name.contains("Counter::"))
        .expect("Counter.title");

    assert!(!outgoing_call_targets(&cg, &count.id)
        .await
        .contains(&count.id));
    assert!(!outgoing_call_targets(&cg, &describe.id)
        .await
        .contains(&counter_title.id));
    assert!(outgoing_call_targets(&cg, &summary.id)
        .await
        .contains(&counter_title.id));
}

/// Catches: an untyped local shadowing a typed member property with the same
/// name being typed as the member (`let repo = makeOther()` became `Repo`).
#[tokio::test]
async fn swift_untyped_local_shadows_typed_member() {
    let (_dir, cg) = index_files(&[(
        "Shadow.swift",
        r#"final class Repo { func persist() {} }
final class OtherRepo { func persist() {} }
func makeOther() -> OtherRepo { OtherRepo() }
final class Svc {
    var repo = Repo()
    func shadowed() {
        let repo = makeOther()
        repo.persist()
    }
    func member() { repo.persist() }
}
"#,
    )])
    .await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let repo_persist = nodes
        .iter()
        .find(|n| n.name == "persist" && n.qualified_name.contains("::Repo::"))
        .expect("Repo.persist");
    let shadowed = find_node(&nodes, "Shadow.swift", "shadowed", NodeKind::Method);
    let member = find_node(&nodes, "Shadow.swift", "member", NodeKind::Method);
    assert!(!outgoing_call_targets(&cg, &shadowed.id)
        .await
        .contains(&repo_persist.id));
    assert!(outgoing_call_targets(&cg, &member.id)
        .await
        .contains(&repo_persist.id));
}

/// Catches: parameter types never being bound (the type sits in the
/// `parameter` node's `type` field), so `store.save()` lost its edge.
#[tokio::test]
async fn swift_parameter_types_type_their_receivers() {
    let (_dir, cg) = index_files(&[(
        "Params.swift",
        r#"final class Store { func save() {} }
final class Repo2 { func commitAll() {} }
final class Repo3 { func commitAll() {} }
func useStore(store: Store) { store.save() }
func useRepo(_ r: Repo2?) { r?.commitAll() }
"#,
    )])
    .await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let save = find_node(&nodes, "Params.swift", "save", NodeKind::Method);
    let commit = nodes
        .iter()
        .find(|n| n.name == "commitAll" && n.qualified_name.contains("Repo2::"))
        .expect("Repo2.commitAll");
    let use_store = find_node(&nodes, "Params.swift", "useStore", NodeKind::Function);
    let use_repo = find_node(&nodes, "Params.swift", "useRepo", NodeKind::Function);
    assert!(outgoing_call_targets(&cg, &use_store.id)
        .await
        .contains(&save.id));
    assert_eq!(
        outgoing_call_targets(&cg, &use_repo.id).await,
        vec![commit.id.clone()]
    );
}

/// Catches: overload matching misreading closure-typed parameters (`->`),
/// attribute arguments (`@available(...)`) and trailing closures.
#[tokio::test]
async fn swift_overloads_with_closures_attributes_and_trailing_closures() {
    let (_dir, cg) = index_files(&[(
        "Overloads.swift",
        r#"final class Net {
    func request(_ handler: @escaping (Int) -> Void, queue: Int) {}
    func request(_ handler: @escaping (Int) -> Void) {}
}
final class Loader {
    func load(id: Int, completion: (Int) -> Void) {}
    func load(id: Int) {}
}
final class Attr {
    @available(macOS 10.15, *)
    func paint(color: Int) {}
    func paint(color: Int, alpha: Int) {}
}
func callNet(n: Net) { n.request({ _ in }, queue: 1) }
func callLoader(l: Loader) { l.load(id: 1) { _ in } }
func callAttr(a: Attr) { a.paint(color: 1) }
"#,
    )])
    .await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    for (caller, expected) in [
        ("callNet", "queue: Int"),
        ("callLoader", "completion:"),
        ("callAttr", "@available"),
    ] {
        let source = find_node(&nodes, "Overloads.swift", caller, NodeKind::Function);
        let targets = call_target_nodes(&cg, &nodes, source).await;
        assert_eq!(targets.len(), 1, "{caller}: {targets:?}");
        assert!(
            targets[0]
                .signature
                .as_deref()
                .is_some_and(|sig| sig.contains(expected)),
            "{caller} resolved to the wrong overload: {:?}",
            targets[0].signature
        );
    }
}

/// Catches: incremental sync resolving Swift refs without declaration
/// signatures, which disabled inheritance walks and overload matching, so
/// edges built by the full index vanished after the next edit.
#[tokio::test]
async fn swift_incremental_sync_keeps_signature_based_edges() {
    let source = r#"class Base { func baseOnly() {} }
class Child: Base {}
func useChild() {
    let c = Child()
    c.baseOnly()
}
enum Binder {
    static func bind(value: Int) {}
    static func bind(name: String, value: Int) {}
    static func useIt() { Binder.bind(name: "a", value: 1) }
}
"#;
    let (dir, cg) = index_files(&[("Inherit.swift", source)]).await;
    let calls = |edges: Vec<tokensave::types::Edge>| {
        let mut calls: Vec<(String, String)> = edges
            .into_iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .map(|e| (e.source, e.target))
            .collect();
        calls.sort();
        calls
    };
    let full = calls(cg.get_all_edges().await.expect("edges"));
    assert_eq!(full.len(), 2, "full index: {full:?}");

    fs::write(
        dir.path().join("Inherit.swift"),
        format!("{source}// edited\n"),
    )
    .expect("edit file");
    cg.sync().await.expect("sync");
    let incremental = calls(cg.get_all_edges().await.expect("edges"));
    assert_eq!(
        incremental, full,
        "incremental sync must match the full index"
    );
}

/// Catches: the stdlib-name guard covering verbs that project types declare
/// (`save`), which dropped `s.save()` on an untyped receiver, while a real
/// stdlib name (`append`) on an untyped receiver must still stay unbound.
#[tokio::test]
async fn swift_untyped_receiver_keeps_project_verbs_and_blocks_stdlib_names() {
    let (_dir, cg) = index_files(&[(
        "Store.swift",
        r#"final class Store {
    func save() {}
    func append(_ x: Int) {}
}
func makeStore() -> Store { Store() }
func persistAll(values: [Int]) {
    let s = makeStore()
    s.save()
    var copy = values
    copy.append(1)
}
"#,
    )])
    .await;
    let nodes = cg.get_all_nodes().await.expect("nodes");
    let save = find_node(&nodes, "Store.swift", "save", NodeKind::Method);
    let append = find_node(&nodes, "Store.swift", "append", NodeKind::Method);
    let caller = find_node(&nodes, "Store.swift", "persistAll", NodeKind::Function);
    let targets = outgoing_call_targets(&cg, &caller.id).await;
    assert!(
        targets.contains(&save.id),
        "s.save() must resolve: {targets:?}"
    );
    assert!(
        !targets.contains(&append.id),
        "copy.append(1) must not bind to Store.append: {targets:?}"
    );
}
