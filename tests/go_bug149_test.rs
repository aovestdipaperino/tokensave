//! Regression tests for #149 — two residual Go false positives that survived
//! the #148 fix:
//!
//! * **Bug 1** (`dead_code`): two packages that share a name (`package jobs`
//!   under both `internal/foo/jobs` and `internal/bar/jobs`) each define a
//!   function with the same name (`NewCleanupWorker`). The resolver keyed a
//!   cross-package selector call on the function name only, so both call edges
//!   collapsed onto one target and the other was flagged dead.
//! * **Bug 2** (`unused_imports`): a bare (un-aliased) semantic-import-
//!   versioning path (`github.com/golang-jwt/jwt/v5`) derived its in-scope
//!   identifier as the literal last segment `v5` instead of the package name
//!   `jwt`, so the import was reported unused with `"unused": "v5"`.
//!
//! The fixtures are faithful transcriptions of the minimal repros in the issue.

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
// Bug 1 — same-named funcs in same-named packages must not collide.
// ---------------------------------------------------------------------------

/// Builds the issue's `bug1-deadcode-collision/` module verbatim.
async fn setup_bug1() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("internal/obs")).unwrap();
    fs::create_dir_all(project.join("internal/foo/jobs")).unwrap();
    fs::create_dir_all(project.join("internal/bar/jobs")).unwrap();

    fs::write(
        project.join("go.mod"),
        "module example.com/tsrepro2\n\ngo 1.22\n",
    )
    .unwrap();

    fs::write(
        project.join("main.go"),
        "package main\n\nfunc main() {\n\twire()\n}\n",
    )
    .unwrap();

    fs::write(
        project.join("wiring.go"),
        r#"package main

import (
	"example.com/tsrepro2/internal/obs"

	foojobs "example.com/tsrepro2/internal/foo/jobs"
	barjobs "example.com/tsrepro2/internal/bar/jobs"
)

func wire() {
	_ = obs.MustCounter("requests") // CONTROL: edge resolves
	_ = foojobs.NewCleanupWorker()  // BUG: collides with bar's -> one "dead"
	_ = barjobs.NewCleanupWorker()  // BUG: collides with foo's -> one "dead"
	_ = foojobs.NewFooWorker()      // CONTROL: distinct name, resolves
	_ = barjobs.NewBarWorker()      // CONTROL: distinct name, resolves
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("internal/obs/obs.go"),
        "package obs\n\nfunc MustCounter(name string) int { return len(name) }\n",
    )
    .unwrap();

    fs::write(
        project.join("internal/foo/jobs/jobs.go"),
        r#"package jobs

func NewCleanupWorker() int { return 1 }
func NewFooWorker() int     { return 11 }
"#,
    )
    .unwrap();

    fs::write(
        project.join("internal/bar/jobs/jobs.go"),
        r#"package jobs

func NewCleanupWorker() int { return 2 }
func NewBarWorker() int     { return 22 }
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn dead_code_does_not_flag_same_name_funcs_in_same_name_packages() {
    let (cg, _dir) = setup_bug1().await;
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
    let dead: Vec<(String, String)> = output["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap_or_default().to_string(),
                s["file"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    // Neither NewCleanupWorker definition may be flagged dead — each is called
    // through its own package qualifier.
    assert!(
        !dead.iter().any(|(name, _)| name == "NewCleanupWorker"),
        "no NewCleanupWorker should be dead; dead={dead:?}"
    );

    // Controls must keep resolving too.
    for name in ["NewFooWorker", "NewBarWorker", "MustCounter"] {
        assert!(
            !dead.iter().any(|(n, _)| n == name),
            "{name} control should NOT be flagged dead; dead={dead:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Bug 2 — bare `/vN` import must not be flagged unused.
// ---------------------------------------------------------------------------

/// Builds the issue's `bug2-unusedimport-vN/` module verbatim (sans go.sum,
/// which only matters for `go build`, not for static extraction).
async fn setup_bug2() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::write(
        project.join("go.mod"),
        "module example.com/bug2\n\ngo 1.22\n\nrequire github.com/golang-jwt/jwt/v5 v5.3.1\n",
    )
    .unwrap();

    fs::write(
        project.join("main.go"),
        "package main\n\nfunc main() {\n\t_ = liveCall()\n\t_ = liveVar()\n}\n",
    )
    .unwrap();

    fs::write(
        project.join("a_live.go"),
        r#"package main

import "github.com/golang-jwt/jwt/v5"

func liveCall() any  { return jwt.NewParser() }
func liveVar() error { return jwt.ErrTokenExpired }
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn unused_imports_does_not_flag_used_versioned_go_import() {
    let (cg, _dir) = setup_bug2().await;
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
                u["name"].as_str().unwrap_or_default().to_string(),
                u["unused"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    // The jwt/v5 import is used (jwt.NewParser / jwt.ErrTokenExpired) and must
    // not be reported — and certainly not with the `v5` smoking-gun identifier.
    assert!(
        !imports.iter().any(|(name, _)| name.contains("jwt/v5")),
        "used jwt/v5 import must not be flagged; imports={imports:?}"
    );
    assert!(
        !imports.iter().any(|(_, unused)| unused == "v5"),
        "no import should derive the bare `v5` identifier; imports={imports:?}"
    );
}

#[tokio::test]
async fn unused_imports_still_flags_truly_unused_versioned_go_import() {
    // A `/vN` path that is imported but never referenced must STILL be flagged,
    // under its real package identifier (`pgx`), not the version segment.
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(project.join("go.mod"), "module example.com/u\n\ngo 1.22\n").unwrap();
    fs::write(
        project.join("a.go"),
        r#"package main

import "github.com/jackc/pgx/v5"

func main() {}
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

    assert!(
        flagged.contains(&"pgx".to_string()),
        "unused pgx/v5 should be flagged under `pgx`, not `v5`; flagged={flagged:?}"
    );
}
