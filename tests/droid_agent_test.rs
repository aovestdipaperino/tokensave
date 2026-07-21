use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokensave::agents::{
    expected_tool_perms, AgentIntegration, DoctorCounters, DroidIntegration, HealthcheckContext,
    InstallContext, InstallScope,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

fn mcp_path(home: &Path) -> PathBuf {
    home.join(".factory/mcp.json")
}

fn agents_md_path(home: &Path) -> PathBuf {
    home.join(".factory/AGENTS.md")
}

fn settings_path(home: &Path) -> PathBuf {
    home.join(".factory/settings.json")
}

// ===========================================================================
// Install content verification
// ===========================================================================

#[test]
fn test_install_creates_mcp_json() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let path = mcp_path(home);
    assert!(path.exists(), "mcp.json should be created");

    let config = read_json(&path);
    let ts = &config["mcpServers"]["tokensave"];
    assert!(ts.is_object(), "mcpServers.tokensave should be an object");
    assert_eq!(
        ts["type"].as_str().unwrap(),
        "stdio",
        "type should be stdio"
    );
    assert_eq!(
        ts["command"].as_str().unwrap(),
        "/usr/local/bin/tokensave",
        "command should be the tokensave binary path"
    );
    let args: Vec<&str> = ts["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(args, vec!["serve"], "args should be [\"serve\"]");
}

#[test]
fn test_install_appends_prompt_rules() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let rules = std::fs::read_to_string(agents_md_path(home)).unwrap();
    assert!(
        rules.contains("## Prefer tokensave MCP tools"),
        "AGENTS.md should contain the tokensave rules marker"
    );
}

#[test]
fn test_install_preserves_existing_server() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = mcp_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"someSetting": true, "mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&path);
    assert!(
        config["someSetting"].as_bool().unwrap(),
        "unrelated top-level key should be preserved"
    );
    assert!(
        config["mcpServers"]["other-tool"].is_object(),
        "existing MCP server should be preserved"
    );
    assert!(
        config["mcpServers"]["tokensave"].is_object(),
        "tokensave should be added"
    );
}

#[test]
fn test_install_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&mcp_path(home));
    let servers = config["mcpServers"].as_object().unwrap();
    let ts_count = servers.keys().filter(|k| *k == "tokensave").count();
    assert_eq!(ts_count, 1, "tokensave should appear exactly once");

    let rules = std::fs::read_to_string(agents_md_path(home)).unwrap();
    assert_eq!(
        rules.matches("## Prefer tokensave MCP tools").count(),
        1,
        "rules block should appear exactly once"
    );
}

#[test]
fn test_primary_config_path_matches_install_target() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let primary = DroidIntegration.primary_config_path(home).unwrap();
    assert!(
        primary.exists(),
        "primary_config_path should exist after install"
    );
    assert_eq!(
        primary,
        mcp_path(home),
        "primary_config_path should match where install wrote"
    );
}

#[test]
fn test_local_install_targets_project() {
    let home_dir = TempDir::new().unwrap();
    let proj_dir = TempDir::new().unwrap();
    let home = home_dir.path();
    let project = proj_dir.path();

    let ctx = make_local_ctx(home, project);
    DroidIntegration.install(&ctx).unwrap();

    assert!(
        project.join(".factory/mcp.json").exists(),
        "--local should write <project>/.factory/mcp.json"
    );
    assert!(
        project.join("AGENTS.md").exists(),
        "--local should write <project>/AGENTS.md"
    );
    assert!(
        !mcp_path(home).exists(),
        "global ~/.factory/mcp.json should not be written for --local"
    );
}

// ===========================================================================
// Uninstall verification
// ===========================================================================

#[test]
fn test_uninstall_removes_only_tokensave() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = mcp_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"mcpServers": {"other-tool": {"command": "other", "args": ["run"]}}}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    assert!(
        path.exists(),
        "config should still exist because another server remains"
    );
    let config = read_json(&path);
    assert!(
        config["mcpServers"]["other-tool"].is_object(),
        "other server should be preserved"
    );
    assert!(
        config
            .get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_none(),
        "tokensave should be removed"
    );
}

#[test]
fn test_uninstall_removes_empty_config_file() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    assert!(
        !mcp_path(home).exists(),
        "mcp.json should be deleted when tokensave was the only entry"
    );
}

#[test]
fn test_uninstall_removes_prompt_rules() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    let path = agents_md_path(home);
    if path.exists() {
        let rules = std::fs::read_to_string(&path).unwrap();
        assert!(
            !rules.contains("tokensave"),
            "tokensave rules should be removed from AGENTS.md"
        );
    }
}

#[test]
fn test_uninstall_preserves_other_agents_md_content() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = agents_md_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "# My rules\n\nKeep this.\n").unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    let rules = std::fs::read_to_string(&path).unwrap();
    assert!(rules.contains("Keep this."), "user content should survive");
    assert!(
        !rules.contains("tokensave"),
        "tokensave rules should be removed"
    );
}

