//! End-to-end regression tests for #224 — Python extractor reference-kind
//! misses that inflated `tokensave_dead_code` / `tokensave_unused_imports`
//! false positives.
//!
//! A second review of the initial fix found the "functions referenced by
//! name" class was only partially covered (parameter defaults and
//! class-level assignments were still missed), and that two other
//! user-facing behaviors — the `__init__.py` unused-import exclusion and
//! cross-file `from x import y` call resolution — had no integration-level
//! coverage. These tests exercise the actual `handle_tool_call` output
//! (not just extractor internals), following the pattern established in
//! `tests/go_bug149_test.rs`.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

fn extract_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

// ---------------------------------------------------------------------------
// P2a/P2b — parameter defaults and class-level assignments must produce
// first-class Uses refs, not just call-argument/module-assignment RHS.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dead_code_does_not_flag_parameter_default_or_class_level_callback() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::write(
        project.join("repro.py"),
        r#"def _default_callback(x):
    return x


def _class_callback(x):
    return x


def _truly_dead():
    return 1


def invoke(callback=_default_callback):
    return callback()


class Registry:
    CALLBACKS = {"x": _class_callback}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_dead_code",
        json!({ "include_public": true }),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let dead: Vec<String> = output["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap_or_default().to_string())
        .collect();

    // Referenced only as a parameter default (P2a) — must not be dead.
    assert!(
        !dead.contains(&"_default_callback".to_string()),
        "_default_callback (parameter default) should not be dead; dead={dead:?}"
    );
    // Referenced only as a class-level attribute value (P2b) — must not be dead.
    assert!(
        !dead.contains(&"_class_callback".to_string()),
        "_class_callback (class-level assignment value) should not be dead; dead={dead:?}"
    );
    // Control: a genuinely unreferenced function must still be flagged, or
    // this test would be vacuous (dead_code reporting nothing at all).
    assert!(
        dead.contains(&"_truly_dead".to_string()),
        "_truly_dead control should be flagged dead; dead={dead:?}"
    );
}

// ---------------------------------------------------------------------------
// P3a — `__init__.py` re-exports must not be flagged unused, while the same
// pattern in a non-`__init__.py` file still is.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unused_imports_excludes_init_py_but_still_flags_other_files() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("pkg")).unwrap();

    fs::write(project.join("pkg/thing.py"), "class Thing:\n    pass\n").unwrap();
    // Re-export: intentionally never referenced within __init__.py itself.
    fs::write(
        project.join("pkg/__init__.py"),
        "from .thing import Thing\n",
    )
    .unwrap();
    // Control: the same "imported but never used in-file" shape, in an
    // ordinary module — must still be flagged.
    fs::write(project.join("other.py"), "import sys\n").unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let imports: Vec<(String, String)> = output["imports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["file"].as_str().unwrap_or_default().to_string(),
                u["unused"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    assert!(
        !imports.iter().any(|(file, _)| file.contains("__init__.py")),
        "__init__.py re-export must never be flagged unused; imports={imports:?}"
    );
    assert!(
        imports
            .iter()
            .any(|(file, unused)| file.contains("other.py") && unused == "sys"),
        "unused `sys` import in an ordinary module should still be flagged; imports={imports:?}"
    );
}

// ---------------------------------------------------------------------------
// P3b — a cross-file `from pkg.mod import fn` call inside a class method
// must resolve; `fn` must not be reported dead.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dead_code_resolves_cross_file_from_import_call_inside_method() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("pkg")).unwrap();

    fs::write(project.join("pkg/__init__.py"), "").unwrap();
    fs::write(
        project.join("pkg/mod.py"),
        "def helper(x):\n    return x + 1\n",
    )
    .unwrap();
    fs::write(
        project.join("main.py"),
        r#"from pkg.mod import helper


class Runner:
    def run(self, x):
        return helper(x)
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(
        &cg,
        "tokensave_dead_code",
        json!({ "include_public": true }),
        None,
        None,
    )
    .await
    .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let dead: Vec<String> = output["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap_or_default().to_string())
        .collect();

    assert!(
        !dead.contains(&"helper".to_string()),
        "helper, called cross-file via `from pkg.mod import helper` inside a method, should not be dead; dead={dead:?}"
    );
}
