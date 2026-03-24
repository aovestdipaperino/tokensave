use tokensave::extraction::KotlinExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    let extractor = KotlinExtractor;
    extractor.extract("test.kt", source)
}

// -----------------------------------------------------------------------
// File node
// -----------------------------------------------------------------------

#[test]
fn test_kt_file_node_is_root() {
    let result = extract("fun main() {}");
    let file_nodes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(file_nodes.len(), 1);
    assert_eq!(file_nodes[0].name, "test.kt");
}

// -----------------------------------------------------------------------
// Package
// -----------------------------------------------------------------------

#[test]
fn test_kt_package() {
    let result = extract("package com.example.app\n\nfun main() {}");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let pkgs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::KotlinPackage)
        .collect();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "com.example.app");
}

// -----------------------------------------------------------------------
// Imports
// -----------------------------------------------------------------------

#[test]
fn test_kt_import() {
    let source = "import kotlin.collections.List\nimport java.io.File";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let imports: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(imports.len(), 2);
    assert!(imports.iter().any(|n| n.name.contains("List")));
    assert!(imports.iter().any(|n| n.name.contains("File")));
}

// -----------------------------------------------------------------------
// Function
// -----------------------------------------------------------------------

#[test]
fn test_kt_function() {
    let source = "fun greet(name: String): String = \"Hello $name\"";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "greet");
}

// -----------------------------------------------------------------------
// Class
// -----------------------------------------------------------------------

#[test]
fn test_kt_class() {
    let source = "class MyClass(val x: Int) {\n  fun hello(): String = \"hi\"\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
}

// -----------------------------------------------------------------------
// Data class
// -----------------------------------------------------------------------

#[test]
fn test_kt_data_class() {
    let source = "data class Person(val name: String, val age: Int)";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let data_classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::DataClass)
        .collect();
    assert_eq!(data_classes.len(), 1);
    assert_eq!(data_classes[0].name, "Person");
}

// -----------------------------------------------------------------------
// Sealed class
// -----------------------------------------------------------------------

#[test]
fn test_kt_sealed_class() {
    let source = r#"sealed class Result {
    data class Success(val value: String) : Result()
    data class Error(val msg: String) : Result()
}"#;
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let sealed: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::SealedClass)
        .collect();
    assert_eq!(sealed.len(), 1);
    assert_eq!(sealed[0].name, "Result");

    // Inner data classes should also be extracted.
    let data_classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::DataClass)
        .collect();
    assert_eq!(data_classes.len(), 2);
}

// -----------------------------------------------------------------------
// Object declaration
// -----------------------------------------------------------------------

#[test]
fn test_kt_object() {
    let source = "object Singleton {\n  val name = \"singleton\"\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let objects: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::KotlinObject)
        .collect();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name, "Singleton");
}

// -----------------------------------------------------------------------
// Companion object
// -----------------------------------------------------------------------

#[test]
fn test_kt_companion_object() {
    let source = r#"class MyClass {
    companion object {
        val CONST = 42
    }
}"#;
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let companions: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::CompanionObject)
        .collect();
    assert_eq!(companions.len(), 1);
    assert_eq!(companions[0].name, "Companion");
}

// -----------------------------------------------------------------------
// Interface
// -----------------------------------------------------------------------

#[test]
fn test_kt_interface() {
    let source = "interface Greeter {\n  fun greet(name: String): String\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let traits: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Trait).collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Greeter");
}

// -----------------------------------------------------------------------
// Enum class with entries
// -----------------------------------------------------------------------

#[test]
fn test_kt_enum_class() {
    let source = "enum class Color {\n  RED,\n  GREEN,\n  BLUE\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Enum).collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 3);
    assert!(variants.iter().any(|n| n.name == "RED"));
    assert!(variants.iter().any(|n| n.name == "GREEN"));
    assert!(variants.iter().any(|n| n.name == "BLUE"));
}

// -----------------------------------------------------------------------
// Property val/var
// -----------------------------------------------------------------------

#[test]
fn test_kt_property_val() {
    let source = "val name: String = \"hello\"";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "name");
    assert!(
        props[0].signature.as_ref().unwrap().contains("val"),
        "signature should contain 'val': {:?}",
        props[0].signature
    );
}

#[test]
fn test_kt_property_var() {
    let source = "var count: Int = 0";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "count");
    assert!(
        props[0].signature.as_ref().unwrap().contains("var"),
        "signature should contain 'var': {:?}",
        props[0].signature
    );
}

// -----------------------------------------------------------------------
// Constructor
// -----------------------------------------------------------------------