#[test]
fn test_uninstall_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.uninstall(&ctx).unwrap();
}

// ===========================================================================
// Healthcheck verification
// ===========================================================================

#[test]
fn test_healthcheck_clean_install_no_issues() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(dc.issues, 0, "clean droid install should have no issues");
}

#[test]
fn test_healthcheck_missing_config_produces_warning() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.warnings > 0 || dc.issues > 0,
        "healthcheck on empty dir should report warnings or issues"
    );
}

#[test]
fn test_healthcheck_detects_missing_mcp_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = mcp_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert!(dc.issues > 0, "healthcheck should detect missing MCP entry");
}

// ===========================================================================
// is_detected / has_tokensave
// ===========================================================================

#[test]
fn test_is_detected_empty_home() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !DroidIntegration.is_detected(home),
        "should not be detected on empty home"
    );
}

#[test]
fn test_is_detected_with_factory_dir() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join(".factory")).unwrap();
    assert!(
        DroidIntegration.is_detected(home),
        "should be detected when ~/.factory exists"
    );
}

#[test]
fn test_has_tokensave_before_and_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    assert!(
        !DroidIntegration.has_tokensave(home),
        "has_tokensave should be false before install"
    );

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    assert!(
        DroidIntegration.has_tokensave(home),
        "has_tokensave should be true after install"
    );

    DroidIntegration.uninstall(&ctx).unwrap();
    assert!(
        !DroidIntegration.has_tokensave(home),
        "has_tokensave should be false after uninstall"
    );
}

// ===========================================================================
// Name / ID
// ===========================================================================

#[test]
fn test_name_and_id() {
    assert_eq!(DroidIntegration.name(), "Factory Droid");
    assert_eq!(DroidIntegration.id(), "droid");
}

// ===========================================================================
// PreToolUse hook install/uninstall/healthcheck
// ===========================================================================

#[test]
fn test_install_writes_pre_tool_use_hook() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let path = settings_path(home);
    assert!(path.exists(), "settings.json should be created");

    let config = read_json(&path);
    let entries = config["hooks"]["PreToolUse"].as_array().unwrap();
    let entry = entries
        .iter()
        .find(|e| e["matcher"].as_str() == Some("^(Execute|Grep)$"))
        .expect("an ^(Execute|Grep)$-matcher entry should be present");
    let command = entry["hooks"][0]["command"].as_str().unwrap();
    assert!(
        command.contains("/usr/local/bin/tokensave"),
        "hook command should reference the tokensave binary"
    );
    assert!(
        command.contains("hook-droid-pre-tool-use"),
        "hook command should invoke the droid PreToolUse subcommand"
    );
}

#[test]
fn test_install_preserves_existing_hooks_and_settings() {
    // Mirrors the owner's live ~/.factory/settings.json: Factory ships its own
    // Stop/Notification/PreToolUse/UserPromptSubmit wrappers plus unrelated
    // top-level keys. Installing tokensave's hook must not disturb any of it.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = settings_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "enabledPlugins": {"core@factory-plugins": true},
            "includeCoAuthoredByDroid": false,
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/opt/owner/permission.sh"}]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/opt/owner/stop.sh"}]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&path);
    assert!(
        config["enabledPlugins"]["core@factory-plugins"]
            .as_bool()
            .unwrap(),
        "unrelated top-level key should be preserved"
    );
    assert!(
        !config["includeCoAuthoredByDroid"].as_bool().unwrap(),
        "unrelated top-level key should be preserved"
    );

    let pre_tool_use = config["hooks"]["PreToolUse"].as_array().unwrap();
    assert!(
        pre_tool_use
            .iter()
            .any(|e| e["hooks"][0]["command"].as_str() == Some("/opt/owner/permission.sh")),
        "owner's existing PreToolUse wrapper should be preserved"
    );
    assert!(
        pre_tool_use
            .iter()
            .any(|e| e["matcher"].as_str() == Some("^(Execute|Grep)$")
                && e["hooks"][0]["command"]
                    .as_str()
                    .is_some_and(|c| c.contains("tokensave"))),
        "tokensave's ^(Execute|Grep)$-matcher hook should be added"
    );
    assert_eq!(
        config["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap(),
        "/opt/owner/stop.sh",
        "unrelated hook event should be untouched"
    );
}

#[test]
fn test_install_preserves_foreign_hook_with_lookalike_command() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = settings_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Grep",
                        "hooks": [{"type": "command", "command": "/opt/hooks/tokensave-helper hook-droid-pre-tool-use"}]
                    },
                    {
                        "matcher": "Grep",
                        "hooks": [{"type": "command", "command": "/usr/local/bin/tokensave hook-droid-pre-tool-use --custom"}]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    DroidIntegration.install(&make_ctx(home)).unwrap();

    let config = read_json(&path);
    let entries = config["hooks"]["PreToolUse"].as_array().unwrap();
    assert!(
        entries.iter().any(|e| {
            e["hooks"][0]["command"].as_str()
                == Some("/opt/hooks/tokensave-helper hook-droid-pre-tool-use")
        }),
        "a foreign hook using a lookalike binary must survive install"
    );
    assert!(
        entries.iter().any(|e| {
            e["hooks"][0]["command"].as_str()
                == Some("/usr/local/bin/tokensave hook-droid-pre-tool-use --custom")
        }),
        "a customized command with trailing arguments must survive install"
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| {
                e["hooks"][0]["command"]
                    .as_str()
                    .is_some_and(|c| c.ends_with("tokensave hook-droid-pre-tool-use"))
            })
            .count(),
        1,
        "tokensave's hook should be installed separately"
    );
}

