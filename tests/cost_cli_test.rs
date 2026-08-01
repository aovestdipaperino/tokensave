use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn tokensave_command(home: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("TOKENSAVE_SKIP_UPDATE_CHECK", "1");
    command
}

fn isolated_home_with_claude_session() -> TempDir {
    let home = TempDir::new().expect("temp home");
    let tokensave_dir = home.path().join(".tokensave");
    fs::create_dir_all(&tokensave_dir).expect("tokensave dir");
    fs::write(
        tokensave_dir.join("config.toml"),
        "last_pricing_fetch_at = 4102444800\n",
    )
    .expect("config");

    let session_dir = home.path().join(".claude/projects/project");
    fs::create_dir_all(&session_dir).expect("claude session dir");
    fs::write(
        session_dir.join("session.jsonl"),
        "{\"type\":\"assistant\",\"message\":{\"id\":\"msg-1\",\"model\":\"claude-opus-4-6\",\"role\":\"assistant\",\"usage\":{\"input_tokens\":1000,\"output_tokens\":200,\"cache_creation_input_tokens\":500,\"cache_read_input_tokens\":800},\"content\":[]},\"timestamp\":\"2026-07-31T12:00:00.000Z\"}\n",
    )
    .expect("claude session");
    home
}

#[test]
fn cost_without_droid_preserves_existing_output() {
    let home = isolated_home_with_claude_session();
    let output = tokensave_command(&home)
        .args(["cost", "all"])
        .output()
        .expect("run tokensave cost");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("1.0k"), "{stdout}");
    assert!(!stdout.contains("Coverage:"), "{stdout}");
    assert!(!stdout.contains("Droid"), "{stdout}");
    assert!(!stdout.contains("Augment"), "{stdout}");
    assert!(!stdout.contains("Copilot"), "{stdout}");

    let json = tokensave_command(&home)
        .args(["cost", "all", "--export", "json"])
        .output()
        .expect("run tokensave cost json");
    assert!(
        json.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&json.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).expect("valid json output");
    let object = value.as_object().expect("json object");
    assert_eq!(value["total_input_tokens"], 1000);
    assert!(!object.contains_key("by_agent"), "{value}");
    assert!(!object.contains_key("coverage"), "{value}");
}

#[test]
fn removed_droid_source_does_not_surface_stale_rows() {
    let home = isolated_home_with_claude_session();
    let droid_dir = home.path().join(".factory/sessions/project");
    fs::create_dir_all(&droid_dir).expect("droid session dir");
    fs::write(
        droid_dir.join("session.settings.json"),
        r#"{
            "providerLockTimestamp": "2026-07-31T12:00:00.000Z",
            "model": "claude-opus-4",
            "tokenUsage": {
                "inputTokens": 2000,
                "outputTokens": 400,
                "factoryCredits": 1500
            }
        }"#,
    )
    .expect("droid session");

    let first = tokensave_command(&home)
        .args(["cost", "all", "--by-agent"])
        .output()
        .expect("ingest Droid source");
    assert!(first.status.success());
    assert!(
        String::from_utf8_lossy(&first.stdout).contains("droid"),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );

    fs::remove_dir_all(home.path().join(".factory")).expect("remove Droid source");

    let second = tokensave_command(&home)
        .args(["cost", "all", "--by-agent"])
        .output()
        .expect("query after Droid removal");
    assert!(second.status.success());
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(!stdout.contains("droid"), "{stdout}");
    assert!(!stdout.contains("Coverage:"), "{stdout}");
}
