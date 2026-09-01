#![cfg(not(windows))]

use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokensave::agents::{
    available_integrations, expected_tool_perms, get_integration, migrate_installed_agents,
    rules_for_agent, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    InstallScope, OmpIntegration,
};
use tokensave::errors::TokenSaveError;
use tokensave::user_config::UserConfig;

static ENV_LOCK: Mutex<()> = Mutex::new(());

const OMP_RULES_MARKER: &str = "<!-- tokensave: managed omp rules -->";

struct PathGuard {
    previous: Option<OsString>,
}

impl PathGuard {
    fn replace(path: &Path) -> Self {
        let previous = std::env::var_os("PATH");
        std::env::set_var("PATH", path);
        Self { previous }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var("PATH", previous);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("OMP_PROFILE");
        std::env::remove_var("PI_PROFILE");
    }
}

fn make_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: expected_tool_perms(),
        scope: InstallScope::Global,
        force_permission_style: false,
    }
}

fn make_local_ctx(home: &Path, project: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: expected_tool_perms(),
        scope: InstallScope::Local {
            project_path: project.to_path_buf(),
        },
        force_permission_style: false,
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn write_script(bin_dir: &Path, body: &str) {
    std::fs::create_dir_all(bin_dir).unwrap();
    let script = bin_dir.join("omp");
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn write_fake_omp(bin_dir: &Path, stdout: &Path, exit_code: i32) {
    write_script(
        bin_dir,
        &format!(
            "[ \"$1 $2\" = \"config path\" ] || exit 64\nprintf '%s\\n' '{}'\nexit {exit_code}",
            stdout.display()
        ),
    );
}

fn config_error_message(error: TokenSaveError) -> String {
    match error {
        TokenSaveError::Config { message } => message,
        other => panic!("expected configuration error, got {other:?}"),
    }
}

fn healthcheck(home: &Path, project: &Path) -> DoctorCounters {
    let mut dc = DoctorCounters::new();
    OmpIntegration.healthcheck(
        &mut dc,
        &HealthcheckContext {
            home: home.to_path_buf(),
            project_path: project.to_path_buf(),
        },
    );
    dc
}

#[test]
fn global_install_uses_resolved_profile_and_preserves_existing_config() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profiles/work/agent");
    let mcp_path = agent_dir.join("mcp.json");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"unrelated":true,"mcpServers":{"other-tool":{"command":"other"}}}"#,
    )
    .unwrap();
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);

    OmpIntegration.install(&make_ctx(&home)).unwrap();

    let config = read_json(&mcp_path);
    assert_eq!(
        config["mcpServers"]["tokensave"]["args"],
        serde_json::json!(["serve"])
    );
    assert!(config["mcpServers"]["other-tool"].is_object());
    assert!(config["unrelated"].as_bool().unwrap());
    let rules_path = agent_dir.join("rules/tokensave.md");
    let rules_before = std::fs::read(&rules_path).unwrap();
    assert!(String::from_utf8_lossy(&rules_before).contains(OMP_RULES_MARKER));
    assert!(!home.join(".omp/agent/mcp.json").exists());

    OmpIntegration.install(&make_ctx(&home)).unwrap();

    let config = read_json(&mcp_path);
    assert_eq!(
        config["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|name| name.as_str() == "tokensave")
            .count(),
        1
    );
    assert_eq!(std::fs::read(rules_path).unwrap(), rules_before);
}

#[test]
fn global_install_inherits_omp_and_pi_profile_selection() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let omp_profile = temp.path().join("omp-profile/agent");
    let pi_profile = temp.path().join("pi-profile/agent");
    write_script(
        &bin_dir,
        &format!(
            r#"[ "$1 $2" = "config path" ] || exit 64
if [ "$OMP_PROFILE" = "work" ]; then
  printf '%s\n' '{}'
elif [ "$PI_PROFILE" = "compat" ]; then
  printf '%s\n' '{}'
else
  exit 65
fi"#,
            omp_profile.display(),
            pi_profile.display()
        ),
    );
    let _path = PathGuard::replace(&bin_dir);

    std::env::set_var("OMP_PROFILE", "work");
    OmpIntegration.install(&make_ctx(&home)).unwrap();
    assert!(omp_profile.join("mcp.json").exists());

    std::env::remove_var("OMP_PROFILE");
    std::env::set_var("PI_PROFILE", "compat");
    OmpIntegration.install(&make_ctx(&home)).unwrap();
    assert!(pi_profile.join("mcp.json").exists());
}

