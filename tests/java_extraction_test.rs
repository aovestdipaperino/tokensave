use codegraph::extraction::JavaExtractor;
use codegraph::extraction::LanguageExtractor;
use codegraph::types::*;

#[test]
fn test_java_extract_package() {
    let source = r#"package com.example.app;

public class Main {
    public static void main(String[] args) {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("src/Main.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let pkgs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Package)
        .collect();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "com.example.app");
}

#[test]
fn test_java_extract_class() {
    let source = r#"package com.example;

/**
 * A simple calculator.
 */
public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Calculator.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Calculator");
    assert_eq!(classes[0].visibility, Visibility::Pub);
    assert!(classes[0]
        .docstring
        .as_ref()
        .unwrap()
        .contains("simple calculator"));
}

#[test]
fn test_java_extract_methods() {
    let source = r#"
public class Foo {
    public void doSomething() {}
    private int compute(int x) { return x * 2; }
    protected String getName() { return "foo"; }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 3);
    let do_something = methods.iter().find(|m| m.name == "doSomething").unwrap();
    assert_eq!(do_something.visibility, Visibility::Pub);
    let compute = methods.iter().find(|m| m.name == "compute").unwrap();
    assert_eq!(compute.visibility, Visibility::Private);
    let get_name = methods.iter().find(|m| m.name == "getName").unwrap();
    assert_eq!(get_name.visibility, Visibility::PubCrate); // protected maps to PubCrate
}

#[test]
fn test_java_extract_constructor() {
    let source = r#"
public class Person {
    private String name;
    public Person(String name) {
        this.name = name;
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Person.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let constructors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].name, "Person");
}

#[test]
fn test_java_extract_interface() {
    let source = r#"
public interface Drawable {
    void draw();
    double area();
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Drawable.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ifaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "Drawable");
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method || n.kind == NodeKind::AbstractMethod)
        .collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_java_extract_enum() {
    let source = r#"
public enum Color {
    RED,
    GREEN,
    BLUE
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Color.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 3);
}

#[test]
fn test_java_extract_fields() {
    let source = r#"
public class Config {
    public static final int MAX_SIZE = 1024;
    private String name;
    protected int port;
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Config.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 3);
    let max_size = fields.iter().find(|f| f.name == "MAX_SIZE").unwrap();
    assert_eq!(max_size.visibility, Visibility::Pub);
}

#[test]
fn test_java_extract_imports() {
    let source = r#"
import java.util.List;
import java.util.Map;
import static java.lang.Math.PI;

public class Foo {}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 3);
}

#[test]
fn test_java_extract_extends_implements() {
    let source = r#"
interface Runnable { void run(); }
class Base {}
class Worker extends Base implements Runnable {
    public void run() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Worker.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let has_extends = result.edges.iter().any(|e| e.kind == EdgeKind::Extends)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Extends);
    assert!(has_extends, "should detect extends");
    let has_implements = result.edges.iter().any(|e| e.kind == EdgeKind::Implements)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Implements);
    assert!(has_implements, "should detect implements");
}

#[test]
fn test_java_extract_annotations() {
    let source = r#"
import java.lang.Override;

public class Foo {
    @Override
    public String toString() {
        return "Foo";
    }

    @Deprecated
    public void oldMethod() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert!(annots.len() >= 2, "should extract annotation usages");
    let has_annotates = result.edges.iter().any(|e| e.kind == EdgeKind::Annotates)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Annotates);
    assert!(has_annotates, "should have Annotates edges");
}

#[test]
fn test_java_extract_inner_class() {
    let source = r#"
public class Outer {
    public class Inner {
        public void innerMethod() {}
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Outer.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let inners: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::InnerClass)
        .collect();
    assert_eq!(inners.len(), 1);
    assert_eq!(inners[0].name, "Inner");
}

#[test]
fn test_java_extract_static_init_block() {
    let source = r#"
public class Registry {
    private static Map<String, Object> cache;
    static {
        cache = new HashMap<>();
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Registry.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let init_blocks: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::InitBlock)
        .collect();
    assert_eq!(init_blocks.len(), 1);
}

#[test]
fn test_java_extract_abstract_method() {
    let source = r#"
public abstract class Shape {
    public abstract double area();
    public void describe() { System.out.println("shape"); }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Shape.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let abstract_methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AbstractMethod)
        .collect();
    assert_eq!(abstract_methods.len(), 1);
    assert_eq!(abstract_methods[0].name, "area");
}

#[test]
fn test_java_extract_generics() {
    let source = r#"
public class Box<T> {
    private T value;
    public T getValue() { return value; }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Box.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let generics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::GenericParam)
        .collect();
    assert!(generics.len() >= 1, "should extract generic type param T");
}

#[test]
fn test_java_extract_call_sites() {
    let source = r#"
public class App {
    public void run() {
        System.out.println("hello");
        helper();
        new ArrayList<>();
    }
    private void helper() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("App.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
}

#[test]
fn test_java_extract_annotation_type() {
    let source = r#"
public @interface MyAnnotation {
    String value();
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("MyAnnotation.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Annotation)
        .collect();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].name, "MyAnnotation");
}

#[test]
fn test_java_file_node_is_root() {
    let source = "public class Main {}";
    let extractor = JavaExtractor;
    let result = extractor.extract("src/Main.java", source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "src/Main.java");
}

#[test]
fn test_java_contains_edges() {
    let source = r#"
public class Foo {
    private int x;
    public void bar() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: Class; Class contains: Field, Method
    assert!(
        contains.len() >= 3,
        "should have Contains edges: {}",
        contains.len()
    );
}

#[test]
fn test_java_qualified_names() {
    let source = r#"
package com.example;

public class App {
    public void run() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("src/App.java", source);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert!(methods[0].qualified_name.contains("App"));
    assert!(methods[0].qualified_name.contains("run"));
}
