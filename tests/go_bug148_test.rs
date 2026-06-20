//! Regression tests for #148 — the Go extractor produced massive `dead_code`
//! and `unused_imports` false positives because it never emitted edges for
//! function *value* references, generic calls, or import usage.

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

/// Builds a Go module exercising every reference class from the bug report,
/// then indexes it.
async fn setup() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("obs")).unwrap();
    fs::create_dir_all(project.join("app")).unwrap();

    fs::write(
        project.join("go.mod"),
        "module example.com/repro\n\ngo 1.22\n",
    )
    .unwrap();

    // Cross-package + generic targets.
    fs::write(
        project.join("obs/obs.go"),
        r#"package obs

type Meter struct{}

// MustCounter is called cross-package (class B).
func MustCounter(m Meter, name string) int { return 0 }

// Distinct is a generic function called cross-package (class B/generic).
func Distinct[T comparable](xs []T) []T { return xs }
"#,
    )
    .unwrap();

    fs::write(
        project.join("app/app.go"),
        r#"package app

import (
	"net/url"

	"example.com/repro/obs"
)

// Bug 1 — selector call (`url.Parse`) AND type reference (`*url.URL`) both
// exercise the net/url import.
func Parse(raw string) (*url.URL, error) { return url.Parse(raw) }

// Class A — direct call, even same-file.
func handler()                { _ = readToken(nil) }
func readToken(r *Req) string { return "" }

// Class B — direct cross-package call + generic cross-package call.
func wire(m obs.Meter, xs []int) {
	_ = obs.MustCounter(m, "x")
	_ = obs.Distinct[int](xs)
}

// Class C — function referenced as a value in a slice / composite literal.
var registrations = []func() error{applyA, applyB}

func applyA() error { return nil }
func applyB() error { return nil }
func Apply() error {
	for _, f := range registrations {
		_ = f()
	}
	return nil
}

// Class D — function passed as a value (argument or struct field), including a
// package-level struct-field reference and a call expression in argument
// position.
type entry struct{ wrap func() }

var chain = []entry{{wrap: Recover}}

func setup(mux *Mux, store *Store) {
	mux.HandleFunc("GET /x", HandleX) // arg
	api.WithMiddleware(tagRoute)      // arg (cross-package selector call)
	queue.AddWorker(NewWorker(store)) // call expression in argument position
}

type Mux struct{}
type Store struct{}
type Worker struct{}

func (m *Mux) HandleFunc(pat string, h func()) {}
func HandleX()                                  {}
func tagRoute()                                 {}
func Recover()                                  {}
func NewWorker(s *Store) *Worker                { return nil }
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn dead_code_no_false_positives_for_referenced_functions() {
    let (cg, _dir) = setup().await;
    // include_public so exported functions (HandleX, MustCounter, …) are
    // candidates — they would all be flagged without the new edges.
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
    let dead: Vec<&str> = output["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();

    for name in [
        "readToken",   // class A — same-file call readToken(nil)
        "MustCounter", // class B — cross-package call obs.MustCounter(...)
        "Distinct",    // class B — generic call obs.Distinct[int](...)
        "applyA",      // class C — registry slice value
        "applyB",      // class C — registry slice value
        "HandleX",     // class D — argument value mux.HandleFunc(_, HandleX)
        "tagRoute",    // class D — argument value api.WithMiddleware(tagRoute)
        "Recover",     // class D — package-level struct-field value {wrap: Recover}
        "NewWorker",   // class D — call expr in argument position AddWorker(NewWorker(...))
    ] {
        assert!(
            !dead.contains(&name),
            "{name} should NOT be flagged dead; dead={dead:?}"
        );
    }
}

#[tokio::test]
async fn unused_imports_no_false_positive_for_used_go_import() {
    let (cg, _dir) = setup().await;
    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let unused: Vec<String> = output["imports"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["name"].as_str().map(String::from))
        .collect();

    assert!(
        !unused.iter().any(|n| n.contains("net/url")),
        "net/url is used via url.Parse and must not be flagged; unused={unused:?}"
    );
    assert!(
        !unused.iter().any(|n| n.contains("obs")),
        "obs is used via obs.MustCounter and must not be flagged; unused={unused:?}"
    );
}

#[tokio::test]
async fn unused_imports_still_flags_truly_unused_go_import() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(project.join("go.mod"), "module example.com/u\n\ngo 1.22\n").unwrap();
    fs::write(
        project.join("p.go"),
        r#"package p

import (
	"net/url"
	"strings"
)

func Parse(raw string) (*url.URL, error) { return url.Parse(raw) }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let unused: Vec<String> = output["imports"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["unused"].as_str().map(String::from))
        .collect();

    assert!(
        unused.contains(&"strings".to_string()),
        "strings is imported but never used and should be flagged; unused={unused:?}"
    );
}

#[tokio::test]
async fn unused_imports_handles_aliased_blank_and_dot_go_imports() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(project.join("go.mod"), "module example.com/u\n\ngo 1.22\n").unwrap();
    fs::write(
        project.join("p.go"),
        r#"package p

import (
	u "net/url"
	_ "image/png"
	"strings"
)

func Parse(raw string) (*u.URL, error) { return u.Parse(raw) }
"#,
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let flagged: Vec<String> = output["imports"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["unused"].as_str().map(String::from))
        .collect();

    // Alias `u` is used → not flagged. Blank import (side-effect) → never
    // flagged. Truly-unused `strings` → flagged.
    assert!(
        !flagged.contains(&"u".to_string()),
        "aliased-and-used import must not be flagged; flagged={flagged:?}"
    );
    assert!(
        !flagged
            .iter()
            .any(|n| n == "png" || n.contains("image/png")),
        "blank import must never be flagged; flagged={flagged:?}"
    );
    assert!(
        flagged.contains(&"strings".to_string()),
        "unused strings should still be flagged; flagged={flagged:?}"
    );
}
