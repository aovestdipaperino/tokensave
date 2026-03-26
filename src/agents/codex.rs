// Rust guideline compliant 2025-10-17
//! OpenAI Codex CLI agent integration.
//!
//! Handles registration of the tokensave MCP server in Codex's config
//! file (`~/.codex/config.toml`), per-tool auto-approval settings,
//! and prompt rules via `AGENTS.md`. Codex has no hook system.

use std::io::Write;
use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::{
    Agent, DoctorCounters, HealthcheckContext, InstallContext,
    load_toml_file, write_toml_file, TOOL_NAMES,
};

/// OpenAI Codex CLI agent.
pub struct CodexAgent;

impl Agent for CodexAgent {
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

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave sync");
        eprintln!("  2. Start a new Codex session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");

        uninstall_mcp_server(&config_path)?;

        let agents_md = codex_dir.join("AGENTS.md");
        uninstall_prompt_rules(&agents_md);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Codex CLI.");
        eprintln!("Start a new Codex session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mCodex CLI integration\x1b[0m");
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");
        doctor_check_config(dc, &config_path);
        doctor_check_prompt(dc, &codex_dir);
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server and auto-approve tools in ~/.codex/config.toml.
fn install_mcp_server(config_path: &Path, tokensave_bin: &str) -> Result<()> {
    let mut config = load_toml_file(config_path);

    // Ensure [mcp_servers.tokensave] exists
    let table = config.as_table_mut().ok_or_else(|| TokenSaveError::Config {
        message: "config.toml is not a TOML table".to_string(),
    })?;

    let servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| TokenSaveError::Config {
            message: "mcp_servers is not a table in config.toml".to_string(),
        })?;

    let mut server_table = toml::map::Map::new();
    server_table.insert(
        "command".to_string(),
        toml::Value::String(tokensave_bin.to_string()),
    );
    server_table.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("serve".to_string())]),
    );

    // Auto-approve all tokensave tools so Codex doesn't prompt for each one
    let mut tools_table = toml::map::Map::new();
    for tool_name in TOOL_NAMES {
        let mut tool_config = toml::map::Map::new();
        tool_config.insert(
            "approval_mode".to_string(),
            toml::Value::String("auto".to_string()),
        );
        tools_table.insert(tool_name.to_string(), toml::Value::Table(tool_config));
    }
    server_table.insert("tools".to_string(), toml::Value::Table(tools_table));

    servers.insert("tokensave".to_string(), toml::Value::Table(server_table));

    write_toml_file(config_path, &config)?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Append prompt rules to AGENTS.md (idempotent).
fn install_prompt_rules(agents_md: &Path) -> Result<()> {
    let marker = "## Prefer tokensave MCP tools";
    let existing = if agents_md.exists() {
        std::fs::read_to_string(agents_md).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(marker) {
        eprintln!("  AGENTS.md already contains tokensave rules, skipping");
        return Ok(());
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(agents_md)
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to open AGENTS.md: {e}"),
        })?;
    write!(
        f,
        "\n{marker}\n\n\
        Before reading source files or scanning the codebase, use the tokensave MCP tools \
        (`tokensave_context`, `tokensave_search`, `tokensave_callers`, `tokensave_callees`, \
        `tokensave_impact`, `tokensave_node`, `tokensave_files`, `tokensave_affected`). \
        They provide instant semantic results from a pre-built knowledge graph and are \
        faster than file reads.\n"
    )
    .ok();
    eprintln!(
        "\x1b[32m✔\x1b[0m Appended tokensave rules to {}",
        agents_md.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.codex/config.toml.
fn uninstall_mcp_server(config_path: &Path) -> Result<()> {
    if !config_path.exists() {
        return Ok(());
    }
    let mut config = load_toml_file(config_path);
    let Some(table) = config.as_table_mut() else {
        return Ok(());
    };
    let Some(servers) = table.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) else {
        return Ok(());
    };
    if servers.remove("tokensave").is_none() {
        eprintln!("  No tokensave MCP server in {}, skipping", config_path.display());
        return Ok(());
    }
    if servers.is_empty() {
        table.remove("mcp_servers");
    }
    if table.is_empty() {
        std::fs::remove_file(config_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            config_path.display()
        );
    } else {
        write_toml_file(config_path, &config)?;
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            config_path.display()
        );
    }
    Ok(())
}

/// Remove tokensave rules from AGENTS.md.
fn uninstall_prompt_rules(agents_md: &Path) {
    if !agents_md.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(agents_md) else {
        return;
    };
    if !contents.contains("tokensave") {
        eprintln!("  AGENTS.md does not contain tokensave rules, skipping");
        return;
    }
    let marker = "## Prefer tokensave MCP tools";
    let Some(start) = contents.find(marker) else {
        return;
    };
    let after_marker = start + marker.len();
    let end = contents[after_marker..]
        .find("\n## ")
        .map(|pos| after_marker + pos)
        .unwrap_or(contents.len());
    let mut new_contents = String::new();
    new_contents.push_str(contents[..start].trim_end());
    let remainder = &contents[end..];
    if !remainder.is_empty() {
        new_contents.push_str("\n\n");
        new_contents.push_str(remainder.trim_start());
    }
    let new_contents = new_contents.trim().to_string();
    if new_contents.is_empty() {
        std::fs::remove_file(agents_md).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            agents_md.display()
        );
    } else {
        std::fs::write(agents_md, format!("{new_contents}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave rules from {}",
            agents_md.display()
        );
    }
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

    let config = load_toml_file(config_path);
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
            .filter(|v| {
                v.get("approval_mode")
                    .and_then(|m| m.as_str())
                    == Some("auto")
            })
            .count()
    });

    if auto_count >= TOOL_NAMES.len() {
        dc.pass(&format!("All {} tools set to auto-approve", TOOL_NAMES.len()));
    } else if auto_count > 0 {
        dc.warn(&format!(
            "{}/{} tools auto-approved — run `tokensave install --agent codex` to update",
            auto_count,
            TOOL_NAMES.len()
        ));
    } else {
        dc.warn("No tools auto-approved — Codex will prompt for each tool call");
    }
}

/// Check AGENTS.md contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, codex_dir: &Path) {
    let agents_md = codex_dir.join("AGENTS.md");
    if agents_md.exists() {
        let has_rules = std::fs::read_to_string(&agents_md)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("AGENTS.md contains tokensave rules");
        } else {
            dc.fail("AGENTS.md missing tokensave rules — run `tokensave install --agent codex`");
        }
    } else {
        dc.warn("~/.codex/AGENTS.md does not exist");
    }
}
