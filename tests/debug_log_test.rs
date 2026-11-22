use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};

fn run(project: &Path, home: &Path, args: &[&str], level: Option<&str>, input: &[u8]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .current_dir(project)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("TOKENSAVE_AUTO_INIT", "0")
        .env("TOKENSAVE_UPDATE_CHECK", "0")
        .env_remove("TOKENSAVE_STATE_DIR")
        .env_remove("TOKENSAVE_DISABLE_SERVER")
        .env_remove("DISABLE_TOKENSAVE")
        .env_remove("TOKENSAVE_DISABLE_GREP_HOOK")
        .env_remove("TOKENSAVE_LOG")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(level) = level {
        command.env("TOKENSAVE_LOG", level);
    }
    let mut child = command.spawn().expect("start tokensave");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .expect("write request payload and close stdin");
    let result = child.wait_with_output().expect("collect tokensave output");
    assert!(
        result.status.success(),
        "tokensave failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn events(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["timestamp_unix_ms"].is_number())
        .collect()
}

#[test]
fn debug_log_activation_precedence_and_cli_output() {
    let dir = tempfile::tempdir().expect("isolated home and directory");
    let path = dir.path();
    let quiet = run(path, path, &["tool"], None, b"");
    assert!(events(&quiet).is_empty());

    let debug = run(path, path, &["--debug", "tool"], Some("off"), b"");
    assert_eq!(quiet.stdout, debug.stdout);
    assert!(events(&debug).iter().any(|e| {
        e["operation"] == "log_level" && e["fields"] == json!({"source":"flag","level":"debug"})
    }));

    let warning = run(
        path,
        path,
        &["--debug", "--log-level", "warn", "tool"],
        None,
        b"",
    );
    assert!(events(&warning).is_empty());

    let invalid = run(path, path, &["tool"], Some("PRIVATE_ENV_VALUE"), b"");
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(stderr.contains("invalid TOKENSAVE_LOG"));
    assert!(!stderr.contains("PRIVATE_ENV_VALUE"));
}

#[test]
fn project_config_enables_logging_and_lower_precedence_sources_are_overridden() {
    let dir = tempfile::tempdir().expect("isolated home and directory");
    let path = dir.path();
    let state = path.join(".tokensave");
    std::fs::create_dir(&state).expect("create project state");
    std::fs::write(
        state.join("config.json"),
        r#"{"version":2,"root_dir":"","exclude":[],"max_file_size":1048576,"extract_docstrings":true,"track_call_sites":true,"log_level":"debug"}"#,
    )
    .expect("write project config");

    let configured = run(path, path, &["tool"], None, b"");
    assert!(events(&configured).iter().any(|event| {
        event["operation"] == "log_level"
            && event["fields"] == json!({"source":"project_file","level":"debug"})
    }));

    let environment = run(path, path, &["tool"], Some("info"), b"");
    assert!(events(&environment).iter().any(|event| {
        event["operation"] == "log_level"
            && event["fields"] == json!({"source":"environment","level":"info"})
    }));

    let flag = run(
        path,
        path,
        &["--log-level", "warn", "tool"],
        Some("debug"),
        b"",
    );
    assert!(events(&flag).is_empty());
}

#[test]
fn invalid_project_log_level_is_a_clear_configuration_error() {
    let dir = tempfile::tempdir().expect("isolated home and directory");
    let path = dir.path();
    let state = path.join(".tokensave");
    std::fs::create_dir(&state).expect("create project state");
    std::fs::write(state.join("config.json"), r#"{"log_level":"verbose"}"#)
        .expect("write project config");

    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    let output = command
        .current_dir(path)
        .env("HOME", path)
        .env("USERPROFILE", path)
        .env("TOKENSAVE_AUTO_INIT", "0")
        .env("TOKENSAVE_UPDATE_CHECK", "0")
        .env_remove("TOKENSAVE_LOG")
        .arg("tool")
        .output()
        .expect("run tokensave");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid log_level"), "{stderr}");
    assert!(
        stderr.contains("expected off, error, warn, info, debug or trace"),
        "{stderr}"
    );
}

#[test]
fn debug_hook_explains_denial_without_logging_shell_input() {
    let project = tempfile::tempdir().expect("isolated project");
    let home = tempfile::tempdir().expect("isolated home");
    std::fs::write(
        project.path().join("lib.rs"),
        "pub fn PrivateSymbolName() {}\n",
    )
    .expect("write indexable source");
    let init = run(
        project.path(),
        home.path(),
        &[
            "--debug",
            "init",
            ".",
            "--no-git-hook",
            "--cpu-usage",
            "low",
        ],
        None,
        b"",
    );
    let init_events = events(&init);
    assert!(init_events.iter().any(|e| e["operation"] == "full_scan"));
    assert!(init_events
        .iter()
        .any(|e| e["operation"] == "create_schema"));

    let input = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "PRIVATE_ENV=SecretArgument grep PrivateSymbolName lib.rs"}
    });
    let input = serde_json::to_vec(&input).expect("serialize hook input");
    let off = run(
        project.path(),
        home.path(),
        &["hook-pre-tool-use"],
        None,
        &input,
    );
    let debug = run(
        project.path(),
        home.path(),
        &["hook-pre-tool-use"],
        Some("debug"),
        &input,
    );
    assert_eq!(off.stdout, debug.stdout);
    let decision: Value = serde_json::from_slice(&debug.stdout).expect("hook protocol JSON");
    assert_eq!(decision["permission"], "deny");
    let debug_events = events(&debug);
    assert!(debug_events.iter().any(|e| {
        e["operation"] == "grep_check"
            && e["fields"]["tool"] == "grep"
            && e["fields"]["check"] == "code_target_eligible"
            && e["fields"]["passed"] == true
    }));
    assert!(debug_events
        .iter()
        .any(|e| e["fields"]["shape"] == "bare_symbol"));
    assert!(debug_events
        .iter()
        .any(|e| e["fields"]["outcome"] == "symbol_found"));
    assert!(debug_events
        .iter()
        .any(|e| { e["operation"] == "grep_decision" && e["fields"]["decision"] == "deny" }));
    let stderr = String::from_utf8_lossy(&debug.stderr);
    for secret in ["SecretArgument", "PrivateSymbolName", "lib.rs"] {
        assert!(!stderr.contains(secret));
    }

    let search = run(
        project.path(),
        home.path(),
        &[
            "--debug",
            "tool",
            "search",
            "--query",
            "PrivateSymbolName",
            "--limit",
            "3",
            "--json",
        ],
        None,
        b"",
    );
    assert!(events(&search).iter().any(|e| {
        e["subsystem"] == "search" && e["operation"] == "parameters" && e["fields"]["limit"] == 3
    }));
    assert!(!String::from_utf8_lossy(&search.stderr).contains("PrivateSymbolName"));
}

