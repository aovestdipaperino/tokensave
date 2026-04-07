use tokensave::extraction::DartExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    DartExtractor::extract_dart("test.dart", source)
}

#[test]
fn test_dart_file_node_is_root() {
    let result = extract("void main() {}");
    let file_nodes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(file_nodes.len(), 1);
    assert_eq!(file_nodes[0].name, "test.dart");
}

#[test]
fn test_dart_library_declaration() {
    let result = extract("library my_lib;\n\nvoid main() {}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let libs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Library)
        .collect();
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "my_lib");
}

#[test]
fn test_dart_import_extraction() {
    let result = extract("import 'dart:core';\nimport 'package:flutter/material.dart';");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let imports: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(imports.len(), 2);
    assert!(imports.iter().any(|n| n.name == "dart:core"));
    assert!(imports
        .iter()
        .any(|n| n.name == "package:flutter/material.dart"));
}

#[test]
fn test_dart_export_extraction() {
    let result = extract("export 'src/my_lib.dart';");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let exports: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].name, "src/my_lib.dart");
}

#[test]
fn test_dart_function_extraction() {
    let result = extract("void greet(String name) {\n  print('Hello $name');\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "greet");
    assert_eq!(fns[0].visibility, Visibility::Pub);
}

#[test]
fn test_dart_class_extraction() {
    let result = extract(
        "class MyClass {\n  String name;\n  void hello() {\n    print('hi');\n  }\n}",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
}

#[test]
fn test_dart_abstract_class() {
    let result = extract("abstract class Animal {\n  void speak();\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Abstract classes are mapped to Interface kind.
    let ifaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "Animal");
}

#[test]
fn test_dart_mixin_extraction() {
    let result = extract("mixin Swimming {\n  void swim() {\n    print('swimming');\n  }\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mixins: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Mixin)
        .collect();
    assert_eq!(mixins.len(), 1);
    assert_eq!(mixins[0].name, "Swimming");
}

#[test]
fn test_dart_extension_extraction() {
    let result = extract(
        "extension StringHelper on String {\n  bool get isBlank => trim().isEmpty;\n}",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let exts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Extension)
        .collect();
    assert_eq!(exts.len(), 1);
    assert_eq!(exts[0].name, "StringHelper");
}

#[test]
fn test_dart_enum_extraction() {
    let result = extract("enum Color {\n  red,\n  green,\n  blue,\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 3);
    assert!(variants.iter().any(|n| n.name == "red"));
    assert!(variants.iter().any(|n| n.name == "green"));
    assert!(variants.iter().any(|n| n.name == "blue"));
}

#[test]
fn test_dart_method_inside_class() {
    let result = extract("class Foo {\n  void bar() {\n    print('bar');\n  }\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "bar");
}

#[test]
fn test_dart_constructor_extraction() {
    let result = extract("class Greeter {\n  String name;\n  Greeter(this.name);\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ctors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(ctors.len(), 1);
    assert!(ctors[0].name.contains("Greeter"));
}

#[test]
fn test_dart_field_extraction() {
    let result = extract("class Foo {\n  String name;\n  int _count;\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert!(fields.len() >= 2, "Expected at least 2 fields, got {:?}", fields);
    assert!(fields.iter().any(|n| n.name == "name"));
    assert!(fields.iter().any(|n| n.name == "_count"));
}

#[test]
fn test_dart_typedef_extraction() {
    let result = extract("typedef IntList = List<int>;");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let types: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].name, "IntList");
}

#[test]
fn test_dart_doc_comment_extraction() {
    let result = extract("/// A greeting function.\nvoid greet() {}");
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
            .contains("greeting function"),
        "docstring was: {:?}",
        fns[0].docstring
    );
}

#[test]
fn test_dart_visibility_private() {
    let result = extract("void _privateFunc() {}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "_privateFunc");
    assert_eq!(fns[0].visibility, Visibility::Private);
}

#[test]
fn test_dart_visibility_public() {
    let result = extract("void publicFunc() {}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "publicFunc");
    assert_eq!(fns[0].visibility, Visibility::Pub);
}

#[test]
fn test_dart_async_function_detection() {
    let result = extract("Future<void> fetchData() async {\n  await Future.delayed(Duration(seconds: 1));\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "fetchData");
    assert!(fns[0].is_async, "function should be async");
}

#[test]
fn test_dart_call_site_tracking() {
    let result = extract(
        "void main() {\n  print('hello');\n  greet('world');\n}",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        calls.len() >= 2,
        "Expected at least 2 calls, got {}: {:?}",
        calls.len(),
        calls
    );
    assert!(
        calls.iter().any(|c| c.reference_name == "print"),
        "Expected a call to 'print', calls: {:?}",
        calls
    );
    assert!(
        calls.iter().any(|c| c.reference_name == "greet"),
        "Expected a call to 'greet', calls: {:?}",
        calls
    );
}

#[test]
fn test_dart_contains_edges() {
    let result = extract("class Foo {\n  void bar() {}\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File -> Class, Class -> Method
    assert!(
        contains_edges.len() >= 2,
        "Expected at least 2 Contains edges, got {}: {:?}",
        contains_edges.len(),
        contains_edges
    );
}

#[test]
fn test_dart_language_extractor_trait() {
    let extractor = DartExtractor;
    assert_eq!(extractor.extensions(), &["dart"]);
    assert_eq!(extractor.language_name(), "Dart");

    let result = extractor.extract("test.dart", "void main() {}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(!result.nodes.is_empty());
}

#[test]
fn test_dart_mixin_with_methods() {
    let result = extract(
        "mixin Logging {\n  void log(String msg) {\n    print(msg);\n  }\n}",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mixins: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Mixin)
        .collect();
    assert_eq!(mixins.len(), 1);

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "log");
}

#[test]
fn test_dart_private_field_visibility() {
    let result = extract("class Foo {\n  int _count = 0;\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert!(fields.len() >= 1);
    let private_field = fields.iter().find(|n| n.name == "_count").unwrap();
    assert_eq!(private_field.visibility, Visibility::Private);
}

#[test]
fn test_dart_qualified_names() {
    let result = extract("class MyClass {\n  void myMethod() {}\n}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let method = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method)
        .unwrap();
    assert!(
        method.qualified_name.contains("MyClass"),
        "qualified name should contain class name: {}",
        method.qualified_name
    );
    assert!(
        method.qualified_name.contains("myMethod"),
        "qualified name should contain method name: {}",
        method.qualified_name
    );
}

#[test]
fn test_dart_no_errors_complex_code() {
    let source = r#"
library my_app;

import 'dart:async';
import 'package:flutter/material.dart';

/// The main application class.
class MyApp extends StatelessWidget {
  final String title;

  const MyApp({required this.title});

  @override
  Widget build(BuildContext context) {
    return Container();
  }
}

abstract class Repository {
  Future<List<String>> getAll();
}

mixin Cacheable {
  final Map<String, dynamic> _cache = {};

  void cacheItem(String key, dynamic value) {
    _cache[key] = value;
  }
}

extension DateTimeHelper on DateTime {
  bool get isWeekend => weekday == 6 || weekday == 7;
}

enum Status {
  pending,
  active,
  completed,
}

typedef Callback = void Function(String);

Future<void> main() async {
  runApp(MyApp(title: 'Hello'));
}
"#;
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Verify all major node types are present.
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Library));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Use));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Interface)); // abstract class
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Mixin));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Extension));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Enum));
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::TypeAlias));
}

#[test]
fn test_dart_annotation_extraction() {
    let source = r#"
@deprecated
class OldWidget {
  @override
  String toString() {
    return 'OldWidget';
  }
}
"#;
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();

    let annot_names: Vec<&str> = annots.iter().map(|a| a.name.as_str()).collect();

    assert!(
        annot_names.contains(&"deprecated"),
        "expected 'deprecated' annotation, got: {:?}",
        annot_names
    );

    assert!(
        annot_names.contains(&"override"),
        "expected 'override' annotation, got: {:?}",
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
