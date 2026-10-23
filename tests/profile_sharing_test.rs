//! CLI sharing checks use a child-process home; the parent environment is unchanged.
#![cfg(unix)]

use std::process::{Command, Output};

fn run(home: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .env("HOME", home)
        .env("TOKENSAVE_PROFILE", "off")
        .env("TOKENSAVE_PROFILE_SHARE", "0")
        .output()
        .unwrap()
}

fn assert_no_consent(home: &std::path::Path) {
    // CLI startup may create config.toml for unrelated version bookkeeping.
    let path = home.join(".tokensave").join("config.toml");
    if let Ok(text) = std::fs::read_to_string(path) {
        let config: toml::Value = toml::from_str(&text).unwrap();
        for field in ["profiling_share_enabled", "profiling_share_asked"] {
            assert_ne!(config.get(field).and_then(toml::Value::as_bool), Some(true));
        }
    }
    for name in ["profile-sharing.json", "profile-sharing.lock"] {
        assert!(!home.join(".tokensave").join(name).exists());
    }
}

#[test]
fn preview_matches_export_byte_for_byte_without_enabling_consent() {
    let tmp = tempfile::tempdir().unwrap();
    let mut snapshot = tokensave::profile::Snapshot::new();
    snapshot.insert(
        "db.get_node_by_id".to_string(),
        tokensave::profile::SeriesSnapshot {
            count: 3,
            sum_us: 30,
            max_us: 10,
            ..Default::default()
        },
    );
    let dir = tmp.path().join(".tokensave").join("profile");
    let mut store = tokensave::profile::storage::Store::new(dir, 14, 32 * 1024 * 1024);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    store.flush(&snapshot, now).unwrap();
    let preview = run(tmp.path(), &["profile", "share", "preview"]);
    let export = run(tmp.path(), &["profile", "export"]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(export.status.success());
    assert_eq!(preview.stdout, export.stdout);
    let json: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(json["engines"]["libsql"]["db.get_node_by_id"]["count"], 3);
    assert_no_consent(tmp.path());
}

#[test]
fn unset_destination_refuses_enable_and_status_reports_the_block() {
    let tmp = tempfile::tempdir().unwrap();
    let enable = run(tmp.path(), &["profile", "share", "enable"]);
    assert!(!enable.status.success());
    assert!(String::from_utf8_lossy(&enable.stderr).contains("endpoint and data policy"));
    let status = run(tmp.path(), &["profile", "share", "status"]);
    assert!(status.status.success());
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("consent: disabled"), "{text}");
    assert!(text.contains("Uploads unavailable"), "{text}");
    assert!(text.contains("Next upload: inactive"), "{text}");
    assert_no_consent(tmp.path());
}

#[test]
fn disable_revokes_consent_without_enabling_profiling_or_counter_uploads() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".tokensave");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "upload_enabled = false\nprofiling_share_enabled = true\n",
    )
    .unwrap();
    let output = run(tmp.path(), &["profile", "share", "disable"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(&config).unwrap();
    assert_ne!(
        parsed
            .get("profiling_share_enabled")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(parsed["upload_enabled"].as_bool(), Some(false));
    assert!(parsed.get("profiling").is_none());
}

#[test]
fn enabling_profiling_with_redirected_input_does_not_grant_sharing_or_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let output = run(tmp.path(), &["profile", "enable"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("[y/N]"));
    let text = std::fs::read_to_string(tmp.path().join(".tokensave").join("config.toml")).unwrap();
    let config: toml::Value = toml::from_str(&text).unwrap();
    assert_eq!(config["profiling"]["mode"].as_str(), Some("light"));
    assert_ne!(
        config
            .get("profiling_share_enabled")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_ne!(
        config
            .get("profiling_share_asked")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
}
