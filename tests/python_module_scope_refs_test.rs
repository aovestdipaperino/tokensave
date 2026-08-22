//! End-to-end regression tests for two Python extractor misses that made
//! `tokensave_dead_code` report live symbols:
//!
//! 1. A call in a module-scope statement (`_KEYMAP = _build_keymap()`,
//!    or a bare `setup()`) produced no `calls` ref, because call sites were
//!    only extracted from function bodies.
//! 2. A method passed by reference through `self` (`Thread(target=self._run)`,
//!    `schedule(self._tick, 1.0)`) produced no ref, because the value-position
//!    scan stopped at every `attribute` node.
//!
//! Same pattern as `tests/python_bug224_test.rs`: index a tempdir project and
//! check the real `tokensave_dead_code` tool output.

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

async fn dead_names(source: &str) -> Vec<String> {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(project.join("repro.py"), source).unwrap();

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
    output["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn dead_code_does_not_flag_function_called_at_module_scope() {
    let dead = dead_names(
        r#"def _build_keymap():
    return {"a": 1}


def _setup():
    pass


def _run():
    pass


def _truly_dead():
    return 1


_KEYMAP = _build_keymap()
_setup()

if __name__ == "__main__":
    _run()
"#,
    )
    .await;

    assert!(
        !dead.contains(&"_run".to_string()),
        "called under `if __name__ == \"__main__\":`: {dead:?}"
    );

    assert!(
        !dead.contains(&"_build_keymap".to_string()),
        "called from a module-level assignment RHS: {dead:?}"
    );
    assert!(
        !dead.contains(&"_setup".to_string()),
        "called from a bare module-level statement: {dead:?}"
    );
    assert!(
        dead.contains(&"_truly_dead".to_string()),
        "control: an uncalled function must still be dead: {dead:?}"
    );
}

#[tokio::test]
async fn dead_code_does_not_flag_method_passed_by_reference_via_self() {
    let dead = dead_names(
        r#"import threading


class Daemon:
    def start(self):
        threading.Thread(target=self._flush_loop, daemon=True).start()
        schedule_interval(self._tick_timer, 1.0)
        self.scanner.on_scan(self._handle_scan)

    def _flush_loop(self):
        pass

    def _tick_timer(self, dt):
        pass

    def _handle_scan(self, code):
        pass

    def _truly_dead(self):
        pass


def schedule_interval(callback, seconds):
    return callback
"#,
    )
    .await;

    for live in ["_flush_loop", "_tick_timer", "_handle_scan"] {
        assert!(
            !dead.contains(&live.to_string()),
            "{live} is passed by reference through self: {dead:?}"
        );
    }
    assert!(
        dead.contains(&"_truly_dead".to_string()),
        "control: an unreferenced method must still be dead: {dead:?}"
    );
}