#[test]
fn global_resolver_rejects_missing_failed_empty_and_multiline_output() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let _path = PathGuard::replace(&bin_dir);

    for (label, script) in [
        ("missing", None),
        (
            "unsuccessful",
            Some("[ \"$1 $2\" = \"config path\" ] || exit 64\nexit 7"),
        ),
        (
            "empty",
            Some("[ \"$1 $2\" = \"config path\" ] || exit 64\nprintf '   \\n'"),
        ),
        (
            "multiline",
            Some("[ \"$1 $2\" = \"config path\" ] || exit 64\nprintf '/one\\n/two\\n'"),
        ),
        (
            "relative",
            Some("[ \"$1 $2\" = \"config path\" ] || exit 64\nprintf 'relative/agent\\n'"),
        ),
    ] {
        let script_path = bin_dir.join("omp");
        if let Some(body) = script {
            write_script(&bin_dir, body);
        } else {
            let _ = std::fs::remove_file(&script_path);
        }

        let message = config_error_message(OmpIntegration.install(&make_ctx(&home)).unwrap_err());
        assert!(
            message.contains("omp config path"),
            "{label} error should name the resolver, got {message:?}"
        );
        assert!(
            !home.join(".omp/agent/mcp.json").exists(),
            "{label} resolver failure must not fall back to the default profile"
        );
    }
}

#[test]
fn global_resolver_times_out_and_reaps_the_child() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    write_script(
        &bin_dir,
        "[ \"$1 $2\" = \"config path\" ] || exit 64\n/bin/sleep 30\nprintf '/too-late\\n'",
    );
    let _path = PathGuard::replace(&bin_dir);

    let started = Instant::now();
    let message = config_error_message(OmpIntegration.install(&make_ctx(&home)).unwrap_err());
    let elapsed = started.elapsed();

    assert!(message.contains("omp config path"));
    assert!(message.contains("timed out"), "unexpected error: {message}");
    assert!(
        elapsed >= Duration::from_secs(9) && elapsed < Duration::from_secs(15),
        "ten-second resolver deadline should be bounded, elapsed {elapsed:?}"
    );
    assert!(!home.join(".omp/agent/mcp.json").exists());
}

#[test]
fn local_install_is_deterministic_and_does_not_invoke_omp() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    let sentinel = temp.path().join("omp-was-called");
    write_script(
        &bin_dir,
        &format!("touch '{}'\nexit 99", sentinel.display()),
    );
    let _path = PathGuard::replace(&bin_dir);

    OmpIntegration
        .install(&make_local_ctx(&home, &project))
        .unwrap();

    assert!(project.join(".omp/mcp.json").exists());
    assert!(project.join(".omp/rules/tokensave.md").exists());
    assert!(!home.join(".omp/agent/mcp.json").exists());
    assert!(!sentinel.exists(), "local install must not invoke omp");
}

