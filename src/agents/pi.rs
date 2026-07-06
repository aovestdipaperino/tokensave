// Rust guideline compliant 2026-06-16
//! Pi coding agent integration.
//!
//! Handles registration of the tokensave MCP server in Pi's MCP config
//! (`$PI_CODING_AGENT_DIR/mcp.json`, or `$HOME/.pi/agent/mcp.json` when the
//! env var is unset) under the `mcpServers.tokensave` key. Pi uses the
//! standard MCP JSON shape, so the server entry matches the other
//! JSON-based integrations (Cursor, Cline): `{ "command", "args": ["serve"] }`.
//!
//! Pi has no documented project-scoped config, so `--local` is not supported.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
};

/// Pi coding agent.
pub struct PiIntegration;

/// Returns the Pi MCP config path.
///
/// Honors `$PI_CODING_AGENT_DIR` when set (`$PI_CODING_AGENT_DIR/mcp.json`),
/// otherwise falls back to `<home>/.pi/agent/mcp.json`.
fn pi_config_path(home: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir).join("mcp.json");
        }
    }
    home.join(".pi/agent/mcp.json")
}

impl AgentIntegration for PiIntegration {
    fn name(&self) -> &'static str {
        "Pi"
    }

    fn id(&self) -> &'static str {
        "pi"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = pi_config_path(&ctx.home);

        if let Some(parent) = mcp_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let backup = backup_config_file(&mcp_path)?;
        let mut settings = match load_json_file_strict(&mcp_path) {
            Ok(v) => v,
            Err(e) => {
                if let Some(ref b) = backup {
                    eprintln!("  Backup preserved at: {}", b.display());
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
        eprintln!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            mcp_path.display()
        );

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Restart Pi — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = pi_config_path(&ctx.home);
        uninstall_mcp_server(&mcp_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Pi.");
        eprintln!("Restart Pi for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mPi integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        let mcp_path = pi_config_path(home);
        mcp_path.exists() || mcp_path.parent().is_some_and(Path::is_dir)
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(pi_config_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = pi_config_path(home);
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

/// Remove the tokensave MCP server entry from Pi's `mcp.json`.
fn uninstall_mcp_server(mcp_path: &Path) {
    if !mcp_path.exists() {
        eprintln!("  {} not found, skipping", mcp_path.display());
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
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return;
    };

    if servers.remove("tokensave").is_none() {
        eprintln!(
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
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else if backup_and_write_json(mcp_path, &settings) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check Pi's `mcp.json` has the tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = pi_config_path(home);

    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent pi` if you use Pi",
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
            "MCP server NOT registered in {} — run `tokensave install --agent pi`",
            mcp_path.display()
        ));
    }
}
