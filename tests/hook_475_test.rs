//! #475: a search *in front of* other work denied the whole command.
//!
//! `extract_grep_invocation` only required the line to *start* with
//! `grep`/`rg`/`ag`, then read everything after it as that grep's arguments.
//! A trailing `&& ./deploy.sh` was therefore invisible, and the deny threw the
//! deploy away along with the search. The guardrail exists to suggest a
//! cheaper search, so it must never destroy work it does not model: a search
//! chained to anything else now passes through.

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

/// An indexed project with one source file the guardrail recognizes as code.
fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::Builder::new()
        .prefix("ts475")
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
    let root = root.canonicalize().unwrap_or(root);
    (tmp, root)
}

/// `evaluate_hook_decision_with_env` takes the tool *input* object, not the
/// whole hook event, and returns an empty string when the call is allowed.
fn is_blocked(command: &str, root: &Path) -> bool {
    let input = serde_json::json!({ "command": command }).to_string();
    evaluate_hook_decision_with_env(&input, &env_rooted_at(root)).contains("\"deny\"")
}

/// The unchained search is still redirected — the guardrail has not been
/// turned off, only stopped from vetoing commands it cannot see all of.
#[test]
fn bare_search_is_still_redirected() {
    let (_tmp, root) = project();
    let cmd = format!("grep -n MySymbol {}", root.join("src/lib.rs").display());
    assert!(
        is_blocked(&cmd, &root),
        "a plain code-file grep should still redirect"
    );
}

#[test]
fn search_followed_by_other_work_passes_through() {
    let (_tmp, root) = project();
    let target = root.join("src/lib.rs");
    for suffix in ["&& git status", "&& ./deploy.sh", "|| true", "; git status"] {
        let cmd = format!("grep -n MySymbol {} {suffix}", target.display());
        assert!(
            !is_blocked(&cmd, &root),
            "chained command must not be denied: {cmd}"
        );
    }
}

#[test]
fn piped_search_passes_through() {
    let (_tmp, root) = project();
    let cmd = format!(
        "grep -n MySymbol {} | head -5",
        root.join("src/lib.rs").display()
    );
    assert!(
        !is_blocked(&cmd, &root),
        "a piped search must not be denied"
    );
}

/// An operator inside quotes is an argument, not a chained command, so the
/// line is still one search and still redirects.
#[test]
fn quoted_operator_is_not_a_chain() {
    let (_tmp, root) = project();
    let cmd = format!(
        "grep -n my_function {} -e 'a;b'",
        root.join("src/lib.rs").display()
    );
    assert!(
        is_blocked(&cmd, &root),
        "an operator inside a quoted argument is not a chained command"
    );
}

/// The same rule governs `find`/`fd`: a discovery call chained to real work
/// must not take that work down with it.
#[test]
fn find_followed_by_other_work_passes_through() {
    let (_tmp, root) = project();
    let cmd = format!("find {} -name '*.rs' && ./deploy.sh", root.display());
    assert!(!is_blocked(&cmd, &root), "chained find must not be denied");
}
