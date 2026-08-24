// Rust guideline compliant 2025-10-17
//! `OpenAI` Codex CLI agent integration.
//!
//! Handles registration of the tokensave MCP server in Codex's config
//! file (`~/.codex/config.toml`), per-tool auto-approval settings,
//! and prompt rules via `AGENTS.md`. Codex has no hook system.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::*;

/// `OpenAI` Codex CLI agent.
pub struct CodexIntegration;

impl AgentIntegration for CodexIntegration {
    fn name(&self) -> &'static str {
        "Codex CLI"
    }

    fn id(&self) -> &'static str {
        "codex"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let codex_dir = ctx.home.join(".codex");
        std::fs::create_dir_all(&codex_dir).ok();
        let config_path = codex_dir.join("config.toml");

        install_mcp_server(&config_path, &ctx.tokensave_bin)?;

        let agents_md = codex_dir.join("AGENTS.md");
        install_prompt_rules(&agents_md)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new Codex session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");

        uninstall_mcp_server(&config_path)?;

        let agents_md = codex_dir.join("AGENTS.md");
        uninstall_prompt_rules(&agents_md);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Codex CLI.");
        crate::agent_note!("Start a new Codex session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mCodex CLI integration\x1b[0m");
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");
        doctor_check_config(dc, &config_path);
        doctor_check_prompt(dc, &codex_dir);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".codex").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".codex/config.toml"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let config = home.join(".codex").join("config.toml");
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

/// Register MCP server and auto-approve tools in ~/.codex/config.toml.
fn install_mcp_server(config_path: &Path, tokensave_bin: &str) -> Result<()> {
    let mut config = load_toml_file(config_path)?;

    // Ensure [mcp_servers.tokensave] exists
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

    // Auto-approve all tokensave tools so Codex doesn't prompt for each one
    let mut tools_table = toml::map::Map::new();
    for tool_name in tool_names() {
        let mut tool_config = toml::map::Map::new();
        tool_config.insert(
            "approval_mode".to_string(),
            toml::Value::String("auto".to_string()),
        );
        tools_table.insert(tool_name.clone(), toml::Value::Table(tool_config));
    }
    server_table.insert("tools".to_string(), toml::Value::Table(tools_table));

    servers.insert("tokensave".to_string(), toml::Value::Table(server_table));

    write_toml_file(config_path, &config)?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Write or refresh the tokensave rules block in AGENTS.md.
fn install_prompt_rules(agents_md: &Path) -> Result<()> {
    let body = rules_for_agent("codex")?;
    write_rules_block(agents_md, "codex", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.codex/config.toml.
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

/// Remove tokensave rules from AGENTS.md (both managed-block and legacy
/// heading-guarded forms).
fn uninstall_prompt_rules(agents_md: &Path) {
    remove_rules_block(agents_md).ok();
    remove_legacy_rules_block(agents_md, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check config.toml has tokensave registered.
fn doctor_check_config(dc: &mut DoctorCounters, config_path: &Path) {
    if !config_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent codex` if you use Codex CLI",
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
            "MCP server NOT registered in {} — run `tokensave install --agent codex`",
            config_path.display()
        ));
        return;
    }
    dc.pass(&format!(
        "MCP server registered in {}",
        config_path.display()
    ));

    // Check tool auto-approval
    let tools = config
        .get("mcp_servers")
        .and_then(|v| v.get("tokensave"))
        .and_then(|v| v.get("tools"))
        .and_then(|v| v.as_table());

    let auto_count = tools.map_or(0, |t| {
        t.values()
            .filter(|v| v.get("approval_mode").and_then(|m| m.as_str()) == Some("auto"))
            .count()
    });

    let tools = tool_names();
    let tools_len = tools.len();
    if auto_count >= tools_len {
        dc.pass(&format!("All {tools_len} tools set to auto-approve"));
    } else if auto_count > 0 {
        dc.warn(&format!(
            "{auto_count}/{tools_len} tools auto-approved — run `tokensave install --agent codex` to update"
        ));
    } else {
        dc.warn("No tools auto-approved — Codex will prompt for each tool call");
    }
}

/// Check AGENTS.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, codex_dir: &Path) {
    let agents_md = codex_dir.join("AGENTS.md");
    check_shared_rules_block(dc, &agents_md, "codex");
}
