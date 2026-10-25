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
    // `MyDb` reads as a class, so the typed sibling `MyDb::some_method` is
    // recorded at the same site too (#597); it is a different ref shape, not
    // the duplicate this test guards against.
    let hits: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls)
        .map(|u| u.reference_name.as_str())
        .filter(|name| !name.contains("::"))
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

#[test]
fn typed_receivers_record_a_typed_call_ref() {
    // #597: each receiver has a static type, recorded as a type expression the
    // resolver evaluates (`step()` is a method's return type, `field` a
    // member's declared type). Untyped receivers get no typed ref.
    let source = "class_name User\nextends RefCounted\n\nvar bus: Bus\nvar hub := Hub.new()\n\n\
func wire(given: Bus, loose) -> void:\n\
\tgiven.subscribe(1)\n\
\tbus.subscribe(2)\n\
\tgiven.again().subscribe(3)\n\
\tvar local := Bus.make()\n\
\tlocal.subscribe(4)\n\
\tvar typed: Bus = Bus.new()\n\
\ttyped.subscribe(5)\n\
\tself.bus.subscribe(6)\n\
\thub.subscribe(7)\n\
\tloose.subscribe(8)\n\
\tvar untyped = Bus.new()\n\
\tuntyped.subscribe(9)\n";
    let result = GdScriptExtractor.extract("user.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let typed: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Calls && u.reference_name.contains("::"))
        .map(|u| u.reference_name.as_str())
        .collect();
    for expected in [
        "Bus::subscribe",
        "Bus::again()::subscribe",
        "Bus::make()::subscribe",
        "User::bus::subscribe",
        "Hub::subscribe",
        "Bus::make",
    ] {
        assert!(
            typed.contains(&expected),
            "missing {expected}, got {typed:?}"
        );
    }
    // given, bus and typed are all `Bus`; then local, self.bus, hub.
    assert_eq!(
        typed.iter().filter(|n| **n == "Bus::subscribe").count(),
        3,
        "got {typed:?}"
    );
    assert!(
        !typed
            .iter()
            .any(|n| n.contains("loose") || n.contains("untyped")),
        "untyped receivers keep name-only resolution, got {typed:?}"
    );
}

#[test]
fn method_used_as_value_records_a_uses_ref() {
    // #598: a method of the enclosing class passed, assigned, stored or
    // returned as a value is a use. A local of the same name shadows it, and a
    // name that is not a method of the class is not recorded.
    let source = "class_name User\n\nvar handler: Callable\n\n\
func wire(bus, _shadowed) -> Callable:\n\
\tbus.subscribe(&\"t\", _on)\n\
\thandler = _on_assigned\n\
\tvar table := [_on_listed]\n\
\tvar d := {\"k\": _on_keyed}\n\
\tbus.subscribe(&\"u\", _shadowed)\n\
\tbus.subscribe(&\"v\", not_a_method)\n\
\treturn _on_returned\n\n\
func _on() -> void:\n\tpass\n\
func _on_assigned() -> void:\n\tpass\n\
func _on_listed() -> void:\n\tpass\n\
func _on_keyed() -> void:\n\tpass\n\
func _on_returned() -> void:\n\tpass\n\
func _shadowed() -> void:\n\tpass\n";
    let result = GdScriptExtractor.extract("user.gd", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|u| u.reference_kind == EdgeKind::Uses)
        .map(|u| u.reference_name.as_str())
        .collect();
    for expected in [
        "_on",
        "_on_assigned",
        "_on_listed",
        "_on_keyed",
        "_on_returned",
    ] {
        assert!(uses.contains(&expected), "missing {expected}, got {uses:?}");
    }
    assert!(
        !uses.contains(&"_shadowed") && !uses.contains(&"not_a_method"),
        "got {uses:?}"
    );
}

// ---------------------------------------------------------------------------
// Graph-level regressions (#597, #598): these index a small Godot project and
// read the resolved edges, since the bugs live in resolution and dead-code
// analysis rather than in what the extractor records.
// ---------------------------------------------------------------------------

mod graph {
    use std::collections::{BTreeSet, HashMap};
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;
    use tokensave::tokensave::TokenSave;
    use tokensave::types::{EdgeKind, Node};

    const BUS: &str = "class_name Bus\nextends RefCounted\n\n\n\
func subscribe(topic: StringName, handler: Callable) -> void:\n\tprint(topic, handler)\n\n\n\
func again() -> Bus:\n\treturn self\n\n\n\
static func make() -> Bus:\n\treturn Bus.new()\n";

    const USER: &str = "class_name User\nextends RefCounted\n\nvar bus: Bus\n\n\n\
func wire(given: Bus) -> void:\n\
\tgiven.subscribe(&\"param\", _on)\n\
\tbus.subscribe(&\"member\", _on)\n\
\tgiven.again().subscribe(&\"chain\", _on)\n\
\tvar local := Bus.make()\n\
\tlocal.subscribe(&\"inferred\", _on)\n\
\tvar typed: Bus = Bus.new()\n\
\ttyped.subscribe(&\"typed_local\", _on)\n\n\n\
func _on() -> void:\n\tpass\n";

    const RADIO: &str = "class_name Radio\nextends RefCounted\n\n\n\
func subscribe(topic: StringName, handler: Callable) -> void:\n\tprint(\"radio\", topic, handler)\n";

    fn write(root: &Path, rel: &str, text: &str) {
        fs::write(root.join(rel), text).unwrap();
    }

    fn node<'a>(nodes: &'a [Node], file: &str, name: &str) -> &'a Node {
        nodes
            .iter()
            .find(|n| n.file_path == file && n.name == name)
            .unwrap_or_else(|| panic!("no node {file}::{name}"))
    }

    /// Distinct call-site lines of incoming edges of `kind` into `file::name`.
    async fn incoming_lines(
        cg: &TokenSave,
        file: &str,
        name: &str,
        kind: EdgeKind,
    ) -> BTreeSet<u32> {
        let nodes = cg.get_all_nodes().await.unwrap();
        let target = node(&nodes, file, name).id.clone();
        cg.get_incoming_edges(&target)
            .await
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == kind)
            .filter_map(|e| e.line)
            .collect()
    }

    async fn full_index(root: &Path) -> TokenSave {
        let cg = TokenSave::init(root).await.unwrap();
        cg.index_all().await.unwrap();
        cg
    }

    /// #597: every receiver below has a static type, so a second class with a
    /// `subscribe` method must not erase the edges into `Bus.subscribe`.
    #[tokio::test]
    async fn typed_receiver_call_survives_a_same_named_method_elsewhere() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "bus.gd", BUS);
        write(dir.path(), "user.gd", USER);
        write(dir.path(), "radio.gd", RADIO);
        let cg = full_index(dir.path()).await;

        let bus = incoming_lines(&cg, "bus.gd", "subscribe", EdgeKind::Calls).await;
        assert_eq!(
            bus.len(),
            5,
            "param, member, chain, inferred and typed-local calls must all reach Bus.subscribe, got lines {bus:?}"
        );
        let radio = incoming_lines(&cg, "radio.gd", "subscribe", EdgeKind::Calls).await;
        assert!(
            radio.is_empty(),
            "nothing calls Radio.subscribe, got lines {radio:?}"
        );
    }

    /// #597: an incremental sync that adds the competing class must agree
    /// with a full index of the same tree.
    #[tokio::test]
    async fn typed_receiver_edges_match_between_incremental_and_full_sync() {
        let inc = TempDir::new().unwrap();
        write(inc.path(), "bus.gd", BUS);
        write(inc.path(), "user.gd", USER);
        let cg = full_index(inc.path()).await;
        write(inc.path(), "radio.gd", RADIO);
        cg.sync().await.unwrap();

        let full = TempDir::new().unwrap();
        write(full.path(), "bus.gd", BUS);
        write(full.path(), "user.gd", USER);
        write(full.path(), "radio.gd", RADIO);
        let full_cg = full_index(full.path()).await;

        for file in ["bus.gd", "radio.gd"] {
            let a = incoming_lines(&cg, file, "subscribe", EdgeKind::Calls).await;
            let b = incoming_lines(&full_cg, file, "subscribe", EdgeKind::Calls).await;
            assert_eq!(a, b, "{file}: incremental and full sync disagree");
        }
    }

    /// #597: a method inherited through `extends`, and a class-qualified
    /// static call, resolve through the type as well.
    #[tokio::test]
    async fn typed_receiver_resolves_inherited_and_static_methods() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "bus.gd", BUS);
        write(dir.path(), "radio.gd", RADIO);
        write(
            dir.path(),
            "hub.gd",
            "class_name Hub\nextends Bus\n\n\nfunc extra() -> void:\n\tpass\n",
        );
        write(
            dir.path(),
            "names.gd",
            "class_name Names\nextends RefCounted\n\n\n\
static func sorted(xs: Array) -> Array:\n\treturn xs\n",
        );
        write(
            dir.path(),
            "list.gd",
            "class_name List\nextends RefCounted\n\n\n\
func sorted() -> Array:\n\treturn []\n",
        );
        write(
            dir.path(),
            "caller.gd",
            "class_name Caller\nextends RefCounted\n\n\n\
func go(hub: Hub) -> void:\n\
\thub.subscribe(&\"inherited\", go)\n\
\tvar xs := Names.sorted([3, 1])\n\
\tprint(xs)\n",
        );
        let cg = full_index(dir.path()).await;

        let bus = incoming_lines(&cg, "bus.gd", "subscribe", EdgeKind::Calls).await;
        assert_eq!(
            bus.len(),
            1,
            "hub.subscribe must reach the inherited Bus.subscribe, got {bus:?}"
        );
        let names = incoming_lines(&cg, "names.gd", "sorted", EdgeKind::Calls).await;
        assert_eq!(
            names.len(),
            1,
            "Names.sorted must reach Names.sorted, got {names:?}"
        );
        let list = incoming_lines(&cg, "list.gd", "sorted", EdgeKind::Calls).await;
        assert!(list.is_empty(), "nothing calls List.sorted, got {list:?}");
    }

    /// #598 (1): a method passed, assigned or stored as a value is used.
    #[tokio::test]
    async fn method_used_as_a_value_gets_a_uses_edge() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "bus.gd", BUS);
        write(
            dir.path(),
            "user.gd",
            "class_name User\nextends RefCounted\n\nvar handler: Callable\n\n\n\
func wire(given: Bus) -> void:\n\
\tgiven.subscribe(&\"param\", _on)\n\
\thandler = _on_assigned\n\
\tvar table := [_on_listed]\n\
\tprint(table)\n\n\n\
func _on() -> void:\n\tpass\n\n\n\
func _on_assigned() -> void:\n\tpass\n\n\n\
func _on_listed() -> void:\n\tpass\n\n\n\
func _never() -> void:\n\tpass\n",
        );
        let cg = full_index(dir.path()).await;

        for name in ["_on", "_on_assigned", "_on_listed"] {
            let uses = incoming_lines(&cg, "user.gd", name, EdgeKind::Uses).await;
            assert!(
                !uses.is_empty(),
                "{name} is used as a value and needs a uses edge"
            );
        }
        let dead = cg.find_dead_code(&[], true, false).await.unwrap();
        let dead: Vec<(&str, &str)> = dead
            .iter()
            .map(|n| (n.file_path.as_str(), n.name.as_str()))
            .collect();
        for name in ["_on", "_on_assigned", "_on_listed"] {
            assert!(
                !dead.contains(&("user.gd", name)),
                "{name} is not dead, got {dead:?}"
            );
        }
        assert!(
            dead.contains(&("user.gd", "_never")),
            "an unreferenced method is still dead, got {dead:?}"
        );
    }

    /// #598 (2): an override of a base method that has callers is what runs
    /// for a subclass instance, so it is not dead.
    #[tokio::test]
    async fn override_of_a_called_base_method_is_not_dead() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "base_entry.gd",
            "class_name BaseEntry\nextends RefCounted\n\n\n\
func read() -> int:\n\treturn _fields().size()\n\n\n\
func _fields() -> Array[String]:\n\treturn []\n\n\n\
func _unused_hook() -> void:\n\tpass\n",
        );
        write(
            dir.path(),
            "item_entry.gd",
            "class_name ItemEntry\nextends BaseEntry\n\n\n\
func _fields() -> Array[String]:\n\treturn [\"mass\"]\n\n\n\
func _unused_hook() -> void:\n\tpass\n",
        );
        write(
            dir.path(),
            "heavy_entry.gd",
            "class_name HeavyEntry\nextends ItemEntry\n\n\n\
func _fields() -> Array[String]:\n\treturn [\"mass\", \"weight\"]\n",
        );
        let cg = full_index(dir.path()).await;

        let dead = cg.find_dead_code(&[], true, false).await.unwrap();
        let mut by_file: HashMap<&str, Vec<&str>> = HashMap::new();
        for n in &dead {
            by_file
                .entry(n.file_path.as_str())
                .or_default()
                .push(n.name.as_str());
        }
        let dead_in = |file: &str, name: &str| by_file.get(file).is_some_and(|v| v.contains(&name));
        assert!(
            !dead_in("item_entry.gd", "_fields"),
            "override of a called base method is live, got {by_file:?}"
        );
        assert!(
            !dead_in("heavy_entry.gd", "_fields"),
            "a grandchild override is live too, got {by_file:?}"
        );
        assert!(
            dead_in("item_entry.gd", "_unused_hook"),
            "an override of an uncalled base method stays dead, got {by_file:?}"
        );
        assert!(
            dead_in("base_entry.gd", "_unused_hook"),
            "the uncalled base method stays dead, got {by_file:?}"
        );
    }

    /// Every edge as `source|target|kind|line`, comparable across two trees
    /// (node ids hash file, kind, name and line, not the database).
    async fn edge_set(cg: &TokenSave) -> BTreeSet<String> {
        cg.get_all_edges()
            .await
            .unwrap()
            .iter()
            .map(|e| format!("{}|{}|{:?}|{:?}", e.source, e.target, e.kind, e.line))
            .collect()
    }

    /// Whether `from_file::from` has a `calls` edge into `to_file::to`.
    async fn calls(cg: &TokenSave, from_file: &str, from: &str, to_file: &str, to: &str) -> bool {
        let nodes = cg.get_all_nodes().await.unwrap();
        let source = node(&nodes, from_file, from).id.clone();
        let target = node(&nodes, to_file, to).id.clone();
        cg.get_all_edges()
            .await
            .unwrap()
            .iter()
            .any(|e| e.kind == EdgeKind::Calls && e.source == source && e.target == target)
    }

    const OTHER: &str = "class_name Other\nextends RefCounted\n\n\n\
func subscribe(cb) -> void:\n\tpass\n";

    /// #597 review: a resolved typed ref must only replace its own sibling,
    /// not a different same-named call on the same line.
    #[tokio::test]
    async fn typed_ref_does_not_suppress_another_call_on_the_same_line() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "bus.gd", BUS);
        write(dir.path(), "other.gd", OTHER);
        write(
            dir.path(),
            "runner.gd",
            "class_name Runner\nextends RefCounted\n\n\n\
func run(given: Bus, o) -> void:\n\
\tgiven.subscribe(o.subscribe(1))\n",
        );
        write(
            dir.path(),
            "user.gd",
            "class_name User\nextends RefCounted\n\n\n\
func run2(given: Bus) -> void:\n\
\tgiven.subscribe(subscribe(1))\n\n\n\
func subscribe(x) -> int:\n\treturn x\n",
        );
        let cg = full_index(dir.path()).await;

        assert!(
            calls(&cg, "user.gd", "run2", "bus.gd", "subscribe").await,
            "the typed call reaches Bus.subscribe"
        );
        assert!(
            calls(&cg, "user.gd", "run2", "user.gd", "subscribe").await,
            "the bare inner call on the same line keeps its edge to User.subscribe"
        );
        // `o.subscribe` is untyped and ties between Bus, Other and User: that
        // ambiguity must survive the typed call on the same line, or dead_code
        // reports Other.subscribe.
        let dead = cg.find_dead_code(&[], true, false).await.unwrap();
        assert!(
            !dead
                .iter()
                .any(|n| n.file_path == "other.gd" && n.name == "subscribe"),
            "Other.subscribe is an ambiguity candidate, not dead"
        );
    }

    /// #597 review: a typed edge depends on declarations in third files. When
    /// one changes, an incremental sync must end where a fresh index does.
    #[tokio::test]
    async fn typed_edges_follow_changed_declarations_incrementally() {
        let hub = "class_name Hub\nextends Bus\n";
        let user = "class_name User\nextends RefCounted\n\nvar bus: Bus\n\n\n\
func make_bus() -> Bus:\n\treturn null\n";
        let caller = "class_name Caller\nextends RefCounted\n\n\n\
func go2(u: User) -> void:\n\tu.bus.subscribe(1)\n\n\n\
func go3(u: User) -> void:\n\tu.make_bus().subscribe(1)\n\n\n\
func go4(h: Hub) -> void:\n\th.subscribe(1)\n";
        let seed = |root: &Path| {
            write(root, "bus.gd", BUS);
            write(root, "other.gd", OTHER);
            write(root, "hub.gd", hub);
            write(root, "user.gd", user);
            write(root, "caller.gd", caller);
        };
        let inc_dir = TempDir::new().unwrap();
        seed(inc_dir.path());
        let inc = full_index(inc_dir.path()).await;
        assert!(calls(&inc, "caller.gd", "go2", "bus.gd", "subscribe").await);
        assert!(calls(&inc, "caller.gd", "go3", "bus.gd", "subscribe").await);
        assert!(calls(&inc, "caller.gd", "go4", "bus.gd", "subscribe").await);

        let edits: [(&str, &str, &str); 3] = [
            ("untype a member", "user.gd", "var bus: Bus\n"),
            ("change a return type", "user.gd", "-> Bus:"),
            ("change an extends base", "hub.gd", "extends Bus"),
        ];
        let replacements = ["var bus\n", "-> Other:", "extends RefCounted"];
        for ((name, file, from), to) in edits.into_iter().zip(replacements) {
            let path = inc_dir.path().join(file);
            let text = fs::read_to_string(&path).unwrap();
            assert!(text.contains(from), "{name}: fixture must contain {from:?}");
            fs::write(&path, text.replacen(from, to, 1)).unwrap();
            inc.sync().await.unwrap();

            let full_dir = TempDir::new().unwrap();
            for rel in ["bus.gd", "other.gd", "hub.gd", "user.gd", "caller.gd"] {
                fs::copy(inc_dir.path().join(rel), full_dir.path().join(rel)).unwrap();
            }
            let full = full_index(full_dir.path()).await;
            assert_eq!(
                edge_set(&inc).await,
                edge_set(&full).await,
                "after {name}: incremental sync and a fresh index disagree"
            );
        }
        // The end state the comparison pins, spelled out.
        assert!(!calls(&inc, "caller.gd", "go2", "bus.gd", "subscribe").await);
        assert!(calls(&inc, "caller.gd", "go3", "other.gd", "subscribe").await);
        assert!(!calls(&inc, "caller.gd", "go3", "bus.gd", "subscribe").await);
        assert!(!calls(&inc, "caller.gd", "go4", "bus.gd", "subscribe").await);
    }
}