#[test]
fn reinstall_refreshes_owned_surfaces_and_preserves_valid_command() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profile/agent");
    let old_bin = temp.path().join("old/tokensave");
    std::fs::create_dir_all(old_bin.parent().unwrap()).unwrap();
    std::fs::write(&old_bin, "#!/bin/sh\n").unwrap();
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    let mut ctx = make_ctx(&home);
    ctx.tokensave_bin = temp
        .path()
        .join("new/tokensave")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(
        agent_dir.join("mcp.json"),
        serde_json::to_vec(&serde_json::json!({
            "mcpServers": {
                "tokensave": {
                    "command": old_bin,
                    "args": ["wrong"]
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let rules_path = agent_dir.join("rules/tokensave.md");
    std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
    std::fs::write(&rules_path, format!("{OMP_RULES_MARKER}\nstale\n")).unwrap();

    OmpIntegration.install(&ctx).unwrap();

    let config = read_json(&agent_dir.join("mcp.json"));
    assert_eq!(
        config["mcpServers"]["tokensave"]["command"],
        serde_json::json!(old_bin)
    );
    assert_eq!(
        config["mcpServers"]["tokensave"]["args"],
        serde_json::json!(["serve"])
    );
    assert_eq!(
        std::fs::read_to_string(rules_path).unwrap(),
        format!("{}\n", rules_for_agent("omp").unwrap().trim_end())
    );
}

#[test]
fn uninstall_preserves_unrelated_json_and_removes_owned_surfaces() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profile/agent");
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::write(
        agent_dir.join("mcp.json"),
        r#"{"unrelated":true,"mcpServers":{"other-tool":{"command":"other"}}}"#,
    )
    .unwrap();

    OmpIntegration.install(&ctx).unwrap();
    OmpIntegration.uninstall(&ctx).unwrap();

    let config = read_json(&agent_dir.join("mcp.json"));
    assert!(config["unrelated"].as_bool().unwrap());
    assert!(config["mcpServers"]["other-tool"].is_object());
    assert!(config["mcpServers"].get("tokensave").is_none());
    assert!(!agent_dir.join("rules/tokensave.md").exists());
}

#[test]
fn uninstall_removes_otherwise_empty_mcp_file() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profile/agent");
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    OmpIntegration.uninstall(&ctx).unwrap();

    assert!(!agent_dir.join("mcp.json").exists());
    assert!(!agent_dir.join("rules/tokensave.md").exists());
}

#[test]
fn uninstall_refuses_to_delete_rule_without_exact_ownership_marker() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profile/agent");
    let rules_path = agent_dir.join("rules/tokensave.md");
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
    std::fs::write(
        &rules_path,
        "# User-authored tokensave rule\n<!-- tokensave rules begin -->\n",
    )
    .unwrap();

    OmpIntegration.uninstall(&make_ctx(&home)).unwrap();

    assert!(rules_path.exists());
}

#[test]
fn doctor_skips_omp_when_neither_global_nor_project_install_is_detected() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let _path = PathGuard::replace(&bin_dir);

    let result = healthcheck(&home, &project);

    assert_eq!(result.issues, 0);
    assert_eq!(result.warnings, 0);
}

#[test]
fn doctor_warns_when_detected_global_surfaces_are_absent() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profile/agent");
    std::fs::create_dir_all(home.join(".omp")).unwrap();
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);

    let result = healthcheck(&home, &project);

    assert_eq!(result.issues, 0);
    assert_eq!(result.warnings, 2);
}

#[test]
fn doctor_warns_when_project_local_surfaces_are_absent() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(project.join(".omp")).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();
    let _path = PathGuard::replace(&bin_dir);

    let result = healthcheck(&home, &project);

    assert_eq!(result.issues, 0);
    assert_eq!(result.warnings, 2);
}

