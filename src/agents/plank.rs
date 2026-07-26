// Rust guideline compliant 2026-07-26
//! Plank coding agent integration.
//!
//! Handles registration of the tokensave MCP server in Plank's MCP config
//! under the `mcpServers.tokensave` key. Plank uses the standard MCP JSON
//! shape, so the server entry matches the other JSON-based integrations
//! (Cursor, Cline, Pi): `{ "command", "args": ["serve"] }`.
//!
//! Plank's configs are hierarchical, and the two scopes live at *different*
//! relative paths — `~/.plank/.mcp.json` for the user scope, `./.mcp.json` at
//! the project root for the project scope — so the path helper matches on the
//! install scope instead of using `InstallContext::base_dir`.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    InstallScope,
};

/// Plank coding agent.
pub struct PlankIntegration;

/// Returns Plank's user-scope MCP config path (`<home>/.plank/.mcp.json`).
fn plank_global_config_path(home: &Path) -> PathBuf {
    home.join(".plank/.mcp.json")
}

/// Returns Plank's project-scope MCP config path (`<project>/.mcp.json`).
fn plank_local_config_path(project_path: &Path) -> PathBuf {
    project_path.join(".mcp.json")
}

/// Returns the config path this install/uninstall run targets.
fn plank_config_path(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => plank_global_config_path(&ctx.home),
        InstallScope::Local { project_path } => plank_local_config_path(project_path),
    }
}

impl AgentIntegration for PlankIntegration {
    fn name(&self) -> &'static str {
        "Plank"
    }

    fn id(&self) -> &'static str {
        "plank"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = plank_config_path(ctx);

        if let Some(parent) = mcp_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let backup = backup_config_file(&mcp_path)?;
        let mut settings = match load_json_file_strict(&mcp_path) {
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
            &ctx.tokensave_bin,
        );
        settings["mcpServers"]["tokensave"] = json!({
            "command": bin,
            "args": ["serve"]
        });

        safe_write_json_file(&mcp_path, &settings, backup.as_deref())?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            mcp_path.display()
        );

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Restart plank — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = plank_config_path(ctx);
        uninstall_mcp_server(&mcp_path);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Plank.");
        crate::agent_note!("Restart plank for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mPlank integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".plank").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(plank_global_config_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = plank_global_config_path(home);
        if !mcp_path.exists() {
            return false;
        }
        let json = load_json_file(&mcp_path);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove the tokensave MCP server entry from Plank's `.mcp.json`.
fn uninstall_mcp_server(mcp_path: &Path) {
    if !mcp_path.exists() {
        crate::agent_note!("  {} not found, skipping", mcp_path.display());
        return;
    }

    let Ok(contents) = std::fs::read_to_string(mcp_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };

    let Some(servers) = settings
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    else {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return;
    };

    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return;
    }

    let is_empty = settings.as_object().is_some_and(|o| {
        o.iter()
            .all(|(k, v)| k == "mcpServers" && v.as_object().is_some_and(serde_json::Map::is_empty))
    });

    if is_empty {
        std::fs::remove_file(mcp_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else if backup_and_write_json(mcp_path, &settings) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check Plank's user-scope `.mcp.json` has the tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = plank_global_config_path(home);

    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent plank` if you use Plank",
            mcp_path.display()
        ));
        return;
    }

    let settings = load_json_file(&mcp_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    if server.and_then(|v| v.as_object()).is_some() {
        dc.pass(&format!("MCP server registered in {}", mcp_path.display()));
    } else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent plank`",
            mcp_path.display()
        ));
    }
}
