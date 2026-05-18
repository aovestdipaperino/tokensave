use std::io::Write;
use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::{
    read_only_tool_names, tool_names, AgentIntegration, DoctorCounters, HealthcheckContext,
    InstallContext, KiroIntegration,
};

fn make_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: Vec::new(),
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn assert_hook(
    agent: &serde_json::Value,
    event: &str,
    matcher: Option<&str>,
    subcommand: &str,
    timeout_ms: u64,
) {
    let hooks = agent["hooks"][event].as_array().unwrap();
    let hook = hooks
        .iter()
        .find(|hook| {
            let matcher_matches = match matcher {
                Some(expected) => hook["matcher"].as_str() == Some(expected),
                None => hook.get("matcher").is_none(),
            };
            matcher_matches
                && hook["command"]
                    .as_str()
                    .is_some_and(|command| command.contains(subcommand))
        })
        .unwrap_or_else(|| panic!("missing hook {event} {matcher:?} {subcommand}"));
    assert_eq!(hook["timeout_ms"].as_u64(), Some(timeout_ms));
}

#[test]
fn test_install_creates_global_mcp_steering_agent_and_default() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    KiroIntegration.install(&ctx).unwrap();

    let mcp_path = home.join(".kiro/settings/mcp.json");
    assert!(mcp_path.exists(), "global Kiro MCP config should exist");
    let mcp = read_json(&mcp_path);
    let server = &mcp["mcpServers"]["tokensave"];
    assert!(server.is_object(), "mcpServers.tokensave should exist");
    assert_eq!(server["command"].as_str(), Some("/usr/local/bin/tokensave"));
    assert_eq!(
        server["args"].as_array().unwrap(),
        &[serde_json::json!("serve")]
    );
    assert_eq!(server["disabled"], serde_json::json!(false));

    let auto_approve = server["autoApprove"].as_array().unwrap();
    assert!(
        !auto_approve.iter().any(|v| v.as_str() == Some("*")),
        "autoApprove should not broadly allow every tokensave tool"
    );
    for tool in read_only_tool_names() {
        assert!(
            auto_approve
                .iter()
                .any(|v| v.as_str() == Some(tool.as_str())),
            "autoApprove should include {tool}"
        );
    }
    for tool in mutating_tool_names() {
        assert!(
            !auto_approve
                .iter()
                .any(|v| v.as_str() == Some(tool.as_str())),
            "autoApprove should not include mutating tool {tool}"
        );
    }

    let steering_path = home.join(".kiro/steering/AGENTS.md");
    assert!(steering_path.exists(), "global Kiro AGENTS.md should exist");
    let steering = std::fs::read_to_string(&steering_path).unwrap();
    assert!(steering.contains("## Prefer tokensave MCP tools"));
    assert!(steering.contains("delegate"));

    let agent_path = home.join(".kiro/agents/tokensave.json");
    assert!(agent_path.exists(), "managed Kiro agent should exist");
    let agent = read_json(&agent_path);
    assert_eq!(agent["name"].as_str(), Some("tokensave"));
    assert_eq!(agent["includeMcpJson"].as_bool(), Some(true));
    assert_eq!(
        agent["prompt"].as_str(),
        Some("file://../steering/AGENTS.md")
    );
    assert_eq!(
        agent["tools"].as_array().unwrap(),
        &[serde_json::json!("*")]
    );
    assert_hook(
        &agent,
        "userPromptSubmit",
        None,
        "hook-kiro-prompt-submit",
        5_000,
    );
    assert_hook(
        &agent,
        "preToolUse",
        Some("delegate"),
        "hook-kiro-pre-tool-use",
        5_000,
    );
    assert_hook(
        &agent,
        "preToolUse",
        Some("subagent"),
        "hook-kiro-pre-tool-use",
        5_000,
    );
    assert_hook(
        &agent,
        "postToolUse",
        Some("fs_write"),
        "hook-kiro-post-tool-use",
        30_000,
    );

    let cli_path = home.join(".kiro/settings/cli.json");
    assert!(cli_path.exists(), "Kiro CLI settings should exist");
    let cli = read_json(&cli_path);
    assert_eq!(cli["chat"]["defaultAgent"].as_str(), Some("tokensave"));
}

