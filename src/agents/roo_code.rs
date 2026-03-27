//! Roo Code agent integration.
//!
//! Handles registration of the tokensave MCP server in Roo Code's
//! `cline_mcp_settings.json` under the `mcpServers.tokensave` key.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{load_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext};

/// Roo Code agent.
pub struct RooCodeIntegration;

/// Returns the Roo Code VS Code extension global storage directory.
fn roo_ext_dir(home: &Path) -> PathBuf {
    super::vscode_data_dir(home)
        .join("User/globalStorage/rooveterinaryinc.roo-cline")
}

impl AgentIntegration for RooCodeIntegration {
    fn name(&self) -> &'static str {
        "Roo Code"
    }

    fn id(&self) -> &'static str {
        "roo-code"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let settings_path = roo_ext_dir(&ctx.home).join("settings/cline_mcp_settings.json");

        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let mut settings = load_json_file(&settings_path);
        settings["mcpServers"]["tokensave"] = json!({
            "command": ctx.tokensave_bin,
            "args": ["serve"],
            "disabled": false
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
        eprintln!("  2. Restart VS Code — tokensave tools are now available in Roo Code");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let settings_path = roo_ext_dir(&ctx.home).join("settings/cline_mcp_settings.json");
        uninstall_mcp_server(&settings_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Roo Code.");
        eprintln!("Restart VS Code for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mRoo Code integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        roo_ext_dir(home).is_dir()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let settings_path = roo_ext_dir(home).join("settings/cline_mcp_settings.json");
        if !settings_path.exists() {
            return false;
        }
        let json = load_json_file(&settings_path);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server entry from Roo Code's cline_mcp_settings.json.
fn uninstall_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        eprintln!("  {} not found, skipping", settings_path.display());
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
        eprintln!("  No tokensave MCP server in {}, skipping", settings_path.display());
        return;
    };

    if servers.remove("tokensave").is_none() {
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }

    let is_empty = settings.as_object().is_some_and(|o| {
        o.iter().all(|(k, v)| {
            k == "mcpServers" && v.as_object().is_some_and(|m| m.is_empty())
        })
    });

    if is_empty {
        std::fs::remove_file(settings_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            settings_path.display()
        );
    } else {
        let pretty = serde_json::to_string_pretty(&settings).unwrap_or_default();
        std::fs::write(settings_path, format!("{pretty}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            settings_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check Roo Code's cline_mcp_settings.json has tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = roo_ext_dir(home).join("settings/cline_mcp_settings.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent roo-code` if you use Roo Code",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings
        .get("mcpServers")
        .and_then(|v| v.get("tokensave"));

    if server.and_then(|v| v.as_object()).is_some() {
        dc.pass(&format!(
            "MCP server registered in {}",
            settings_path.display()
        ));
    } else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent roo-code`",
            settings_path.display()
        ));
    }
}
