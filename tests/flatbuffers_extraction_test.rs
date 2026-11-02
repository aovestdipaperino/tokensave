#![cfg(feature = "lang-flatbuffers")]

use tokensave::extraction::FlatBuffersExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

use crate::common::names_of;

fn extract(source: &str) -> ExtractionResult {
    FlatBuffersExtractor.extract("schema.fbs", source)
}

#[test]
fn file_node_is_emitted() {
    let result = extract("namespace MyGame;\n");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["schema.fbs".to_string()]);
}

#[test]
fn extracts_namespace_as_module() {
    let result = extract("namespace MyGame.Sample;\n");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["MyGame.Sample".to_string()]);
}

#[test]
fn extracts_table_as_struct() {
    let source = "table Monster {\n  hp: short;\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let structs = names_of(&result, NodeKind::Struct);
    assert_eq!(structs, vec!["Monster".to_string()]);
}

#[test]
fn extracts_struct_keyword_as_struct() {
    let source = "struct Vec3 {\n  x: float;\n  y: float;\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let structs = names_of(&result, NodeKind::Struct);
    assert_eq!(structs, vec!["Vec3".to_string()]);
    let node = result.nodes.iter().find(|n| n.name == "Vec3").unwrap();
    assert_eq!(node.signature.as_deref(), Some("struct Vec3"));
}

#[test]
fn extracts_table_fields_with_types() {
    let source = "table Monster {\n  hp: short = 100;\n  name: string;\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields = names_of(&result, NodeKind::Field);
    assert_eq!(fields, vec!["hp".to_string(), "name".to_string()]);
    let hp = result.nodes.iter().find(|n| n.name == "hp").unwrap();
    assert_eq!(hp.signature.as_deref(), Some("hp: short = 100"));
}

#[test]
fn fields_are_contained_by_table() {
    let source = "table Monster {\n  hp: short;\n}\n";
    let result = extract(source);
    let table = result.nodes.iter().find(|n| n.name == "Monster").unwrap();
    let field = result.nodes.iter().find(|n| n.name == "hp").unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == table.id && e.target == field.id && e.kind == EdgeKind::Contains));
}

#[test]
fn extracts_enum_and_variants() {
    let source = "enum Color: byte {\n  Red = 0,\n  Green,\n  Blue = 2\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums = names_of(&result, NodeKind::Enum);
    assert_eq!(enums, vec!["Color".to_string()]);
    let variants = names_of(&result, NodeKind::EnumVariant);
    assert_eq!(
        variants,
        vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()]
    );
}

#[test]
fn extracts_union_with_bare_members() {
    let source = "union Any {\n  Monster,\n  Weapon\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let unions = names_of(&result, NodeKind::Union);
    assert_eq!(unions, vec!["Any".to_string()]);
    let members = names_of(&result, NodeKind::Field);
    assert_eq!(members, vec!["Monster".to_string(), "Weapon".to_string()]);
}

#[test]
fn extracts_include_as_use() {
    let source = "include \"other.fbs\";\ntable A {\n  x: int;\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses = names_of(&result, NodeKind::Use);
    assert_eq!(uses, vec!["other.fbs".to_string()]);
}

#[test]
fn extracts_rpc_service_and_methods() {
    let source = "rpc_service MonsterStorage {\n  Store(Monster): StoreResponse;\n  Retrieve(StoreRequest): Monster;\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let services = names_of(&result, NodeKind::Interface);
    assert_eq!(services, vec!["MonsterStorage".to_string()]);
    let methods = names_of(&result, NodeKind::Method);
    assert_eq!(methods, vec!["Store".to_string(), "Retrieve".to_string()]);
}

#[test]
fn realistic_sample_extracts_all_kinds() {
    let source = "include \"base.fbs\";\n\
                  namespace MyGame.Sample;\n\
                  enum Color: byte { Red = 0, Green, Blue = 2 }\n\
                  struct Vec3 { x: float; y: float; }\n\
                  table Monster { pos: Vec3; hp: short = 100; name: string; }\n\
                  union Any { Monster, Vec3 }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(
        names_of(&result, NodeKind::Use),
        vec!["base.fbs".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Module),
        vec!["MyGame.Sample".to_string()]
    );
    assert_eq!(names_of(&result, NodeKind::Enum), vec!["Color".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::Struct),
        vec!["Vec3".to_string(), "Monster".to_string()]
    );
    assert_eq!(names_of(&result, NodeKind::Union), vec!["Any".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::Field),
        vec![
            "x".to_string(),
            "y".to_string(),
            "pos".to_string(),
            "hp".to_string(),
            "name".to_string(),
            "Monster".to_string(),
            "Vec3".to_string()
        ]
    );
}

#[test]
fn extensions_are_fbs() {
    let ext = FlatBuffersExtractor;
    assert_eq!(ext.extensions(), &["fbs"]);
}

#[test]
fn language_name_is_flatbuffers() {
    let ext = FlatBuffersExtractor;
    assert_eq!(ext.language_name(), "FlatBuffers");
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = extract("");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
}

#[test]
fn malformed_input_returns_partial_result_without_panicking() {
    let result = extract("table Broken {\n");
    assert!(!result.errors.is_empty(), "expected parse errors");
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["schema.fbs".to_string()]);
}
