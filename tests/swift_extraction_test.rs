use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::SwiftExtractor;
use tokensave::types::{EdgeKind, NodeKind, Visibility};

#[test]
fn test_swift_extract_imports() {
    let source = r#"import Foundation
import UIKit
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("sample.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().any(|n| n.name == "Foundation"));
    assert!(uses.iter().any(|n| n.name == "UIKit"));
}

#[test]
fn test_swift_extract_class() {
    let source = r#"/// Base class.
class Base {
    let name: String

    init(name: String) {
        self.name = name
    }

    func description() -> String {
        return name
    }
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("base.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Base");
    assert!(
        classes[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("Base class"),
        "docstring: {:?}",
        classes[0].docstring
    );
}

#[test]
fn test_swift_class_inheritance() {
    let source = r#"class Base {}
class Connection: Base {}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("conn.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let extends: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(!extends.is_empty(), "expected Extends refs for inheritance");
    assert!(
        extends.iter().any(|r| r.reference_name == "Base"),
        "expected Extends ref to Base"
    );
}

#[test]
fn test_swift_function_vs_method() {
    let source = r#"func topLevel() {}

class Foo {
    func method() {}
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("funcs.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "topLevel");

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "method");
}

#[test]
fn test_swift_struct_with_fields_and_methods() {
    let source = r#"struct Point {
    let x: Double
    let y: Double

    func distance(to other: Point) -> Double {
        return 0.0
    }
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("point.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert!(
        fields.len() >= 2,
        "expected >= 2 stored fields, got {}",
        fields.len()
    );

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "distance");
}

#[test]
fn test_swift_enum_with_variants() {
    let source = r#"enum LogLevel {
    case debug
    case info
    case warning
    case error
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("log.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "LogLevel");

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 4);
    assert!(variants.iter().any(|v| v.name == "debug"));
    assert!(variants.iter().any(|v| v.name == "info"));
    assert!(variants.iter().any(|v| v.name == "warning"));
    assert!(variants.iter().any(|v| v.name == "error"));
}

#[test]
fn test_swift_protocol_as_interface() {
    let source = r#"/// Serializable protocol.
protocol Serializable {
    func toJson() -> [String: Any]
    func toJsonString() -> String
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("proto.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let ifaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "Serializable");
    assert!(
        ifaces[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("Serializable"),
        "protocol should have docstring"
    );

    // Protocol functions should be methods.
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_swift_extension() {
    let source = r#"extension String {
    func toSlug() -> String {
        return lowercased()
    }
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("ext.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let exts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Extension)
        .collect();
    assert_eq!(exts.len(), 1);
    assert_eq!(exts[0].name, "String");

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "toSlug");
}

#[test]
fn test_swift_constructor() {
    let source = r#"class Foo {
    init(name: String) {}
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("foo.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let ctors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(ctors.len(), 1);
    assert_eq!(ctors[0].name, "init");
}

#[test]
fn test_swift_call_sites() {
    let source = r#"func greet() {
    print("hello")
}

func main() {
    greet()
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("main.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
    assert!(
        call_refs.iter().any(|r| r.reference_name == "print"),
        "should find print call"
    );
    assert!(
        call_refs.iter().any(|r| r.reference_name == "greet"),
        "should find greet call"
    );
}

/// Catches: call names that keep raw expression text (`48 + Double`,
/// `await poll(x)`), drop instance receivers (`worker.start`, `items.allSatisfy`),
/// or drop the type receiver of a static call.
#[test]
fn test_swift_callee_names_by_receiver_shape() {
    let source = include_str!("fixtures/swift_callee_shapes.swift");
    let result = SwiftExtractor.extract("swift_callee_shapes.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mut names: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "Cache::open",
            "Double",
            "Inner::make",
            "helper",
            "items.allSatisfy(0:)",
            "poll",
            "worker.start"
        ]
    );
}

#[test]
fn test_swift_typed_local_and_self_property_receivers() {
    let source = r#"import Foundation
@testable import Smack

struct CustomStore {
    func saveSnapshot() {}
}

class Worker {
    let store: CustomStore
    var items: [String] = []

    init(store: CustomStore) {
        self.store = store
    }

    func append(_ value: String) {
        var localBytes = Data()
        localBytes.append(0x01)
        items.append(value)
        store.saveSnapshot()
        let encoder = JSONEncoder()
        _ = try? encoder.encode(value)
        self.runHelper()
    }

    func runHelper() {}
}
"#;
    let result = SwiftExtractor.extract("Worker.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let use_names: Vec<&str> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .map(|n| n.name.as_str())
        .collect();
    assert!(use_names.contains(&"Foundation"));
    assert!(use_names.contains(&"Smack"));

    let call_names: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();

    assert!(call_names.contains(&"Data::append"), "got {call_names:?}");
    assert!(
        !call_names.contains(&"localBytes.append"),
        "stdlib receiver should not emit untyped ref: {call_names:?}"
    );
    assert!(call_names.contains(&"Array::append"), "got {call_names:?}");
    assert!(
        call_names.contains(&"JSONEncoder::encode"),
        "got {call_names:?}"
    );
    assert!(
        call_names.contains(&"CustomStore::saveSnapshot"),
        "got {call_names:?}"
    );
    assert!(call_names.contains(&"self.runHelper"), "got {call_names:?}");
}

#[test]
fn test_swift_missing_call_shapes_computed_props_refs_and_selector() {
    let source = r#"import SwiftUI

struct ItemView: View {
    let title: String
    var body: some View {
        VStack {
            header
            chart.frame(width: 100)
            Button(action: handleTap) {
                Text(title)
            }
        }
    }

    private var header: some View { Text(title) }
    private var chart: some View { Text("chart") }

    func handleTap() {}

    func checkIP(_ value: String) -> Bool {
        return !isValidIP(value) && [value].map(normalizeField).count > 0
    }

    func registerTarget(_ target: AnyObject) -> Selector {
        #selector(handleTap)
    }

    private func isValidIP(_ value: String) -> Bool { !value.isEmpty }
    private func normalizeField(_ value: String) -> String { value }
}
"#;
    let result = SwiftExtractor.extract("ItemView.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_names: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();

    assert!(
        call_names.iter().any(|n| n.ends_with("header")),
        "expected computed property read 'header' in {call_names:?}"
    );
    assert!(
        call_names.iter().any(|n| n.ends_with("chart")),
        "expected computed property read 'chart' in {call_names:?}"
    );
    assert!(
        call_names.iter().any(|n| n.ends_with("handleTap")),
        "expected function ref / selector 'handleTap' in {call_names:?}"
    );
    assert!(
        call_names.iter().any(|n| n.ends_with("isValidIP")),
        "expected prefix expression call '!isValidIP' in {call_names:?}"
    );
    assert!(
        call_names.iter().any(|n| n.ends_with("normalizeField")),
        "expected higher-order function ref 'normalizeField' in {call_names:?}"
    );
    assert!(
        !call_names
            .iter()
            .any(|n| *n == "title" || n.ends_with("::title")),
        "stored property 'title' must not emit a Calls ref: {call_names:?}"
    );
}

#[test]
fn test_swift_docstrings() {
    let source = r#"/// Initializes the system.
func setup() {}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("doc.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(
        fns[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("Initializes the system"),
        "docstring: {:?}",
        fns[0].docstring
    );
}

#[test]
fn test_swift_file_node_is_root() {
    let source = r#"func main() {}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("main.swift", source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "main.swift");
}

#[test]
fn test_swift_contains_edges() {
    let source = r#"class Foo {
    let bar: Int

    func baz() {}
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("foo.swift", source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: Class; Class contains: Property, Method
    assert!(
        contains.len() >= 3,
        "should have >= 3 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_swift_typealias() {
    let source = r#"typealias CompletionHandler = (Bool) -> Void
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("alias.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let aliases: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "CompletionHandler");
}

#[test]
fn test_swift_top_level_const() {
    let source = r#"let maxConnections = 100
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("const.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "maxConnections");
}

#[test]
fn test_swift_visibility_private() {
    let source = r#"class Foo {
    private func secret() {}
    func public_method() {}
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("vis.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let secret = result
        .nodes
        .iter()
        .find(|n| n.name == "secret")
        .expect("secret method not found");
    assert_eq!(secret.visibility, Visibility::Private);

    let public_method = result
        .nodes
        .iter()
        .find(|n| n.name == "public_method")
        .expect("public_method not found");
    assert_eq!(public_method.visibility, Visibility::Pub);
}

#[test]
fn test_swift_async_function() {
    let source = r#"class Conn {
    func connect() async throws {
        print("connecting")
    }
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("async.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let connect = result
        .nodes
        .iter()
        .find(|n| n.name == "connect")
        .expect("connect method not found");
    assert!(connect.is_async, "connect should be async");
}

#[test]
fn test_swift_annotation_extraction() {
    let source = r#"
@objc class MyController {
    @discardableResult
    func doWork() -> Bool {
        return true
    }

    @available(iOS 13, *)
    func newFeature() {
        print("new")
    }
}
"#;
    let extractor = SwiftExtractor;
    let result = extractor.extract("attrs.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();

    let annot_names: Vec<&str> = annots.iter().map(|a| a.name.as_str()).collect();

    assert!(
        annot_names.contains(&"objc"),
        "expected 'objc' annotation, got: {:?}",
        annot_names
    );

    assert!(
        annot_names.contains(&"discardableResult"),
        "expected 'discardableResult' annotation, got: {:?}",
        annot_names
    );

    assert!(
        annot_names.contains(&"available"),
        "expected 'available' annotation, got: {:?}",
        annot_names
    );

    // Verify Annotates edges exist
    let annotates_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert!(
        !annotates_edges.is_empty(),
        "expected Annotates edges, found none"
    );
    assert_eq!(
        annotates_edges.len(),
        annots.len(),
        "each AnnotationUsage should have an Annotates edge"
    );

    // Verify Annotates unresolved refs exist
    let annotates_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(
        annotates_refs.len(),
        annots.len(),
        "each AnnotationUsage should have an Annotates unresolved ref"
    );
}

/// Catches: `max_nesting` staying 0 for every Swift function because the
/// nesting node kind did not exist in tree-sitter-swift.
#[test]
fn test_swift_max_nesting() {
    let source = include_str!("fixtures/swift_nesting.swift");
    let result = SwiftExtractor.extract("swift_nesting.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let nesting = |name: &str| {
        result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Function && n.name == name)
            .map(|n| n.max_nesting)
    };
    assert_eq!(nesting("deep"), Some(4), "function body, if, for, if");
    assert_eq!(nesting("flat"), Some(1), "function body only");
}

#[test]
fn test_swift_complexity_loops_branches_logical_and_halstead() {
    let source = r#"
func empty() {}

func complexWork(items: [Int], fallback: Int?) -> Int {
    var sum = fallback ?? 0
    let factor = sum > 10 ? 2 : 1
    for item in items {
        if item > 0 && (item < 100 || item == 200) {
            sum += item * factor
        }
    }
    var count = 0
    while count < 3 {
        count += 1
    }
    repeat {
        count -= 1
    } while count > 0
    return sum + count
}
"#;
    let result = SwiftExtractor.extract("ComplexitySample.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let find_fn = |name: &str| {
        result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Function && n.name == name)
            .unwrap_or_else(|| panic!("missing function {name}"))
    };

    let empty = find_fn("empty");
    assert_eq!(empty.loops, 0, "empty function must have 0 loops");
    assert_eq!(empty.branches, 0, "empty function must have 0 branches");
    assert_eq!(
        empty.cognitive_complexity, 0,
        "empty function cognitive must be 0"
    );

    let work = find_fn("complexWork");
    assert_eq!(
        work.loops, 3,
        "expected for-in + while + repeat-while = 3 loops, got {}",
        work.loops
    );
    assert!(
        work.branches >= 3,
        "expected if + ternary + nil-coalescing >= 3 branches, got {}",
        work.branches
    );
    assert!(
        work.cognitive_complexity >= 6,
        "expected cognitive_complexity >= 6 with loops/if/&&/||, got {}",
        work.cognitive_complexity
    );
    assert!(
        work.distinct_operators > 0,
        "expected distinct_operators > 0"
    );
    assert!(work.distinct_operands > 0, "expected distinct_operands > 0");
    assert!(work.total_operators > 0, "expected total_operators > 0");
    assert!(work.total_operands > 0, "expected total_operands > 0");
    let volume = tokensave::extraction::complexity::halstead_volume(
        work.distinct_operators,
        work.distinct_operands,
        work.total_operators,
        work.total_operands,
    );
    assert!(
        volume > 0.0,
        "expected positive halstead_volume, got {volume}"
    );
}

#[test]
fn test_swift_stored_fields_vs_computed_properties_and_docstrings() {
    let source = r#"
class Account {
    /// Stored identifier.
    let id: Int = 1

    /// Stored mutable balance with observer.
    var balance: Double = 0 {
        didSet { }
    }

    /// Computed summary label.
    var summary: String {
        return "\(id):\(balance)"
    }
}
"#;
    let result = SwiftExtractor.extract("Account.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let find = |name: &str| {
        result
            .nodes
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("missing node {name}"))
    };

    let id_node = find("id");
    assert_eq!(id_node.kind, NodeKind::Field, "stored let must be Field");
    assert!(
        id_node
            .docstring
            .as_deref()
            .is_some_and(|d| d.contains("Stored identifier")),
        "missing docstring on stored let: {:?}",
        id_node.docstring
    );

    let balance_node = find("balance");
    assert_eq!(
        balance_node.kind,
        NodeKind::Field,
        "didSet/willSet property must remain stored Field"
    );
    assert!(
        balance_node
            .docstring
            .as_deref()
            .is_some_and(|d| d.contains("mutable balance")),
        "missing docstring on observer var: {:?}",
        balance_node.docstring
    );

    let summary_node = find("summary");
    assert_eq!(
        summary_node.kind,
        NodeKind::Property,
        "computed property must remain Property"
    );
    assert!(
        summary_node
            .docstring
            .as_deref()
            .is_some_and(|d| d.contains("Computed summary label")),
        "missing docstring on computed property: {:?}",
        summary_node.docstring
    );
}

#[test]
fn test_swift_adversarial_prefix_self_generics_and_sibling_names() {
    let source = r#"
class Session {
    let repo: Repo<User>
    var series: [Int] { [1, 2] }

    func series(enabled: Bool) -> [Int] {
        guard !Self.hasCRLF(in: "a"), let session else { return [] }
        session?.repo.save()
        return repo.save()
    }

    static func hasCRLF(in text: String) -> Bool { false }
}
"#;
    let result = SwiftExtractor.extract("Adversarial.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let series_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "series")
        .expect("series method");
    let calls: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls && r.from_node_id == series_fn.id)
        .map(|r| r.reference_name.as_str())
        .collect();

    assert!(
        calls.contains(&"hasCRLF(in:)"),
        "!Self.hasCRLF(in:) must unwrap `!` before `Self`: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.contains("<expr>.hasCRLF")),
        "!Self.hasCRLF must not fall back to <expr>: {calls:?}"
    );
    assert!(
        calls.contains(&"Repo::save"),
        "generic property `repo: Repo<User>` must emit `Repo::save`: {calls:?}"
    );
    assert!(
        !calls.contains(&"Session::series"),
        "func declaration identifier `series` must not emit read edge to sibling property `series`: {calls:?}"
    );
}

#[test]
fn test_swift_full_extractor_branches_types_selectors_attributes_and_malformed() {
    let source = r#"
import Foundation

internal enum Mode: RawRepresentable {
    var rawValue: Int { 0 }
    init?(rawValue: Int) { nil }
    case fast, slow
}

public protocol Renderable {
    var titleText: String { get }
    func draw(in rect: String)
}

@available(*, deprecated, message: "use v2")
// bridge comment between attributes
@objc
class Widget: Foundation.NSObject, Sendable {
    let optSvc: Service? = nil
    let unwrappedSvc: Service! = nil
    let mapProp: [String: Int] = [:]
    let store: Store = Store()
    var arrLit = [1, 2]
    var dictLit = ["k": 1]
    var strLit = "hello"
    var mlStr = """
    multi
    """

    var labelProp: String { "lbl" }

    func extremelyLongSignatureWithUtf8AndMultiLineParameters_alphaBetaGammaDeltaEpsilonZeta(
        firstParameterWithDescriptiveName p1: Dictionary<String, Int>,
        secondParameterWithDescriptiveName p2: [Service],
        thirdParameterWithDescriptiveName p3: String = "café_résumé_über_long_padding_value_1234567890",
        fourthParameterWithDescriptiveName p4: Bool = false
    ) -> [String] {
        func nestedHelper() {}
        class NestedInner {}
        protocol NestedProto {}

        let castSvc = optSvc as? Service
        let ctorArr = [Int]()
        let me = self
        let items = [1, 2, 3]
        let fns = items.map { item, (idx: Int) in
            optSvc!.ping()
            castSvc?.ping()
            ctorArr.append(idx)
            me.trigger()
            return item
        }
        if let localOpt = optSvc {
            localOpt.ping()
        }
        guard let guardedOpt = unwrappedSvc else { return [] }
        for entry in fns {
            _ = entry
        }
        let s = Store.shared
        s.save()
        let viaSelfStore = self.store
        viaSelfStore.save()
        self.store.save()

        _ = .makeDefault()
        _ = getBuilder().build()
        _ = items.map(externalFreeTransform)
        _ = Button(action: self.trigger)
        _ = Button(action: Self.trigger)
        _ = Button(action: self.externalAction)
        _ = Button(action: Helper.formatItem)
        _ = Button(action: Widget.self)
        _ = Button(action: .compact)

        _ = #selector(trigger)
        _ = #selector(Widget.trigger)
        _ = #selector(self.trigger)
        _ = #selector(self.untrackedSelector)
        _ = #selector(externalSelectorFn)
        _ = #selector(labelProp)
        _ = labelProp()
        _ = otherObj.labelProp()
        _ = guardedOpt
        return []
    }

    @objc func trigger() {}
    internal func internalTrigger() {}
    static func makeDefault() -> Widget { Widget() }
}
"#;
    let res = SwiftExtractor.extract("FullBranches.swift", source);
    assert!(res.errors.is_empty(), "errors: {:?}", res.errors);

    let long_fn = res
        .nodes
        .iter()
        .find(|n| n.name.starts_with("extremelyLongSignature"))
        .expect("long_fn");
    let sig = long_fn.signature.as_deref().expect("signature");
    assert!(
        sig.len() <= 300,
        "signature exceeds 300 bytes: {}",
        sig.len()
    );

    let mode_enum = res
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Enum && n.name == "Mode")
        .expect("Mode enum");
    assert_eq!(mode_enum.visibility, Visibility::PubCrate);
    let internal_fn = res
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "internalTrigger")
        .expect("internalTrigger");
    assert_eq!(internal_fn.visibility, Visibility::PubCrate);

    assert!(res
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::AnnotationUsage && n.name == "available"));
    assert!(res
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::AnnotationUsage && n.name == "objc"));

    let call_names: Vec<&str> = res
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls && r.from_node_id == long_fn.id)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(call_names.contains(&"Service::ping"), "{call_names:?}");
    assert!(call_names.contains(&"Store::save"), "{call_names:?}");
    assert!(call_names.contains(&"Array::map"), "{call_names:?}");
    assert!(call_names.contains(&"makeDefault"), "{call_names:?}");
    assert!(
        call_names.contains(&"externalFreeTransform"),
        "{call_names:?}"
    );
    assert!(call_names.contains(&"Helper::formatItem"), "{call_names:?}");
    assert!(call_names.contains(&"Widget::trigger"), "{call_names:?}");
    assert!(call_names.contains(&"untrackedSelector"), "{call_names:?}");
    assert!(call_names.contains(&"externalSelectorFn"), "{call_names:?}");

    // Exercise partial syntax paths cleanly without empty-name panics.
    let partial = "import \nextension {}\nclass {}\nprotocol {}\n@custom\nfunc f() {}\n";
    let partial_res = SwiftExtractor.extract("Partial.swift", partial);
    assert!(partial_res.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Cover UTF-8 multi-byte signature truncation boundary (`cut -= 1`),
    // unparenthesized closure params, postfix/init/literal expressions, `#selector`
    // edge forms, leading-dot shorthand calls, and tuple destructuring.
    let pad_293 = "a".repeat(293);
    let extra_source = format!(
        r#"
postfix operator ^^^
postfix func ^^^ (_ x: Box) -> Box {{ x }}
struct Box {{}}
func makeBox() -> Box {{ Box() }}

protocol Props {{
    var protoProp: Int {{ get }}
}}

func {pad_293}中Suffix(param: Int) {{
    let pair: (Int, String) = (1, "x")
    let (firstItem, secondItem) = pair
    let builtInit = Store.init()
    let arrTyped = [Int]()
    let dictTyped = [String: Int]()
    let boxed = makeBox()^^^
    var untyped = unknownInit()
    let fnUnparen = {{ item, idx in
        helper(item, idx)
    }}
    _ = items.map(.compact)
    _ = #selector(handleCall(_:))
    _ = #selector(123)
    _ = #selector()
    take(.makeDefault())
    _ = firstItem
    _ = secondItem
    _ = builtInit
    _ = arrTyped
    _ = dictTyped
    _ = boxed
    _ = fnUnparen
    _ = untyped
}}
"#
    );
    let extra_res = SwiftExtractor.extract("ExtraBranches.swift", &extra_source);
    let utf8_fn = extra_res
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name.ends_with("Suffix"))
        .expect("utf8_fn");
    let utf8_sig = utf8_fn.signature.as_deref().expect("utf8_sig");
    assert!(utf8_sig.len() < 300, "sig len {}", utf8_sig.len());
}

#[test]
fn test_swift_private_extension_guards_nested_fns_shorthand_trailing_and_defaults() {
    let source = r#"import Foundation

private extension Array {
    mutating func append(optional: String?, key: String) {}
    public func exposedHelper() {}
    var countTag: Int { 1 }
}

struct Repo {
    func save() {}
}

struct Collector {
    static func appleSignature(_ path: String) -> Bool { true }
    init(isAppleSigned: (String) -> Bool = Collector.appleSignature) {}
}

struct HeroView {
    var body: String { uptime }
    var uptime: String {
        style(.body.weight())
    }
    var isEnabled: Bool { true }
    func check() -> Bool { !isEnabled }
}

final class Reader {
    func servers(for key: String) -> [String] { [] }
    private static func parseValue(_ raw: String?) -> String? { raw }
    static func parse(_ object: [String: Any], maybeRepo: Repo?) {
        func localHelper() {}
        func flush() {
            localHelper()
            _ = parseValue("x")
        }
        flush()
        guard let servers = object["mcpServers"] as? [String: String],
              let repo = maybeRepo else {
            return
        }
        _ = servers["main"]
        _ = servers[computeKey()]
        repo.save()
        _ = installedTerminals { true }
    }
}
"#;
    let result = SwiftExtractor.extract("EdgeCases.swift", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // B1: private extension and unannotated members inherit Private; explicit `public` keeps Pub.
    let priv_ext = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Extension && n.name == "Array")
        .expect("Array extension");
    assert_eq!(priv_ext.visibility, Visibility::Private);
    let priv_append = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "append")
        .expect("append method");
    assert_eq!(priv_append.visibility, Visibility::Private);
    let pub_helper = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "exposedHelper")
        .expect("exposedHelper method");
    assert_eq!(pub_helper.visibility, Visibility::Pub);
    let priv_prop = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Property && n.name == "countTag")
        .expect("countTag prop");
    assert_eq!(priv_prop.visibility, Visibility::Private);

    // Collect calls emitted by `Reader::parse`.
    let parse_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "parse")
        .expect("parse method");
    let parse_calls: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.from_node_id == parse_node.id && r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();

    // B2: `servers["main"]` must not emit `servers`, while `computeKey()` inside subscript is kept,
    // and `guard let repo` stays bound in the enclosing frame.
    assert!(
        !parse_calls.contains(&"servers"),
        "subscript must not emit call to `servers`: {parse_calls:?}"
    );
    assert!(
        parse_calls.contains(&"computeKey"),
        "call inside subscript index must be kept: {parse_calls:?}"
    );
    assert!(
        parse_calls.contains(&"Repo::save"),
        "guard let binding must persist after guard: {parse_calls:?}"
    );

    // B4: nested local `flush` attributes inner `parseValue` call to `parse`, without emitting `flush` or `localHelper`.
    assert!(
        parse_calls.contains(&"parseValue"),
        "call inside nested local func must be attributed to enclosing method: {parse_calls:?}"
    );
    assert!(
        !parse_calls.contains(&"flush") && !parse_calls.contains(&"localHelper"),
        "local func calls should stay local: {parse_calls:?}"
    );

    // B6a: single trailing-closure call emits `(0:)`.
    assert!(
        parse_calls.contains(&"installedTerminals(0:)"),
        "single trailing-closure call should carry `(0:)`: {parse_calls:?}"
    );

    // B5: `.body.weight()` must NOT emit `HeroView::body`, while `!isEnabled` keeps `HeroView::isEnabled`.
    let uptime_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Property && n.name == "uptime")
        .expect("uptime property");
    let uptime_calls: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.from_node_id == uptime_node.id && r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        !uptime_calls.contains(&"HeroView::body"),
        "leading-dot `.body` must not emit computed property read: {uptime_calls:?}"
    );
    let check_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "check")
        .expect("check method");
    let check_calls: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.from_node_id == check_node.id && r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        check_calls.contains(&"HeroView::isEnabled"),
        "`!isEnabled` must still emit computed property read: {check_calls:?}"
    );

    // B6c: default parameter value emits `Collector::appleSignature`.
    let init_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Constructor && n.name == "init")
        .expect("Collector.init");
    let init_calls: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.from_node_id == init_node.id && r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        init_calls.contains(&"Collector::appleSignature"),
        "default param function ref missing: {init_calls:?}"
    );
}
