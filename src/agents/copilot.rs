// Rust guideline compliant 2025-10-17
//! GitHub Copilot integration.
//!
//! Handles registration of the tokensave MCP server in:
//! - VS Code's dedicated `User/mcp.json` (1.102+, GA MCP, parsed as JSONC)
//!   under `servers.tokensave`. `install` never writes `settings.json`; a
//!   pre-#266 `mcp.servers.tokensave` entry there is read as a fallback by
//!   `doctor`/`has_tokensave` so those checks don't nag, and it's cleaned up
//!   on `uninstall`. Note that entry isn't inert: VS Code itself actively
//!   migrates it into its own MCP registry (making it the live
//!   registration) and strips the key back out of `settings.json` once
//!   done, so the fallback mostly only matters in the transient window
//!   before that migration runs (see issue #266).
//! - Copilot CLI's `~/.copilot/mcp-config.json` under `mcpServers.tokensave`
//! - `JetBrains` plugin's `~/.config/github-copilot/intellij/mcp.json` under
//!   `servers.tokensave` (only when the plugin's config dir exists)

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    load_jsonc_file, load_jsonc_file_strict, parse_jsonc, safe_write_json_file, AgentIntegration,
    DoctorCounters, HealthcheckContext, InstallContext,
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
        let vscode_mcp_json_path = super::vscode_data_dir(&ctx.home).join("User/mcp.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");

        install_vscode_mcp_json_server(&vscode_mcp_json_path, &ctx.tokensave_bin)?;

        let insiders_mcp_json_path =
            super::vscode_insiders_data_dir(&ctx.home).join("User/mcp.json");
        if insiders_mcp_json_path
            .parent()
            .is_some_and(std::path::Path::exists)
        {
            install_vscode_mcp_json_server(&insiders_mcp_json_path, &ctx.tokensave_bin)?;
        }
        install_cli_mcp_server(&cli_settings_path, &ctx.tokensave_bin)?;

        // JetBrains plugin config lives under ~/.config/github-copilot/intellij.
        // Only install when ~/.config/github-copilot exists (the Copilot plugin
        // creates it on sign-in); otherwise we'd litter homes of non-JetBrains
        // users with an unused config tree.
        let jetbrains_dir = super::copilot_jetbrains_dir(&ctx.home);
        let jetbrains_detected = jetbrains_dir.parent().is_some_and(std::path::Path::exists);
        if jetbrains_detected {
            install_jetbrains_mcp_server(&jetbrains_dir.join("mcp.json"), &ctx.tokensave_bin)?;
        }

        // Install prompt rules
        let vscode_instructions =
            super::vscode_data_dir(&ctx.home).join("User/prompts/copilot-instructions.md");
        install_prompt_rules(&vscode_instructions)?;
        let insiders_instructions =
            super::vscode_insiders_data_dir(&ctx.home).join("User/prompts/copilot-instructions.md");
        if super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .exists()
        {
            install_prompt_rules(&insiders_instructions)?;
        }
        let cli_instructions = super::copilot_cli_dir(&ctx.home).join("copilot-instructions.md");
        install_prompt_rules(&cli_instructions)?;
        if jetbrains_detected {
            // JetBrains reads global instructions from this file, not from the
            // VS Code User/prompts location.
            install_prompt_rules(&jetbrains_dir.join("global-copilot-instructions.md"))?;
        }

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Restart VS Code and/or start a new Copilot CLI session");
        if jetbrains_detected {
            crate::agent_note!(
                "     For JetBrains IDEs, restart the IDE (MCP config is read at startup)"
            );
        }
        crate::agent_note!("     tokensave tools are now available in GitHub Copilot");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let vscode_settings_path = super::vscode_data_dir(&ctx.home).join("User/settings.json");
        let vscode_mcp_json_path = super::vscode_data_dir(&ctx.home).join("User/mcp.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");
        uninstall_vscode_mcp_json_server(&vscode_mcp_json_path);
        uninstall_vscode_mcp_server(&vscode_settings_path);
        let insiders_settings_path =
            super::vscode_insiders_data_dir(&ctx.home).join("User/settings.json");
        let insiders_mcp_json_path =
            super::vscode_insiders_data_dir(&ctx.home).join("User/mcp.json");
        uninstall_vscode_mcp_json_server(&insiders_mcp_json_path);
        uninstall_vscode_mcp_server(&insiders_settings_path);
        uninstall_cli_mcp_server(&cli_settings_path);
        let jetbrains_dir = super::copilot_jetbrains_dir(&ctx.home);
        uninstall_jetbrains_mcp_server(&jetbrains_dir.join("mcp.json"));

        let vscode_instructions =
            super::vscode_data_dir(&ctx.home).join("User/prompts/copilot-instructions.md");
        uninstall_prompt_rules(&vscode_instructions);
        let insiders_instructions =
            super::vscode_insiders_data_dir(&ctx.home).join("User/prompts/copilot-instructions.md");
        uninstall_prompt_rules(&insiders_instructions);
        let cli_instructions = super::copilot_cli_dir(&ctx.home).join("copilot-instructions.md");
        uninstall_prompt_rules(&cli_instructions);
        uninstall_prompt_rules(&jetbrains_dir.join("global-copilot-instructions.md"));

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from GitHub Copilot.");
        crate::agent_note!(
            "Restart VS Code, JetBrains IDEs, and/or start a new Copilot CLI session for changes to take effect."
        );
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mGitHub Copilot integration\x1b[0m");
        doctor_check_vscode_settings(dc, &super::vscode_data_dir(&ctx.home), "VS Code");
        doctor_check_vscode_settings(
            dc,
            &super::vscode_insiders_data_dir(&ctx.home),
            "VS Code Insiders",
        );
        doctor_check_cli_settings(dc, &ctx.home);
        doctor_check_jetbrains_settings(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        super::vscode_data_dir(home).join("User").is_dir()
            || super::vscode_insiders_data_dir(home).join("User").is_dir()
            || super::copilot_cli_dir(home).is_dir()
            || super::copilot_jetbrains_dir(home)
                .parent()
                .is_some_and(Path::is_dir)
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(super::vscode_data_dir(home).join("User/mcp.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let vscode_mcp_json_path = super::vscode_data_dir(home).join("User/mcp.json");
        let vscode_settings_path = super::vscode_data_dir(home).join("User/settings.json");
        let insiders_mcp_json_path = super::vscode_insiders_data_dir(home).join("User/mcp.json");
        let insiders_settings_path =
            super::vscode_insiders_data_dir(home).join("User/settings.json");
        let cli_settings_path = super::copilot_cli_dir(home).join("mcp-config.json");

        let vscode_mcp_json_has_tokensave = if vscode_mcp_json_path.exists() {
            let json = load_jsonc_file(&vscode_mcp_json_path);
            json.get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        // Legacy fallback: settings.json may still hold a pre-#266 registration.
        let vscode_has_tokensave = if vscode_settings_path.exists() {
            let json = load_jsonc_file(&vscode_settings_path);
            json.get("mcp")
                .and_then(|v| v.get("servers"))
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let insiders_mcp_json_has_tokensave = if insiders_mcp_json_path.exists() {
            let json = load_jsonc_file(&insiders_mcp_json_path);
            json.get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let insiders_has_tokensave = if insiders_settings_path.exists() {
            let json = load_jsonc_file(&insiders_settings_path);
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

        let jetbrains_settings_path = super::copilot_jetbrains_dir(home).join("mcp.json");
        let jetbrains_has_tokensave = if jetbrains_settings_path.exists() {
            let json = load_json_file(&jetbrains_settings_path);
            json.get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        vscode_mcp_json_has_tokensave
            || vscode_has_tokensave
            || insiders_mcp_json_has_tokensave
            || insiders_has_tokensave
            || cli_has_tokensave
            || jetbrains_has_tokensave
    }
}

/// Register MCP server in VS Code's dedicated `User/mcp.json` (1.102+, GA
/// MCP). This is the authoritative user-level MCP config on modern VS Code —
/// the legacy `settings.json` `mcp.servers` key is migrated out of and no
/// longer read for MCP by current releases (issue #266).
///
/// Uses the same top-level `servers` shape as the `JetBrains` plugin's
/// `mcp.json`, but — unlike `JetBrains` — VS Code expects a `type` field.
fn install_vscode_mcp_json_server(mcp_json_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = mcp_json_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(mcp_json_path)?;
    let mut config = match load_jsonc_file_strict(mcp_json_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                crate::agent_note!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };
    let bin = crate::agents::preserve_mcp_command(
        config.pointer("/servers/tokensave/command"),
        tokensave_bin,
    );
    config["servers"]["tokensave"] = json!({
        "type": "stdio",
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(mcp_json_path, &config, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        mcp_json_path.display()
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
                crate::agent_note!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };
    let bin = crate::agents::preserve_mcp_command(
        settings.pointer("/mcpServers/tokensave/command"),
        tokensave_bin,
    );
    settings["mcpServers"]["tokensave"] = json!({
        "type": "stdio",
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// Register MCP server in the `JetBrains` plugin's mcp.json.
///
/// The `JetBrains` plugin uses the VS Code `mcp.json` shape (top-level
/// `servers` key), not the CLI's `mcpServers` key. It rejects unknown
/// fields conservatively, so no `type` field is written.
fn install_jetbrains_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).ok();
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
        settings.pointer("/servers/tokensave/command"),
        tokensave_bin,
    );
    settings["servers"]["tokensave"] = json!({
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    crate::agent_note!(
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
        crate::agent_note!("  {} not found, skipping", settings_path.display());
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
        crate::agent_note!(
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

    // Always write back (never delete settings.json — it has other VS Code settings).
    // backup_and_write_json leaves a .bak so any mistake is recoverable (issue #63).
    if backup_and_write_json(settings_path, &settings) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            settings_path.display()
        );
    }
}

/// Remove MCP server entry from VS Code's `User/mcp.json`.
fn uninstall_vscode_mcp_json_server(mcp_json_path: &Path) {
    if !mcp_json_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(mcp_json_path) else {
        return;
    };
    let mut config = parse_jsonc(&contents);
    let Some(servers) = config.get_mut("servers").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_json_path.display()
        );
        return;
    }
    if servers.is_empty() {
        config.as_object_mut().map(|o| o.remove("servers"));
    }
    let is_empty = config.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(mcp_json_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_json_path.display()
        );
    } else if backup_and_write_json(mcp_json_path, &config) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_json_path.display()
        );
    }
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

/// Remove MCP server entry from the `JetBrains` plugin's mcp.json.
fn uninstall_jetbrains_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(settings_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = settings.get_mut("servers").and_then(|v| v.as_object_mut()) else {
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
        settings.as_object_mut().map(|o| o.remove("servers"));
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

// ---------------------------------------------------------------------------
// Prompt rules helpers
// ---------------------------------------------------------------------------

/// Append prompt rules to a copilot-instructions.md file (idempotent).
fn install_prompt_rules(instructions_path: &Path) -> Result<()> {
    use std::io::Write;
    let marker = "## Prefer tokensave MCP tools";
    let existing = if instructions_path.exists() {
        std::fs::read_to_string(instructions_path).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(marker) {
        crate::agent_note!(
            "  {} already contains tokensave rules, skipping",
            instructions_path.display()
        );
        return Ok(());
    }
    if let Some(parent) = instructions_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(instructions_path)
        .map_err(|e| crate::errors::TokenSaveError::Config {
            message: format!("failed to open {}: {e}", instructions_path.display()),
        })?;
    write!(
        f,
        "\n{marker}\n\n\
        Before reading source files or scanning the codebase, use the tokensave MCP tools \
        (`tokensave_context`, `tokensave_search`, `tokensave_callers`, `tokensave_callees`, \
        `tokensave_impact`, `tokensave_node`, `tokensave_files`, `tokensave_affected`). \
        They provide instant semantic results from a pre-built knowledge graph and are \
        faster than file reads.\n\n\
        If a code analysis question cannot be fully answered by tokensave MCP tools, \
        try querying the SQLite database directly at `.tokensave/tokensave.db` \
        (tables: `nodes`, `edges`, `files`). Use SQL to answer complex structural queries \
        that go beyond what the built-in tools expose.\n"
    )
    .map_err(|e| crate::errors::TokenSaveError::Config {
        message: format!("failed to write {}: {e}", instructions_path.display()),
    })?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave rules to {}",
        instructions_path.display()
    );
    Ok(())
}

/// Remove tokensave rules from a copilot-instructions.md file.
fn uninstall_prompt_rules(instructions_path: &Path) {
    if !instructions_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(instructions_path) else {
        return;
    };
    if !contents.contains("tokensave") {
        return;
    }
    let marker = "## Prefer tokensave MCP tools";
    let Some(start) = contents.find(marker) else {
        return;
    };
    let after_marker = start + marker.len();
    let end = contents[after_marker..]
        .find("\n## ")
        .map_or(contents.len(), |pos| after_marker + pos);
    let mut new_contents = String::new();
    new_contents.push_str(contents[..start].trim_end());
    let remainder = &contents[end..];
    if !remainder.is_empty() {
        new_contents.push_str("\n\n");
        new_contents.push_str(remainder.trim_start());
    }
    let new_contents = new_contents.trim().to_string();
    if new_contents.is_empty() {
        std::fs::remove_file(instructions_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            instructions_path.display()
        );
    } else {
        std::fs::write(instructions_path, format!("{new_contents}\n")).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave rules from {}",
            instructions_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Report whether an MCP server entry's `args` include `"serve"`.
fn report_serve_arg(dc: &mut DoctorCounters, server: &serde_json::Map<String, serde_json::Value>) {
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

/// Check VS Code (or VS Code Insiders) has the tokensave MCP server
/// registered in the modern `User/mcp.json`, falling back to the legacy
/// `settings.json` `mcp.servers` entry (issue #266). Passes if either is
/// valid; warns if both are present since that's a stale duplicate.
fn doctor_check_vscode_settings(dc: &mut DoctorCounters, vscode_dir: &Path, label: &str) {
    let mcp_json_path = vscode_dir.join("User/mcp.json");
    let settings_path = vscode_dir.join("User/settings.json");

    if !mcp_json_path.exists() && !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use GitHub Copilot in {label}",
            mcp_json_path.display()
        ));
        return;
    }

    let mcp_json_server = if mcp_json_path.exists() {
        load_jsonc_file(&mcp_json_path)
            .get("servers")
            .and_then(|v| v.get("tokensave"))
            .and_then(serde_json::Value::as_object)
            .cloned()
    } else {
        None
    };

    let legacy_server = if settings_path.exists() {
        load_jsonc_file(&settings_path)
            .get("mcp")
            .and_then(|v| v.get("servers"))
            .and_then(|v| v.get("tokensave"))
            .and_then(serde_json::Value::as_object)
            .cloned()
    } else {
        None
    };

    match (&mcp_json_server, &legacy_server) {
        (Some(server), _) => {
            dc.pass(&format!(
                "MCP server registered in {}",
                mcp_json_path.display()
            ));
            report_serve_arg(dc, server);
            if legacy_server.is_some() {
                dc.warn(&format!(
                    "tokensave is also registered in legacy {} — modern VS Code migrates this into its own MCP registry and clears the key automatically, so it's likely just a stale leftover; you can remove mcp.servers.tokensave there to avoid confusion",
                    settings_path.display()
                ));
            }
        }
        (None, Some(server)) => {
            dc.pass(&format!(
                "MCP server registered in legacy {}",
                settings_path.display()
            ));
            report_serve_arg(dc, server);
        }
        (None, None) => {
            dc.fail(&format!(
                "MCP server NOT registered in {} — run `tokensave install --agent copilot`",
                mcp_json_path.display()
            ));
        }
    }
}

/// Check the `JetBrains` plugin's mcp.json has tokensave MCP server registered.
fn doctor_check_jetbrains_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = super::copilot_jetbrains_dir(home).join("mcp.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use GitHub Copilot in JetBrains IDEs",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("servers").and_then(|v| v.get("tokensave"));

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
