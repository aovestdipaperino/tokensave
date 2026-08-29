//! The grep guardrail must not block paths the project itself excludes from
//! indexing — #448.
//!
//! The hook blocked any symbol-shaped grep whose target resolved under the
//! project root. But "inside the tree" is not "inside the index": a project's
//! own `.tokensave/config.json` `exclude` globs keep `node_modules`, `vendor`,
//! `build`, `target` and friends out of the graph entirely. Redirecting a
//! grep against those to `tokensave_search` sends the caller to a tool that
//! provably cannot answer, and the only way through was
//! `TOKENSAVE_DISABLE_GREP_HOOK=1` — a full round-trip for a query nothing
//! else can serve.
//!
//! Same shape as #435, which fixed the out-of-tree half of the same
//! confusion: the indexer and the hook were reading two different notions of
//! "in scope".

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::hooks::{evaluate_hook_decision_with_env, HookEnv};

/// Write a complete config carrying `exclude`. Built from the real default
/// and saved through `save_config`, so the file has every field a project's
/// own config has — a hand-rolled partial JSON fails to parse, and the hook
/// then falls back to its old behavior, which would make these tests pass for
/// the wrong reason.
fn write_config(root: &Path, exclude: &[&str]) {
    let config = TokenSaveConfig {
        root_dir: root.to_string_lossy().to_string(),
        exclude: exclude.iter().map(|s| (*s).to_string()).collect(),
        ..TokenSaveConfig::default()
    };
    save_config(root, &config).expect("save config");
}

fn env_rooted_at(root: &Path) -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: Some(root.to_path_buf()),
    }
}

/// An indexed project holding both an indexed source tree and two excluded
/// vendored trees. The config is written explicitly rather than defaulted, so
/// the test states the exclusions it depends on.
fn project_with_excluded_dirs() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".tokensave")).expect("create .tokensave");
    std::fs::write(root.join(".tokensave").join("tokensave.db"), b"").expect("write db");
    write_config(
        &root,
        &[
            "**/node_modules/**",
            "**/node_modules",
            "vendor/**",
            "vendor",
        ],
    );

    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src").join("lib.rs"),
        "fn someExportedName() {}\n",
    )
    .expect("lib");

    let pkg = root.join("node_modules").join("some-package").join("lib");
    std::fs::create_dir_all(&pkg).expect("create node_modules");
    std::fs::write(
        pkg.join("index.d.ts"),
        "export function someExportedName(): void;\n",
    )
    .expect("write d.ts");

    std::fs::create_dir_all(root.join("vendor").join("dep")).expect("create vendor");
    std::fs::write(
        root.join("vendor").join("dep").join("a.go"),
        "func someExportedName() {}\n",
    )
    .expect("write go");

    (tmp, root)
}

fn decide(command: &str, env: &HookEnv) -> String {
    let input = serde_json::json!({ "command": command }).to_string();
    evaluate_hook_decision_with_env(&input, env)
}

fn is_blocked(decision: &str) -> bool {
    decision.contains("\"deny\"") || decision.to_lowercase().contains("stop:")
}

/// The reporter's exact command. `tokensave_search` returns nothing for it,
/// because the project's own config keeps `node_modules` out of the index.
#[test]
fn a_grep_into_an_excluded_vendor_directory_is_not_redirected() {
    let (_tmp, root) = project_with_excluded_dirs();
    let env = env_rooted_at(&root);

    for target in [
        "node_modules/some-package/lib/index.d.ts",
        "node_modules/some-package/lib",
        "node_modules",
        "vendor/dep/a.go",
        "vendor",
    ] {
        let decision = decide(&format!("grep -n \"someExportedName\" {target}"), &env);
        assert!(
            !is_blocked(&decision),
            "grep into excluded path {target} must pass through, got: {decision}"
        );
    }
}

/// The guardrail must still do its job for the code the index actually
/// covers. Without this, the fix above could pass by disabling the hook.
#[test]
fn a_grep_into_indexed_source_is_still_redirected() {
    let (_tmp, root) = project_with_excluded_dirs();
    let env = env_rooted_at(&root);

    let decision = decide("grep -n \"someExportedName\" src/lib.rs", &env);
    assert!(
        is_blocked(&decision),
        "indexed source must still be redirected, got: {decision}"
    );
}

/// An absolute spelling of an excluded path is the same path, and must be
/// treated the same way — the containment check already canonicalizes, so the
/// exclusion check has to as well.
#[test]
fn an_absolute_spelling_of_an_excluded_path_is_also_allowed() {
    let (_tmp, root) = project_with_excluded_dirs();
    let env = env_rooted_at(&root);

    let abs = root
        .join("node_modules")
        .join("some-package")
        .join("lib")
        .join("index.d.ts");
    let decision = decide(
        &format!("grep -n \"someExportedName\" {}", abs.display()),
        &env,
    );
    assert!(
        !is_blocked(&decision),
        "absolute excluded path must pass through, got: {decision}"
    );
}

/// A project whose config excludes nothing keeps the previous behavior
/// exactly — the fix must be driven by the config, not by directory names the
/// hook recognizes on its own.
#[test]
fn without_an_exclude_glob_the_same_path_is_still_redirected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".tokensave")).expect("create .tokensave");
    std::fs::write(root.join(".tokensave").join("tokensave.db"), b"").expect("write db");
    write_config(&root, &[]);
    let pkg = root.join("node_modules").join("some-package").join("lib");
    std::fs::create_dir_all(&pkg).expect("create node_modules");
    std::fs::write(
        pkg.join("index.d.ts"),
        "export function someExportedName(): void;\n",
    )
    .expect("write d.ts");

    let decision = decide(
        "grep -n \"someExportedName\" node_modules/some-package/lib/index.d.ts",
        &env_rooted_at(&root),
    );
    assert!(
        is_blocked(&decision),
        "with nothing excluded, the guardrail still applies, got: {decision}"
    );
}
