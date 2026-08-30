//! #480: the grep hook denied a whole command when the extra work was a
//! command substitution or a redirect rather than a `&&` chain.
//!
//! #475/#476 established the rule: a search is only ever a *suggestion*, so it
//! must never veto a line it does not fully model. `has_chained_command` only
//! looked for `&&`, `||`, `;`, `|` and `&`, so a substitution or an output
//! redirect was invisible to it and the whole command was denied — silently
//! discarding the substitution's side effects, or the file the caller meant to
//! write. Both now fall through untouched.
//!
//! `2>`/`2>>` is deliberately still denied: routing the search's own stderr to
//! /dev/null leaves nothing for the denial to discard, so the line really is
//! just the search.

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::hooks::{evaluate_hook_decision_with_env, HookEnv};

fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::Builder::new()
        .prefix("ts480")
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
    for file in ["one.rs", "lib.rs"] {
        std::fs::write(root.join("src").join(file), "fn my_function() {}\n").expect("write source");
    }
    let root = root.canonicalize().expect("canonicalize root");
    (tmp, root)
}

fn env_rooted_at(root: &Path) -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: Some(root.to_path_buf()),
    }
}

fn is_blocked(command: &str, root: &Path) -> bool {
    let input = serde_json::json!({ "command": command }).to_string();
    evaluate_hook_decision_with_env(&input, &env_rooted_at(root)).contains("\"deny\"")
}

#[test]
fn a_command_substitution_passes_through() {
    let (_tmp, root) = project();
    // The substitution runs its own command. Denying the line means it never
    // runs at all, which is exactly the failure #475 ruled out.
    for command in [
        "grep -n my_function src/lib.rs `date +%s`",
        "grep -n my_function src/lib.rs $(date +%s)",
        r#"grep -n "$(cat patterns.txt)" src/lib.rs"#,
        "grep -n \"`cat patterns.txt`\" src/lib.rs",
    ] {
        assert!(
            !is_blocked(command, &root),
            "a substitution must not be denied: {command}"
        );
    }
}

#[test]
fn an_output_redirect_passes_through() {
    let (_tmp, root) = project();
    // The caller asked for a file. A denial produces no file, and the redirect
    // is work the hook cannot offer an equivalent for.
    for command in [
        "grep -n my_function src/lib.rs > /tmp/hits",
        "grep -n my_function src/lib.rs >> /tmp/hits",
        "grep -n my_function src/lib.rs 1> /tmp/hits",
        "grep -n my_function src/lib.rs >/tmp/hits",
    ] {
        assert!(
            !is_blocked(command, &root),
            "an output redirect must not be denied: {command}"
        );
    }
}

#[test]
fn stderr_suppression_is_still_deniable() {
    let (_tmp, root) = project();
    // Unlike the cases above, `2>` discards output rather than producing it:
    // the line still *is* just the search, so the graph is a real substitute
    // and the suggestion stands. This is the row the fix deliberately leaves
    // alone.
    for command in [
        "grep -n my_function src/lib.rs 2>/dev/null",
        "grep -n my_function src/lib.rs 2>> /tmp/err",
    ] {
        assert!(
            is_blocked(command, &root),
            "stderr suppression discards nothing, so it stays deniable: {command}"
        );
    }
}

#[test]
fn quoting_does_not_by_itself_open_the_gate() {
    let (_tmp, root) = project();
    // The redirect scan skips quoted regions, so pin that a quoted pattern is
    // still redirected on its own and only falls through once a real redirect
    // is appended. Without the first assert the second proves nothing.
    assert!(
        is_blocked(r#"grep -n "my_function" src/lib.rs"#, &root),
        "a quoted pattern is still a symbol lookup"
    );
    assert!(
        !is_blocked(r#"grep -n "my_function" src/lib.rs > /tmp/hits"#, &root),
        "appending a redirect is what makes it fall through"
    );
}

#[test]
fn find_gets_the_same_treatment() {
    let (_tmp, root) = project();
    // Both parsers share `has_chained_command`, so the fix must hold for the
    // discovery-by-name path too.
    assert!(
        is_blocked("find . -name '*.rs'", &root),
        "a plain find is still redirected"
    );
    for command in [
        "find . -name '*.rs' > /tmp/files",
        "find . -name '*.rs' $(pwd)",
    ] {
        assert!(
            !is_blocked(command, &root),
            "find must fall through on unmodeled work too: {command}"
        );
    }
}
