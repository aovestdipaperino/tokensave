use tokensave::extraction::CppExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_cpp_file_node_is_root() {
    let source = r#"
int main() {
    return 0;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("test.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.cpp");
}

#[test]
fn test_cpp_function_definition() {
    let source = r#"
int add(int a, int b) {
    return a + b;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("math.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "add");
    assert!(fns[0].signature.is_some());
    let sig = fns[0].signature.as_ref().unwrap();
    assert!(sig.contains("int add(int a, int b)"), "signature: {}", sig);
}

#[test]
fn test_cpp_class_with_methods_and_fields() {
    let source = r#"
class Dog {
public:
    int age;
    void bark() {
        // woof
    }
private:
    int secret;
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("dog.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Dog");

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1, "methods: {:?}", methods);
    assert_eq!(methods[0].name, "bark");
    assert_eq!(methods[0].visibility, Visibility::Pub);

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 2, "fields: {:?}", fields);

    let age_field = fields.iter().find(|f| f.name == "age").expect("age field");
    assert_eq!(age_field.visibility, Visibility::Pub);

    let secret_field = fields
        .iter()
        .find(|f| f.name == "secret")
        .expect("secret field");
    assert_eq!(secret_field.visibility, Visibility::Private);
}

#[test]
fn test_cpp_constructor_and_destructor() {
    let source = r#"
class Foo {
public:
    Foo() {}
    Foo(int x) {}
    ~Foo() {}
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("foo.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let constructors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert!(
        constructors.len() >= 2,
        "should have 2 constructors, got: {:?}",
        constructors
    );

    // Destructor can also be a Method with special name
    let destructors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method && n.name.starts_with('~'))
        .collect();
    assert_eq!(destructors.len(), 1, "destructors: {:?}", destructors);
}

#[test]
fn test_cpp_namespace() {
    let source = r#"
namespace mylib {
    void helper() {}
    int value = 42;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("lib.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let namespaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Namespace)
        .collect();
    assert_eq!(namespaces.len(), 1);
    assert_eq!(namespaces[0].name, "mylib");

    // Namespace should contain the function
    let ns_id = &namespaces[0].id;
    let contains_from_ns: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && e.source == *ns_id)
        .collect();
    assert!(
        contains_from_ns.len() >= 1,
        "namespace should contain children, got: {:?}",
        contains_from_ns
    );

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "helper");
}

#[test]
fn test_cpp_template() {
    let source = r#"
template <typename T>
T maximum(T a, T b) {
    return (a > b) ? a : b;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("tmpl.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let templates: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Template)
        .collect();
    assert_eq!(templates.len(), 1, "templates: {:?}", templates);
    assert_eq!(templates[0].name, "maximum");
}

#[test]
fn test_cpp_virtual_methods() {
    let source = r#"
class Shape {
public:
    virtual void draw() {}
    virtual double area() = 0;
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("shape.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();

    let abstract_methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AbstractMethod)
        .collect();

    // draw is a virtual method, area is abstract (pure virtual)
    assert_eq!(methods.len(), 1, "methods: {:?}", methods);
    assert_eq!(methods[0].name, "draw");

    assert_eq!(
        abstract_methods.len(),
        1,
        "abstract_methods: {:?}",
        abstract_methods
    );
    assert_eq!(abstract_methods[0].name, "area");
}

#[test]
fn test_cpp_access_specifiers() {
    let source = r#"
class Widget {
    int default_priv;
public:
    int pub_field;
protected:
    int prot_field;
private:
    int priv_field;
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("widget.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 4, "fields: {:?}", fields);

    let default_priv = fields
        .iter()
        .find(|f| f.name == "default_priv")
        .expect("default_priv");
    assert_eq!(default_priv.visibility, Visibility::Private);

    let pub_field = fields
        .iter()
        .find(|f| f.name == "pub_field")
        .expect("pub_field");
    assert_eq!(pub_field.visibility, Visibility::Pub);

    let prot_field = fields
        .iter()
        .find(|f| f.name == "prot_field")
        .expect("prot_field");
    assert_eq!(prot_field.visibility, Visibility::PubSuper);

    let priv_field = fields
        .iter()
        .find(|f| f.name == "priv_field")
        .expect("priv_field");
    assert_eq!(priv_field.visibility, Visibility::Private);
}

#[test]
fn test_cpp_inheritance() {
    let source = r#"
class Animal {
public:
    void eat() {}
};

class Dog : public Animal {
public:
    void bark() {}
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("animals.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 2, "classes: {:?}", classes);

    // Dog should have an Extends unresolved ref to Animal
    let extends_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(
        extends_refs.len() >= 1,
        "should have Extends refs, got: {:?}",
        extends_refs
    );
    assert!(extends_refs.iter().any(|r| r.reference_name == "Animal"));
}

#[test]
fn test_cpp_struct_default_public() {
    let source = r#"
struct Point {
    int x;
    int y;
    void print() {}
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("point.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");

    // Struct members default to public
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    for field in &fields {
        assert_eq!(
            field.visibility,
            Visibility::Pub,
            "struct field {} should be public",
            field.name
        );
    }
}

#[test]
fn test_cpp_enum() {
    let source = r#"
enum Color {
    RED,
    GREEN,
    BLUE
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("color.cpp", source);
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
    assert!(variants.iter().any(|v| v.name == "RED"));
    assert!(variants.iter().any(|v| v.name == "GREEN"));
    assert!(variants.iter().any(|v| v.name == "BLUE"));
}

#[test]
fn test_cpp_union() {
    let source = r#"
union Data {
    int i;
    float f;
    char c;
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("data.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let unions: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Union)
        .collect();
    assert_eq!(unions.len(), 1);
    assert_eq!(unions[0].name, "Data");
}

#[test]
fn test_cpp_typedef() {
    let source = r#"
typedef unsigned long ulong;
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("types.hpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let typedefs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Typedef)
        .collect();
    assert_eq!(typedefs.len(), 1);
    assert_eq!(typedefs[0].name, "ulong");
}

#[test]
fn test_cpp_preprocessor_and_include() {
    let source = r#"
#define MAX_SIZE 1024
#include <iostream>
#include "myheader.h"
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("main.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let macros: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PreprocessorDef)
        .collect();
    assert_eq!(macros.len(), 1);
    assert_eq!(macros[0].name, "MAX_SIZE");

    let includes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Include)
        .collect();
    assert_eq!(includes.len(), 2);
}

#[test]
fn test_cpp_using_declaration() {
    let source = r#"
using namespace std;
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("main.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 1, "nodes: {:?}", result.nodes);
}

#[test]
fn test_cpp_docstring_block_comment() {
    let source = r#"
/* Adds two integers together. */
int add(int a, int b) {
    return a + b;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("math.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let doc = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(
        doc.contains("Adds two integers"),
        "docstring: {:?}",
        doc
    );
}

#[test]
fn test_cpp_docstring_line_comment() {
    let source = r#"
// Multiplies two numbers.
int mul(int a, int b) {
    return a * b;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("math.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let doc = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(
        doc.contains("Multiplies two numbers"),
        "docstring: {:?}",
        doc
    );
}

#[test]
fn test_cpp_docstring_triple_slash() {
    let source = r#"
/// Divides two numbers.
/// Returns the quotient.
int divide(int a, int b) {
    return a / b;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("math.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let doc = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(doc.contains("Divides two numbers"), "docstring: {:?}", doc);
    assert!(doc.contains("Returns the quotient"), "docstring: {:?}", doc);
}

#[test]
fn test_cpp_call_site_tracking() {
    let source = r#"
int helper(int x) {
    return x * 2;
}

int main() {
    int result = helper(5);
    return 0;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("main.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        call_refs.len() >= 1,
        "should have call refs for helper, got: {:?}",
        call_refs
    );
    assert!(call_refs.iter().any(|r| r.reference_name == "helper"));
}

#[test]
fn test_cpp_contains_edges() {
    let source = r#"
void foo() {}
void bar() {}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("test.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(
        contains.len() >= 2,
        "should have Contains edges from File to Functions, got: {}",
        contains.len()
    );
}

#[test]
fn test_cpp_contains_edges_class_to_members() {
    let source = r#"
class Rect {
public:
    int width;
    int height;
    int area() { return width * height; }
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("rect.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let class_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.name == "Rect")
        .expect("should have Rect class");

    let contains_from_class: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && e.source == class_node.id)
        .collect();
    // 2 fields + 1 method = 3
    assert_eq!(
        contains_from_class.len(),
        3,
        "Rect should contain 3 members, got: {:?}",
        contains_from_class
    );
}

#[test]
fn test_cpp_static_function_private() {
    let source = r#"
static int helper(int x) {
    return x * 2;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("utils.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "helper");
    assert_eq!(fns[0].visibility, Visibility::Private);
}

#[test]
fn test_cpp_non_static_function_pub() {
    let source = r#"
int public_func() {
    return 42;
}
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("api.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "public_func");
    assert_eq!(fns[0].visibility, Visibility::Pub);
}

#[test]
fn test_cpp_extensions() {
    let extractor = CppExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"cpp"));
    assert!(exts.contains(&"cc"));
    assert!(exts.contains(&"cxx"));
    assert!(exts.contains(&"hpp"));
    assert!(exts.contains(&"hxx"));
    assert!(exts.contains(&"hh"));
}

#[test]
fn test_cpp_language_name() {
    let extractor = CppExtractor;
    assert_eq!(extractor.language_name(), "C++");
}

#[test]
fn test_cpp_template_class() {
    let source = r#"
template <typename T>
class Container {
public:
    T value;
    T get() { return value; }
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("container.hpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let templates: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Template)
        .collect();
    assert_eq!(templates.len(), 1, "templates: {:?}", templates);
    assert_eq!(templates[0].name, "Container");
}

#[test]
fn test_cpp_enum_class() {
    let source = r#"
enum class Direction {
    Up,
    Down,
    Left,
    Right
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("direction.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Direction");

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 4);
}

#[test]
fn test_cpp_multiple_inheritance() {
    let source = r#"
class A {};
class B {};
class C : public A, public B {};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("multi.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let extends_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(
        extends_refs.len() >= 2,
        "should have 2 Extends refs, got: {:?}",
        extends_refs
    );
    assert!(extends_refs.iter().any(|r| r.reference_name == "A"));
    assert!(extends_refs.iter().any(|r| r.reference_name == "B"));
}

#[test]
fn test_cpp_attributes_on_function_and_class() {
    let source = r#"
[[nodiscard]]
int getValue() { return 42; }

[[deprecated("use newFunc")]]
void oldFunc() {}

class [[nodiscard]] Result {
};
"#;
    let extractor = CppExtractor;
    let result = extractor.extract("attr.cpp", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Should have 3 AnnotationUsage nodes: nodiscard, deprecated, nodiscard
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert!(annots.len() >= 3, "expected at least 3 annotations, got: {:?}", annots.iter().map(|a| &a.name).collect::<Vec<_>>());
    assert!(annots.iter().any(|a| a.name == "nodiscard"));
    assert!(annots.iter().any(|a| a.name == "deprecated"));

    // Should have Annotates edges.
    let annotates_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert!(annotates_edges.len() >= 3, "expected at least 3 Annotates edges");

    // Should have Annotates unresolved refs.
    let annot_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Annotates)
        .collect();
    assert!(annot_refs.len() >= 3, "expected at least 3 Annotates refs");
}
