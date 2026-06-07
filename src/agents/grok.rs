// Rust guideline compliant 2025-10-17
//! Grok Build (xAI Grok CLI / TUI) agent integration.
//!
//! Handles registration of the tokensave MCP server in Grok's native config
//! file (`~/.grok/config.toml`) using the documented `[mcp_servers.tokensave]`
//! table form, and prompt rules via `~/.grok/AGENTS.md` (and project-scoped
//! `.grok/AGENTS.md`). Grok has no hook system; permissions are handled via
//! its TUI / `permission_mode` settings.

use std::io::Write;
use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::{
    load_toml_file, write_toml_file, AgentIntegration, DoctorCounters, HealthcheckContext,
    InstallContext,
};

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

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Start a new Grok Build session — tokensave tools are now available via search_tool + use_tool");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let grok_dir = ctx.home.join(".grok");
        let config_path = grok_dir.join("config.toml");

        uninstall_mcp_server(&config_path)?;

        let agents_md = grok_dir.join("AGENTS.md");
        uninstall_prompt_rules(&agents_md);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Grok Build.");
        eprintln!("Start a new Grok Build session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mGrok Build integration\x1b[0m");
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

    let mut server_table = toml::map::Map::new();
    server_table.insert(
        "command".to_string(),
        toml::Value::String(tokensave_bin.to_string()),
    );
    server_table.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("serve".to_string())]),
    );
    // Explicit enabled is optional (defaults true in Grok) but makes the entry clear.
    server_table.insert("enabled".to_string(), toml::Value::Boolean(true));

    servers.insert("tokensave".to_string(), toml::Value::Table(server_table));

    write_toml_file(config_path, &config)?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Append prompt rules to ~/.grok/AGENTS.md (idempotent).
/// Grok supports AGENTS.md (global and .grok/AGENTS.md project-scoped) for
/// instructions that influence the system prompt.
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
        Before reading source files or scanning the codebase with file tools or \
        sub-agents (especially the `explore` sub-agent), use the tokensave MCP tools. \
        Discover them with the built-in `search_tool` (query for \"tokensave\" or a code \
        concept) and invoke via `use_tool` with the namespaced name \
        (e.g. `tokensave__tokensave_context`, `tokensave__tokensave_search`, \
        `tokensave__tokensave_callers`, `tokensave__tokensave_callees`, \
        `tokensave__tokensave_impact`, `tokensave__tokensave_node`, \
        `tokensave__tokensave_files`, `tokensave__tokensave_affected`).\n\n\
        They provide instant semantic results from a pre-built knowledge graph and are \
        faster and lower-token than raw file reads or sub-agent exploration.\n\n\
        If a code analysis question cannot be fully answered by tokensave MCP tools, \
        try querying the SQLite database directly at `.tokensave/tokensave.db` \
        (tables: `nodes`, `edges`, `files`). Use SQL to answer complex structural queries \
        that go beyond what the built-in tools expose.\n\n\
        If you discover a gap where an extractor, schema, or tokensave tool could be \
        improved to answer a question natively, propose to the user that they open an issue \
        at https://github.com/aovestdipaperino/tokensave describing the limitation. \
        **Remind the user to strip any sensitive or proprietary code from the bug description \
        before submitting.**\n"
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
        eprintln!(
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

/// Check AGENTS.md (in ~/.grok/) contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, grok_dir: &Path) {
    let agents_md = grok_dir.join("AGENTS.md");
    if agents_md.exists() {
        let has_rules = std::fs::read_to_string(&agents_md)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("AGENTS.md contains tokensave rules");
        } else {
            dc.fail("AGENTS.md missing tokensave rules — run `tokensave install --agent grok`");
        }
    } else {
        dc.warn("~/.grok/AGENTS.md does not exist (rules are optional but recommended)");
    }
}
