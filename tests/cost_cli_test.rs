use std::fs;
use std::process::Command;

use tempfile::TempDir;

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
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["cost", "all"])
        .env("HOME", home.path())
        .env("TOKENSAVE_SKIP_UPDATE_CHECK", "1")
        .output()
        .expect("run tokensave cost");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(!stdout.contains("Coverage:"), "{stdout}");
    assert!(!stdout.contains("Droid"), "{stdout}");
    assert!(!stdout.contains("Augment"), "{stdout}");
    assert!(!stdout.contains("Copilot"), "{stdout}");
}
