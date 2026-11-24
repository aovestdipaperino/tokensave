//! #1067: `tokensave install` pre-approved every tokensave tool, write tools
//! included, in each agent that supports approvals, and offered no way to opt
//! out. `manage_permissions = false` in `~/.tokensave/config.toml` hands the
//! approval settings to the user: install, reinstall and uninstall leave them
//! as found, and doctor reports them as user-managed.
//!
//! Every opt-out test has a negative control that runs the same install
//! without the setting and fails if the grant is *not* written, so the tests
//! cannot pass by the install silently doing nothing.

use std::path::Path;

use tempfile::TempDir;
use tokensave::agents::constants::MANAGE_PERMISSIONS_KEY;
use tokensave::agents::{
    AgentIntegration, ClaudeIntegration, CodexIntegration, DoctorCounters, GeminiIntegration,
    HealthcheckContext, KiroIntegration, QwenIntegration,
};

use crate::common::{make_install_ctx, read_json};

/// Writes `manage_permissions = false` to `home`'s tokensave config.
fn opt_out(home: &Path) {
    let dir = home.join(".tokensave");
    std::fs::create_dir_all(&dir).expect("create .tokensave");
    std::fs::write(
        dir.join("config.toml"),
        format!("{MANAGE_PERMISSIONS_KEY} = false\n"),
    )
    .expect("write config.toml");
}

fn write_json(path: &Path, value: &serde_json::Value) {
    std::fs::create_dir_all(path.parent().expect("parent dir")).expect("create parent");
    let text = serde_json::to_string_pretty(value).expect("serialize json");
    std::fs::write(path, format!("{text}\n")).expect("write json");
}

fn claude_allow(home: &Path) -> Vec<String> {
    read_json(&home.join(".claude").join("settings.json"))["permissions"]["allow"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn doctor(integration: &dyn AgentIntegration, home: &Path) -> DoctorCounters {
    let mut dc = DoctorCounters::new();
    let ctx = HealthcheckContext {
        home: home.to_path_buf(),
        project_path: home.to_path_buf(),
    };
    integration.healthcheck(&mut dc, &ctx);
    dc
}

// ---------------------------------------------------------------------------
// Claude Code
// ---------------------------------------------------------------------------

#[test]
fn claude_install_writes_no_grant_when_user_managed() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    opt_out(home);
    ClaudeIntegration
        .install(&make_install_ctx(home))
        .expect("install");

    let allow = claude_allow(home);
    assert!(
        !allow.iter().any(|p| p.starts_with("mcp__tokensave")),
        "manage_permissions = false must not grant tools, got: {allow:?}"
    );
    assert!(
        read_json(&home.join(".claude.json"))["mcpServers"]["tokensave"].is_object(),
        "the MCP server must still be registered"
    );
}

#[test]
fn claude_install_grants_tools_by_default() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    ClaudeIntegration
        .install(&make_install_ctx(home))
        .expect("install");
    assert!(
        claude_allow(home)
            .iter()
            .any(|p| p == "mcp__tokensave__tokensave_search"),
        "negative control: the default install should grant read-only tools"
    );
}

#[test]
fn claude_user_managed_grant_survives_reinstall_and_uninstall() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    let settings_path = home.join(".claude").join("settings.json");
    let own = vec![
        "Bash(git status)".to_string(),
        "mcp__tokensave__tokensave_search".to_string(),
    ];
    write_json(
        &settings_path,
        &serde_json::json!({ "permissions": { "allow": own } }),
    );
    opt_out(home);

    let ctx = make_install_ctx(home);
    ClaudeIntegration.install(&ctx).expect("install");
    ClaudeIntegration.install(&ctx).expect("reinstall");
    assert_eq!(claude_allow(home), own, "install must keep the user's list");

    ClaudeIntegration.uninstall(&ctx).expect("uninstall");
    assert_eq!(
        claude_allow(home),
        own,
        "uninstall must not remove grants the user manages"
    );
}

