//! #648, #649, #654: grep-hook gaps and an unhelpful denial message.
//!
//! #648 listed command shapes that still let a symbol search through:
//! `git grep` (which searches the working tree unless given a revision), a
//! search batched behind a read-only command such as `sed -n …p` or another
//! grep, an alternation where only one branch is symbol-shaped, a call-site
//! pattern with something after the `(`, and a search piped into a read-only
//! filter such as `head`.
//!
//! #649: the denial pointed usages at `tokensave_callers_for`, which takes node
//! ids and is not a core tool. #654: the denial should carry the concrete
//! replacement call, with the symbol from the denied command, so an agent can
//! copy it verbatim.

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::hooks::{evaluate_hook_decision_with_env, HookEnv};

fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::Builder::new()
        .prefix("ts648")
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
    std::fs::create_dir_all(root.join("docs")).expect("create docs");
    std::fs::write(root.join("src").join("x.ts"), "function MySymbol() {}\n").expect("write ts");
    std::fs::write(root.join("src").join("lib.rs"), "fn my_symbol() {}\n").expect("write rs");
    std::fs::write(root.join("x.ts"), "MySymbol();\n").expect("write root ts");
    std::fs::write(root.join("notes.md"), "notes\n").expect("write notes");
    std::fs::write(root.join("docs").join("guide.md"), "MySymbol\n").expect("write docs");
    let root = root.canonicalize().expect("canonicalize root");
    (tmp, root)
}

fn env_rooted_at(root: &Path) -> HookEnv {
    HookEnv {
        in_tokensave_project: true,
        disable_grep_hook: false,
        project_root: Some(root.to_path_buf()),
        cwd: Some(root.to_path_buf()),
    }
}

fn decision(command: &str, root: &Path) -> String {
    let input = serde_json::json!({ "command": command }).to_string();
    evaluate_hook_decision_with_env(&input, &env_rooted_at(root))
}

fn is_blocked(command: &str, root: &Path) -> bool {
    decision(command, root).contains("\"deny\"")
}

