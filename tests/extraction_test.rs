use codegraph::extraction::{LanguageRegistry, RustExtractor};
use codegraph::types::*;

#[test]
fn test_extract_function() {
    let source = r#"
/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    let result = RustExtractor::extract("src/math.rs", source);
    assert!(result.errors.is_empty());
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "add");
    assert_eq!(fns[0].visibility, Visibility::Pub);
    assert!(fns[0].signature.as_ref().unwrap().contains("fn add"));
    assert!(fns[0]
        .docstring
        .as_ref()
        .unwrap()
        .contains("Adds two numbers"));
}

#[test]
fn test_extract_struct_with_fields() {
    let source = r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
}
"#;
    let result = RustExtractor::extract("src/geo.rs", source);
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
    assert_eq!(fields.len(), 2);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(contains.len() >= 2);
}

#[test]
fn test_extract_enum() {
    let source = r#"
pub enum Color {
    Red,
    Green,
    Blue,
}
"#;
    let result = RustExtractor::extract("src/color.rs", source);
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
fn test_extract_trait() {
    let source = r#"
pub trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}
"#;
    let result = RustExtractor::extract("src/draw.rs", source);
    let traits: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Trait)
        .collect();
    assert_eq!(traits.len(), 1);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_extract_impl_block() {
    let source = r#"
struct Circle { radius: f64 }
impl Circle {
    pub fn new(radius: f64) -> Self { Circle { radius } }
    pub fn area(&self) -> f64 { std::f64::consts::PI * self.radius * self.radius }
}
"#;
    let result = RustExtractor::extract("src/circle.rs", source);
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
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_extract_trait_impl() {
    let source = r#"
trait Greet { fn hello(&self) -> String; }
struct Person { name: String }
impl Greet for Person {
    fn hello(&self) -> String { format!("Hello, {}", self.name) }
}
"#;
    let result = RustExtractor::extract("src/greet.rs", source);
    // Should have an Implements unresolved ref or edge.
    let has_implements = result.edges.iter().any(|e| e.kind == EdgeKind::Implements)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Implements);
    assert!(has_implements, "should have implements edge or ref");
}

#[test]
fn test_extract_use_declarations() {
    let source = r#"
use std::collections::HashMap;
use crate::types::Node;
"#;
    let result = RustExtractor::extract("src/lib.rs", source);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2);
}

#[test]
fn test_extract_call_sites() {
    let source = r#"
fn helper() -> i32 { 42 }
fn main() {
    let x = helper();
    println!("{}", x);
}
"#;
    let result = RustExtractor::extract("src/main.rs", source);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
}

#[test]
fn test_extract_async_function() {
    let source = r#"
pub async fn fetch_data(url: &str) -> Result<String, Error> {
    Ok("data".to_string())
}
"#;
    let result = RustExtractor::extract("src/http.rs", source);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].is_async);
}

#[test]
fn test_extract_const_and_static() {
    let source = r#"
pub const MAX_SIZE: usize = 1024;
static COUNTER: AtomicU64 = AtomicU64::new(0);
"#;
    let result = RustExtractor::extract("src/globals.rs", source);
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
fn test_extract_type_alias() {
    let source = r#"
pub type Result<T> = std::result::Result<T, Error>;
"#;
    let result = RustExtractor::extract("src/types.rs", source);
    let aliases: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "Result");
}

#[test]
fn test_extract_module() {
    let source = r#"
pub mod utils {
    pub fn helper() {}
}
"#;
    let result = RustExtractor::extract("src/lib.rs", source);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "utils");
}

#[test]
fn test_extract_derive_macros() {
    let source = r#"
#[derive(Debug, Clone, Serialize)]
pub struct Config { pub name: String }
"#;
    let result = RustExtractor::extract("src/config.rs", source);
    let derives: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::DerivesMacro)
        .collect();
    assert!(
        !derives.is_empty(),
        "should have derives_macro unresolved refs"
    );
    let names: Vec<&str> = derives.iter().map(|r| r.reference_name.as_str()).collect();
    assert!(names.contains(&"Debug"));
    assert!(names.contains(&"Clone"));
    assert!(names.contains(&"Serialize"));
}

#[test]
fn test_file_node_is_root() {
    let source = "fn main() {}";
    let result = RustExtractor::extract("src/main.rs", source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "src/main.rs");
}

#[test]
fn test_qualified_names() {
    let source = r#"
mod server {
    pub fn handle_request() {}
}
"#;
    let result = RustExtractor::extract("src/lib.rs", source);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].qualified_name.contains("server"));
    assert!(fns[0].qualified_name.contains("handle_request"));
}

#[test]
fn test_language_registry_finds_rust_extractor() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("src/main.rs").is_some());
    assert!(registry.extractor_for_file("lib.rs").is_some());
}

#[test]
fn test_language_registry_finds_go_extractor() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("main.go").is_some());
    assert!(registry.extractor_for_file("pkg/server.go").is_some());
}

#[test]
fn test_language_registry_finds_java_extractor() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("Main.java").is_some());
    assert!(registry
        .extractor_for_file("src/com/example/App.java")
        .is_some());
}

#[test]
fn test_language_registry_returns_none_for_unknown() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("script.py").is_none());
    assert!(registry.extractor_for_file("style.css").is_none());
    assert!(registry.extractor_for_file("README.md").is_none());
}

#[test]
fn test_language_registry_supported_extensions() {
    let registry = LanguageRegistry::new();
    let exts = registry.supported_extensions();
    assert!(exts.contains(&"rs"));
    assert!(exts.contains(&"go"));
    assert!(exts.contains(&"java"));
}
