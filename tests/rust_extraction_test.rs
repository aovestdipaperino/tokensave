use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::RustExtractor;
use tokensave::types::*;

#[test]
fn test_rust_file_node_is_root() {
    let source = r#"fn main() {}"#;
    let extractor = RustExtractor;
    let result = extractor.extract("test.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.rs");
}

#[test]
fn test_rust_function() {
    let source = r#"
/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn helper() {}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("math.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(
        fns.len(),
        2,
        "expected 2 functions, got: {:?}",
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    let add_fn = fns.iter().find(|f| f.name == "add").expect("add not found");
    assert_eq!(add_fn.visibility, Visibility::Pub);
    assert!(add_fn
        .docstring
        .as_deref()
        .unwrap_or("")
        .contains("Adds two numbers"));
    assert!(add_fn.signature.as_deref().unwrap_or("").contains("fn add"));
    let helper = fns
        .iter()
        .find(|f| f.name == "helper")
        .expect("helper not found");
    assert_eq!(helper.visibility, Visibility::Private);
}

#[test]
fn test_rust_async_function() {
    let source = r#"
pub async fn fetch_data() -> String {
    String::new()
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("async.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].is_async, "expected async function");
}

#[test]
fn test_rust_struct_and_fields() {
    let source = r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
    label: String,
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("types.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
    assert_eq!(structs[0].visibility, Visibility::Pub);

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(
        fields.len(),
        3,
        "expected 3 fields, got: {:?}",
        fields.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fields
        .iter()
        .any(|f| f.name == "x" && f.visibility == Visibility::Pub));
    assert!(fields
        .iter()
        .any(|f| f.name == "label" && f.visibility == Visibility::Private));
}

#[test]
fn test_rust_enum_and_variants() {
    let source = r#"
pub enum Color {
    Red,
    Green,
    Blue(u8, u8, u8),
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("color.rs", source);
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
    assert_eq!(
        variants.len(),
        3,
        "expected 3 variants, got: {:?}",
        variants.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(variants.iter().any(|v| v.name == "Red"));
    assert!(variants.iter().any(|v| v.name == "Blue"));
}

#[test]
fn test_rust_trait() {
    let source = r#"
pub trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("draw.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let traits: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Trait)
        .collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Drawable");
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_rust_impl_block() {
    let source = r#"
struct Rect { w: f64, h: f64 }

impl Rect {
    pub fn new(w: f64, h: f64) -> Self {
        Rect { w, h }
    }

    pub fn area(&self) -> f64 {
        self.w * self.h
    }
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("rect.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let impls: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Impl)
        .collect();
    assert_eq!(impls.len(), 1);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(
        methods.len(),
        2,
        "methods: {:?}",
        methods.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(methods.iter().any(|m| m.name == "new"));
    assert!(methods.iter().any(|m| m.name == "area"));
    // Contains edges from impl to methods
    assert!(result.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

#[test]
fn test_rust_use_declarations() {
    let source = r#"
use std::collections::HashMap;
use std::io::{self, Read};
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("imports.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(
        uses.len(),
        2,
        "expected 2 use decls, got: {:?}",
        uses.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_rust_const_and_static() {
    let source = r#"
pub const MAX_SIZE: usize = 1024;
static COUNTER: u32 = 0;
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("consts.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MAX_SIZE");
    let statics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Static)
        .collect();
    assert_eq!(statics.len(), 1);
    assert_eq!(statics[0].name, "COUNTER");
}

#[test]
fn test_rust_type_alias() {
    let source = r#"
pub type Result<T> = std::result::Result<T, Error>;
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("types.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let aliases: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "Result");
}

#[test]
fn test_rust_module() {
    let source = r#"
pub mod inner {
    pub fn foo() {}
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("lib.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "inner");
    // The function inside the module should be extracted too
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "foo");
}

#[test]
fn test_rust_complexity_branches_and_loops() {
    let source = r#"
fn complex(x: i32) -> i32 {
    if x > 0 {
        for i in 0..x {
            if i % 2 == 0 {
                return i;
            }
        }
    }
    0
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("complex.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let f = &fns[0];
    assert!(
        f.branches >= 2,
        "expected >= 2 branches, got {}",
        f.branches
    );
    assert!(f.loops >= 1, "expected >= 1 loop, got {}", f.loops);
    assert!(f.returns >= 1, "expected >= 1 return, got {}", f.returns);
    assert!(
        f.max_nesting >= 2,
        "expected >= 2 nesting, got {}",
        f.max_nesting
    );
}

#[test]
fn test_rust_unsafe_and_unwrap_detection() {
    let source = r#"
fn risky(v: Option<i32>) -> i32 {
    let val = v.unwrap();
    unsafe {
        std::ptr::read(&val)
    }
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("risky.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let f = &fns[0];
    assert!(
        f.unsafe_blocks >= 1,
        "expected >= 1 unsafe block, got {}",
        f.unsafe_blocks
    );
    assert!(
        f.unchecked_calls >= 1,
        "expected >= 1 unchecked call (unwrap), got {}",
        f.unchecked_calls
    );
}

#[test]
fn test_rust_derive_macro_edge() {
    let source = r#"
#[derive(Debug, Clone)]
pub struct Foo {
    val: i32,
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("foo.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let derives: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::DerivesMacro)
        .collect();
    assert!(
        derives.len() >= 2,
        "expected >= 2 DerivesMacro refs for #[derive(Debug, Clone)], got {}",
        derives.len()
    );
}

#[test]
fn test_rust_call_sites() {
    let source = r#"
fn caller() {
    helper();
    std::io::stdout();
}
fn helper() {}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("calls.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Calls),
        "expected Calls refs"
    );
}

#[test]
fn test_rust_trait_impl() {
    let source = r#"
trait Greet {
    fn hello(&self) -> String;
}

struct Bot;

impl Greet for Bot {
    fn hello(&self) -> String {
        "Hi".to_string()
    }
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("greet.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let impls: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Impl)
        .collect();
    assert_eq!(impls.len(), 1);
    // The impl should reference the trait
    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Implements),
        "expected Implements ref for `impl Greet for Bot`"
    );
}

#[test]
fn test_rust_empty_source() {
    let extractor = RustExtractor;
    let result = extractor.extract("empty.rs", "");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_rust_annotation_extraction() {
    let source = r#"
#[test]
fn my_test() {}

#[cfg(test)]
#[allow(dead_code)]
fn guarded_fn() {}

#[inline]
pub fn fast_add(a: i32, b: i32) -> i32 { a + b }

#[derive(Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub name: String,
}
"#;
    let extractor = RustExtractor;
    let result = extractor.extract("attrs.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();

    let annot_names: Vec<&str> = annots.iter().map(|a| a.name.as_str()).collect();

    // #[test] on my_test
    assert!(
        annot_names.contains(&"test"),
        "expected 'test' annotation, got: {:?}",
        annot_names
    );

    // #[cfg(test)] on guarded_fn
    assert!(
        annot_names.contains(&"cfg"),
        "expected 'cfg' annotation, got: {:?}",
        annot_names
    );

    // #[allow(dead_code)] on guarded_fn
    assert!(
        annot_names.contains(&"allow"),
        "expected 'allow' annotation, got: {:?}",
        annot_names
    );

    // #[inline] on fast_add
    assert!(
        annot_names.contains(&"inline"),
        "expected 'inline' annotation, got: {:?}",
        annot_names
    );

    // #[serde(rename_all = "camelCase")] on Config (derive is skipped)
    assert!(
        annot_names.contains(&"serde"),
        "expected 'serde' annotation, got: {:?}",
        annot_names
    );

    // derive should NOT be in AnnotationUsage — it's handled separately by DerivesMacro
    assert!(
        !annot_names.contains(&"derive"),
        "derive should not appear as AnnotationUsage, got: {:?}",
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

#[test]
fn test_attrs_start_line_walks_back_over_doc_comments_and_attrs() {
    let source = r#"
/// First doc line.
/// Second doc line.
#[inline]
#[must_use]
pub fn double(x: i32) -> i32 {
    x * 2
}

pub fn no_attrs(y: i32) -> i32 {
    y
}
"#;
    let result = RustExtractor.extract("doc.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let double = result
        .nodes
        .iter()
        .find(|n| n.name == "double")
        .expect("double not found");
    let no_attrs = result
        .nodes
        .iter()
        .find(|n| n.name == "no_attrs")
        .expect("no_attrs not found");

    // `double` has 2 doc lines + 2 attribute lines preceding `pub fn double`.
    // Source is 0-indexed by tree-sitter. With the leading newline, the first
    // doc comment is at row 1 and `pub fn double` is at row 5.
    assert!(
        double.attrs_start_line < double.start_line,
        "attrs_start_line ({}) should be < start_line ({}) when leading docs/attrs present",
        double.attrs_start_line,
        double.start_line
    );
    assert_eq!(
        double.start_line - double.attrs_start_line,
        4,
        "expected attrs_start_line to walk back over 2 doc + 2 attribute lines"
    );

    // `no_attrs` has nothing leading it that should count — its blank gap means
    // the walk stops, so attrs_start_line == start_line.
    assert_eq!(no_attrs.attrs_start_line, no_attrs.start_line);
}

#[test]
fn test_emit_type_refs_for_struct_fields() {
    let source = r#"
pub struct Container {
    pub name: String,
    pub items: Vec<MyItem>,
    count: usize,
}
"#;
    let result = RustExtractor.extract("c.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let type_of_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::TypeOf)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        type_of_refs.contains(&"String"),
        "expected TypeOf ref to String, got {type_of_refs:?}"
    );
    assert!(
        type_of_refs.contains(&"Vec"),
        "expected TypeOf ref to Vec, got {type_of_refs:?}"
    );
    assert!(
        type_of_refs.contains(&"MyItem"),
        "expected TypeOf ref to MyItem (inner generic), got {type_of_refs:?}"
    );
}

#[test]
fn test_emit_type_refs_for_function_signatures() {
    let source = r#"
pub fn make(name: String, count: usize) -> Result<MyType, MyError> {
    todo!()
}
"#;
    let result = RustExtractor.extract("f.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let type_of: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::TypeOf)
        .map(|r| r.reference_name.as_str())
        .collect();
    let returns: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Returns)
        .map(|r| r.reference_name.as_str())
        .collect();

    assert!(
        type_of.contains(&"String"),
        "param TypeOf String missing: {type_of:?}"
    );
    assert!(
        returns.contains(&"Result"),
        "Returns Result missing: {returns:?}"
    );
    assert!(
        returns.contains(&"MyType"),
        "Returns MyType missing: {returns:?}"
    );
    assert!(
        returns.contains(&"MyError"),
        "Returns MyError missing: {returns:?}"
    );
}
