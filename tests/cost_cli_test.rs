use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn tokensave_command(home: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .env("HOME", home.path())
        .env("TOKENSAVE_SKIP_UPDATE_CHECK", "1");
    command
}

fn isolated_home() -> TempDir {
    let home = TempDir::new().expect("temp home");
    let tokensave_dir = home.path().join(".tokensave");
    fs::create_dir_all(&tokensave_dir).expect("tokensave dir");
    fs::write(
        tokensave_dir.join("config.toml"),
        "last_pricing_fetch_at = 4102444800\n",
    )
    .expect("config");
    home
}

fn isolated_home_with_claude_session() -> TempDir {
    let home = isolated_home();
    let session_dir = home.path().join(".claude").join("projects").join("project");
    fs::create_dir_all(&session_dir).expect("claude session dir");
    fs::write(
        session_dir.join("session.jsonl"),
        "{\"type\":\"assistant\",\"message\":{\"id\":\"msg-1\",\"model\":\"claude-opus-4-6\",\"role\":\"assistant\",\"usage\":{\"input_tokens\":1000,\"output_tokens\":200,\"cache_creation_input_tokens\":500,\"cache_read_input_tokens\":800},\"content\":[]},\"timestamp\":\"2026-07-31T12:00:00.000Z\"}\n",
    )
    .expect("claude session");
    home
}

fn write_copilot_session(home: &TempDir) {
    let session_dir = home
        .path()
        .join(".copilot")
        .join("session-state")
        .join("copilot-session");
    fs::create_dir_all(&session_dir).expect("Copilot session dir");
    fs::write(
        session_dir.join("events.jsonl"),
        r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":900000000000,"totalPremiumRequests":0,"modelCacheState":[{"modelId":"gpt-5.4","cacheTtlSeconds":1800}]},"id":"checkpoint-1","timestamp":"2026-07-31T12:00:00.000Z"}
"#,
    )
    .expect("Copilot checkpoint");
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
    assert!(
        stdout.contains(
            "Coverage: Claude complete; Droid absent; Copilot absent; Codex absent; Augment not imported"
        ),
        "{stdout}"
    );

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
fn cost_json_includes_copilot_by_agent_and_coverage_without_droid() {
    let home = isolated_home();
    write_copilot_session(&home);

    let output = tokensave_command(&home)
        .args(["cost", "all", "--by-agent", "--export", "json"])
        .output()
        .expect("run tokensave cost json");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    let agents = value["by_agent"].as_array().expect("by_agent array");
    let copilot = agents
        .iter()
        .find(|agent| agent["agent"] == "copilot")
        .expect("Copilot row");
    assert!(copilot["cost_usd"].is_null());
    assert_eq!(copilot["credits"], 900_000_000_000u64);
    assert!(
        value["coverage"]
            .as_array()
            .expect("coverage array")
            .iter()
            .any(|coverage| coverage["agent"] == "copilot"),
        "coverage must include Copilot: {value}"
    );
}

#[test]
fn copilot_table_and_csv_render_native_aiu_without_usd() {
    let home = isolated_home_with_claude_session();
    write_copilot_session(&home);

    let table = tokensave_command(&home)
        .args(["cost", "all", "--by-agent"])
        .output()
        .expect("run tokensave cost table");
    assert!(table.status.success());
    let table = String::from_utf8_lossy(&table.stdout);
    let copilot_row = table.lines().find(|line| line.contains("copilot")).unwrap();
    assert!(copilot_row.contains("n/a"), "{copilot_row}");
    assert!(copilot_row.contains("900 AIU"), "{copilot_row}");

    let csv = tokensave_command(&home)
        .args(["cost", "all", "--by-agent", "--export", "csv"])
        .output()
        .expect("run tokensave cost CSV");
    assert!(csv.status.success());
    let csv = String::from_utf8_lossy(&csv.stdout);
    let copilot_row = csv
        .lines()
        .find(|line| line.starts_with("copilot,"))
        .unwrap();
    assert!(
        copilot_row.starts_with("copilot,,900000000000,"),
        "{copilot_row}"
    );
}

#[test]
fn removed_droid_source_does_not_surface_stale_rows() {
    let home = isolated_home_with_claude_session();
    let droid_dir = home
        .path()
        .join(".factory")
        .join("sessions")
        .join("project");
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
    assert!(
        stdout.contains(
            "Coverage: Claude complete; Droid absent; Copilot absent; Codex absent; Augment not imported"
        ),
        "{stdout}"
    );
}

#[test]
fn removed_copilot_source_does_not_surface_stale_rows() {
    let home = isolated_home_with_claude_session();
    write_copilot_session(&home);

    let first = tokensave_command(&home)
        .args(["cost", "all", "--by-agent"])
        .output()
        .expect("ingest Copilot source");
    assert!(first.status.success());
    assert!(
        String::from_utf8_lossy(&first.stdout).contains("copilot"),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );

    fs::remove_dir_all(home.path().join(".copilot")).expect("remove Copilot source");

    let second = tokensave_command(&home)
        .args(["cost", "all", "--by-agent"])
        .output()
        .expect("query after Copilot removal");
    assert!(second.status.success());
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(!stdout.contains("copilot"), "{stdout}");
    assert!(
        stdout.contains("Copilot absent"),
        "coverage should disclose absent Copilot source: {stdout}"
    );
}