#[test]
fn debug_mcp_keeps_stdout_protocol_safe_and_request_fields_private() {
    let project = tempfile::tempdir().expect("isolated project");
    let home = tempfile::tempdir().expect("isolated home");
    let mut input = String::new();
    for id in 0..16 {
        input.push_str(&format!(
            "{}\n",
            json!({"jsonrpc":"2.0","id":id,"method":"ping"})
        ));
    }
    input.push_str(&format!(
        "{}\n",
        json!({
            "jsonrpc": "2.0", "id": "PRIVATE_REQUEST_ID", "method": "tools/call",
            "params": {"name": "PRIVATE_TOOL_NAME", "arguments": {"query": "PRIVATE_QUERY"}}
        })
    ));
    let output = run(
        project.path(),
        home.path(),
        &["serve", "--path", "."],
        Some("debug"),
        input.as_bytes(),
    );
    let replies: Vec<Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("only JSON-RPC on stdout"))
        .collect();
    assert_eq!(replies.len(), 17);
    assert!(replies.iter().all(|reply| reply["jsonrpc"] == "2.0"));
    assert!(replies
        .iter()
        .any(|reply| reply["id"] == "PRIVATE_REQUEST_ID"));
    let events = events(&output);
    let requests = events
        .iter()
        .filter(|e| e["operation"] == "request_finish")
        .count();
    assert_eq!(requests, 17);
    let finished = events
        .iter()
        .find(|e| e["operation"] == "request_finish" && e["fields"]["method"] == "tools/call")
        .expect("tool request completion");
    assert_eq!(finished["level"], "error");
    let id = &finished["request_id"];
    assert!(id.as_u64().is_some_and(|id| id > 0));
    assert!(events
        .iter()
        .any(|e| e["operation"] == "tool_start" && &e["request_id"] == id));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_"));
}
