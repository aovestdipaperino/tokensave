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

fn run_local(home: &std::path::Path, cwd: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env_remove("TOKENSAVE_PROFILE")
        .env("TOKENSAVE_PROFILE_SHARE", "0")
        .output()
        .unwrap()
}

fn repository(parent: &std::path::Path, name: &str) -> std::path::PathBuf {
    let root = parent.join(name);
    std::fs::create_dir(&root).unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .arg(&root)
        .status()
        .unwrap()
        .success());
    root
}

fn successful(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn local_controls_are_hidden_and_do_not_change_global_or_sibling_preferences() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repository(tmp.path(), "repo");
    let other = repository(tmp.path(), "other");
    let nested = repo.join("src");
    std::fs::create_dir(&nested).unwrap();
    successful(run_local(tmp.path(), &repo, &["profile", "enable"]));
    let global_path = tmp.path().join(".tokensave").join("config.toml");
    let global_before = std::fs::read_to_string(&global_path).unwrap();
    let global: toml::Value = toml::from_str(&global_before).unwrap();
    assert_eq!(global["profiling"]["mode"].as_str(), Some("light"));
    // Unconfigured --local status is independently off, without creating a file.
    let status = successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "status", "--local"],
    ));
    assert!(status.contains("Profiling: off"), "{status}");
    let local_path = repo.join(".tokensave").join("profile.toml");
    assert!(!local_path.exists());
    successful(run_local(
        tmp.path(),
        &nested,
        &["profile", "enable", "--local", "--detailed", "--for", "2h"],
    ));
    let local: toml::Value =
        toml::from_str(&std::fs::read_to_string(&local_path).unwrap()).unwrap();
    assert_eq!(local["profiling"]["mode"].as_str(), Some("detailed"));
    assert_eq!(local["profiling_share_enabled"].as_bool(), Some(false));
    assert_eq!(
        std::fs::read_to_string(&global_path).unwrap(),
        global_before
    );
    assert!(!other.join(".tokensave").join("profile.toml").exists());
    for args in [
        vec!["profile", "--help"],
        vec!["profile", "enable", "--help"],
        vec!["profile", "share", "enable", "--help"],
    ] {
        let help = successful(run_local(tmp.path(), &repo, &args));
        assert!(!help.contains("--local"), "{help}");
    }
    successful(run_local(
        tmp.path(),
        &nested,
        &["profile", "share", "disable", "--local"],
    ));
    let status = successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "--local", "share", "status"],
    ));
    assert!(status.contains("consent: disabled"));
    let failed = run_local(
        tmp.path(),
        &repo,
        &["profile", "share", "enable", "--local"],
    );
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("endpoint and data policy"));
    successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "disable", "--local"],
    ));
    let local: toml::Value =
        toml::from_str(&std::fs::read_to_string(&local_path).unwrap()).unwrap();
    assert_eq!(local["profiling"]["mode"].as_str(), Some("off"));
    assert!(local["profiling"].get("detailed_expires_at").is_none());
    assert_eq!(std::fs::read_to_string(global_path).unwrap(), global_before);
}

#[test]
fn local_reports_preview_and_reset_never_use_global_aggregates() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repository(tmp.path(), "repo");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let local_dir = repo.join(".tokensave").join("profile");
    let global_dir = tmp.path().join(".tokensave").join("profile");
    for (dir, count) in [(&local_dir, 3), (&global_dir, 91)] {
        let mut snapshot = tokensave::profile::Snapshot::new();
        snapshot.insert(
            "db.get_node_by_id".into(),
            tokensave::profile::SeriesSnapshot {
                count,
                sum_us: count * 10,
                max_us: 10,
                ..Default::default()
            },
        );
        tokensave::profile::storage::Store::new(dir.clone(), 14, 32 * 1024 * 1024)
            .flush(&snapshot, now)
            .unwrap();
    }
    let preview = successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "share", "preview", "--local"],
    ));
    let export = successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "export", "--local"],
    ));
    assert_eq!(preview, export);
    let json: serde_json::Value = serde_json::from_str(&export).unwrap();
    assert_eq!(json["engines"]["libsql"]["db.get_node_by_id"]["count"], 3);
    successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "reset", "--local"],
    ));
    assert!(tokensave::profile::storage::load(&local_dir, 0).is_empty());
    assert!(!tokensave::profile::storage::load(&global_dir, 0).is_empty());
}

