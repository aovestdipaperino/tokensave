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

/// Index `files` (path, source) in a tempdir and return the
/// `(name, file)` pairs `tokensave_dead_code` reports.
async fn dead_entries(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    for (path, source) in files {
        fs::write(project.join(path), source).unwrap();
    }

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
        .map(|s| {
            (
                s["name"].as_str().unwrap_or_default().to_string(),
                s["file"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

async fn dead_names(source: &str) -> Vec<String> {
    dead_entries(&[("repro.py", source)])
        .await
        .into_iter()
        .map(|(name, _)| name)
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

#[tokio::test]
async fn dead_code_does_not_flag_function_called_from_simple_statements_at_module_scope() {
    let dead = dead_names(
        r#"def _check():
    return True


def _fail():
    return RuntimeError("x")


def _dispatch():
    pass


def _truly_dead():
    return 1


assert _check()

if not _check():
    raise _fail()

match 1:
    case 1:
        _dispatch()
"#,
    )
    .await;

    for live in ["_check", "_fail", "_dispatch"] {
        assert!(
            !dead.contains(&live.to_string()),
            "{live} is called from a module-scope statement: {dead:?}"
        );
    }
    assert!(
        dead.contains(&"_truly_dead".to_string()),
        "control: {dead:?}"
    );
}

#[tokio::test]
async fn self_attribute_that_shadows_a_method_is_a_field_read_not_a_reference() {
    let dead = dead_names(
        r#"class Job:
    def __init__(self):
        self.status = 1

    def read(self):
        return self.status

    def run(self):
        return self.worker

    def status(self):
        pass

    def worker(self):
        pass
"#,
    )
    .await;

    assert!(
        dead.contains(&"status".to_string()),
        "`self.status` is an instance attribute, so the `status` method is dead: {dead:?}"
    );
    assert!(
        !dead.contains(&"worker".to_string()),
        "`self.worker` is never assigned, so it references the method: {dead:?}"
    );
}

#[tokio::test]
async fn self_reference_resolves_to_the_method_on_the_same_class_not_a_same_named_class_elsewhere()
{
    let used = r#"class Daemon:
    def start(self):
        schedule(self._flush_loop)

    def _flush_loop(self):
        pass


def schedule(callback):
    return callback
"#;
    let unused = r#"class Daemon:
    def _flush_loop(self):
        pass
"#;
    // The unreferenced class is in the file that sorts first, so a
    // first-suffix-match resolver would bind the reference to it.
    let dead = dead_entries(&[("a.py", unused), ("b.py", used)]).await;

    assert!(
        !dead
            .iter()
            .any(|(n, f)| n == "_flush_loop" && f.ends_with("b.py")),
        "b.py::Daemon::_flush_loop is referenced: {dead:?}"
    );
    assert!(
        dead.iter()
            .any(|(n, f)| n == "_flush_loop" && f.ends_with("a.py")),
        "a.py::Daemon::_flush_loop is not referenced and must stay dead: {dead:?}"
    );
}

#[tokio::test]
async fn nested_class_self_assignment_does_not_shadow_outer_class_method() {
    let dead = dead_names(
        r#"class Outer:
    class Inner:
        def __init__(self):
            self.worker = 1

    def run(self):
        return self.worker

    def worker(self):
        pass
"#,
    )
    .await;

    assert!(
        !dead.contains(&"worker".to_string()),
        "Inner's `self.worker` is a different receiver: {dead:?}"
    );
}

#[tokio::test]
async fn annotated_self_assignment_shadows_a_method() {
    let dead = dead_names(
        r#"class Job:
    def __init__(self):
        self.status: int = 1

    def read(self):
        return self.status

    def status(self):
        pass
"#,
    )
    .await;

    assert!(
        dead.contains(&"status".to_string()),
        "`self.status: int = 1` binds an attribute: {dead:?}"
    );
}

#[tokio::test]
async fn class_body_binding_that_overwrites_a_method_shadows_it() {
    let dead = dead_names(
        r#"class C:
    def hook(self):
        pass

    hook = None

    def run(self):
        return self.hook
"#,
    )
    .await;

    assert!(
        dead.contains(&"hook".to_string()),
        "`hook = None` in the class body overwrites the method: {dead:?}"
    );
}

#[tokio::test]
async fn property_setter_assignment_does_not_hide_the_accessors() {
    let dead = dead_names(
        r#"class Job:
    def __init__(self):
        self.status = 1

    @property
    def status(self):
        return self._status

    @status.setter
    def status(self, value):
        self._status = value

    def read(self):
        return self.status
"#,
    )
    .await;

    // The getter and the setter share one qualified name
    // (`repro.py::Job::status`), so the resolver binds the read to one of
    // them. Before this fix `self.status = 1` suppressed the ref and both
    // were reported dead.
    let dead_status = dead.iter().filter(|n| *n == "status").count();
    assert!(
        dead_status < 2,
        "`self.status` reads the property, so at least one accessor is live: {dead:?}"
    );
}

#[tokio::test]
async fn module_scope_parameter_default_call_and_decorator_are_references() {
    let dead = dead_names(
        r#"def _build():
    return 1


def _register(fn):
    return fn


def _truly_dead():
    return 1


def f(x=_build()):
    return x


@_register
def g():
    pass
"#,
    )
    .await;

    assert!(
        !dead.contains(&"_build".to_string()),
        "called in a parameter default: {dead:?}"
    );
    assert!(
        !dead.contains(&"_register".to_string()),
        "applied as a decorator: {dead:?}"
    );
    assert!(
        dead.contains(&"_truly_dead".to_string()),
        "control: {dead:?}"
    );
}

#[tokio::test]
async fn class_header_and_decorator_argument_calls_are_references() {
    let dead = dead_names(
        r#"def _make_base():
    return object


def _register(tag):
    return lambda fn: fn


def _build():
    return "tag"


def _truly_dead():
    return 1


class C(_make_base()):
    pass


@_register(_build())
def g():
    pass
"#,
    )
    .await;

    assert!(
        !dead.contains(&"_make_base".to_string()),
        "called in a class header: {dead:?}"
    );
    assert!(
        !dead.contains(&"_build".to_string()),
        "called in a decorator argument: {dead:?}"
    );
    assert!(
        dead.contains(&"_truly_dead".to_string()),
        "control: {dead:?}"
    );
}
