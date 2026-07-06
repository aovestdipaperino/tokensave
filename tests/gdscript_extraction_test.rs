#![cfg(feature = "lang-gdscript")]
//! GDScript (Godot 4.x) extraction tests.

use tokensave::extraction::GdScriptExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

const SAMPLE: &str = r#"
class_name Player extends CharacterBody2D

signal died(reason)

const MAX_HP = 100

var hp: int = 100

enum State { IDLE, RUNNING, JUMPING }

func _ready():
    hp = MAX_HP
    take_damage(10)

static func spawn(pos):
    pass

func take_damage(amount):
    var local_var = amount
    hp -= local_var

class Inventory:
    var items = []
    func add_item(item):
        items.append(item)
"#;

fn extract_sample() -> ExtractionResult {
    let extractor = GdScriptExtractor;
    let result = extractor.extract("player.gd", SAMPLE);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

fn names_of(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    result
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.name.clone())
        .collect()
}

#[test]
fn file_root_present() {
    let r = extract_sample();
    let files: Vec<_> = r
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "player.gd");
}

#[test]
fn class_name_extracted_as_class_node() {
    let r = extract_sample();
    let classes: Vec<_> = r
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(
        classes.len(),
        1,
        "expected 1 class, got {:?}",
        names_of(&r, NodeKind::Class)
    );
    assert_eq!(classes[0].name, "Player");
}

#[test]
fn extends_edge_recorded() {
    let r = extract_sample();
    assert!(
        r.unresolved_refs
            .iter()
            .any(|u| u.reference_kind == EdgeKind::Extends && u.reference_name == "CharacterBody2D"),
        "expected Extends ref to CharacterBody2D, got {:?}",
        r.unresolved_refs
    );
}

#[test]
fn signal_extracted() {
    let r = extract_sample();
    let signals = names_of(&r, NodeKind::Signal);
    assert_eq!(signals, vec!["died".to_string()]);
}

#[test]
fn const_extracted() {
    let r = extract_sample();
    let consts = names_of(&r, NodeKind::Const);
    assert_eq!(consts, vec!["MAX_HP".to_string()]);
}

#[test]
fn class_level_field_extracted_but_not_local_var() {
    let r = extract_sample();
    let fields = names_of(&r, NodeKind::Field);
    assert!(fields.contains(&"hp".to_string()), "fields: {fields:?}");
    assert!(
        !fields.contains(&"local_var".to_string()),
        "local var inside a function body must not be emitted as a Field: {fields:?}"
    );
    // Nor should it show up under any other node kind.
    assert!(
        r.nodes.iter().all(|n| n.name != "local_var"),
        "local var must not be emitted as any node kind at all"
    );
}

#[test]
fn enum_and_variants_extracted() {
    let r = extract_sample();
    let enums: Vec<_> = r
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "State");

    let variants = names_of(&r, NodeKind::EnumVariant);
    assert_eq!(variants.len(), 3, "variants: {variants:?}");
    for v in ["IDLE", "RUNNING", "JUMPING"] {
        assert!(variants.contains(&v.to_string()), "missing variant {v}");
    }
}

#[test]
fn top_level_functions_split_correctly() {
    let r = extract_sample();
    // Top-level script functions (not inside a nested `class X:`) are
    // classified as Function, per the mapping table's file-scope row.
    let fns = names_of(&r, NodeKind::Function);
    for name in ["_ready", "spawn", "take_damage"] {
        assert!(
            fns.contains(&name.to_string()),
            "missing function {name}: {fns:?}"
        );
    }
}

#[test]
fn inner_class_and_its_method_extracted() {
    let r = extract_sample();
    let inner = names_of(&r, NodeKind::InnerClass);
    assert_eq!(inner, vec!["Inventory".to_string()]);

    // Inside a nested `class X:` block, functions become Method.
    let methods = names_of(&r, NodeKind::Method);
    assert_eq!(methods, vec!["add_item".to_string()]);

    // Inventory's own `var items = []` is still a Field.
    let fields = names_of(&r, NodeKind::Field);
    assert!(fields.contains(&"items".to_string()), "fields: {fields:?}");
}

#[test]
fn call_sites_recorded() {
    let r = extract_sample();
    let has_call = |name: &str| {
        r.unresolved_refs
            .iter()
            .any(|u| u.reference_kind == EdgeKind::Calls && u.reference_name == name)
    };
    assert!(has_call("take_damage"), "expected call to take_damage");
    assert!(has_call("append"), "expected attribute_call to append");
}

#[test]
fn contains_edges_present() {
    let r = extract_sample();
    assert!(r.edges.iter().any(|e| e.kind == EdgeKind::Contains));
}

#[test]
fn constructor_definition_maps_to_constructor_node() {
    let source = r#"
class_name Widget

func _init(x, y):
    pass
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("widget.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ctors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(ctors.len(), 1, "expected 1 constructor, got {ctors:?}");
    assert_eq!(ctors[0].name, "_init");
}

#[test]
fn no_class_name_falls_back_to_module() {
    let source = r#"
extends Node

func ready_up():
    pass
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("no_class_name.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 1, "expected 1 module, got {modules:?}");
    assert_eq!(modules[0].name, "no_class_name");
    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|u| u.reference_kind == EdgeKind::Extends && u.reference_name == "Node"),
        "expected standalone extends to still be recorded without class_name"
    );
    assert!(
        !result.nodes.iter().any(|n| n.kind == NodeKind::Class),
        "should not emit a Class node without class_name"
    );
}

#[test]
fn empty_source() {
    let extractor = GdScriptExtractor;
    let result = extractor.extract("empty.gd", "");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
}
