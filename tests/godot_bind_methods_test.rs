#![cfg(feature = "lang-gdscript")]
//! Regression tests for issue #269: godot-cpp `_bind_methods` bindings.
//!
//! A Godot 4 GDExtension exposes C++ methods to GDScript through
//! `ClassDB::bind_method(D_METHOD("name", ...), &Class::method)` calls inside
//! `_bind_methods`. Before the fix, the bound method pointer (`&Class::method`)
//! was invisible to the C++ extractor, so the method got no incoming edge from
//! the binding, and `_bind_methods` itself (an engine-invoked callback) was
//! reported as dead code.

use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{Edge, Node};

/// Builds a temp project mirroring the issue's repro: a C++ registry class that
/// binds an instance method and a static method in `_bind_methods`, plus a
/// GDScript caller that invokes both through the singleton / class name.
async fn setup_godot_project() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/registry.cpp"),
        r#"
#include <godot_cpp/classes/object.hpp>
using namespace godot;

class MyRegistry : public Object {
    GDCLASS(MyRegistry, Object)
    static MyRegistry *singleton;
protected:
    static void _bind_methods();
public:
    static MyRegistry *get_singleton() { return singleton; }
    void register_item(const Dictionary &cfg) {
        int x = 1;
    }
    static void register_global(const Dictionary &cfg) {
        int y = 2;
    }
};

MyRegistry *MyRegistry::singleton = nullptr;

void MyRegistry::_bind_methods() {
    ClassDB::bind_method(D_METHOD("register_item", "cfg"), &MyRegistry::register_item);
    ClassDB::bind_static_method("MyRegistry", D_METHOD("register_global", "cfg"), &MyRegistry::register_global);
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/game.gd"),
        r#"
extends Node

func _ready():
    MyRegistry.get_singleton().register_item({ "id": "example" })
    MyRegistry.register_global({ "id": "global" })
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

/// True if a `Calls` edge exists whose source node is named `from` and target
/// node is named `to`.
fn has_call_edge(edges: &[Edge], by_id: &HashMap<String, &Node>, from: &str, to: &str) -> bool {
    edges.iter().any(|e| {
        by_id.get(&e.source).map(|n| n.name.as_str()) == Some(from)
            && by_id.get(&e.target).map(|n| n.name.as_str()) == Some(to)
    })
}

#[tokio::test]
async fn gdscript_call_resolves_to_bound_cpp_method() {
    let (cg, _dir) = setup_godot_project().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let edges = cg.get_all_edges().await.unwrap();
    let by_id: HashMap<String, &Node> = nodes.iter().map(|n| (n.id.clone(), n)).collect();

    // The GDScript `_ready` call site reaches the C++ method by name.
    assert!(
        has_call_edge(&edges, &by_id, "_ready", "register_item"),
        "GDScript call should resolve to the C++ MyRegistry::register_item method"
    );
    assert!(
        has_call_edge(&edges, &by_id, "_ready", "register_global"),
        "GDScript call should resolve to the C++ MyRegistry::register_global method"
    );
}

#[tokio::test]
async fn bind_methods_links_to_exposed_methods() {
    let (cg, _dir) = setup_godot_project().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let edges = cg.get_all_edges().await.unwrap();
    let by_id: HashMap<String, &Node> = nodes.iter().map(|n| (n.id.clone(), n)).collect();

    // The `&MyRegistry::register_item` pointer argument produces a Calls edge
    // from `_bind_methods` to the bound method (bind_method form).
    assert!(
        has_call_edge(&edges, &by_id, "_bind_methods", "register_item"),
        "_bind_methods should have a Calls edge to register_item"
    );
    // The `&MyRegistry::register_global` argument of the bind_static_method form
    // is handled the same way.
    assert!(
        has_call_edge(&edges, &by_id, "_bind_methods", "register_global"),
        "_bind_methods should have a Calls edge to register_global (bind_static_method)"
    );
}

#[tokio::test]
async fn binding_boilerplate_not_reported_dead() {
    let (cg, _dir) = setup_godot_project().await;
    // include_public=true stresses the analysis: register_item / register_global
    // are public methods, so without their incoming binding edges they would be
    // reported dead here.
    let dead = cg.find_dead_code(&[], true, true).await.unwrap();
    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();

    assert!(
        !dead_names.contains(&"_bind_methods"),
        "_bind_methods is an engine-invoked callback and must not be dead code, got {dead_names:?}"
    );
    assert!(
        !dead_names.contains(&"register_item"),
        "bound method register_item must not be dead code, got {dead_names:?}"
    );
    assert!(
        !dead_names.contains(&"register_global"),
        "bound method register_global must not be dead code, got {dead_names:?}"
    );
    // The singleton accessor is reached by GDScript's `get_singleton()` call and
    // must not be dead either.
    assert!(
        !dead_names.contains(&"get_singleton"),
        "get_singleton is called from GDScript and must not be dead code, got {dead_names:?}"
    );
}
