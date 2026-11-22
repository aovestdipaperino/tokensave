// Rust guideline compliant 2025-10-17
//! Qwen Code agent integration.
//!
//! Qwen Code is an open-source coding CLI forked from Gemini CLI, so it shares
//! Gemini's configuration shape: an MCP server registry in
//! `~/.qwen/settings.json` and prompt rules in `~/.qwen/QWEN.md`. Qwen Code has
//! no hook system; tool auto-approval is handled via the `trust: true` flag on
//! the MCP server entry.

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::*;
/// Qwen Code agent.
pub struct QwenIntegration;

impl AgentIntegration for QwenIntegration {
    fn name(&self) -> &'static str {
        "Qwen Code"
    }

    fn id(&self) -> &'static str {
        "qwen"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let qwen_dir = ctx.base_dir().join(".qwen");
        std::fs::create_dir_all(&qwen_dir).ok();
        let settings_path = qwen_dir.join("settings.json");

        install_mcp_server(&settings_path, &ctx.tokensave_bin, ctx)?;

        let qwen_md = qwen_dir.join("QWEN.md");
        install_prompt_rules(&qwen_md)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!(
            "  2. Start a new Qwen Code session — tokensave tools are now available"
        );
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let qwen_dir = ctx.base_dir().join(".qwen");
        let settings_path = qwen_dir.join("settings.json");

        if has_user_trust(&settings_path, ctx) {
            crate::agent_note!("  MCP entry kept because it contains user-managed trust.");
        } else {
            uninstall_mcp_server(&settings_path);
        }

        let qwen_md = qwen_dir.join("QWEN.md");
        uninstall_prompt_rules(&qwen_md);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Qwen Code.");
        crate::agent_note!("Start a new Qwen Code session for changes to take effect.");
        Ok(())
    }

    fn plan_uninstall(
        &self,
        ctx: &InstallContext,
        plan: &mut crate::agents::uninstall::UninstallPlan,
    ) -> Result<()> {
        let qwen_dir = ctx.base_dir().join(".qwen");
        let settings_path = qwen_dir.join("settings.json");

        if has_user_trust(&settings_path, ctx) {
            plan.skipped("MCP server", "contains user-managed trust");
        } else {
            plan.mcp(&settings_path, &["mcpServers"], false);
        }

        let qwen_md = qwen_dir.join("QWEN.md");
        plan.rules(&qwen_md, crate::agents::rules::LEGACY_RULES_MARKER, &[]);
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mQwen Code integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home, permissions_management_enabled(&ctx.home));
        doctor_check_prompt(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".qwen").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".qwen").join("settings.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let settings = home.join(".qwen").join("settings.json");
        if !settings.exists() {
            return false;
        }
        let json = super::load_json_file(&settings);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

fn has_user_trust(path: &Path, ctx: &InstallContext) -> bool {
    !permissions_management_enabled(&ctx.home)
        && load_json_file(path)
            .pointer("/mcpServers/tokensave/trust")
            .is_some()
}

/// Register MCP server in ~/.qwen/settings.json.
///
/// With `manage_permissions` false (#1067) the `trust` flag is the user's:
/// an existing value is carried over unchanged and none is added.
fn install_mcp_server(
    settings_path: &Path,
    tokensave_bin: &str,
    ctx: &InstallContext,
) -> Result<()> {
    if crate::agents::write_approvals_requested(&ctx.tool_permissions) {
        crate::agent_note!("  This integration uses whole-server trust. Exact write-tool approvals were not applied; use the agent's interactive prompts.");
    }
    let backup = backup_config_file(settings_path)?;
    let mut settings = match load_json_file_strict(settings_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                crate::agent_note!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };

    let bin = crate::agents::preserve_mcp_command(
        settings.pointer("/mcpServers/tokensave/command"),
        tokensave_bin,
    );
    let mut entry = settings
        .pointer("/mcpServers/tokensave")
        .filter(|entry| entry.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    entry["command"] = json!(bin);
    entry["args"] = json!(["serve"]);
    if permissions_management_enabled(&ctx.home) {
        if ctx.force_permission_style {
            if let Some(entry) = entry.as_object_mut() {
                entry.remove("trust");
            }
        } else if entry["trust"].as_bool() == Some(true) {
            crate::agent_note!("  Existing whole-server trust kept. Run tokensave install --agent qwen --reset-tool-approvals to restore prompts.");
        }
    } else {
        crate::agent_note!("{PERMISSIONS_USER_MANAGED_NOTE}");
    }
    settings["mcpServers"]["tokensave"] = entry;

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// Write or refresh the tokensave rules block in QWEN.md.
fn install_prompt_rules(qwen_md: &Path) -> Result<()> {
    let body = rules_for_agent("qwen")?;
    write_rules_block(qwen_md, "qwen", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.qwen/settings.json.
fn uninstall_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(settings_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = settings
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }
    if servers.is_empty() {
        settings.as_object_mut().map(|o| o.remove("mcpServers"));
    }
    let is_empty = settings.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(settings_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            settings_path.display()
        );
    } else if backup_and_write_json(settings_path, &settings) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            settings_path.display()
        );
    }
}

/// Remove tokensave rules from QWEN.md (both managed-block and
/// legacy heading-guarded forms).
fn uninstall_prompt_rules(qwen_md: &Path) {
    remove_rules_block(qwen_md).ok();
    remove_legacy_rules_block(qwen_md, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check settings.json has tokensave registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path, manage_permissions: bool) {
    let settings_path = home.join(".qwen").join("settings.json");
    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent qwen` if you use Qwen Code",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    let Some(server) = server.and_then(|v| v.as_object()) else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent qwen`",
            settings_path.display()
        ));
        return;
    };
    dc.pass(&format!(
        "MCP server registered in {}",
        settings_path.display()
    ));

    // Check command includes "serve"
    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent qwen`");
    }

    if !manage_permissions {
        dc.pass(PERMISSIONS_USER_MANAGED_DOCTOR);
        return;
    }

    // Check trust flag
    let is_trusted = server
        .get("trust")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if is_trusted {
        dc.warn("Existing server-wide trust approves write tools. Run tokensave install --agent qwen --reset-tool-approvals to restore prompts.");
    } else {
        dc.pass("MCP server missing trust: true — Qwen Code will prompt for each tool call");
    }
}

/// Check QWEN.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let qwen_md = home.join(".qwen").join("QWEN.md");
    check_shared_rules_block(dc, &qwen_md, "qwen");
}
