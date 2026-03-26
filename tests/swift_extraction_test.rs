use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::SwiftExtractor;
use tokensave::types::*;

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
        classes[0].docstring.as_ref().unwrap().contains("Base class"),
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

    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert!(props.len() >= 2, "expected >= 2 properties, got {}", props.len());

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
