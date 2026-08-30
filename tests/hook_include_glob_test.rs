//! A `Bash` grep narrowed by an include glob was classified from its search
//! root alone.
//!
//! `evaluate_grep_tool_input` has honoured the native `Grep` tool's `glob`
//! field since #279: a glob is more specific evidence than the path, so
//! `pattern` + `glob: "*.md"` passes through. The `Bash` path threw the same
//! information away — `extract_grep_invocation` discarded every `-`-prefixed
//! token — so `grep -rn foo --include='*.md' .` was judged on `.`, denied as a
//! code search, and the message even claimed it "targets a code file".
//!
//! Two shapes were affected, both spellings of the same intent:
//!   grep -rn foo --include='*.md' .     (also `--include *.md`)
//!   rg -g '*.md' foo .                  (also `-t md`)
//!
//! The `rg` form was doubly broken: with `-g` unrecognised, its value became
//! the *pattern* and the real pattern became a target.

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::hooks::{evaluate_hook_decision_with_env, HookEnv};

fn env_rooted_at(root: &Path) -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: Some(root.to_path_buf()),
    }
}

/// An indexed project holding both source and documentation.
fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::Builder::new()
        .prefix("tsglob")
        .tempdir()
        .expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".tokensave")).expect("create .tokensave");
    std::fs::write(root.join(".tokensave").join("tokensave.db"), b"").expect("write db");
    let config = TokenSaveConfig {
        root_dir: root.to_string_lossy().to_string(),
        ..TokenSaveConfig::default()
    };
    save_config(&root, &config).expect("save config");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src").join("lib.rs"), "fn my_function() {}\n").expect("write source");
    std::fs::write(root.join("README.md"), "# my_function\n").expect("write doc");
    let root = root.canonicalize().expect("canonicalize root");
    (tmp, root)
}

fn is_blocked(command: &str, env: &HookEnv) -> bool {
    let input = serde_json::json!({ "command": command }).to_string();
    evaluate_hook_decision_with_env(&input, env).contains("\"deny\"")
}

#[test]
fn an_include_glob_for_docs_passes_through() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    for command in [
        "grep -rn my_function --include=*.md .",
        "grep -rn my_function --include *.md .",
        "grep -rn my_function --include='*.md' .",
    ] {
        assert!(
            !is_blocked(command, &env),
            "a search narrowed to docs is not a symbol lookup the graph can answer: {command}"
        );
    }
}

#[test]
fn an_include_glob_for_source_still_redirects() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    for command in [
        "grep -rn my_function --include=*.rs .",
        "grep -rn my_function --include='*.rs' src",
    ] {
        assert!(
            is_blocked(command, &env),
            "a code glob confirms rather than contradicts the redirect: {command}"
        );
    }
}

/// One non-code member is enough. A mixed search spans files the graph cannot
/// answer for, and a partial answer is worse than sending the grep through.
#[test]
fn a_mixed_include_set_passes_through() {
    let (_tmp, root) = project();
    assert!(!is_blocked(
        "grep -rn my_function --include=*.rs --include=*.md .",
        &env_rooted_at(&root)
    ));
}

/// With `-g` unrecognised, `*.md` was read as the pattern and `my_function` as
/// a target — so the verdict was about the wrong string entirely.
#[test]
fn ripgrep_glob_and_type_flags_are_read() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    assert!(!is_blocked("rg -g *.md my_function .", &env));
    assert!(!is_blocked("rg --glob=*.md my_function .", &env));
    assert!(!is_blocked("rg -t md my_function .", &env));
    assert!(is_blocked("rg -g *.rs my_function .", &env));
    assert!(is_blocked("rg -t rust my_function .", &env));
}

/// `--exclude` says what is *not* searched, so its value must not be mistaken
/// for the file set that is.
#[test]
fn an_exclude_glob_does_not_stand_in_for_the_target() {
    let (_tmp, root) = project();
    assert!(is_blocked(
        "grep -rn my_function --exclude=*.md .",
        &env_rooted_at(&root)
    ));
}

/// An out-of-tree search stays out of scope however it is narrowed: the
/// containment check runs before the glob is consulted.
///
/// Expressed as a native `Grep` payload rather than a shell command, because
/// an absolute path is the only way to say "outside this project" and a
/// Windows path in a command string has its backslashes eaten by the hook's
/// own shell-unescaping.
#[test]
fn a_code_glob_does_not_pull_an_outside_path_into_scope() {
    let (_tmp, root) = project();
    let outside = tempfile::Builder::new()
        .prefix("tsglob-outside")
        .tempdir()
        .expect("tempdir");
    let outside = outside.path().canonicalize().expect("canonicalize");
    let input = serde_json::json!({
        "pattern": "my_function",
        "output_mode": "content",
        "path": outside.to_string_lossy(),
        "glob": "*.rs",
    })
    .to_string();
    assert!(
        !evaluate_hook_decision_with_env(&input, &env_rooted_at(&root)).contains("\"deny\""),
        "a code glob describes the file set, not which project owns it"
    );
}