#[test]
fn claude_doctor_reports_user_managed_instead_of_missing() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    opt_out(home);
    ClaudeIntegration
        .install(&make_install_ctx(home))
        .expect("install");
    let user_managed = doctor(&ClaudeIntegration, home);

    // Negative control: the same install state with management on is a
    // doctor failure (every permission missing).
    std::fs::remove_file(home.join(".tokensave").join("config.toml")).expect("remove config");
    let managed = doctor(&ClaudeIntegration, home);

    assert!(
        managed.issues > user_managed.issues,
        "missing grants must only be flagged while tokensave manages them \
         (managed: {}, user-managed: {})",
        managed.issues,
        user_managed.issues
    );
}

// ---------------------------------------------------------------------------
// Codex CLI
// ---------------------------------------------------------------------------

fn codex_tools(home: &Path) -> Option<toml::Value> {
    let text = std::fs::read_to_string(home.join(".codex").join("config.toml")).ok()?;
    let config: toml::Value = toml::from_str(&text).ok()?;
    config
        .get("mcp_servers")?
        .get("tokensave")?
        .get("tools")
        .cloned()
}

#[test]
fn codex_install_writes_no_approvals_when_user_managed() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    opt_out(home);
    CodexIntegration
        .install(&make_install_ctx(home))
        .expect("install");
    assert!(
        codex_tools(home).is_none(),
        "manage_permissions = false must not write approval_mode"
    );
}

#[test]
fn codex_install_approves_tools_by_default() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    CodexIntegration
        .install(&make_install_ctx(home))
        .expect("install");
    assert!(
        codex_tools(home).is_some(),
        "negative control: the default install should auto-approve tools"
    );
}

#[test]
fn codex_reinstall_keeps_user_approvals() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    let config_path = home.join(".codex").join("config.toml");
    std::fs::create_dir_all(config_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &config_path,
        "[mcp_servers.tokensave]\ncommand = \"tokensave\"\nargs = [\"serve\"]\n\n\
         [mcp_servers.tokensave.tools.tokensave_search]\napproval_mode = \"auto\"\n",
    )
    .expect("write codex config");
    opt_out(home);

    CodexIntegration
        .install(&make_install_ctx(home))
        .expect("reinstall");
    let tools = codex_tools(home).expect("user approvals kept");
    let table = tools.as_table().expect("tools table");
    assert_eq!(table.len(), 1, "no approval added: {table:?}");
    assert!(table.contains_key("tokensave_search"));
}

// ---------------------------------------------------------------------------
// Gemini CLI and Qwen Code
// ---------------------------------------------------------------------------

fn trust_after_install(
    integration: &dyn AgentIntegration,
    home: &Path,
    dir: &str,
) -> serde_json::Value {
    integration
        .install(&make_install_ctx(home))
        .expect("install");
    read_json(&home.join(dir).join("settings.json"))["mcpServers"]["tokensave"]["trust"].clone()
}

#[test]
fn gemini_and_qwen_install_omit_trust_when_user_managed() {
    for (integration, dir) in [
        (&GeminiIntegration as &dyn AgentIntegration, ".gemini"),
        (&QwenIntegration as &dyn AgentIntegration, ".qwen"),
    ] {
        let temp = TempDir::new().expect("temp home");
        opt_out(temp.path());
        assert!(
            trust_after_install(integration, temp.path(), dir).is_null(),
            "{dir}: manage_permissions = false must not write trust"
        );
    }
}

#[test]
fn gemini_and_qwen_leave_trust_unset_by_default() {
    for (integration, dir) in [
        (&GeminiIntegration as &dyn AgentIntegration, ".gemini"),
        (&QwenIntegration as &dyn AgentIntegration, ".qwen"),
    ] {
        let temp = TempDir::new().expect("temp home");
        assert_eq!(
            trust_after_install(integration, temp.path(), dir),
            serde_json::Value::Null,
            "{dir}: negative control: the default install should leave trust unset"
        );
    }
}

#[test]
fn gemini_reinstall_keeps_user_trust_choice() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    write_json(
        &home.join(".gemini").join("settings.json"),
        &serde_json::json!({
            "mcpServers": { "tokensave": { "command": "tokensave", "args": ["serve"], "trust": false } }
        }),
    );
    opt_out(home);
    assert_eq!(
        trust_after_install(&GeminiIntegration, home, ".gemini"),
        serde_json::Value::Bool(false),
        "reinstall must keep the user's trust value"
    );
}

