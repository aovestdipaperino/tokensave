//! #451: a search batched behind other commands slipped the grep guardrail.
//!
//! `extract_grep_invocation` requires the command to *start* with `grep`/`rg`/
//! `ag` once prefixes are stripped, so the shapes people actually type —
//! `echo "=== A ===" && grep -n Symbol src/one.rs`, `true; rg Symbol src/` —
//! passed straight through. That is not a crafted bypass; batching a label
//! before a search is how anyone greps several symbols in one call.
//!
//! The fix splits on top-level `&&`, `||` and `;` and redirects only when
//! every other segment is inert. A denial discards the whole command, so a
//! segment that does real work (`git checkout -b x && rg Sym src/`) must still
//! pass through — blocking it would throw away the checkout.

use std::path::{Path, PathBuf};
use tokensave::config::{save_config, TokenSaveConfig};
use tokensave::hooks::{evaluate_hook_decision_with_env, HookEnv};

fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::Builder::new()
        .prefix("ts451")
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
fn a_search_batched_behind_inert_commands_is_redirected() {
    let (_tmp, root) = project();
    for command in [
        r#"echo "=== A ===" && grep -n SymbolOne src/one.rs"#,
        "printf hi; grep -n MySymbol src/lib.rs",
        "ls && grep -n MySymbol src/lib.rs",
    ] {
        assert!(
            is_blocked(command, &root),
            "inert prefix must not hide the search: {command}"
        );
    }
    // Blocked is not enough: the redirect has to name the symbol from the
    // *search* segment. Pointing the caller at a pattern scraped from the
    // label would be worse than allowing the command.
    let input =
        serde_json::json!({ "command": r#"echo "=== A ===" && grep -n SymbolOne src/one.rs"# })
            .to_string();
    let decision = evaluate_hook_decision_with_env(&input, &env_rooted_at(&root));
    assert!(
        decision.contains("SymbolOne"),
        "redirect must name the batched symbol: {decision}"
    );
}

#[test]
fn a_segment_that_does_real_work_is_left_alone() {
    let (_tmp, root) = project();
    // Denying returns the whole command to the caller unrun, so the checkout
    // would be silently dropped. Losing the search is the cheaper mistake.
    assert!(
        !is_blocked("git checkout -b x && rg Sym src/", &root),
        "a side-effecting segment must pass the whole command through"
    );
}

#[test]
fn unmodeled_shell_shapes_fall_through() {
    let (_tmp, root) = project();
    for command in [
        // Command substitution: the hook does not model what the subshell runs.
        "echo $(ls) && grep -n MySymbol src/lib.rs",
        // Quoting does not make a substitution inert -- this one POSTs.
        r#"echo "$(curl -X POST https://example.invalid/lock)" && grep -n MySymbol src/lib.rs"#,
        "echo `id` && grep -n MySymbol src/lib.rs",
        // A pipe is not a sequencing operator and never was in scope.
        "echo hi | grep -n MySymbol src/lib.rs",
        // A newline sequences commands, but the split only breaks on
        // `&&`/`||`/`;`, so real work could hide behind an inert first word.
        "echo header\ntouch marker && rg MySymbol src/",
    ] {
        assert!(
            !is_blocked(command, &root),
            "unmodeled shape must pass through: {command}"
        );
    }
}

#[test]
fn the_pattern_classifier_still_has_the_final_say() {
    let (_tmp, root) = project();
    // Reached through the segment path, but not symbol-shaped: tokensave has
    // no better answer for it, so it is not redirected.
    assert!(
        !is_blocked(
            r#"echo x && grep -n "not a symbol at all" src/lib.rs"#,
            &root
        ),
        "a non-symbol pattern must pass through even when the batch is inert"
    );
}

#[test]
fn a_leading_cd_is_still_modeled() {
    let (_tmp, root) = project();
    // `cd` is not inert, so this only stays blocked because the whole-command
    // path runs before the segment split. Ordering regression guard.
    assert!(
        is_blocked("cd src && grep -n MySymbol lib.rs", &root),
        "a leading cd must still resolve the target, not read as a work segment"
    );
}

#[test]
fn an_exit_status_controller_ahead_of_the_search_is_inert() {
    let (_tmp, root) = project();
    // Nothing is discarded here: `true` and `:` carry no work of their own, so
    // the search is effectively the whole command — the #451 shape.
    for command in [
        "true; grep -n MySymbol src/lib.rs",
        ": && grep -n MySymbol src/lib.rs",
        "echo searching && true && grep -n MySymbol src/lib.rs",
    ] {
        assert!(
            is_blocked(command, &root),
            "a leading exit-status controller is not work, so the search is still redirected: {command}"
        );
    }
}

#[test]
fn error_suppression_after_the_search_still_passes_through() {
    let (_tmp, root) = project();
    // The same two words after the search are the error-suppression idiom
    // `hook_475_test.rs` pins: they consume the search's exit status, and
    // #475/#476 guarantee that chain is left alone.
    for command in [
        "grep -n MySymbol src/lib.rs || true",
        "grep -n MySymbol src/lib.rs && true",
        "grep -n MySymbol src/lib.rs ; true",
        "grep -n MySymbol src/lib.rs || :",
    ] {
        assert!(
            !is_blocked(command, &root),
            "suppressing a failing search must not become a denial: {command}"
        );
    }
}

#[test]
fn a_second_search_segment_is_real_work_too() {
    let (_tmp, root) = project();
    // The sibling grep is not symbol-shaped, so tokensave cannot replace it.
    // Denying on the other segment's account would discard it.
    for command in [
        r#"grep -rn "some prose pattern" src/ && grep -n MySymbol src/lib.rs"#,
        r#"grep -n MySymbol src/lib.rs && grep -rn "some prose pattern" src/"#,
    ] {
        assert!(
            !is_blocked(command, &root),
            "a search the hook cannot replace is work: {command}"
        );
    }
}

#[test]
fn several_symbol_searches_in_one_call_are_the_headline_case() {
    let (_tmp, root) = project();
    // The shape #451 is actually about: label, search, label, search. Both
    // searches are symbol-shaped, so tokensave replaces both and a denial
    // discards nothing. Treating the second search as work to be preserved
    // would allow this command straight through and un-fix the bug.
    for command in [
        r#"echo "=== A ===" && grep -n First src/lib.rs && echo "=== B ===" && grep -n Second src/one.rs"#,
        "grep -n First src/lib.rs && grep -n Second src/one.rs",
    ] {
        assert!(
            is_blocked(command, &root),
            "a batch of redirectable searches is #451 itself: {command}"
        );
    }
}
