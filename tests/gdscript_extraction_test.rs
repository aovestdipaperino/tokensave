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
    assert!(has_call("take_damage"), "expected bare call to take_damage");
    // `items.append(item)` -- an attribute_call now carries its receiver
    // (`items.append`, not bare `append`) so the resolver can disambiguate a
    // same-named method on an unrelated class; matches the `receiver.method`
    // convention the Python/TS/JS extractors already use.
    assert!(
        has_call("items.append"),
        "expected receiver-qualified attribute_call items.append, got: {:?}",
        r.unresolved_refs
            .iter()
            .filter(|u| u.reference_kind == EdgeKind::Calls)
            .map(|u| u.reference_name.as_str())
            .collect::<Vec<_>>()
    );
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

#[test]
fn attribute_call_receiver_preserved_for_preload_alias() {
    // `XScript.some_method()` where XScript is a `const X = preload(...)`
    // alias (or a direct class_name receiver) — the receiver must be
    // preserved so a future resolver strategy can disambiguate against a
    // same-named method elsewhere, instead of the receiver being silently
    // discarded (the pre-fix behavior).
    let source = r#"
class_name Foo

const XScript = preload("res://bar.gd")

func run():
    XScript.some_method(1, 2)
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|u| u.reference_kind == EdgeKind::Calls
                && u.reference_name == "XScript.some_method"),
        "expected receiver-qualified XScript.some_method, got: {:?}",
        result
            .unresolved_refs
            .iter()
            .filter(|u| u.reference_kind == EdgeKind::Calls)
            .map(|u| u.reference_name.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn callable_string_dispatch_target_captured() {
    // `Callable(receiver, "method_name")` — Godot's string-keyed
    // deferred-dispatch idiom (`.connect()`, `call_deferred`, dispatch
    // tables). The string argument names a real method that would otherwise
    // show zero incoming edges and misreport as dead code.
    let source = r#"
class_name Foo

func _ready():
    var cb = Callable(self, "_on_button_pressed")
    other.connect("pressed", Callable(self, "_on_button_pressed"))

func _on_button_pressed():
    pass
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let hits = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls && u.reference_name == "_on_button_pressed")
        .count();
    assert_eq!(
        hits,
        2,
        "expected 2 Callable(...) string-target refs to _on_button_pressed, got: {:?}",
        result
            .unresolved_refs
            .iter()
            .filter(|u| u.reference_kind == EdgeKind::Calls)
            .map(|u| u.reference_name.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn callable_non_callable_call_not_misread() {
    // A 2-argument call to something that is NOT literally named `Callable`
    // must not be mistaken for the dispatch idiom (e.g. a normal function
    // that happens to take a string second argument).
    let source = r#"
class_name Foo

func run():
    some_other_function(self, "not_a_method_ref")
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        !result
            .unresolved_refs
            .iter()
            .any(|u| u.reference_kind == EdgeKind::Calls && u.reference_name == "not_a_method_ref"),
        "must not treat a non-Callable call's string arg as a dispatch target: {:?}",
        result.unresolved_refs
    );
}

#[test]
fn bare_dotted_attribute_call_argument_captured() {
    // `get_or_create(_h, MyDb._load_from_registry)` — a function reference
    // passed BY VALUE (no call parens), the lazy-init/dispatch-table idiom
    // this codebase's BaseDatabaseCache pattern relies on. Previously
    // invisible to the extractor entirely (no call/attribute_call node at
    // that position), so the referenced function showed zero incoming edges.
    let source = r#"
class_name Foo

static var _h

static func _load_from_registry():
    pass

static func get_or_create(h, loader):
    pass

func run():
    get_or_create(_h, Foo._load_from_registry)
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(
        result
            .unresolved_refs
            .iter()
            .any(|u| u.reference_kind == EdgeKind::Calls
                && u.reference_name == "Foo._load_from_registry"),
        "expected a bare dotted-attribute call-argument ref to Foo._load_from_registry, got: {:?}",
        result
            .unresolved_refs
            .iter()
            .filter(|u| u.reference_kind == EdgeKind::Calls)
            .map(|u| u.reference_name.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn bare_dotted_attribute_call_still_recorded_normally() {
    // A bare dotted attribute that IS a call (`MyDb.some_method()`) must
    // still be recorded via the normal attribute_call path, not double
    // counted or dropped by the new bare-argument scan (which only matches
    // the non-call, non-subscript leaf shape).
    let source = r#"
class_name Foo

func run(a, b):
    MyDb.some_method(a, b)
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let hits: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls)
        .map(|u| u.reference_name.as_str())
        .collect();
    assert_eq!(
        hits,
        vec!["MyDb.some_method"],
        "expected exactly one receiver-qualified call ref, not a duplicate or a dropped one: {hits:?}"
    );
}

#[test]
fn call_deferred_string_target_captured() {
    // `call_deferred("method_name")` (bare or receiver-qualified) -- Godot's
    // deferred-call API, string-named. Same invisibility problem as
    // Callable(...): the target only shows up as a string, not a real edge.
    let source = r#"
class_name Foo

func _ready():
    call_deferred("_late_init")
    other.call_deferred("_late_init")

func _late_init():
    pass
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let hits = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls && u.reference_name == "_late_init")
        .count();
    assert_eq!(
        hits,
        2,
        "expected 2 call_deferred(...) string-target refs to _late_init, got: {:?}",
        result
            .unresolved_refs
            .iter()
            .filter(|u| u.reference_kind == EdgeKind::Calls)
            .map(|u| u.reference_name.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn connect_bare_callback_reference_captured() {
    // `signal.connect(callback)` -- Godot's signal-connect API, `callback` a
    // bare identifier or bare dotted-attribute function reference (no call
    // parens). This is the single most common false-negative source found in
    // this codebase's own dead-code audits.
    let source = r#"
class_name Foo

func _ready():
    pressed.connect(_on_pressed)
    other_signal.connect(Foo._static_handler)

func _on_pressed():
    pass

static func _static_handler():
    pass
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls)
        .map(|u| u.reference_name.as_str())
        .collect();
    assert!(
        calls.contains(&"_on_pressed"),
        "expected bare identifier connect target _on_pressed, got: {calls:?}"
    );
    assert!(
        calls.contains(&"Foo._static_handler"),
        "expected dotted connect target Foo._static_handler, got: {calls:?}"
    );
}

#[test]
fn connect_with_call_argument_not_double_counted() {
    // `signal.connect(some_call())` -- the argument IS itself a call, not a
    // bare reference. Must not be misread as a bare-identifier/attribute
    // connect target (it already gets its own normal Calls edge via the
    // inner call itself).
    let source = r#"
class_name Foo

func _ready():
    pressed.connect(make_callback())

func make_callback() -> Callable:
    return Callable()
"#;
    let extractor = GdScriptExtractor;
    let result = extractor.extract("foo.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls)
        .map(|u| u.reference_name.as_str())
        .collect();
    assert!(
        calls.contains(&"make_callback"),
        "expected the inner call itself recorded: {calls:?}"
    );
    // The connect call itself is also recorded, receiver-qualified per the
    // attribute_call receiver fix.
    assert!(
        calls.contains(&"pressed.connect"),
        "expected connect itself recorded (receiver-qualified): {calls:?}"
    );
}
