//! #452: the grep guardrail missed two large classes of real greps.
//!
//! 1. When the target was a directory, the decision came from a list of eight
//!    hardcoded basenames (`src`, `tests`, `lib`, …). That set reads as
//!    Rust/Go/JS convention: it misses the standard Python layout, where the
//!    source directory is named after the package, and any project using
//!    `core/`, `api/`, `server/`. The name was consulted even when the path had
//!    already been resolved *inside* the indexed tree — where containment, not
//!    the name, is the real answer.
//!
//! 2. A definition-anchored pattern — `def place_on_grid`, `class MyError`,
//!    `place_on_grid(` — passed through, because the classifier required the
//!    whole pattern to be a bare identifier. Those are the highest-value
//!    redirects, not the least: anchoring that way is the idiomatic way to
//!    grep for a declaration, which is exactly `tokensave_search`.

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

/// An indexed project whose source directories are named nothing like `src`.
fn project() -> (tempfile::TempDir, PathBuf) {
    // A non-dot prefix: the default `.tmpXXXX` name makes every path inside
    // the tempdir look like it lives under a hidden directory, which the
    // project's own exclude globs drop.
    let tmp = tempfile::Builder::new()
        .prefix("ts452")
        .tempdir()
        .expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".tokensave")).expect("create .tokensave");
    std::fs::write(root.join(".tokensave").join("tokensave.db"), b"").expect("write db");
    let config = TokenSaveConfig {
        root_dir: root.to_string_lossy().to_string(),
        exclude: vec!["vendor/**".to_string(), "vendor".to_string()],
        ..TokenSaveConfig::default()
    };
    save_config(&root, &config).expect("save config");
    for dir in ["mypkg", "core", "api", "src", "vendor"] {
        std::fs::create_dir_all(root.join(dir)).expect("create dir");
        std::fs::write(
            root.join(dir).join("mod.py"),
            "def my_function():\n    pass\n",
        )
        .expect("write source");
    }
    // A directory inside the tree that holds no source: containment alone must
    // not be enough to redirect it.
    std::fs::create_dir_all(root.join("docs")).expect("create docs");
    std::fs::write(root.join("docs").join("guide.md"), "# guide\n").expect("write doc");
    // macOS puts tempdirs behind a `/var` -> `/private/var` symlink; the hook
    // canonicalizes the grep target, so the root it compares against must be
    // canonical too or every path reads as out-of-tree.
    let root = root.canonicalize().expect("canonicalize root");
    (tmp, root)
}

/// A native `Grep` payload. Used wherever the target must be an *absolute*
/// path: a shell command carrying a Windows path has its backslashes eaten by
/// the hook's own shell-unescaping, and a relative target resolves against the
/// process cwd rather than the project root, so neither can express "this
/// exact directory inside the project".
fn grep_is_blocked(path: &Path, env: &HookEnv) -> bool {
    let input = serde_json::json!({
        "pattern": "my_function",
        "output_mode": "content",
        "path": path.to_string_lossy(),
    })
    .to_string();
    evaluate_hook_decision_with_env(&input, env).contains("\"deny\"")
}

fn is_blocked(command: &str, env: &HookEnv) -> bool {
    // `evaluate_hook_decision_with_env` takes the tool *input* object, not the
    // whole hook event.
    let input = serde_json::json!({ "command": command }).to_string();
    evaluate_hook_decision_with_env(&input, env).contains("\"deny\"")
}

#[test]
fn in_tree_source_dirs_are_recognized_whatever_they_are_named() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    for dir in ["mypkg", "core", "api", "src"] {
        assert!(
            grep_is_blocked(&root.join(dir), &env),
            "{dir}/ resolves inside the indexed tree and must be redirected"
        );
    }
}

#[test]
fn excluded_and_out_of_tree_dirs_still_pass_through() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    // Inside the tree but excluded from the index: #448's rule still wins.
    assert!(
        !grep_is_blocked(&root.join("vendor"), &env),
        "excluded dir must pass through"
    );
    // Outside the project entirely.
    assert!(
        !grep_is_blocked(&root.join("..").join("elsewhere"), &env),
        "out-of-tree dir must pass through"
    );
    // In-tree, not excluded, but holding no source the index could answer for.
    assert!(
        !grep_is_blocked(&root.join("docs"), &env),
        "a doc-only dir must pass through"
    );
}

#[test]
fn definition_anchored_patterns_are_redirected() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    for pattern in [
        "def place_on_grid",
        "class MyError",
        "place_on_grid(",
        "fn handle_request",
        "func HandleRequest",
        "^def foo",
        "struct Node",
    ] {
        let cmd = format!("grep -rn '{pattern}' src");
        assert!(
            is_blocked(&cmd, &env),
            "`{pattern}` is a declaration lookup"
        );
    }
}

#[test]
fn prose_and_structural_patterns_still_pass_through() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    for pattern in [
        // Two words, neither a declaration keyword.
        "return foo",
        // A keyword prefix but more than one identifier after it.
        "class MyError extends Base",
        // Keyword with nothing to name.
        "def ",
        // Structural regex, not an identifier (#449's shape).
        "(res, ctx)",
        "TODO: fix this",
        "error handling",
    ] {
        let cmd = format!("grep -rn '{pattern}' src");
        assert!(
            !is_blocked(&cmd, &env),
            "`{pattern}` is not a symbol lookup"
        );
    }
}

#[test]
fn a_keyword_prefix_requires_a_word_break() {
    let (_tmp, root) = project();
    let env = env_rooted_at(&root);
    // A keyword is only an anchor when a word break follows it, so `typeof x`
    // is not read as the `type` keyword followed by the name `of x`, and
    // `default handler` is not `def` + `ault handler`.
    // Targeted at a real in-tree source dir via a Grep payload, so the
    // decision turns on the *pattern* and not on the target failing to
    // resolve — otherwise this would pass even if the classifier changed.
    for pattern in ["typeof x", "default handler", "className foo"] {
        let input = serde_json::json!({
            "pattern": pattern,
            "output_mode": "content",
            "path": root.join("mypkg").to_string_lossy(),
        })
        .to_string();
        assert!(
            !evaluate_hook_decision_with_env(&input, &env).contains("\"deny\""),
            "`{pattern}` has no word break after a keyword and must pass through"
        );
    }
    // Control: the same target with a real definition pattern must block, so
    // the assertions above cannot pass because the target was never eligible.
    let control = serde_json::json!({
        "pattern": "def place_on_grid",
        "output_mode": "content",
        "path": root.join("mypkg").to_string_lossy(),
    })
    .to_string();
    assert!(
        evaluate_hook_decision_with_env(&control, &env).contains("\"deny\""),
        "the target must be one the guardrail would otherwise act on"
    );
}
