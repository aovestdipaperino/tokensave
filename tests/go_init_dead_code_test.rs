//! Regression test for #346 (part 2a): Go `func init()` must not be reported
//! as dead code.
//!
//! A package-level `init` in Go is invoked implicitly by the runtime at package
//! initialization and can never be called explicitly — exactly like the Rust
//! trait-impl methods (#137), the Godot `_bind_methods` callback (#269), and
//! `main`, all of which the dead-code query already exempts. Without the
//! exemption, every `init` in a Go codebase is a false positive (21 of 33
//! flags in the issue's sample were `init`).
//!
//! The exemption must stay narrow: a genuinely unreferenced Go function is
//! still dead code and must still be reported.

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

/// A Go module with an implicitly-invoked `init`, a genuinely dead helper, and
/// a live function reached from `init`, so the test can prove the exemption is
/// scoped to `init` and does not suppress real dead code.
async fn setup() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(project.join("go.mod"), "module example.com/u\n\ngo 1.22\n").unwrap();
    fs::write(
        project.join("main.go"),
        r#"
package main

import "fmt"

var registry = map[string]int{}

// init is runtime-dispatched at package load; it has no explicit caller.
func init() {
    registry["seed"] = seedValue()
}

// seedValue is reached only from init — it is alive.
func seedValue() int {
    return 42
}

// unusedHelper is referenced by nothing; it is genuinely dead.
func unusedHelper() int {
    return 7
}

func main() {
    fmt.Println(registry)
}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn go_init_is_not_reported_dead() {
    let (cg, _dir) = setup().await;
    // include_public=true stresses the analysis; `init` is unexported (private)
    // but must be exempt regardless of the visibility filter.
    let dead = cg.find_dead_code(&[], true, false).await.unwrap();
    let dead_names: Vec<&str> = dead.iter().map(|n| n.name.as_str()).collect();

    assert!(
        !dead_names.contains(&"init"),
        "Go `init` is runtime-dispatched and must not be dead code, got {dead_names:?}"
    );
    // seedValue is called from init, so it must not be dead either.
    assert!(
        !dead_names.contains(&"seedValue"),
        "seedValue is called from init and must not be dead code, got {dead_names:?}"
    );
    // The exemption must not swallow real dead code.
    assert!(
        dead_names.contains(&"unusedHelper"),
        "an unreferenced Go function must still be reported dead, got {dead_names:?}"
    );
}
