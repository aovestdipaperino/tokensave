use tokensave::extraction::CExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_c_file_node_is_root() {
    let source = r#"
int main() {
    return 0;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("test.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.c");
}

#[test]
fn test_c_function_definition() {
    let source = r#"
int add(int a, int b) {
    return a + b;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("math.c", source);
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
fn test_c_function_declaration_prototype() {
    let source = r#"
int add(int a, int b);
void process(const char *data);
"#;
    let extractor = CExtractor;
    let result = extractor.extract("math.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2, "nodes: {:?}", result.nodes);
    let add_fn = fns.iter().find(|f| f.name == "add").unwrap();
    assert!(add_fn.signature.is_some());
}

#[test]
fn test_c_struct_with_fields() {
    let source = r#"
struct Point {
    int x;
    int y;
    float z;
};
"#;
    let extractor = CExtractor;
    let result = extractor.extract("point.h", source);
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
    assert_eq!(fields.len(), 3);
    assert!(fields.iter().any(|f| f.name == "x"));
    assert!(fields.iter().any(|f| f.name == "y"));
    assert!(fields.iter().any(|f| f.name == "z"));
}

#[test]
fn test_c_union() {
    let source = r#"
union Data {
    int i;
    float f;
    char c;
};
"#;
    let extractor = CExtractor;
    let result = extractor.extract("data.h", source);
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
fn test_c_union_with_fields() {
    let source = r#"
union Data {
    int i;
    float f;
};
"#;
    let extractor = CExtractor;
    let result = extractor.extract("data.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let union_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Union && n.name == "Data")
        .expect("should have Data union");

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 2, "union should have 2 fields");
    assert!(fields.iter().any(|f| f.name == "i"));
    assert!(fields.iter().any(|f| f.name == "f"));

    let contains_from_union: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && e.source == union_node.id)
        .collect();
    assert_eq!(
        contains_from_union.len(),
        2,
        "Data union should contain 2 fields via Contains edges"
    );
}

#[test]
fn test_c_enum_with_constants() {
    let source = r#"
enum Color {
    RED,
    GREEN,
    BLUE
};
"#;
    let extractor = CExtractor;
    let result = extractor.extract("color.h", source);
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
fn test_c_typedef() {
    let source = r#"
typedef unsigned long ulong;
"#;
    let extractor = CExtractor;
    let result = extractor.extract("types.h", source);
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
fn test_c_preprocessor_define() {
    let source = r#"
#define MAX_SIZE 1024
#define PI 3.14159
"#;
    let extractor = CExtractor;
    let result = extractor.extract("defs.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let macros: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PreprocessorDef)
        .collect();
    assert_eq!(macros.len(), 2);
    assert!(macros.iter().any(|m| m.name == "MAX_SIZE"));
    assert!(macros.iter().any(|m| m.name == "PI"));
}

#[test]
fn test_c_include() {
    let source = r#"
#include <stdio.h>
#include "myheader.h"
"#;
    let extractor = CExtractor;
    let result = extractor.extract("main.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let includes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Include)
        .collect();
    assert_eq!(includes.len(), 2);
}

#[test]
fn test_c_global_variable() {
    let source = r#"
int global_counter = 0;
const char *name = "hello";
"#;
    let extractor = CExtractor;
    let result = extractor.extract("globals.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let statics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Static)
        .collect();
    assert!(
        !statics.is_empty(),
        "should have global variables as Static nodes, got: {:?}",
        result.nodes
    );
}

#[test]
fn test_c_static_function_private() {
    let source = r#"
static int helper(int x) {
    return x * 2;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("utils.c", source);
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
fn test_c_non_static_function_pub() {
    let source = r#"
int public_func(void) {
    return 42;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("api.c", source);
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
fn test_c_docstring_block_comment() {
    let source = r#"
/* Adds two integers together. */
int add(int a, int b) {
    return a + b;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("math.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let doc = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(doc.contains("Adds two integers"), "docstring: {:?}", doc);
}

#[test]
fn test_c_docstring_line_comment() {
    let source = r#"
// Multiplies two numbers.
int mul(int a, int b) {
    return a * b;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("math.c", source);
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
fn test_c_call_site_tracking() {
    let source = r#"
int helper(int x) {
    return x * 2;
}

int main() {
    int result = helper(5);
    printf("Result: %d\n", result);
    return 0;
}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("main.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        call_refs.len() >= 2,
        "should have call refs for helper and printf, got: {:?}",
        call_refs
    );
    assert!(call_refs.iter().any(|r| r.reference_name == "helper"));
    assert!(call_refs.iter().any(|r| r.reference_name == "printf"));
}

#[test]
fn test_c_contains_edges_file_to_function() {
    let source = r#"
void foo() {}
void bar() {}
"#;
    let extractor = CExtractor;
    let result = extractor.extract("test.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File -> foo, File -> bar = at least 2
    assert!(
        contains.len() >= 2,
        "should have Contains edges from File to Functions, got: {}",
        contains.len()
    );
}

#[test]
fn test_c_contains_edges_struct_to_field() {
    let source = r#"
struct Rect {
    int width;
    int height;
};
"#;
    let extractor = CExtractor;
    let result = extractor.extract("rect.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let struct_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Struct && n.name == "Rect")
        .expect("should have Rect struct");

    let contains_from_struct: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && e.source == struct_node.id)
        .collect();
    assert_eq!(
        contains_from_struct.len(),
        2,
        "Rect should contain 2 fields"
    );
}

#[test]
fn test_c_function_pointer_typedef() {
    let source = r#"
typedef int (*compare_fn)(const void *, const void *);
"#;
    let extractor = CExtractor;
    let result = extractor.extract("types.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let typedefs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Typedef)
        .collect();
    assert_eq!(typedefs.len(), 1);
    assert_eq!(typedefs[0].name, "compare_fn");
}

#[test]
fn test_c_typedef_struct() {
    let source = r#"
typedef struct {
    int x;
    int y;
} Point;
"#;
    let extractor = CExtractor;
    let result = extractor.extract("point.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Should have a Typedef node for Point
    let typedefs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Typedef)
        .collect();
    assert_eq!(typedefs.len(), 1, "nodes: {:?}", result.nodes);
    assert_eq!(typedefs[0].name, "Point");

    // Should also have a Struct node
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);

    // The struct should have fields
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 2);
}

#[test]
fn test_c_enum_with_values() {
    let source = r#"
enum LogLevel {
    LOG_DEBUG = 0,
    LOG_INFO = 1,
    LOG_WARN = 2,
    LOG_ERROR = 3
};
"#;
    let extractor = CExtractor;
    let result = extractor.extract("log.h", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 4);
}

#[test]
fn test_c_global_variable_docstring() {
    let source = r#"
/* The global counter for tracking state. */
int global_counter = 0;
"#;
    let extractor = CExtractor;
    let result = extractor.extract("globals.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let statics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Static)
        .collect();
    assert_eq!(statics.len(), 1);
    let doc = statics[0]
        .docstring
        .as_ref()
        .expect("global variable should have docstring");
    assert!(doc.contains("global counter"), "docstring: {:?}", doc);
}

#[test]
fn test_c_static_global_variable() {
    let source = r#"
static int counter = 0;
"#;
    let extractor = CExtractor;
    let result = extractor.extract("state.c", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let statics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Static)
        .collect();
    assert_eq!(statics.len(), 1);
    assert_eq!(statics[0].name, "counter");
    assert_eq!(statics[0].visibility, Visibility::Private);
}

#[test]
fn test_c_extensions() {
    let extractor = CExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"c"));
    assert!(exts.contains(&"h"));
}

#[test]
fn test_c_language_name() {
    let extractor = CExtractor;
    assert_eq!(extractor.language_name(), "C");
}