#[test]
fn test_install_preserves_existing_mcp_config_and_writes_backup() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let mcp_path = home.join(".kiro/settings/mcp.json");
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"other":{"command":"other-bin"}},"theme":"dark"}"#,
    )
    .unwrap();

    let ctx = make_ctx(home);
    KiroIntegration.install(&ctx).unwrap();

    let mcp = read_json(&mcp_path);
    assert!(mcp["mcpServers"]["tokensave"].is_object());
    assert!(mcp["mcpServers"]["other"].is_object());
    assert_eq!(mcp["theme"].as_str(), Some("dark"));
    assert!(
        home.join(".kiro/settings/mcp.json.bak").exists(),
        "install should preserve a backup before rewriting existing config"
    );
}

#[test]
fn test_install_and_uninstall_preserve_existing_steering_content() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    let steering_path = home.join(".kiro/steering/AGENTS.md");
    std::fs::create_dir_all(steering_path.parent().unwrap()).unwrap();
    std::fs::write(
        &steering_path,
        "## Existing Kiro guidance\n\nKeep this user-authored guidance.\n",
    )
    .unwrap();

    KiroIntegration.install(&ctx).unwrap();

    let installed = std::fs::read_to_string(&steering_path).unwrap();
    assert!(installed.contains("## Existing Kiro guidance"));
    assert!(installed.contains("Keep this user-authored guidance."));
    assert!(installed.contains("## Prefer tokensave MCP tools"));

    KiroIntegration.uninstall(&ctx).unwrap();

    let uninstalled = std::fs::read_to_string(&steering_path).unwrap();
    assert!(uninstalled.contains("## Existing Kiro guidance"));
    assert!(uninstalled.contains("Keep this user-authored guidance."));
    assert!(!uninstalled.contains("## Prefer tokensave MCP tools"));
}

#[test]
fn test_uninstall_preserves_user_steering_after_tokensave_block() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    KiroIntegration.install(&ctx).unwrap();

    let steering_path = home.join(".kiro/steering/AGENTS.md");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&steering_path)
        .unwrap()
        .write_all(b"\nUser guidance appended after setup without a new heading.\n")
        .unwrap();

    KiroIntegration.uninstall(&ctx).unwrap();

    let uninstalled = std::fs::read_to_string(&steering_path).unwrap();
    assert!(uninstalled.contains("User guidance appended after setup without a new heading."));
    assert!(!uninstalled.contains("## Prefer tokensave MCP tools"));
}

#[test]
fn test_uninstall_leaves_edited_legacy_steering_without_end_marker() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    let steering_path = home.join(".kiro/steering/AGENTS.md");
    std::fs::create_dir_all(steering_path.parent().unwrap()).unwrap();
    std::fs::write(
        &steering_path,
        "## Prefer tokensave MCP tools\n\nLegacy tokensave guidance edited by the user.\n\
User guidance under the same heading.\n",
    )
    .unwrap();

    KiroIntegration.uninstall(&ctx).unwrap();

    let uninstalled = std::fs::read_to_string(&steering_path).unwrap();
    assert!(uninstalled.contains("Legacy tokensave guidance edited by the user."));
    assert!(uninstalled.contains("User guidance under the same heading."));
}

#[test]
fn test_uninstall_removes_tokensave_and_preserves_other_mcp_servers() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    let mcp_path = home.join(".kiro/settings/mcp.json");
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"other":{"command":"other-bin"}},"theme":"dark"}"#,
    )
    .unwrap();

    KiroIntegration.install(&ctx).unwrap();
    KiroIntegration.uninstall(&ctx).unwrap();

    let mcp = read_json(&mcp_path);
    assert!(mcp["mcpServers"]["other"].is_object());
    assert!(mcp["mcpServers"].get("tokensave").is_none());
    assert_eq!(mcp["theme"].as_str(), Some("dark"));

    assert!(!home.join(".kiro/agents/tokensave.json").exists());
    let cli = std::fs::read_to_string(home.join(".kiro/settings/cli.json")).unwrap_or_default();
    assert!(
        !cli.contains("defaultAgent"),
        "uninstall should remove tokensave default agent"
    );
    let steering =
        std::fs::read_to_string(home.join(".kiro/steering/AGENTS.md")).unwrap_or_default();
    assert!(!steering.contains("## Prefer tokensave MCP tools"));
}

#[test]
fn test_install_and_uninstall_preserve_user_managed_custom_agent() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    let agent_path = home.join(".kiro/agents/tokensave.json");
    std::fs::create_dir_all(agent_path.parent().unwrap()).unwrap();
    let custom_agent = serde_json::json!({
        "name": "tokensave",
        "description": "User-managed custom agent",
        "includeMcpJson": true,
        "hooks": {
            "preToolUse": [
                {
                    "matcher": "delegate",
                    "command": "echo user-managed hook"
                }
            ]
        }
    });
    std::fs::write(
        &agent_path,
        serde_json::to_string_pretty(&custom_agent).unwrap(),
    )
    .unwrap();

    KiroIntegration.install(&ctx).unwrap();
    assert_eq!(read_json(&agent_path), custom_agent);
    assert!(
        !home.join(".kiro/settings/cli.json").exists(),
        "install should not point defaultAgent at a user-managed tokensave agent"
    );

    KiroIntegration.uninstall(&ctx).unwrap();
    assert_eq!(read_json(&agent_path), custom_agent);
}