fn reason(command: &str, root: &Path) -> String {
    let out = decision(command, root);
    let v: serde_json::Value = serde_json::from_str(&out).expect("deny decision is JSON");
    v["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

// ---------------------------------------------------------------------------
// #648: shapes that must now be denied
// ---------------------------------------------------------------------------

#[test]
fn shapes_from_648_are_denied() {
    let (_tmp, root) = project();
    for command in [
        "git grep -n MySymbol",
        "git grep -n MySymbol -- src",
        "sed -n 1,20p notes.md; grep -rn MySymbol src",
        r#"grep -n "a b" x.ts; grep -rn MySymbol src --include=*.ts"#,
        r#"grep -n "MySymbol\|throw new Error" src/x.ts"#,
        r#"grep -n "MySymbol(name" src/x.ts"#,
    ] {
        assert!(is_blocked(command, &root), "must be denied: {command}");
    }
}

#[test]
fn already_correct_shapes_stay_denied() {
    let (_tmp, root) = project();
    for command in ["grep -rn MySymbol src/", "ls; grep -rn MySymbol src"] {
        assert!(is_blocked(command, &root), "must stay denied: {command}");
    }
}

#[test]
fn a_search_piped_into_a_read_only_filter_is_denied() {
    let (_tmp, root) = project();
    for command in [
        "grep -rn MySymbol src | head -20",
        "rg -n MySymbol src | sort | uniq",
        "grep -rn MySymbol src 2>/dev/null | wc -l",
        "echo start; grep -rn MySymbol src | tail -5",
    ] {
        assert!(is_blocked(command, &root), "must be denied: {command}");
    }
}

// ---------------------------------------------------------------------------
// #648: what must keep passing through
// ---------------------------------------------------------------------------

#[test]
fn side_effecting_neighbours_still_pass_through() {
    let (_tmp, root) = project();
    for command in [
        // #475: a denial would discard the other work.
        "sed -i s/a/b/ notes.md; grep -rn MySymbol src",
        "grep -rn MySymbol src | xargs rm",
        "grep -rn MySymbol src | tee out.txt",
        "grep -rn MySymbol src > out.txt",
        "git checkout -b x && git grep -n MySymbol",
        "grep -rn MySymbol src || true",
    ] {
        assert!(!is_blocked(command, &root), "must pass through: {command}");
    }
}

#[test]
fn non_code_and_history_searches_still_pass_through() {
    let (_tmp, root) = project();
    for command in [
        "grep -rn MySymbol docs --include='*.md'",
        "sed -n 1,20p notes.md; grep -rn MySymbol . --include='*.md'",
        "git grep -n MySymbol HEAD~3",
        "git grep -n MySymbol main -- src",
        "git grep -n MySymbol -- '*.md'",
        "ls src | grep MySymbol",
        r#"grep -n "failed to connect\|throw new Error" src/x.ts"#,
        "sed -n 1p notes.md; TOKENSAVE_DISABLE_GREP_HOOK=1 grep -rn MySymbol src",
    ] {
        assert!(!is_blocked(command, &root), "must pass through: {command}");
    }
}

#[test]
fn the_env_opt_out_still_wins_for_compound_commands() {
    let (_tmp, root) = project();
    let env = HookEnv {
        disable_grep_hook: true,
        ..env_rooted_at(&root)
    };
    let input = serde_json::json!({
        "command": "sed -n 1,20p notes.md; grep -rn MySymbol src | head"
    })
    .to_string();
    assert!(evaluate_hook_decision_with_env(&input, &env).is_empty());
}

// ---------------------------------------------------------------------------
// #649 / #654: the denial names a core tool and the concrete call
// ---------------------------------------------------------------------------

#[test]
fn denial_never_points_at_callers_for() {
    let (_tmp, root) = project();
    for command in [
        "grep -rn MySymbol src",
        r#"grep -rn "\bMySymbol\b" src"#,
        "grep -rn 'MySymbol(' src",
        "grep -rn 'Alpha|Beta' src",
        "grep -rn 'class MySymbol' src",
    ] {
        let r = reason(command, &root);
        assert!(!r.contains("callers_for"), "{command}: {r}");
        assert!(!r.contains("signature_search"), "{command}: {r}");
    }
}

#[test]
fn denial_carries_copyable_calls_with_the_symbol() {
    let (_tmp, root) = project();
    let r = reason("grep -rn MySymbol src", &root);
    assert!(
        r.contains(r#"tokensave_search {"query": "MySymbol"}"#),
        "definition call missing: {r}"
    );
    assert!(
        r.contains(r#"tokensave_search {"query": ".MySymbol(", "literal": true}"#),
        "method-use call missing: {r}"
    );
    assert!(
        r.contains(r#"tokensave_search {"query": "MySymbol(", "literal": true}"#),
        "every-call call missing: {r}"
    );
}

#[test]
fn denial_names_the_symbol_from_each_shape() {
    let (_tmp, root) = project();
    for (command, symbol) in [
        ("git grep -n MySymbol", "MySymbol"),
        (r#"grep -n "MySymbol(name" src/x.ts"#, "MySymbol"),
        (
            r#"grep -n "MySymbol\|throw new Error" src/x.ts"#,
            "MySymbol",
        ),
        ("sed -n 1,20p notes.md; grep -rn OtherSym src", "OtherSym"),
        ("grep -rn 'def my_symbol' src", "my_symbol"),
    ] {
        let r = reason(command, &root);
        let call = format!(r#"tokensave_search {{"query": "{symbol}"}}"#);
        assert!(r.contains(&call), "{command}: expected `{call}` in {r}");
    }
}

#[test]
fn alternation_denial_lists_one_call_per_name() {
    let (_tmp, root) = project();
    let r = reason("grep -rnE 'Alpha|Beta' src", &root);
    for name in ["Alpha", "Beta"] {
        let call = format!(r#"tokensave_search {{"query": "{name}"}}"#);
        assert!(r.contains(&call), "expected `{call}` in {r}");
    }
}
