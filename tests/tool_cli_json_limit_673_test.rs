//! Exercise help routing (#1120) and uncapped JSON output (#673) through the
//! real `tokensave tool` command.

use std::process::{Command, Stdio};

use tempfile::TempDir;

fn tokensave(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run tokensave");
    assert!(
        output.status.success(),
        "tokensave {args:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn cli_tool_help_forms() {
    let subcommand_help = tokensave(&["help", "tool"]);
    assert_eq!(tokensave(&["tool", "--help"]), subcommand_help);
    let short_help = tokensave(&["tool", "-h"]);
    for help in [&subcommand_help, &short_help] {
        // Clap uses the executable's file name, which can include .exe.
        assert!(
            help.lines().any(|line| {
                line.strip_prefix("Usage: ")
                    .and_then(|usage| usage.split_whitespace().nth(1))
                    == Some("tool")
            }),
            "expected tool subcommand usage in:\n{help}"
        );
    }

    for flag in ["--help", "-h"] {
        let tool_help = tokensave(&["tool", "search", flag]);
        assert!(tool_help.starts_with("tokensave tool search"));
        assert!(tool_help.contains("--query"));
    }
}

#[test]
fn cli_search_json_is_not_capped_and_parses() {
    let dir = TempDir::new().unwrap();
    let source: String = (0..300)
        .map(|i| {
            format!(
                "/// Handler number {i}.\npub fn shared_handler_name_{i:04}(first_argument: u32, second_argument: String) -> u32 {{ first_argument }}\n\n"
            )
        })
        .collect();
    std::fs::write(dir.path().join("lib.rs"), source).unwrap();
    let root = dir.path().to_str().unwrap();
    tokensave(&["init", root]);

    let args = [
        "--query",
        "shared_handler_name",
        "--project",
        root,
        "--limit",
        "300",
        "--format",
        "json",
    ];
    let out = tokensave(&[&["tool", "search"], args.as_slice()].concat());
    let after_separator = tokensave(&[&["tool", "search", "--"], args.as_slice()].concat());
    assert_eq!(out, after_separator);

    assert!(
        out.len() > 15_000,
        "fixture must exceed the MCP cap to test anything: {} bytes",
        out.len()
    );
    assert!(!out.contains("[... truncated at"), "CLI output was cut");
    let v: serde_json::Value = serde_json::from_str(&out).expect("CLI JSON must parse");
    let items = v
        .as_array()
        .expect("an uncut search result is a bare array");
    assert_eq!(items.len(), 300);
}
