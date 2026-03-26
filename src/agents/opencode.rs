// Rust guideline compliant 2025-10-17
//! OpenCode agent integration.
//!
//! Handles registration of the tokensave MCP server in OpenCode's config
//! file (`.opencode.json` in `$HOME` or `$XDG_CONFIG_HOME/opencode/`),
//! and prompt rules via `OPENCODE.md`. OpenCode has no hook system or
//! declarative tool permissions — it uses interactive runtime approval.

use std::io::Write;
use std::path::Path;

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{load_json_file, Agent, DoctorCounters, HealthcheckContext, InstallContext};

/// OpenCode agent.
pub struct OpenCodeAgent;

impl Agent for OpenCodeAgent {
    fn name(&self) -> &'static str {
        "OpenCode"
    }

    fn id(&self) -> &'static str {
        "opencode"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let config_path = opencode_config_path(&ctx.home);
        install_mcp_server(&config_path, &ctx.tokensave_bin)?;

        let global_prompt = opencode_prompt_path(&ctx.home);
        install_prompt_rules(&global_prompt)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave sync");
        eprintln!("  2. Start a new OpenCode session — tokensave tools are now available");
        eprintln!("  3. OpenCode will prompt for approval on first use of each tool");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let config_path = opencode_config_path(&ctx.home);
        uninstall_mcp_server(&config_path);

        let global_prompt = opencode_prompt_path(&ctx.home);
        uninstall_prompt_rules(&global_prompt);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from OpenCode.");
        eprintln!("Start a new OpenCode session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mOpenCode integration\x1b[0m");
        doctor_check_config(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }
}

// ---------------------------------------------------------------------------
// Config path resolution
// ---------------------------------------------------------------------------

/// Returns the path to opencode config (global).
/// Checks `$HOME/.config/opencode/opencode.json` first (modern location),
/// then `$XDG_CONFIG_HOME/opencode/opencode.json`, then `$HOME/.opencode.json` (legacy).
fn opencode_config_path(home: &Path) -> std::path::PathBuf {
    let modern = home.join(".config/opencode/opencode.json");
    if modern.exists() {
        return modern;
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg_path = std::path::PathBuf::from(xdg).join("opencode/opencode.json");
        if xdg_path.exists() {
            return xdg_path;
        }
    }
    home.join(".opencode.json")
}

/// Returns the path to the global OPENCODE.md prompt file.
fn opencode_prompt_path(home: &Path) -> std::path::PathBuf {
    let modern = home.join(".config/opencode/OPENCODE.md");
    if modern.exists() || home.join(".config/opencode").exists() {
        return modern;
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg_dir = std::path::PathBuf::from(xdg).join("opencode");
        if xdg_dir.exists() {
            return xdg_dir.join("OPENCODE.md");
        }
    }
    home.join("OPENCODE.md")
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server in .opencode.json.
fn install_mcp_server(config_path: &Path, tokensave_bin: &str) -> Result<()> {
    let mut config = load_json_file(config_path);
    config["mcpServers"]["tokensave"] = json!({
        "type": "stdio",
        "command": tokensave_bin,
        "args": ["serve"]
    });
    let pretty = serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string());
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(config_path, format!("{pretty}\n")).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write {}: {e}", config_path.display()),
    })?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Append prompt rules to OPENCODE.md (idempotent).
fn install_prompt_rules(prompt_path: &Path) -> Result<()> {
    let marker = "## Prefer tokensave MCP tools";
    let existing = if prompt_path.exists() {
        std::fs::read_to_string(prompt_path).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(marker) {
        eprintln!("  OPENCODE.md already contains tokensave rules, skipping");
        return Ok(());
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(prompt_path)
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to open OPENCODE.md: {e}"),
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
        prompt_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from .opencode.json.
fn uninstall_mcp_server(config_path: &Path) {
    if !config_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(config_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = config.get_mut("mcpServers").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if servers.remove("tokensave").is_none() {
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            config_path.display()
        );
        return;
    }
    if servers.is_empty() {
        config.as_object_mut().map(|o| o.remove("mcpServers"));
    }
    let is_empty = config.as_object().is_some_and(|o| o.is_empty());
    if is_empty {
        std::fs::remove_file(config_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            config_path.display()
        );
    } else {
        let pretty = serde_json::to_string_pretty(&config).unwrap_or_default();
        std::fs::write(config_path, format!("{pretty}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            config_path.display()
        );
    }
}

/// Remove tokensave rules from OPENCODE.md.
fn uninstall_prompt_rules(prompt_path: &Path) {
    if !prompt_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(prompt_path) else {
        return;
    };
    if !contents.contains("tokensave") {
        eprintln!("  OPENCODE.md does not contain tokensave rules, skipping");
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
        std::fs::remove_file(prompt_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            prompt_path.display()
        );
    } else {
        std::fs::write(prompt_path, format!("{new_contents}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave rules from {}",
            prompt_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check .opencode.json has tokensave registered.
fn doctor_check_config(dc: &mut DoctorCounters, home: &Path) {
    let config_path = opencode_config_path(home);
    if !config_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent opencode` if you use OpenCode",
            config_path.display()
        ));
        return;
    }

    let config = load_json_file(&config_path);
    let mcp_entry = &config["mcpServers"]["tokensave"];
    if !mcp_entry.is_object() {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent opencode`",
            config_path.display()
        ));
        return;
    }
    dc.pass(&format!(
        "MCP server registered in {}",
        config_path.display()
    ));

    let args_ok = mcp_entry["args"]
        .as_array()
        .is_some_and(|a| a.first().and_then(|v| v.as_str()) == Some("serve"));
    if args_ok {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent opencode`");
    }
}

/// Check OPENCODE.md contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let prompt_path = opencode_prompt_path(home);
    if prompt_path.exists() {
        let has_rules = std::fs::read_to_string(&prompt_path)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("OPENCODE.md contains tokensave rules");
        } else {
            dc.fail(
                "OPENCODE.md missing tokensave rules — run `tokensave install --agent opencode`",
            );
        }
    } else {
        dc.warn("OPENCODE.md does not exist");
    }
}