#[test]
fn doctor_warns_for_missing_but_detects_present_broken_global_surfaces() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profile/agent");
    let mcp_path = agent_dir.join("mcp.json");
    let rules_path = agent_dir.join("rules/tokensave.md");
    std::fs::create_dir_all(home.join(".omp")).unwrap();
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);
    OmpIntegration.install(&ctx).unwrap();

    assert_eq!(healthcheck(&home, &project).issues, 0);

    std::fs::remove_file(&mcp_path).unwrap();
    let missing_mcp = healthcheck(&home, &project);
    assert_eq!(missing_mcp.issues, 0);
    assert_eq!(missing_mcp.warnings, 1);
    OmpIntegration.install(&ctx).unwrap();

    let mut config = read_json(&mcp_path);
    config["mcpServers"]["tokensave"]["args"] = serde_json::json!(["wrong"]);
    std::fs::write(&mcp_path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(healthcheck(&home, &project).issues > 0);
    OmpIntegration.install(&ctx).unwrap();

    let mut config = read_json(&mcp_path);
    config["mcpServers"]["tokensave"]["command"] = serde_json::json!("other");
    std::fs::write(&mcp_path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(healthcheck(&home, &project).issues > 0);
    OmpIntegration.install(&ctx).unwrap();

    std::fs::remove_file(&rules_path).unwrap();
    let missing_rules = healthcheck(&home, &project);
    assert_eq!(missing_rules.issues, 0);
    assert_eq!(missing_rules.warnings, 1);
    OmpIntegration.install(&ctx).unwrap();

    std::fs::write(&rules_path, format!("{OMP_RULES_MARKER}\nstale\n")).unwrap();
    assert!(healthcheck(&home, &project).issues > 0);
}

#[test]
fn doctor_reports_resolver_failure_and_still_checks_present_local_surfaces() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(home.join(".omp")).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();
    let _path = PathGuard::replace(&bin_dir);
    OmpIntegration
        .install(&make_local_ctx(&home, &project))
        .unwrap();

    let resolver_only = healthcheck(&home, &project);
    assert_eq!(
        resolver_only.issues, 1,
        "a correct local install should add no issue beyond the failed global resolver"
    );

    std::fs::write(project.join(".omp/mcp.json"), "{ malformed").unwrap();
    let malformed_local = healthcheck(&home, &project);
    assert!(
        malformed_local.issues > resolver_only.issues,
        "doctor must independently validate an existing project-local .omp surface"
    );
}

#[test]
fn doctor_accepts_valid_project_local_install_when_global_resolver_fails() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    let sentinel = temp.path().join("omp-was-called");
    write_script(
        &bin_dir,
        &format!("printf called > '{}'\nexit 99", sentinel.display()),
    );
    let _path = PathGuard::replace(&bin_dir);
    OmpIntegration
        .install(&make_local_ctx(&home, &project))
        .unwrap();

    let result = healthcheck(&home, &project);

    assert_eq!(result.issues, 0);
    assert_eq!(result.warnings, 0);
    assert!(
        sentinel.exists(),
        "doctor must probe OMP to discover unrecorded custom profiles"
    );
}

#[test]
fn default_profile_detection_survives_resolver_failure() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let sentinel = temp.path().join("omp-was-called");
    write_script(
        &bin_dir,
        &format!("printf called > '{}'\nexit 99", sentinel.display()),
    );
    let _path = PathGuard::replace(&bin_dir);
    let mcp_path = home.join(".omp/agent/mcp.json");
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"other-tool":{"command":"other"}}}"#,
    )
    .unwrap();

    assert!(OmpIntegration.is_detected(&home));
    assert!(!OmpIntegration.has_tokensave(&home));
    assert!(sentinel.exists());
    std::fs::remove_file(&sentinel).unwrap();

    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"other-tool":{"command":"other"},"tokensave":{"command":"tokensave","args":["serve"]}}}"#,
    )
    .unwrap();
    assert!(OmpIntegration.has_tokensave(&home));
    assert_eq!(OmpIntegration.primary_config_path(&home), None);
    assert!(sentinel.exists());
}

#[test]
fn custom_profile_without_registry_is_detected_migrated_and_reported() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profiles/work/agent");
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);

    OmpIntegration.install(&make_ctx(&home)).unwrap();
    std::fs::remove_file(home.join(".tokensave/omp-profiles.json")).unwrap();

    assert!(!home.join(".omp").exists());
    assert!(OmpIntegration.is_detected(&home));
    assert!(OmpIntegration.has_tokensave(&home));
    assert_eq!(
        OmpIntegration.primary_config_path(&home),
        Some(agent_dir.join("mcp.json"))
    );

    let mut config = UserConfig::default();
    migrate_installed_agents(&home, &mut config);
    assert_eq!(config.installed_agents, vec!["omp"]);
    assert_eq!(healthcheck(&home, &project).issues, 0);
}

