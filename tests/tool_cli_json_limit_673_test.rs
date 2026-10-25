//! `tokensave tool` prints whole results, so `format: "json"` output piped to
//! a script always parses (#673). The MCP server's 15,000-character cap is
//! for an agent's context window; on the CLI it used to cut JSON mid-object.
//!
//! Driven through the real binary because the cap is lifted in the CLI path.

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

    let out = tokensave(&[
        "tool",
        "search",
        "--project",
        root,
        "--query",
        "shared_handler_name",
        "--limit",
        "300",
        "--format",
        "json",
    ]);

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
