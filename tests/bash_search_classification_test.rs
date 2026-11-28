//! Offline Bash search classification used by `tokensave discover` (#1118).
//!
//! The verdict depends on the command text only, so a transcript replayed on
//! another machine classifies the same way.

use tokensave::hooks::{classify_bash_search, BashSearchExclusion as Why, BashSearchVerdict};

fn verdict(command: &str) -> BashSearchVerdict {
    classify_bash_search(command)
}

#[test]
fn stdin_and_command_output_are_not_repository_searches() {
    for command in [
        "git diff | grep handle_request",
        "build-command 2>&1 | grep handle_request",
        "grep handle_request < src/lib.rs",
        "grep handle_request",
        "grep handle_request <<< 'handle_request'",
        "grep handle_request - src/lib.rs",
        "grep handle_request -- src/lib.rs -",
        "rg handle_request - src/lib.rs",
    ] {
        assert!(
            matches!(
                verdict(command),
                BashSearchVerdict::NotSearch | BashSearchVerdict::Excluded(_)
            ),
            "{command}"
        );
    }
    assert!(matches!(
        verdict("grep handle_request src/lib.rs"),
        BashSearchVerdict::Replaceable
    ));
    assert_eq!(
        verdict("grep -nr handle_request"),
        BashSearchVerdict::Replaceable
    );
}

#[test]
fn declaration_prefixed_searches_of_a_js_or_ts_file_are_replaceable() {
    for command in [
        r#"grep -n "const EXAMPLE_VALUE" src/example.js"#,
        r#"grep -n "let handler" web/app.ts"#,
        r#"rg -n "var legacyFlag" lib/old.mjs"#,
    ] {
        assert_eq!(
            verdict(command),
            BashSearchVerdict::Replaceable,
            "{command}"
        );
    }
}

#[test]
fn declaration_prefix_outside_a_js_or_ts_file_is_excluded() {
    for command in [
        r#"grep -rn "const EXAMPLE_VALUE" src/"#,
        r#"grep -n "const MAX_LEN" src/lib.rs"#,
    ] {
        assert_eq!(
            verdict(command),
            BashSearchVerdict::Excluded(Why::DeclarationOutsideJsTs),
            "{command}"
        );
    }
}

#[test]
fn symbol_searches_of_code_are_replaceable() {
    for command in [
        r#"grep -rn "fn parse_config" src/"#,
        "grep -n parse_config src/lib.rs",
        r#"rg -n "\bparse_config\b" src"#,
        "git grep -n parse_config src",
        "cd src && grep -n parse_config lib.rs",
        "grep -rn parse_config src | head -20",
        "rg parse_config",
        "sudo grep -n parse_config src/lib.rs 2>/dev/null",
    ] {
        assert_eq!(
            verdict(command),
            BashSearchVerdict::Replaceable,
            "{command}"
        );
    }
}

#[test]
fn ineligible_searches_carry_their_reason() {
    let cases = [
        (
            "TOKENSAVE_DISABLE_GREP_HOOK=1 grep -n parse_config src/lib.rs",
            Why::ExplicitOptOut,
        ),
        (
            "grep -n parse_config src/lib.rs && cargo build",
            Why::CompoundCommand,
        ),
        (
            "grep -rn parse_config src | xargs sed -i s/a/b/",
            Why::CompoundCommand,
        ),
        (
            "grep -rn parse_config $(git ls-files)",
            Why::UnsupportedShellSyntax,
        ),
        ("git grep -n parse_config main", Why::RevisionSearch),
        (r#"grep -rn "foo.*bar" src/"#, Why::NonSymbolPattern),
        (
            r#"grep -rn "parse_config\|load_config" src/"#,
            Why::NonSymbolPattern,
        ),
        (r#"grep -n "TODO" docs/notes.md"#, Why::NonCodeTarget),
        (
            "grep -rn parse_config --include='*.md' .",
            Why::NonCodeTarget,
        ),
        ("grep -n error build.log", Why::NonCodeTarget),
    ];
    for (command, why) in cases {
        assert_eq!(
            verdict(command),
            BashSearchVerdict::Excluded(why),
            "{command}"
        );
    }
}

#[test]
fn commands_that_are_not_searches_are_not_classified() {
    for command in [
        "cargo test --workspace",
        "cargo test 2>&1 | grep FAILED",
        "ls -la src",
        "echo grep -n parse_config src/lib.rs",
    ] {
        assert_eq!(verdict(command), BashSearchVerdict::NotSearch, "{command}");
    }
}

#[test]
fn exclusion_codes_round_trip() {
    for why in Why::ALL {
        assert_eq!(Why::from_code(why.as_str()), Some(why));
        assert!(!why.description().is_empty());
    }
    assert_eq!(Why::from_code("not_a_code"), None);
}