#[test]
fn doctor_checks_custom_profile_when_default_directory_is_absent() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profiles/work/agent");
    let mcp_path = agent_dir.join("mcp.json");
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    OmpIntegration.install(&make_ctx(&home)).unwrap();
    std::fs::remove_file(home.join(".tokensave/omp-profiles.json")).unwrap();

    let mut config = read_json(&mcp_path);
    config["mcpServers"]["tokensave"]["args"] = serde_json::json!(["wrong"]);
    std::fs::write(&mcp_path, serde_json::to_vec(&config).unwrap()).unwrap();

    assert!(!home.join(".omp").exists());
    assert!(healthcheck(&home, &project).issues > 0);
}

#[test]
fn profile_change_uninstall_cleans_original_without_populating_new_profile() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let original = temp.path().join("profiles/original/agent");
    let active = temp.path().join("profiles/active/agent");
    write_fake_omp(&bin_dir, &original, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    write_fake_omp(&bin_dir, &active, 0);
    OmpIntegration.uninstall(&ctx).unwrap();

    assert!(!original.join("mcp.json").exists());
    assert!(!original.join("rules/tokensave.md").exists());
    assert!(
        !active.exists(),
        "uninstall must never populate a new profile"
    );
}

#[test]
fn uninstall_uses_recorded_profile_when_resolver_disappears() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let original = temp.path().join("profiles/original/agent");
    write_fake_omp(&bin_dir, &original, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    std::fs::remove_file(bin_dir.join("omp")).unwrap();
    OmpIntegration.uninstall(&ctx).unwrap();

    assert!(!original.join("mcp.json").exists());
    assert!(!original.join("rules/tokensave.md").exists());
    assert!(!home.join(".tokensave/omp-profiles.json").exists());
}

#[test]
fn reinstall_refreshes_recorded_and_current_profiles() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let original = temp.path().join("profiles/original/agent");
    let active = temp.path().join("profiles/active/agent");
    write_fake_omp(&bin_dir, &original, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    let mut original_config = read_json(&original.join("mcp.json"));
    original_config["mcpServers"]["tokensave"]["args"] = serde_json::json!(["stale"]);
    std::fs::write(
        original.join("mcp.json"),
        serde_json::to_vec(&original_config).unwrap(),
    )
    .unwrap();
    std::fs::write(
        original.join("rules/tokensave.md"),
        format!("{OMP_RULES_MARKER}\nstale\n"),
    )
    .unwrap();

    std::fs::create_dir_all(&active).unwrap();
    write_fake_omp(&bin_dir, &active, 0);
    OmpIntegration.install(&ctx).unwrap();

    for profile in [&original, &active] {
        assert_eq!(
            read_json(&profile.join("mcp.json"))["mcpServers"]["tokensave"]["args"],
            serde_json::json!(["serve"])
        );
        assert_eq!(
            std::fs::read_to_string(profile.join("rules/tokensave.md")).unwrap(),
            format!("{}\n", rules_for_agent("omp").unwrap().trim_end())
        );
    }
}

#[test]
fn reinstall_forgets_deleted_profiles_without_recreating_them() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let deleted = temp.path().join("profiles/deleted/agent");
    let active = temp.path().join("profiles/active/agent");
    write_fake_omp(&bin_dir, &deleted, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    std::fs::remove_dir_all(&deleted).unwrap();
    std::fs::create_dir_all(&active).unwrap();
    write_fake_omp(&bin_dir, &active, 0);

    OmpIntegration.install(&ctx).unwrap();

    assert!(!deleted.exists());
    assert_eq!(
        read_json(&home.join(".tokensave/omp-profiles.json"))["agent_dirs"],
        serde_json::json!([active])
    );
}

#[test]
fn broken_recorded_profile_does_not_block_installing_current_profile() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let recorded = temp.path().join("profiles/a-recorded/agent");
    let active = temp.path().join("profiles/b-active/agent");
    write_fake_omp(&bin_dir, &recorded, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    std::fs::write(recorded.join("mcp.json"), "{ malformed").unwrap();
    std::fs::create_dir_all(&active).unwrap();
    write_fake_omp(&bin_dir, &active, 0);

    assert!(OmpIntegration.install(&ctx).is_err());
    assert!(omp_has_tokensave(&active.join("mcp.json")));
    assert!(active.join("rules/tokensave.md").exists());
}