// ---------------------------------------------------------------------------
// Kiro
// ---------------------------------------------------------------------------

fn kiro_allowed_tools(home: &Path) -> serde_json::Value {
    read_json(&home.join(".kiro").join("agents").join("tokensave.json"))["allowedTools"].clone()
}

#[test]
fn kiro_agent_preapproves_nothing_when_user_managed() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    opt_out(home);
    KiroIntegration
        .install(&make_install_ctx(home))
        .expect("install");
    assert!(
        kiro_allowed_tools(home).is_null(),
        "manage_permissions = false must not write allowedTools"
    );
}

#[test]
fn kiro_agent_preapproves_tools_by_default() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    KiroIntegration
        .install(&make_install_ctx(home))
        .expect("install");
    assert!(
        kiro_allowed_tools(home).is_array(),
        "negative control: the default install should write allowedTools"
    );
}

#[test]
fn kiro_reinstall_keeps_user_allowed_tools() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    let ctx = make_install_ctx(home);
    KiroIntegration.install(&ctx).expect("install");

    // The user narrows the approvals in tokensave's own agent file, then
    // opts out so reinstall keeps them.
    let agent_path = home.join(".kiro").join("agents").join("tokensave.json");
    let mut agent = read_json(&agent_path);
    agent["allowedTools"] = serde_json::json!(["@tokensave/tokensave_search"]);
    write_json(&agent_path, &agent);
    opt_out(home);

    KiroIntegration.install(&ctx).expect("reinstall");
    assert_eq!(
        kiro_allowed_tools(home),
        serde_json::json!(["@tokensave/tokensave_search"]),
        "reinstall must keep the user's allowedTools"
    );
}

#[test]
fn user_owned_trust_survives_uninstall() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    opt_out(home);
    let ctx = make_install_ctx(home);
    GeminiIntegration.install(&ctx).expect("install");
    let path = home.join(".gemini").join("settings.json");
    let mut config = read_json(&path);
    config["mcpServers"]["tokensave"]["trust"] = serde_json::json!(true);
    std::fs::write(&path, serde_json::to_vec(&config).expect("serialize")).expect("write settings");
    let mut plan = tokensave::agents::uninstall::UninstallPlan::default();
    plan.plan_agent(&GeminiIntegration, &ctx);
    let report = plan.apply(tokensave::agents::uninstall::UninstallOutput::default());
    assert_eq!(report.failed, 0);
    assert_eq!(read_json(&path)["mcpServers"]["tokensave"]["trust"], true);
}

#[test]
fn codex_reinstall_preserves_exact_opt_in_without_adding_other_writes() {
    let dir = TempDir::new().expect("temp home");
    let home = dir.path();
    let mut ctx = make_install_ctx(home);
    ctx.tool_permissions =
        tokensave::agents::requested_tool_perms(&["tokensave_diagnostics".into()])
            .expect("exact opt-in");
    CodexIntegration.install(&ctx).expect("install");
    ctx.tool_permissions = tokensave::agents::expected_tool_perms();
    CodexIntegration.install(&ctx).expect("reinstall");
    let path = home.join(".codex").join("config.toml");
    let config = std::fs::read_to_string(&path)
        .expect("config")
        .parse::<toml::Table>()
        .expect("TOML");
    let tools = &config["mcp_servers"]["tokensave"]["tools"];
    assert_eq!(
        tools["tokensave_diagnostics"]["approval_mode"].as_str(),
        Some("auto")
    );
    assert!(tools.get("tokensave_str_replace").is_none());
    assert_eq!(
        tools["tokensave_search"]["approval_mode"].as_str(),
        Some("auto")
    );
}

#[test]
fn unknown_and_wildcard_write_opt_ins_are_rejected() {
    for tool in ["tokensave_new_tool", "tokensave_*", "tokensave_search"] {
        assert!(tokensave::agents::requested_tool_perms(&[tool.into()]).is_err());
    }
}
