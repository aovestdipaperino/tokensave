//! Regression test for #598 (part 3): Godot engine virtuals such as `_ready`
//! and `_process` must not be reported as dead code.
//!
//! The engine calls them, never project code, so they carry no incoming edge,
//! the same way Go `init` (#346) and godot-cpp `_bind_methods` (#269) do. The
//! exemption is scoped to `.gd` files and to the engine's names: an
//! unreferenced GDScript method is still dead code.
#![cfg(feature = "lang-gdscript")]

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

#[tokio::test]
async fn godot_engine_virtuals_are_not_reported_dead() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(
        project.join("panel.gd"),
        "class_name Panel2\nextends Button\n\n\n\
func _ready() -> void:\n\tpass\n\n\n\
func _process(delta: float) -> void:\n\tpass\n\n\n\
func _unused_helper() -> void:\n\tpass\n",
    )
    .unwrap();
    // The same name outside a `.gd` file keeps the ordinary rule.
    fs::write(project.join("tool.py"), "def _ready():\n    return 1\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let dead = cg.find_dead_code(&[], true, false).await.unwrap();
    let dead: Vec<(&str, &str)> = dead
        .iter()
        .map(|n| (n.file_path.as_str(), n.name.as_str()))
        .collect();

    assert!(
        !dead.contains(&("panel.gd", "_ready")) && !dead.contains(&("panel.gd", "_process")),
        "engine virtuals must not be dead code, got {dead:?}"
    );
    assert!(
        dead.contains(&("panel.gd", "_unused_helper")),
        "an unreferenced GDScript method must still be dead, got {dead:?}"
    );
    assert!(
        dead.contains(&("tool.py", "_ready")),
        "the exemption must be scoped to .gd files, got {dead:?}"
    );
}