#[test]
fn test_install_preserves_existing_custom_default_agent_choice() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    let cli_path = home.join(".kiro/settings/cli.json");
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(
        &cli_path,
        r#"{"chat":{"defaultAgent":"my-team-agent"},"telemetry":{"enabled":false}}"#,
    )
    .unwrap();

    KiroIntegration.install(&ctx).unwrap();

    let cli = read_json(&cli_path);
    assert_eq!(cli["chat"]["defaultAgent"].as_str(), Some("my-team-agent"));
    assert_eq!(cli["telemetry"]["enabled"].as_bool(), Some(false));
    assert!(home.join(".kiro/agents/tokensave.json").exists());
}

#[test]
fn test_install_replaces_builtin_default_agent_choice() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    let cli_path = home.join(".kiro/settings/cli.json");
    std::fs::create_dir_all(cli_path.parent().unwrap()).unwrap();
    std::fs::write(&cli_path, r#"{"chat":{"defaultAgent":"kiro_default"}}"#).unwrap();

    KiroIntegration.install(&ctx).unwrap();

    let cli = read_json(&cli_path);
    assert_eq!(cli["chat"]["defaultAgent"].as_str(), Some("tokensave"));
}

#[test]
fn test_has_tokensave_tracks_global_mcp_entry() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    assert!(!KiroIntegration.has_tokensave(home));

    KiroIntegration.install(&ctx).unwrap();
    assert!(KiroIntegration.has_tokensave(home));

    KiroIntegration.uninstall(&ctx).unwrap();
    assert!(!KiroIntegration.has_tokensave(home));
}

#[test]
fn test_healthcheck_clean_install_has_no_issues_or_warnings() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let ctx = make_ctx(home);

    KiroIntegration.install(&ctx).unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    KiroIntegration.healthcheck(&mut dc, &hctx);

    assert_eq!(dc.issues, 0, "clean Kiro install should have no issues");
    assert_eq!(dc.warnings, 0, "clean Kiro install should have no warnings");
}

#[test]
fn test_healthcheck_fails_when_workspace_mcp_disables_tokensave() {
    let home_dir = TempDir::new().unwrap();
    let project_dir = TempDir::new().unwrap();
    let home = home_dir.path();
    let project = project_dir.path();
    let ctx = make_ctx(home);

    KiroIntegration.install(&ctx).unwrap();

    let workspace_mcp_path = project.join(".kiro/settings/mcp.json");
    std::fs::create_dir_all(workspace_mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &workspace_mcp_path,
        r#"{"mcpServers":{"tokensave":{"command":"/usr/local/bin/tokensave","args":["serve"],"disabled":true}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: project.to_path_buf(),
    };
    KiroIntegration.healthcheck(&mut dc, &hctx);

    assert!(
        dc.issues > 0,
        "workspace Kiro MCP override that disables tokensave should be unhealthy"
    );
}

#[test]
fn test_healthcheck_fails_when_workspace_mcp_shadows_global_command() {
    let home_dir = TempDir::new().unwrap();
    let project_dir = TempDir::new().unwrap();
    let home = home_dir.path();
    let project = project_dir.path();
    let ctx = make_ctx(home);

    KiroIntegration.install(&ctx).unwrap();

    let workspace_mcp_path = project.join(".kiro/settings/mcp.json");
    std::fs::create_dir_all(workspace_mcp_path.parent().unwrap()).unwrap();
    std::fs::write(
        &workspace_mcp_path,
        r#"{"mcpServers":{"tokensave":{"command":"other-tokensave","args":["serve"],"disabled":false}}}"#,
    )
    .unwrap();

    let mut dc = DoctorCounters::new();
    let hctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: project.to_path_buf(),
    };
    KiroIntegration.healthcheck(&mut dc, &hctx);

    assert!(
        dc.issues > 0,
        "workspace Kiro MCP override with a different command should be unhealthy"
    );
}

fn mutating_tool_names() -> Vec<String> {
    let read_only: std::collections::HashSet<String> = read_only_tool_names().into_iter().collect();
    tool_names()
        .into_iter()
        .filter(|name| !read_only.contains(name))
        .collect()
}
