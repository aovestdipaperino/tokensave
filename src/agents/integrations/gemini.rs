//! Gemini CLI agent integration.
//!
//! Handles registration of the tokensave MCP server in Gemini CLI's config
//! file (`~/.gemini/settings.json`), and prompt rules via `~/.gemini/GEMINI.md`.
//! Gemini CLI has no hook system. Tool auto-approval is handled via the
//! `trust: true` flag on the MCP server entry.

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::*;

/// Gemini CLI agent.
pub struct GeminiIntegration;

impl AgentIntegration for GeminiIntegration {
    fn name(&self) -> &'static str {
        "Gemini CLI"
    }

    fn id(&self) -> &'static str {
        "gemini"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let gemini_dir = ctx.base_dir().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).ok();
        let settings_path = gemini_dir.join("settings.json");

        install_mcp_server(&settings_path, &ctx.tokensave_bin)?;

        let gemini_md = gemini_dir.join("GEMINI.md");
        install_prompt_rules(&gemini_md)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!(
            "  2. Start a new Gemini CLI session — tokensave tools are now available"
        );
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let gemini_dir = ctx.base_dir().join(".gemini");
        let settings_path = gemini_dir.join("settings.json");

        uninstall_mcp_server(&settings_path);

        let gemini_md = gemini_dir.join("GEMINI.md");
        uninstall_prompt_rules(&gemini_md);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Gemini CLI.");
        crate::agent_note!("Start a new Gemini CLI session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mGemini CLI integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".gemini").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".gemini/settings.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let settings = home.join(".gemini").join("settings.json");
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

/// Register MCP server in ~/.gemini/settings.json.
fn install_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
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
        "command": bin,
        "args": ["serve"],
        "trust": true
    });

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// Write or refresh the tokensave rules block in GEMINI.md.
fn install_prompt_rules(gemini_md: &Path) -> Result<()> {
    let body = rules_for_agent("gemini")?;
    write_rules_block(gemini_md, "gemini", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.gemini/settings.json.
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

/// Remove tokensave rules from GEMINI.md (both managed-block and
/// legacy heading-guarded forms).
fn uninstall_prompt_rules(gemini_md: &Path) {
    remove_rules_block(gemini_md).ok();
    remove_legacy_rules_block(gemini_md, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check settings.json has tokensave registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = home.join(".gemini").join("settings.json");
    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent gemini` if you use Gemini CLI",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    let Some(server) = server.and_then(|v| v.as_object()) else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent gemini`",
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
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent gemini`");
    }

    // Check trust flag
    let is_trusted = server
        .get("trust")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if is_trusted {
        dc.pass("MCP server has trust: true (tools auto-approved)");
    } else {
        dc.warn("MCP server missing trust: true — Gemini will prompt for each tool call");
    }
}

/// Check GEMINI.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let gemini_md = home.join(".gemini").join("GEMINI.md");
    check_shared_rules_block(dc, &gemini_md, "gemini");
}
