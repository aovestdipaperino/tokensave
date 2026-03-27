//! GitHub Copilot (VS Code) agent integration.
//!
//! Handles registration of the tokensave MCP server in VS Code's
//! `settings.json` under the `mcp.servers.tokensave` key.

use std::path::Path;

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{load_jsonc_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext};

/// GitHub Copilot (VS Code) agent.
pub struct CopilotIntegration;

impl AgentIntegration for CopilotIntegration {
    fn name(&self) -> &'static str {
        "GitHub Copilot (VS Code)"
    }

    fn id(&self) -> &'static str {
        "copilot"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let settings_path = super::vscode_data_dir(&ctx.home).join("User/settings.json");

        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let mut settings = load_jsonc_file(&settings_path);
        settings["mcp"]["servers"]["tokensave"] = json!({
            "type": "stdio",
            "command": ctx.tokensave_bin,
            "args": ["serve"]
        });

        let pretty = serde_json::to_string_pretty(&settings).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(&settings_path, format!("{pretty}\n")).map_err(|e| TokenSaveError::Config {
            message: format!("failed to write {}: {e}", settings_path.display()),
        })?;
        eprintln!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            settings_path.display()
        );

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave sync");
        eprintln!("  2. Restart VS Code — tokensave tools are now available in GitHub Copilot");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let settings_path = super::vscode_data_dir(&ctx.home).join("User/settings.json");
        uninstall_mcp_server(&settings_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from GitHub Copilot (VS Code).");
        eprintln!("Restart VS Code for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mGitHub Copilot (VS Code) integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        super::vscode_data_dir(home).join("User").is_dir()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let settings_path = super::vscode_data_dir(home).join("User/settings.json");
        if !settings_path.exists() {
            return false;
        }
        let json = load_jsonc_file(&settings_path);
        json.get("mcp")
            .and_then(|v| v.get("servers"))
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server entry from VS Code settings.json.
/// Does not delete the file even if the object becomes empty (other VS Code
/// settings may still exist).
fn uninstall_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        eprintln!(
            "  {} not found, skipping",
            settings_path.display()
        );
        return;
    }

    let mut settings = load_jsonc_file(settings_path);

    // Remove mcpServers.tokensave
    let removed = settings
        .get_mut("mcp")
        .and_then(|mcp| mcp.get_mut("servers"))
        .and_then(|servers| servers.as_object_mut())
        .and_then(|map| map.remove("tokensave"))
        .is_some();

    if !removed {
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }

    // Clean up empty "servers" object
    if let Some(mcp) = settings.get_mut("mcp") {
        let servers_empty = mcp
            .get("servers")
            .and_then(|v| v.as_object())
            .is_some_and(|o| o.is_empty());
        if servers_empty {
            mcp.as_object_mut().map(|o| o.remove("servers"));
        }

        // Clean up empty "mcp" object
        let mcp_empty = settings
            .get("mcp")
            .and_then(|v| v.as_object())
            .is_some_and(|o| o.is_empty());
        if mcp_empty {
            settings.as_object_mut().map(|o| o.remove("mcp"));
        }
    }

    // Always write back (never delete settings.json — it has other VS Code settings)
    let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
    std::fs::write(settings_path, format!("{pretty}\n")).ok();
    eprintln!(
        "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
        settings_path.display()
    );
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check VS Code settings.json has tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = super::vscode_data_dir(home).join("User/settings.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use GitHub Copilot",
            settings_path.display()
        ));
        return;
    }

    let settings = load_jsonc_file(&settings_path);
    let server = settings
        .get("mcp")
        .and_then(|v| v.get("servers"))
        .and_then(|v| v.get("tokensave"));

    let Some(server) = server.and_then(|v| v.as_object()) else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent copilot`",
            settings_path.display()
        ));
        return;
    };
    dc.pass(&format!(
        "MCP server registered in {}",
        settings_path.display()
    ));

    // Check args include "serve"
    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent copilot`");
    }
}