#[test]
fn uninstall_attempts_every_profile_and_retains_failed_ownership() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let broken = temp.path().join("profiles/a-broken/agent");
    let healthy = temp.path().join("profiles/b-healthy/agent");
    write_fake_omp(&bin_dir, &broken, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    std::fs::create_dir_all(&healthy).unwrap();
    write_fake_omp(&bin_dir, &healthy, 0);
    OmpIntegration.install(&ctx).unwrap();
    std::fs::write(broken.join("mcp.json"), "{ malformed").unwrap();

    assert!(OmpIntegration.uninstall(&ctx).is_err());
    assert!(!healthy.join("mcp.json").exists());
    assert!(!healthy.join("rules/tokensave.md").exists());

    let registry = read_json(&home.join(".tokensave/omp-profiles.json"));
    assert_eq!(registry["agent_dirs"], serde_json::json!([broken]));
}

#[test]
fn uninstall_retains_ownership_when_rules_removal_fails() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profiles/work/agent");
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);
    let ctx = make_ctx(&home);

    OmpIntegration.install(&ctx).unwrap();
    let rules_dir = agent_dir.join("rules");
    std::fs::set_permissions(&rules_dir, std::fs::Permissions::from_mode(0o500)).unwrap();

    let result = OmpIntegration.uninstall(&ctx);

    std::fs::set_permissions(&rules_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(agent_dir.join("rules/tokensave.md").exists());
    assert_eq!(
        read_json(&home.join(".tokensave/omp-profiles.json"))["agent_dirs"],
        serde_json::json!([agent_dir])
    );
}

#[test]
fn malformed_profile_registry_blocks_install_without_touching_omp() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profiles/work/agent");
    let registry = home.join(".tokensave/omp-profiles.json");
    std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
    std::fs::write(&registry, "{ malformed").unwrap();
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);

    let message = config_error_message(OmpIntegration.install(&make_ctx(&home)).unwrap_err());

    assert!(message.contains("OMP profile registry"), "{message}");
    assert_eq!(std::fs::read_to_string(registry).unwrap(), "{ malformed");
    assert!(!agent_dir.exists());
}

#[test]
fn unsupported_or_relative_profile_registry_blocks_install_without_touching_omp() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let bin_dir = temp.path().join("bin");
    let agent_dir = temp.path().join("profiles/work/agent");
    let registry = home.join(".tokensave/omp-profiles.json");
    std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
    write_fake_omp(&bin_dir, &agent_dir, 0);
    let _path = PathGuard::replace(&bin_dir);

    for contents in [
        r#"{"version":99,"agent_dirs":[]}"#,
        r#"{"version":1,"agent_dirs":["relative/agent"]}"#,
    ] {
        std::fs::write(&registry, contents).unwrap();

        let message = config_error_message(OmpIntegration.install(&make_ctx(&home)).unwrap_err());

        assert!(message.contains("OMP profile registry"), "{message}");
        assert_eq!(std::fs::read_to_string(&registry).unwrap(), contents);
        assert!(!agent_dir.exists());
    }
}

fn omp_has_tokensave(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .is_some_and(|config| config.pointer("/mcpServers/tokensave").is_some())
}

#[test]
fn registry_and_rules_expose_omp_independently_from_pi() {
    assert_eq!(get_integration("omp").unwrap().id(), "omp");
    assert_eq!(get_integration("pi").unwrap().id(), "pi");
    assert!(available_integrations().contains(&"omp"));

    let omp = rules_for_agent("omp").unwrap();
    assert_ne!(omp, rules_for_agent("claude").unwrap());
    assert!(omp.starts_with("---\nalwaysApply: true\n---\n"));
    assert!(omp.contains(OMP_RULES_MARKER));
    assert!(omp.contains("multi-hop"));
    assert!(omp.contains("LSP"));
    assert!(omp.contains("scout"));
    assert!(omp.contains("graph_root"));
    assert!(omp.contains("graph_branch"));
    assert!(omp.contains("SQL fallback"));
    assert!(omp.contains("Tool gaps"));
    assert!(!omp.contains("NEVER use Agent(subagent_type=Explore)"));
}
