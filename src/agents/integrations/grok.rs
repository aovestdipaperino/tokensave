// Rust guideline compliant 2025-10-17
//! Grok Build (xAI Grok CLI / TUI) agent integration.
//!
//! Handles registration of the tokensave MCP server in Grok's native config
//! file (`~/.grok/config.toml`) using the documented `[mcp_servers.tokensave]`
//! table form, and prompt rules via `~/.grok/AGENTS.md` (and project-scoped
//! `.grok/AGENTS.md`). Grok has no hook system; permissions are handled via
//! its TUI / `permission_mode` settings.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::*;

/// Grok Build agent.
pub struct GrokIntegration;

impl AgentIntegration for GrokIntegration {
    fn name(&self) -> &'static str {
        "Grok Build"
    }

    fn id(&self) -> &'static str {
        "grok"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let grok_dir = ctx.home.join(".grok");
        std::fs::create_dir_all(&grok_dir).ok();
        let config_path = grok_dir.join("config.toml");

        install_mcp_server(&config_path, &ctx.tokensave_bin)?;

        let agents_md = grok_dir.join("AGENTS.md");
        install_prompt_rules(&agents_md)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new Grok Build session — tokensave tools are now available via search_tool + use_tool");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let grok_dir = ctx.home.join(".grok");
        let config_path = grok_dir.join("config.toml");

        uninstall_mcp_server(&config_path)?;

        let agents_md = grok_dir.join("AGENTS.md");
        uninstall_prompt_rules(&agents_md);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Grok Build.");
        crate::agent_note!("Start a new Grok Build session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mGrok Build integration\x1b[0m");
        let grok_dir = ctx.home.join(".grok");
        let config_path = grok_dir.join("config.toml");
        doctor_check_config(dc, &config_path);
        doctor_check_prompt(dc, &grok_dir);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".grok").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".grok/config.toml"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let config = home.join(".grok").join("config.toml");
        if !config.exists() {
            return false;
        }
        // If the file is unparseable, conservatively report "not installed"
        // so the caller treats it like a fresh install path.
        super::load_toml_file(&config).is_ok_and(|toml| {
            toml.get("mcp_servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        })
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server under [`mcp_servers.tokensave`] in ~/.grok/config.toml.
fn install_mcp_server(config_path: &Path, tokensave_bin: &str) -> Result<()> {
    let mut config = load_toml_file(config_path)?;

    let table = config
        .as_table_mut()
        .ok_or_else(|| TokenSaveError::Config {
            message: "config.toml is not a TOML table".to_string(),
        })?;

    let servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| TokenSaveError::Config {
            message: "mcp_servers is not a table in config.toml".to_string(),
        })?;

    let bin = crate::agents::preserve_mcp_command_str(
        servers
            .get("tokensave")
            .and_then(|t| t.get("command"))
            .and_then(toml::Value::as_str),
        tokensave_bin,
    );
    let mut server_table = toml::map::Map::new();
    server_table.insert("command".to_string(), toml::Value::String(bin));
    server_table.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("serve".to_string())]),
    );
    // Explicit enabled is optional (defaults true in Grok) but makes the entry clear.
    server_table.insert("enabled".to_string(), toml::Value::Boolean(true));

    servers.insert("tokensave".to_string(), toml::Value::Table(server_table));

    write_toml_file(config_path, &config)?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Append prompt rules to ~/.grok/AGENTS.md (idempotent).
/// Grok supports AGENTS.md (global and .grok/AGENTS.md project-scoped) for
/// Write or refresh the tokensave rules block in AGENTS.md.
fn install_prompt_rules(agents_md: &Path) -> Result<()> {
    let body = rules_for_agent("grok")?;
    write_rules_block(agents_md, "grok", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.grok/config.toml.
fn uninstall_mcp_server(config_path: &Path) -> Result<()> {
    if !config_path.exists() {
        return Ok(());
    }
    let mut config = load_toml_file(config_path)?;
    let Some(table) = config.as_table_mut() else {
        return Ok(());
    };
    let Some(servers) = table.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) else {
        return Ok(());
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            config_path.display()
        );
        return Ok(());
    }
    if servers.is_empty() {
        table.remove("mcp_servers");
    }
    if table.is_empty() {
        std::fs::remove_file(config_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            config_path.display()
        );
    } else {
        write_toml_file(config_path, &config)?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            config_path.display()
        );
    }
    Ok(())
}

/// Remove tokensave rules from AGENTS.md (both managed-block and
/// legacy heading-guarded forms).
fn uninstall_prompt_rules(agents_md: &Path) {
    remove_rules_block(agents_md).ok();
    remove_legacy_rules_block(agents_md, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check config.toml has tokensave registered under [`mcp_servers.tokensave`].
fn doctor_check_config(dc: &mut DoctorCounters, config_path: &Path) {
    if !config_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent grok` if you use Grok Build",
            config_path.display()
        ));
        return;
    }

    let config = match load_toml_file(config_path) {
        Ok(c) => c,
        Err(e) => {
            dc.fail(&format!("{e}"));
            return;
        }
    };
    let has_server = config
        .get("mcp_servers")
        .and_then(|v| v.get("tokensave"))
        .and_then(|v| v.as_table())
        .is_some();

    if !has_server {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent grok`",
            config_path.display()
        ));
        return;
    }
    dc.pass(&format!(
        "MCP server registered in {}",
        config_path.display()
    ));

    // Light validation of the entry (command/args present and looks reasonable)
    let server = config
        .get("mcp_servers")
        .and_then(|v| v.get("tokensave"))
        .and_then(|v| v.as_table());

    if let Some(s) = server {
        if let Some(cmd) = s.get("command").and_then(|v| v.as_str()) {
            if !cmd.is_empty() {
                dc.pass(&format!("MCP server command present: {cmd}"));
            }
        }
        let has_serve = s
            .get("args")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
        if has_serve {
            dc.pass("MCP server args include \"serve\"");
        } else {
            dc.warn("MCP server args missing \"serve\" — consider re-running install");
        }
    }
}

/// Check AGENTS.md (in ~/.grok/) contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, grok_dir: &Path) {
    let agents_md = grok_dir.join("AGENTS.md");
    check_shared_rules_block(dc, &agents_md, "grok");
}
