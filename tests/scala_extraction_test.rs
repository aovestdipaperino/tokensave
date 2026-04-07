use tokensave::extraction::ScalaExtractor;
use tokensave::types::{EdgeKind, NodeKind};

fn extract(source: &str) -> tokensave::types::ExtractionResult {
    ScalaExtractor::extract_scala("test.scala", source)
}

#[test]
fn test_scala_file_node_is_root() {
    let result = extract("object Main");
    let file_nodes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(file_nodes.len(), 1);
    assert_eq!(file_nodes[0].name, "test.scala");
}

#[test]
fn test_scala_extract_package() {
    let result = extract("package com.example.app\n\nobject Main");
    let pkgs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ScalaPackage)
        .collect();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "com.example.app");
}

#[test]
fn test_scala_extract_import() {
    let result = extract("import scala.collection.mutable.ListBuffer\nimport java.io._");
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(imports.len(), 2);
    assert!(imports.iter().any(|n| n.name.contains("ListBuffer")));
}

#[test]
fn test_scala_extract_class() {
    let result = extract("class MyClass(val x: Int) {\n  def hello(): String = \"hi\"\n}");
    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
}

#[test]
fn test_scala_extract_case_class() {
    let result = extract("case class Person(name: String, age: Int)");
    let case_classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::CaseClass)
        .collect();
    assert_eq!(case_classes.len(), 1);
    assert_eq!(case_classes[0].name, "Person");
}

#[test]
fn test_scala_extract_trait() {
    let result = extract("trait Greeter {\n  def greet(name: String): String\n}");
    let traits: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Trait).collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Greeter");
}

#[test]
fn test_scala_extract_abstract_method_in_trait() {
    let result = extract("trait Greeter {\n  def greet(name: String): String\n}");
    let abstract_methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AbstractMethod)
        .collect();
    assert_eq!(abstract_methods.len(), 1);
    assert_eq!(abstract_methods[0].name, "greet");
}

#[test]
fn test_scala_extract_object() {
    let result = extract("object Main {\n  def main(args: Array[String]): Unit = println(\"hi\")\n}");
    let objects: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ScalaObject)
        .collect();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "Main");
}

#[test]
fn test_scala_extract_method() {
    let result = extract("object Main {\n  def hello(name: String): String = s\"Hello $name\"\n}");
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "hello");
}

#[test]
fn test_scala_extract_function() {
    let result = extract("def topLevel(x: Int): Int = x + 1");
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "topLevel");
}

#[test]
fn test_scala_extract_val() {
    let result = extract("object Config {\n  val name: String = \"app\"\n}");
    let vals: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::ValField).collect();
    assert_eq!(vals.len(), 1);
    assert_eq!(vals[0].name, "name");
}

#[test]
fn test_scala_extract_var() {
    let result = extract("object State {\n  var count: Int = 0\n}");
    let vars: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::VarField).collect();
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].name, "count");
}

#[test]
fn test_scala_extract_type_alias() {
    let result = extract("object Types {\n  type StringMap = Map[String, String]\n}");
    let types: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].name, "StringMap");
}

#[test]
fn test_scala_extract_class_params_as_fields() {
    let result = extract("class Point(val x: Int, val y: Int, z: Int)");
    let vals: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::ValField).collect();
    // x and y are val params, z is a plain param (also extracted as ValField but private)
    assert!(vals.len() >= 2);
    assert!(vals.iter().any(|n| n.name == "x"));
    assert!(vals.iter().any(|n| n.name == "y"));
}

#[test]
fn test_scala_contains_edges() {
    let result = extract("object Main {\n  def hello(): Unit = ()\n}");
    let contains_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File → Object, Object → Method
    assert!(contains_edges.len() >= 2);
}

#[test]
fn test_scala_extract_call_sites() {
    let result = extract(
        "object Main {\n  def run(): Unit = {\n    println(\"hello\")\n    foo()\n  }\n}",
    );
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(calls.len() >= 2);
    assert!(calls.iter().any(|c| c.reference_name == "println"));
    assert!(calls.iter().any(|c| c.reference_name == "foo"));
}

#[test]
fn test_scala_visibility_private() {
    let result = extract("class Foo {\n  private def secret(): Unit = ()\n}");
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].visibility, tokensave::types::Visibility::Private);
}

#[test]
fn test_scala_visibility_default_is_public() {
    let result = extract("class Foo {\n  def open(): Unit = ()\n}");
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].visibility, tokensave::types::Visibility::Pub);
}

#[test]
fn test_scala_qualified_names() {
    let result = extract("object Main {\n  def hello(): Unit = ()\n}");
    let method = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method)
        .unwrap();
    assert!(method.qualified_name.contains("Main"));
    assert!(method.qualified_name.contains("hello"));
}

#[test]
fn test_scala_annotations_on_class_and_function() {
    let source = r#"
@deprecated
@throws(classOf[Exception])
class MyClass {
  @tailrec
  def factorial(n: Int): Int = if (n <= 1) 1 else n * factorial(n - 1)
}
"#;
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Should have 3 AnnotationUsage nodes: deprecated, throws, tailrec
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert_eq!(annots.len(), 3, "expected 3 annotations, got: {:?}", annots.iter().map(|a| &a.name).collect::<Vec<_>>());
    assert!(annots.iter().any(|a| a.name == "deprecated"));
    assert!(annots.iter().any(|a| a.name == "throws"));
    assert!(annots.iter().any(|a| a.name == "tailrec"));

    // Should have Annotates edges.
    let annotates_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annotates_edges.len(), 3, "expected 3 Annotates edges");

    // Should have Annotates unresolved refs.
    let annot_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annot_refs.len(), 3, "expected 3 Annotates refs");
}

#[test]
fn test_scala_scaladoc() {
    let result = extract(
        "/** A greeting object. */\nobject Greeter {\n  /** Says hi. */\n  def hi(): String = \"hi\"\n}",
    );
    let obj = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::ScalaObject)
        .unwrap();
    assert!(obj.docstring.as_ref().unwrap().contains("greeting"));
}
