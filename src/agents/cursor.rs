//! Cursor agent integration.
//!
//! Handles registration of the tokensave MCP server in Cursor's
//! `~/.cursor/mcp.json` under the `mcpServers.tokensave` key.

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_config_file, load_json_file, load_json_file_strict, safe_write_json_file,
    AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
};

/// Cursor agent.
pub struct CursorIntegration;

impl AgentIntegration for CursorIntegration {
    fn name(&self) -> &'static str {
        "Cursor"
    }

    fn id(&self) -> &'static str {
        "cursor"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = ctx.home.join(".cursor/mcp.json");

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
        settings["mcpServers"]["tokensave"] = json!({
            "command": ctx.tokensave_bin,
            "args": ["serve"]
        });

        safe_write_json_file(&mcp_path, &settings, backup.as_deref())?;
        eprintln!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            mcp_path.display()
        );

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave sync");
        eprintln!("  2. Restart Cursor — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = ctx.home.join(".cursor/mcp.json");
        uninstall_mcp_server(&mcp_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Cursor.");
        eprintln!("Restart Cursor for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mCursor integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".cursor").is_dir()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = home.join(".cursor/mcp.json");
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

/// Remove MCP server entry from ~/.cursor/mcp.json.
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
        eprintln!("  No tokensave MCP server in {}, skipping", mcp_path.display());
        return;
    };

    if servers.remove("tokensave").is_none() {
        eprintln!("  No tokensave MCP server in {}, skipping", mcp_path.display());
        return;
    }

    let is_empty = settings.as_object().is_some_and(|o| {
        o.iter().all(|(k, v)| {
            k == "mcpServers" && v.as_object().is_some_and(|m| m.is_empty())
        })
    });

    if is_empty {
        std::fs::remove_file(mcp_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else {
        let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
        std::fs::write(mcp_path, format!("{pretty}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check ~/.cursor/mcp.json has tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = home.join(".cursor/mcp.json");

    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent cursor` if you use Cursor",
            mcp_path.display()
        ));
        return;
    }

    let settings = load_json_file(&mcp_path);
    let server = settings
        .get("mcpServers")
        .and_then(|v| v.get("tokensave"));

    if server.and_then(|v| v.as_object()).is_some() {
        dc.pass(&format!(
            "MCP server registered in {}",
            mcp_path.display()
        ));
    } else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent cursor`",
            mcp_path.display()
        ));
    }
}
