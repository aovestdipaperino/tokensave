#![cfg(not(windows))]

use std::fs;
use std::process::Command;

#[test]
fn global_install_warns_when_binary_is_under_cargo_target() {
    let home = tempfile::tempdir().expect("temp home");
    let scratch = tempfile::tempdir().expect("temp build root");
    let binary = scratch.path().join("target/debug/tokensave");
    fs::create_dir_all(binary.parent().expect("binary parent")).expect("create target/debug");
    fs::copy(env!("CARGO_BIN_EXE_tokensave"), &binary).expect("copy tokensave binary");

    let output = Command::new(&binary)
        .args(["install", "--agent", "claude", "--git-hook", "no"])
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("CLAUDE_CONFIG_DIR")
        .output()
        .expect("run tokensave install");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("agent config references Cargo build output"));
    assert!(stderr.contains("cargo clean"));

    let settings =
        fs::read_to_string(home.path().join(".claude/settings.json")).expect("Claude settings");
    assert!(settings.contains("/target/debug/tokensave"));
}
