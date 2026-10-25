//! Exercise global settings and SQL diagnostics through the actual CLI.
#![cfg(not(windows))]

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

fn command(home: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .stdin(Stdio::null());
    command
}

fn run(home: &TempDir, args: &[&str]) -> Output {
    let out = command(home).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn records(stderr: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["event"] == "tokensave.sql")
        .collect()
}

#[test]
fn global_debug_flag_controls_cli_and_mcp_without_polluting_stdout() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let root = project.path().to_str().unwrap();
    std::fs::write(project.path().join("lib.rs"), "pub fn debug_example() {}\n").unwrap();
    let settings = run(&home, &["config", "debug-sql"]);
    assert!(String::from_utf8_lossy(&settings.stdout).contains("debug_sql = false"));
    // Avoid a hardware benchmark in this isolated test home.
    std::fs::write(
        home.path().join(".tokensave").join("config.toml"),
        "upload_enabled = false\n[benchmark]\nenabled = false\n",
    )
    .unwrap();
    run(&home, &["init", root, "--no-git-hook"]);
    let args = [
        "tool",
        "search",
        "--project",
        root,
        "--query",
        "debug_example",
        "--format",
        "json",
    ];
    let baseline = run(&home, &args);
    assert!(records(&baseline.stderr).is_empty());
    run(
        &home,
        &[
            "config",
            "debug-sql",
            "--enabled",
            "true",
            "--explain",
            "true",
        ],
    );
    let debug = run(&home, &args);
    let baseline: Value = serde_json::from_slice(&baseline.stdout).unwrap();
    let actual: Value = serde_json::from_slice(&debug.stdout).unwrap();
    assert_eq!(baseline, actual);
    let logged = records(&debug.stderr);
    assert!(logged.iter().any(|r| r["database"] == "project"));
    assert!(logged.iter().any(|r| r["plan"]["rows"].is_array()));

    // Finite stdin: EOF owns shutdown. File-backed stderr avoids pipe pressure
    // while the child writes SQL diagnostics. wait_with_output reaps the child.
    let stderr_path = home.path().join("mcp-stderr.log");
    let stderr = std::fs::File::create(&stderr_path).unwrap();
    let mut child = command(&home)
        .args(["serve", "--path", root])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
    writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tokensave_search","arguments":{"query":"debug_example"}}})).unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout must contain JSON-RPC only"))
        .collect();
    assert!(
        responses
            .iter()
            .any(|r| r["id"] == 2 && r["result"].is_object()),
        "{responses:?}"
    );
    assert!(!records(&std::fs::read(stderr_path).unwrap()).is_empty());
    run(&home, &["config", "debug-sql", "--enabled", "false"]);
    let disabled = run(&home, &args);
    assert!(records(&disabled.stderr).is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&disabled.stdout).unwrap(),
        baseline
    );
}
