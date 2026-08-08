//! Regression tests for #346 (parts 2c and 2d): calls that sit outside a
//! function body must still count as uses.
//!
//! The reporter measured a 97% false-positive rate on Go and 3/3 on TypeScript,
//! and observed that the TS failures were "the simplest possible case (same
//! file, same closure/module scope, direct call)". They were: the extractors
//! only attributed call sites to an enclosing *function* node, so a call made
//! anywhere else — at module top level, or inside a callback passed to a
//! function that initializes a `const` — was recorded against nothing at all
//! and its target looked uncalled. Go had the mirror-image gap for a function
//! used as a value rather than called.
//!
//! The exemptions must stay narrow: a genuinely unreferenced symbol is still
//! dead code, so every fixture here carries a real dead function as a control.

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

async fn dead_names(project: &std::path::Path) -> Vec<String> {
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    let mut names: Vec<String> = cg
        .find_dead_code(&[], true, false)
        .await
        .unwrap()
        .into_iter()
        .map(|node| node.name)
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn typescript_module_level_call_keeps_its_target_alive() {
    // `registerHandlers()` runs at import time. It is a side effect, it sits in
    // no function, and it is unambiguously a use of the callee.
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/toast.ts"),
        r#"function registerHandlers(): void {
  console.log("registered");
}

registerHandlers();

function neverReferenced(): void {
  console.log("dead");
}

export function useToast(): string {
  return "toast";
}
"#,
    )
    .unwrap();

    let dead = dead_names(dir.path()).await;
    assert!(
        !dead.contains(&"registerHandlers".to_string()),
        "a top-level call is a use; got dead: {dead:?}"
    );
    assert!(
        dead.contains(&"neverReferenced".to_string()),
        "the control must still be reported dead; got: {dead:?}"
    );
}

#[tokio::test]
async fn typescript_calls_inside_a_const_initializer_callback_count() {
    // The Pinia/Vue shape from the report: helpers defined at module scope and
    // called only from the setup closure passed to `defineStore(...)`. The
    // arrow gets no graph node of its own, so before the fix its calls were
    // attributed nowhere.
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/favorites.ts"),
        r#"function loadFromStorage(): string[] {
  return [];
}

function saveToStorage(items: string[]): void {
  void items;
}

function orphanHelper(): void {
  // referenced by nobody
}

export const useFavorites = defineStore("favorites", () => {
  const items = loadFromStorage();
  function add(x: string) {
    items.push(x);
    saveToStorage(items);
  }
  return { items, add };
});
"#,
    )
    .unwrap();

    let dead = dead_names(dir.path()).await;
    for live in ["loadFromStorage", "saveToStorage"] {
        assert!(
            !dead.contains(&live.to_string()),
            "{live} is called from the store setup closure; got dead: {dead:?}"
        );
    }
    assert!(
        dead.contains(&"orphanHelper".to_string()),
        "the control must still be reported dead; got: {dead:?}"
    );
}

#[tokio::test]
async fn go_function_assigned_as_a_value_is_not_dead() {
    // `var SandboxSuffixFunc = randomSandboxSuffix` is a reference, not a call,
    // so a scan that only looks for call expressions finds nothing.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("go.mod"),
        "module example.com/u\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("registry.go"),
        r#"package broker

var SandboxSuffixFunc = randomSandboxSuffix

func randomSandboxSuffix() string {
	return "abc"
}

func reservationIDFromRun() string {
	return "never-called"
}
"#,
    )
    .unwrap();

    let dead = dead_names(dir.path()).await;
    assert!(
        !dead.contains(&"randomSandboxSuffix".to_string()),
        "a function used as a value is alive; got dead: {dead:?}"
    );
    assert!(
        dead.contains(&"reservationIDFromRun".to_string()),
        "the genuinely dead function must still be reported; got: {dead:?}"
    );
}

#[tokio::test]
async fn go_same_package_test_caller_keeps_the_target_alive() {
    // Reported as a plain missed same-package edge; it resolves correctly, and
    // this pins that down so it cannot regress alongside the changes above.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("go.mod"),
        "module example.com/u\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("registry.go"),
        "package broker\n\nvar registry []string\n\nfunc reset() {\n\tregistry = nil\n}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("broker_test.go"),
        "package broker\n\nimport \"testing\"\n\nfunc TestReset(t *testing.T) {\n\treset()\n}\n",
    )
    .unwrap();

    let dead = dead_names(dir.path()).await;
    assert!(
        !dead.contains(&"reset".to_string()),
        "a same-package test caller is a use; got dead: {dead:?}"
    );
}
