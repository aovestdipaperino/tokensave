//! #450: an index whose *scope* is wrong is reported at server start, not
//! refused.
//!
//! `serve`'s existing guardrail refuses an *uninitialized* directory and says
//! nothing about one that is initialized and absurd — a home directory
//! carrying a `.tokensave/`, which every later `serve` inherits and keeps
//! syncing. Retroactively applying #396's cap would decide for the user which
//! of their existing setups stop working, so this warns and continues, with a
//! config switch for the user who meant it.

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::index_scope::{scope_warnings, OVERSIZED_INDEX_BYTES};

fn initialize(root: &Path, db_bytes: u64) {
    std::fs::create_dir_all(root.join(".tokensave")).expect("create .tokensave");
    let db = root.join(".tokensave").join("tokensave.db");
    let file = std::fs::File::create(&db).expect("create db");
    file.set_len(db_bytes).expect("size db");
    let config = TokenSaveConfig {
        root_dir: root.to_string_lossy().to_string(),
        ..TokenSaveConfig::default()
    };
    save_config(root, &config).expect("save config");
}

fn tmp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("ts450")
        .tempdir()
        .expect("tempdir")
}

fn paths(dir: &tempfile::TempDir) -> (PathBuf, PathBuf) {
    (dir.path().join("home"), dir.path().join("home/code/proj"))
}

#[test]
fn an_indexed_home_directory_is_reported_from_any_working_directory() {
    let dir = tmp();
    let (home, proj) = paths(&dir);
    initialize(&home, 1024);
    initialize(&proj, 1024);

    // Standing in an ordinary project that happens to live under an indexed
    // home: the home is still the problem, and nobody is standing in it.
    let warnings = scope_warnings(&proj, Some(&home));
    assert_eq!(warnings.len(), 1, "got: {warnings:?}");
    assert!(
        warnings[0].contains("Home directory is initialized as a project"),
        "got: {}",
        warnings[0]
    );
}

#[test]
fn an_ordinary_project_under_an_unindexed_home_is_quiet() {
    let dir = tmp();
    let (home, proj) = paths(&dir);
    std::fs::create_dir_all(&home).expect("create home");
    initialize(&proj, 1024);
    assert!(
        scope_warnings(&proj, Some(&home)).is_empty(),
        "the ordinary case must say nothing at all"
    );
}

#[test]
fn an_oversized_index_is_reported_with_its_size() {
    let dir = tmp();
    let (home, proj) = paths(&dir);
    std::fs::create_dir_all(&home).expect("create home");
    initialize(&proj, OVERSIZED_INDEX_BYTES + 1);

    let warnings = scope_warnings(&proj, Some(&home));
    assert_eq!(warnings.len(), 1, "got: {warnings:?}");
    assert!(
        warnings[0].contains("Index is unusually large"),
        "got: {}",
        warnings[0]
    );
}

#[test]
fn an_index_just_under_the_threshold_is_quiet() {
    let dir = tmp();
    let (home, proj) = paths(&dir);
    std::fs::create_dir_all(&home).expect("create home");
    initialize(&proj, OVERSIZED_INDEX_BYTES - 1);
    assert!(
        scope_warnings(&proj, Some(&home)).is_empty(),
        "the threshold must not fire below itself"
    );
}

/// Standing *in* the indexed home directory must not report it twice — once as
/// the home and once as an oversized current project.
#[test]
fn the_home_directory_is_reported_once_when_it_is_also_the_project() {
    let dir = tmp();
    let (home, _) = paths(&dir);
    initialize(&home, OVERSIZED_INDEX_BYTES + 1);

    let warnings = scope_warnings(&home, Some(&home));
    assert_eq!(warnings.len(), 1, "got: {warnings:?}");
    assert!(
        warnings[0].contains("Home directory"),
        "got: {}",
        warnings[0]
    );
}

/// The escape hatch. The detection still fires — this is about what the user
/// is shown, so `scope_warnings` stays honest and only the printer is quiet.
#[test]
fn the_config_switch_silences_the_warning_without_silencing_the_detection() {
    let dir = tmp();
    let (home, _) = paths(&dir);
    initialize(&home, 1024);

    let mut config = tokensave::config::load_config(&home).expect("load config");
    config.suppress_scope_warning = true;
    save_config(&home, &config).expect("save config");

    assert!(
        tokensave::config::load_config(&home)
            .expect("reload")
            .suppress_scope_warning,
        "the switch must round-trip through the config file"
    );
    assert!(
        !scope_warnings(&home, Some(&home)).is_empty(),
        "the detection itself must not depend on the display switch"
    );
}

/// A config written before #450 has no such key, and must keep working.
#[test]
fn a_config_without_the_key_defaults_to_warning() {
    let dir = tmp();
    let root = dir.path().join("legacy");
    initialize(&root, 1024);
    // A real config with only this key removed — a hand-rolled minimal JSON
    // fails to parse on unrelated fields and would pass for the wrong reason.
    let path = root.join(".tokensave").join("config.json");
    let raw = std::fs::read_to_string(&path).expect("read config");
    let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse config");
    value
        .as_object_mut()
        .expect("config object")
        .remove("suppress_scope_warning");
    std::fs::write(&path, value.to_string()).expect("write legacy config");

    let config = tokensave::config::load_config(&root).expect("load legacy config");
    assert!(
        !config.suppress_scope_warning,
        "an older config must keep getting the warning"
    );
}