#[test]
fn explicit_project_path_uses_local_profiling_even_from_another_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repository(tmp.path(), "repo");
    let other = repository(tmp.path(), "other");
    std::fs::write(repo.join("lib.rs"), "pub fn example() -> u32 { 42 }\n").unwrap();
    successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "enable", "--local"],
    ));
    successful(run_local(
        tmp.path(),
        &other,
        &["init", repo.to_str().unwrap(), "--no-git-hook"],
    ));
    let data = tokensave::profile::storage::load(&repo.join(".tokensave").join("profile"), 0);
    assert!(!data.is_empty());
    assert!(!tmp.path().join(".tokensave").join("profile").exists());
    assert!(!other.join(".tokensave").join("profile").exists());
    // The environment still wins over the selected repository's preferences.
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(&other)
        .args(["sync", repo.to_str().unwrap()])
        .env("HOME", tmp.path())
        .env("TOKENSAVE_PROFILE", "off")
        .output()
        .unwrap();
    successful(output);
    let after = tokensave::profile::storage::load(&repo.join(".tokensave").join("profile"), 0);
    assert_eq!(
        serde_json::to_value(after).unwrap(),
        serde_json::to_value(data).unwrap()
    );
}

#[test]
fn local_mcp_omits_selected_graph_calls_and_keeps_default_project_calls() {
    use std::io::Write;
    use std::process::Stdio;
    let tmp = tempfile::tempdir().unwrap();
    let repo = repository(tmp.path(), "repo");
    let other = repository(tmp.path(), "other");
    for root in [&repo, &other] {
        std::fs::write(root.join("lib.rs"), "pub fn example() {}\n").unwrap();
        successful(run_local(
            tmp.path(),
            root,
            &["init", root.to_str().unwrap(), "--no-git-hook"],
        ));
    }
    successful(run_local(
        tmp.path(),
        &repo,
        &["profile", "enable", "--local"],
    ));
    let serve = |arguments: serde_json::Value| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .current_dir(&other)
            .args(["serve", "--path", repo.to_str().unwrap()])
            .env("HOME", tmp.path())
            .env_remove("TOKENSAVE_PROFILE")
            .env("TOKENSAVE_PROFILE_SHARE", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let messages = [
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tokensave_search","arguments":arguments}}),
        ];
        let mut stdin = child.stdin.take().unwrap();
        for message in messages {
            writeln!(stdin, "{message}").unwrap();
        }
        drop(stdin);
        let output = successful(child.wait_with_output().unwrap());
        let response: serde_json::Value = output
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|v| v["id"] == 2)
            .unwrap();
        assert!(response.get("error").is_none(), "{response}");
        assert_ne!(response["result"]["isError"], true, "{response}");
    };
    let dir = repo.join(".tokensave").join("profile");
    serve(serde_json::json!({"query":"example", "graph_root":other}));
    // Startup may record work on the default project, but the selected
    // repository's tool call must never enter this collector.
    let selected = tokensave::profile::storage::load(&dir, 0);
    assert!(selected
        .values()
        .all(|snapshot| !snapshot.contains_key("tool.tokensave_search")));
    serve(serde_json::json!({"query":"example"}));
    let data = tokensave::profile::storage::load(&dir, 0);
    assert_eq!(data["libsql"]["tool.tokensave_search"].count, 1);
    assert!(!other.join(".tokensave").join("profile").exists());
    assert!(!tmp.path().join(".tokensave").join("profile").exists());
}