#[test]
fn test_kt_constructor() {
    let source = "class Foo(x: Int) {\n  constructor(x: Int, y: Int) : this(x)\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ctors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(ctors.len(), 1);
    assert_eq!(ctors[0].name, "constructor");
}

// -----------------------------------------------------------------------
// Annotation
// -----------------------------------------------------------------------

#[test]
fn test_kt_annotation() {
    let source = "@Deprecated(\"use other\")\nfun oldFunc() {}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].name, "Deprecated");

    // Should have an Annotates edge.
    let annotates_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert!(
        !annotates_edges.is_empty(),
        "expected at least one Annotates edge"
    );
}

// -----------------------------------------------------------------------
// Extension function
// -----------------------------------------------------------------------

#[test]
fn test_kt_extension_function() {
    let source = "fun String.addExcl(): String = this + \"!\"";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "addExcl");
    // Extension function signature should include receiver type.
    let sig = fns[0].signature.as_ref().unwrap();
    assert!(
        sig.contains("String.addExcl"),
        "extension signature should include receiver type: {:?}",
        sig
    );
}

// -----------------------------------------------------------------------
// KDoc docstring
// -----------------------------------------------------------------------

#[test]
fn test_kt_kdoc() {
    let source = "/** This is a KDoc comment */\nfun documented() {}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].docstring.is_some(), "expected docstring");
    assert!(
        fns[0].docstring.as_ref().unwrap().contains("KDoc comment"),
        "docstring: {:?}",
        fns[0].docstring
    );
}

// -----------------------------------------------------------------------
// Visibility modifiers
// -----------------------------------------------------------------------

#[test]
fn test_kt_visibility_public() {
    let source = "fun publicFunc() {}";
    let result = extract(source);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns[0].visibility, Visibility::Pub);
}

#[test]
fn test_kt_visibility_private() {
    let source = "private fun secret() {}";
    let result = extract(source);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].visibility, Visibility::Private);
}

#[test]
fn test_kt_visibility_internal() {
    let source = "internal fun packageLevel() {}";
    let result = extract(source);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].visibility, Visibility::PubCrate);
}

#[test]
fn test_kt_visibility_protected() {
    let source = "open class Base {\n  protected fun familyOnly() {}\n}";
    let result = extract(source);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].visibility, Visibility::PubSuper);
}

// -----------------------------------------------------------------------
// Call site tracking
// -----------------------------------------------------------------------

#[test]
fn test_kt_call_site() {
    let source = "fun caller() { println(\"hello\") }";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        !calls.is_empty(),
        "expected at least one call site"
    );
    assert!(
        calls.iter().any(|c| c.reference_name == "println"),
        "expected println call, got: {:?}",
        calls
    );
}

// -----------------------------------------------------------------------
// Contains edges
// -----------------------------------------------------------------------

#[test]
fn test_kt_contains_edges() {
    let source = "class MyClass {\n  fun hello() {}\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File->Class and Class->Method at minimum
    assert!(
        contains_edges.len() >= 2,
        "expected at least 2 Contains edges, got {}",
        contains_edges.len()
    );
}

// -----------------------------------------------------------------------
// Method inside class
// -----------------------------------------------------------------------

#[test]
fn test_kt_method_inside_class() {
    let source = "class MyClass {\n  fun hello(): String = \"hi\"\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "hello");
}

// -----------------------------------------------------------------------
// Abstract method in interface
// -----------------------------------------------------------------------

#[test]
fn test_kt_abstract_method_in_interface() {
    let source = "interface Greeter {\n  fun greet(name: String): String\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let abstract_methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AbstractMethod)
        .collect();
    assert_eq!(abstract_methods.len(), 1);
    assert_eq!(abstract_methods[0].name, "greet");
}

// -----------------------------------------------------------------------
// Suspend function
// -----------------------------------------------------------------------

#[test]
fn test_kt_suspend_function() {
    let source = "suspend fun fetchData(): String = \"data\"";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].is_async, "suspend function should have is_async=true");
}

// -----------------------------------------------------------------------
// Property inside object
// -----------------------------------------------------------------------

#[test]
fn test_kt_property_inside_object() {
    let source = "object Config {\n  val name: String = \"app\"\n}";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "name");
}

// -----------------------------------------------------------------------
// LanguageExtractor trait implementation
// -----------------------------------------------------------------------

#[test]
fn test_kt_extensions() {
    let extractor = KotlinExtractor;
    assert!(extractor.extensions().contains(&"kt"));
    assert!(extractor.extensions().contains(&"kts"));
}

#[test]
fn test_kt_language_name() {
    let extractor = KotlinExtractor;
    assert_eq!(extractor.language_name(), "Kotlin");
}
