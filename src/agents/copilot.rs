//! GitHub Copilot integration.
//!
//! Handles registration of the tokensave MCP server in both:
//! - VS Code's `settings.json` under `mcp.servers.tokensave`
//! - Copilot CLI's `~/.copilot/mcp-config.json` under `mcpServers.tokensave`

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_config_file, load_json_file, load_json_file_strict, load_jsonc_file,
    load_jsonc_file_strict, safe_write_json_file, AgentIntegration, DoctorCounters,
    HealthcheckContext, InstallContext,
};

/// GitHub Copilot agent.
pub struct CopilotIntegration;

impl AgentIntegration for CopilotIntegration {
    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }

    fn id(&self) -> &'static str {
        "copilot"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let vscode_settings_path = super::vscode_data_dir(&ctx.home).join("User/settings.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");

        install_vscode_mcp_server(&vscode_settings_path, &ctx.tokensave_bin)?;
        install_cli_mcp_server(&cli_settings_path, &ctx.tokensave_bin)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Restart VS Code and/or start a new Copilot CLI session");
        eprintln!("     tokensave tools are now available in GitHub Copilot");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let vscode_settings_path = super::vscode_data_dir(&ctx.home).join("User/settings.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");
        uninstall_vscode_mcp_server(&vscode_settings_path);
        uninstall_cli_mcp_server(&cli_settings_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from GitHub Copilot.");
        eprintln!(
            "Restart VS Code and/or start a new Copilot CLI session for changes to take effect."
        );
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mGitHub Copilot integration\x1b[0m");
        doctor_check_vscode_settings(dc, &ctx.home);
        doctor_check_cli_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        super::vscode_data_dir(home).join("User").is_dir() || super::copilot_cli_dir(home).is_dir()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let vscode_settings_path = super::vscode_data_dir(home).join("User/settings.json");
        let cli_settings_path = super::copilot_cli_dir(home).join("mcp-config.json");

        let vscode_has_tokensave = if vscode_settings_path.exists() {
            let json = load_jsonc_file(&vscode_settings_path);
            json.get("mcp")
                .and_then(|v| v.get("servers"))
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let cli_has_tokensave = if cli_settings_path.exists() {
            let json = load_json_file(&cli_settings_path);
            json.get("mcpServers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        vscode_has_tokensave || cli_has_tokensave
    }
}

/// Register MCP server in VS Code settings.json.
fn install_vscode_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(settings_path)?;
    let mut settings = match load_jsonc_file_strict(settings_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                eprintln!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };
    settings["mcp"]["servers"]["tokensave"] = json!({
        "type": "stdio",
        "command": tokensave_bin,
        "args": ["serve", "-p", "${workspaceFolder}"],
        "cwd": "${workspaceFolder}"
    });

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// Register MCP server in Copilot CLI's ~/.copilot/mcp-config.json.
fn install_cli_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(settings_path)?;
    let mut settings = match load_json_file_strict(settings_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                eprintln!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };
    settings["mcpServers"]["tokensave"] = json!({
        "type": "stdio",
        "command": tokensave_bin,
        "args": ["serve", "-p", "${workspaceFolder}"],
        "cwd": "${workspaceFolder}"
    });

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server entry from VS Code settings.json.
/// Does not delete the file even if the object becomes empty (other VS Code
/// settings may still exist).
fn uninstall_vscode_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        eprintln!("  {} not found, skipping", settings_path.display());
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
            .is_some_and(serde_json::Map::is_empty);
        if servers_empty {
            mcp.as_object_mut().map(|o| o.remove("servers"));
        }

        // Clean up empty "mcp" object
        let mcp_empty = settings
            .get("mcp")
            .and_then(|v| v.as_object())
            .is_some_and(serde_json::Map::is_empty);
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

/// Remove MCP server entry from Copilot CLI's ~/.copilot/mcp-config.json.
fn uninstall_cli_mcp_server(settings_path: &Path) {
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
        eprintln!(
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

/// Check VS Code settings.json has tokensave MCP server registered.
fn doctor_check_vscode_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = super::vscode_data_dir(home).join("User/settings.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use GitHub Copilot in VS Code",
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

/// Check Copilot CLI mcp-config.json has tokensave MCP server registered.
fn doctor_check_cli_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = super::copilot_cli_dir(home).join("mcp-config.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use Copilot CLI",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

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
