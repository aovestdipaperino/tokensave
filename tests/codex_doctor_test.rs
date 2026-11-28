//! Codex MCP availability diagnostics (#587).
//!
//! Codex drops an MCP server that has not listed its tools within its
//! startup timeout, and a session then simply has no `tokensave_*` tools.
//! These tests pin what `install` writes to avoid that and what `doctor`
//! reports when it can still happen.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokensave::agents::{AgentIntegration, CodexIntegration, DoctorCounters, HealthcheckContext};

use crate::common::make_install_ctx_with_real_bin;

fn config_path(home: &Path) -> PathBuf {
    home.join(".codex").join("config.toml")
}

fn read_server(home: &Path) -> toml::Table {
    let text = std::fs::read_to_string(config_path(home)).unwrap();
    let config: toml::Table = text.parse().unwrap();
    config["mcp_servers"]["tokensave"]
        .as_table()
        .expect("tokensave server table")
        .clone()
}

fn edit_server(home: &Path, edit: impl FnOnce(&mut toml::Table)) {
    let text = std::fs::read_to_string(config_path(home)).unwrap();
    let mut config: toml::Table = text.parse().unwrap();
    let server = config
        .get_mut("mcp_servers")
        .and_then(|s| s.get_mut("tokensave"))
        .and_then(toml::Value::as_table_mut)
        .expect("tokensave server table");
    edit(server);
    std::fs::write(config_path(home), toml::to_string(&config).unwrap()).unwrap();
}

fn healthcheck(home: &Path, project: &Path) -> DoctorCounters {
    let mut dc = DoctorCounters::new();
    CodexIntegration.healthcheck(
        &mut dc,
        &HealthcheckContext {
            home: home.to_path_buf(),
            project_path: project.to_path_buf(),
        },
    );
    dc
}

#[test]
fn install_sets_a_startup_timeout_that_covers_a_first_index() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    CodexIntegration
        .install(&make_install_ctx_with_real_bin(home))
        .unwrap();
    assert_eq!(
        read_server(home)["startup_timeout_sec"].as_integer(),
        Some(60)
    );
}

#[test]
fn reinstall_keeps_a_timeout_the_user_chose() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_install_ctx_with_real_bin(home);
    CodexIntegration.install(&ctx).unwrap();
    edit_server(home, |server| {
        server.insert("startup_timeout_sec".into(), toml::Value::Integer(120));
    });
    CodexIntegration.install(&ctx).unwrap();
    assert_eq!(
        read_server(home)["startup_timeout_sec"].as_integer(),
        Some(120)
    );

    edit_server(home, |server| {
        server.remove("startup_timeout_sec");
        server.insert("startup_timeout_ms".into(), toml::Value::Integer(30_000));
    });
    CodexIntegration.install(&ctx).unwrap();
    let server = read_server(home);
    assert!(!server.contains_key("startup_timeout_sec"));
    assert_eq!(server["startup_timeout_ms"].as_integer(), Some(30_000));
}

#[test]
fn a_pre_timeout_install_gets_one_actionable_warning() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    CodexIntegration
        .install(&make_install_ctx_with_real_bin(home))
        .unwrap();
    let current = healthcheck(home, home);
    edit_server(home, |server| {
        server.remove("startup_timeout_sec");
    });
    let legacy = healthcheck(home, home);
    assert_eq!(legacy.issues, 0);
    assert_eq!(legacy.warnings, current.warnings + 1);
}

#[test]
fn a_disabled_server_is_an_issue() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    CodexIntegration
        .install(&make_install_ctx_with_real_bin(home))
        .unwrap();
    edit_server(home, |server| {
        server.insert("enabled".into(), toml::Value::Boolean(false));
    });
    assert_eq!(healthcheck(home, home).issues, 1);
}

#[test]
fn a_command_that_cannot_be_launched_is_an_issue() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    CodexIntegration
        .install(&make_install_ctx_with_real_bin(home))
        .unwrap();
    let missing = home.join("gone").join("tokensave");
    edit_server(home, |server| {
        server.insert(
            "command".into(),
            toml::Value::String(missing.to_string_lossy().into_owned()),
        );
    });
    assert_eq!(healthcheck(home, home).issues, 1);
}

#[test]
fn tool_filters_that_hide_tokensave_tools_warn() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    CodexIntegration
        .install(&make_install_ctx_with_real_bin(home))
        .unwrap();
    let baseline = healthcheck(home, home);
    edit_server(home, |server| {
        server.insert(
            "disabled_tools".into(),
            toml::Value::Array(vec![toml::Value::String("tokensave_search".into())]),
        );
    });
    let filtered = healthcheck(home, home);
    assert_eq!(filtered.issues, 0);
    assert_eq!(filtered.warnings, baseline.warnings + 1);
}

/// Live startup probe, against stand-in servers written as shell scripts.
#[cfg(unix)]
mod probe {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    use tokensave::agents::mcp_probe::{probe_stdio_server, ProbeError, ProbeStage};