#[test]
fn test_install_hook_idempotent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&settings_path(home));
    let entries = config["hooks"]["PreToolUse"].as_array().unwrap();
    let count = entries
        .iter()
        .filter(|e| {
            e["hooks"][0]["command"]
                .as_str()
                .is_some_and(|c| c.contains("tokensave"))
        })
        .count();
    assert_eq!(
        count, 1,
        "tokensave's hook entry should appear exactly once"
    );
}

#[test]
fn test_install_migrates_stale_execute_only_matcher() {
    // An install written by an older tokensave used the `Execute`-only matcher.
    // Re-installing must upgrade it in place to `^(Execute|Grep)$`, not leave a
    // duplicate entry that would fire the hook twice.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = settings_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Execute",
                        "hooks": [{"type": "command", "command": "/usr/local/bin/tokensave hook-droid-pre-tool-use"}]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let config = read_json(&path);
    let entries = config["hooks"]["PreToolUse"].as_array().unwrap();
    let tokensave_entries: Vec<_> = entries
        .iter()
        .filter(|e| {
            e["hooks"][0]["command"]
                .as_str()
                .is_some_and(|c| c.contains("hook-droid-pre-tool-use"))
        })
        .collect();
    assert_eq!(
        tokensave_entries.len(),
        1,
        "stale entry must be migrated in place, not duplicated"
    );
    assert_eq!(
        tokensave_entries[0]["matcher"].as_str(),
        Some("^(Execute|Grep)$"),
        "matcher should be upgraded to the widened value"
    );
}

#[test]
fn test_uninstall_removes_stale_execute_only_matcher() {
    // uninstall must recognize a tokensave entry by its subcommand even when
    // the matcher is the older `Execute`-only value.
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = settings_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Execute",
                        "hooks": [{"type": "command", "command": "/usr/local/bin/tokensave hook-droid-pre-tool-use"}]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.uninstall(&ctx).unwrap();

    // The only hook was tokensave's, so uninstall empties and removes the file.
    let still_present = path.exists()
        && read_json(&path)["hooks"]["PreToolUse"]
            .as_array()
            .is_some_and(|arr| {
                arr.iter().any(|e| {
                    e["hooks"][0]["command"]
                        .as_str()
                        .is_some_and(|c| c.contains("hook-droid-pre-tool-use"))
                })
            });
    assert!(!still_present, "stale Execute-only entry should be removed");
}

#[test]
fn test_local_install_writes_project_settings() {
    let home_dir = TempDir::new().unwrap();
    let proj_dir = TempDir::new().unwrap();
    let home = home_dir.path();
    let project = proj_dir.path();

    let ctx = make_local_ctx(home, project);
    DroidIntegration.install(&ctx).unwrap();

    assert!(
        project.join(".factory/settings.json").exists(),
        "--local should write <project>/.factory/settings.json"
    );
    assert!(
        !settings_path(home).exists(),
        "global ~/.factory/settings.json should not be written for --local"
    );
}

#[test]
fn test_uninstall_removes_only_tokensave_hook() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let path = settings_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/opt/owner/permission.sh"}]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();
    DroidIntegration.uninstall(&ctx).unwrap();

    let config = read_json(&path);
    let pre_tool_use = config["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(
        pre_tool_use.len(),
        1,
        "only tokensave's entry should be removed"
    );
    assert_eq!(
        pre_tool_use[0]["hooks"][0]["command"].as_str().unwrap(),
        "/opt/owner/permission.sh",
        "owner's PreToolUse wrapper should survive uninstall"
    );
}

#[test]
fn test_uninstall_hook_without_install_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.uninstall(&ctx).unwrap();
}

#[test]
fn test_healthcheck_detects_missing_hook() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    let path = settings_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"hooks": {}}"#).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert!(dc.issues > 0, "healthcheck should detect missing hook");
}

#[test]
fn test_healthcheck_detects_stale_execute_only_matcher() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let path = settings_path(home);
    let mut config = read_json(&path);
    config["hooks"]["PreToolUse"][0]["matcher"] = serde_json::json!("Execute");
    std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert!(
        dc.issues > 0,
        "healthcheck should require the current matcher so users know to reinstall"
    );
}

#[test]
fn test_healthcheck_passes_after_install() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);
    DroidIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    DroidIntegration.healthcheck(&mut dc, &hctx);
    assert_eq!(
        dc.issues, 0,
        "clean droid install (including the hook) should have no issues"
    );
}