    const INITIALIZE_RESPONSE: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"stand-in","version":"0"}}}"#;
    const TOOLS_RESPONSE: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"tokensave_search"},{"name":"tokensave_context"}]}}"#;

    /// A stand-in named `tokensave` that waits `delay` before answering, or
    /// exits at once when `delay` is `None`.
    fn stand_in(dir: &Path, delay: Option<&str>) -> PathBuf {
        let body = match delay {
            Some(delay) => format!(
                "#!/bin/sh\nread -r line\nsleep {delay}\nprintf '%s\\n' '{INITIALIZE_RESPONSE}'\n\
                 read -r line\nread -r line\nprintf '%s\\n' '{TOOLS_RESPONSE}'\ncat >/dev/null\n"
            ),
            None => "#!/bin/sh\nexit 3\n".to_string(),
        };
        let bin = dir.join("bin").join("tokensave");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, body).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    fn indexed_project(dir: &Path) -> PathBuf {
        let project = dir.join("project");
        let state = tokensave::config::get_tokensave_dir(&project);
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("tokensave.db"), b"").unwrap();
        project
    }

    fn serve_args() -> Vec<String> {
        vec!["serve".to_string()]
    }

    #[test]
    fn a_prompt_server_completes_the_handshake() {
        let dir = TempDir::new().unwrap();
        let bin = stand_in(dir.path(), Some("0"));
        let timings = probe_stdio_server(
            &bin,
            &serve_args(),
            dir.path(),
            &[],
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(timings.tool_count, 2);
        assert!(timings.initialize <= timings.total);
    }

    #[test]
    fn a_hung_server_times_out_within_the_deadline() {
        let dir = TempDir::new().unwrap();
        let bin = stand_in(dir.path(), Some("30"));
        let started = std::time::Instant::now();
        let err = probe_stdio_server(
            &bin,
            &serve_args(),
            dir.path(),
            &[],
            Duration::from_millis(300),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProbeError::TimedOut {
                stage: ProbeStage::Initialize,
                ..
            }
        ));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the probe must kill a hung server"
        );
    }

    #[test]
    fn a_server_that_exits_is_reported() {
        let dir = TempDir::new().unwrap();
        let bin = stand_in(dir.path(), None);
        let err = probe_stdio_server(
            &bin,
            &serve_args(),
            dir.path(),
            &[],
            Duration::from_secs(20),
        )
        .unwrap_err();
        assert!(matches!(err, ProbeError::Exited { .. }), "{err:?}");
    }

    fn install_stand_in(home: &Path, bin: &Path) {
        let mut ctx = make_install_ctx_with_real_bin(home);
        ctx.tokensave_bin = bin.to_string_lossy().into_owned();
        CodexIntegration.install(&ctx).unwrap();
    }

    #[test]
    fn doctor_passes_a_server_that_starts_within_the_timeout() {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let bin = stand_in(dir.path(), Some("0"));
        install_stand_in(&home, &bin);
        let project = indexed_project(dir.path());
        let with_probe = healthcheck(&home, &project);
        let without_probe = healthcheck(&home, &home);
        assert_eq!(with_probe.issues, 0);
        assert_eq!(with_probe.warnings, without_probe.warnings);
    }

    #[test]
    fn doctor_fails_a_server_slower_than_the_codex_timeout() {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let bin = stand_in(dir.path(), Some("2"));
        install_stand_in(&home, &bin);
        edit_server(&home, |server| {
            server.insert("startup_timeout_sec".into(), toml::Value::Integer(1));
        });
        let project = indexed_project(dir.path());
        assert_eq!(healthcheck(&home, &project).issues, 1);
    }

    #[test]
    fn doctor_fails_a_server_that_exits_at_startup() {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let bin = stand_in(dir.path(), None);
        install_stand_in(&home, &bin);
        let project = indexed_project(dir.path());
        assert_eq!(healthcheck(&home, &project).issues, 1);
    }
}

#[cfg(windows)]
#[tokio::test]
async fn codex_bash_events_advise_on_standalone_and_compound_powershell_searches() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let project = TempDir::new().expect("project");
    std::fs::write(project.path().join("lib.rs"), "pub fn handle_request() {}").expect("source");
    let _index = tokensave::tokensave::TokenSave::init(project.path())
        .await
        .expect("index");

    for command in [
        "Select-String 'handle_request' './lib.rs'",
        "Write-Output marker; Select-String 'handle_request' './lib.rs'",
    ] {
        let event = serde_json::json!({
            "session_id": "test-session",
            "cwd": project.path(),
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": "test-call",
            "tool_input": { "command": command }
        });
        let mut hook = Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .arg("hook-pre-tool-use-codex")
            .current_dir(project.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start hook");
        write!(hook.stdin.take().expect("hook stdin"), "{event}").expect("send event");
        let output = hook.wait_with_output().expect("read hook output");
        assert!(output.status.success());
        let output: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
        let decision = &output["hookSpecificOutput"];
        assert_eq!(decision["hookEventName"], "PreToolUse");
        assert!(decision.get("permissionDecision").is_none(), "{output}");
        assert!(decision.get("updatedInput").is_none(), "{output}");
        assert!(
            decision["additionalContext"]
                .as_str()
                .is_some_and(|context| context.contains("handle_request")),
            "{output}"
        );
    }
}
