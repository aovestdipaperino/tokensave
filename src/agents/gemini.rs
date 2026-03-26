//! Gemini CLI agent integration.
//!
//! Handles registration of the tokensave MCP server in Gemini CLI's config
//! file (`~/.gemini/settings.json`), and prompt rules via `~/.gemini/GEMINI.md`.
//! Gemini CLI has no hook system. Tool auto-approval is handled via the
//! `trust: true` flag on the MCP server entry.

use std::io::Write;
use std::path::Path;

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{load_json_file, Agent, DoctorCounters, HealthcheckContext, InstallContext};

/// Gemini CLI agent.
pub struct GeminiAgent;

impl Agent for GeminiAgent {
    fn name(&self) -> &'static str {
        "Gemini CLI"
    }

    fn id(&self) -> &'static str {
        "gemini"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let gemini_dir = ctx.home.join(".gemini");
        std::fs::create_dir_all(&gemini_dir).ok();
        let settings_path = gemini_dir.join("settings.json");

        install_mcp_server(&settings_path, &ctx.tokensave_bin)?;

        let gemini_md = gemini_dir.join("GEMINI.md");
        install_prompt_rules(&gemini_md)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave sync");
        eprintln!("  2. Start a new Gemini CLI session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let gemini_dir = ctx.home.join(".gemini");
        let settings_path = gemini_dir.join("settings.json");

        uninstall_mcp_server(&settings_path);

        let gemini_md = gemini_dir.join("GEMINI.md");
        uninstall_prompt_rules(&gemini_md);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Gemini CLI.");
        eprintln!("Start a new Gemini CLI session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mGemini CLI integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server in ~/.gemini/settings.json.
fn install_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
    let mut settings = load_json_file(settings_path);

    settings["mcpServers"]["tokensave"] = json!({
        "command": tokensave_bin,
        "args": ["serve"],
        "trust": true
    });

    let pretty = serde_json::to_string_pretty(&settings).unwrap_or_else(|_| "{}".to_string());
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(settings_path, format!("{pretty}\n")).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write {}: {e}", settings_path.display()),
    })?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// Append prompt rules to GEMINI.md (idempotent).
fn install_prompt_rules(gemini_md: &Path) -> Result<()> {
    let marker = "## Prefer tokensave MCP tools";
    let existing = if gemini_md.exists() {
        std::fs::read_to_string(gemini_md).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(marker) {
        eprintln!("  GEMINI.md already contains tokensave rules, skipping");
        return Ok(());
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(gemini_md)
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to open GEMINI.md: {e}"),
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
        gemini_md.display()
    );
    Ok(())
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
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }
    if servers.is_empty() {
        settings.as_object_mut().map(|o| o.remove("mcpServers"));
    }
    let is_empty = settings.as_object().is_some_and(|o| o.is_empty());
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

/// Remove tokensave rules from GEMINI.md.
fn uninstall_prompt_rules(gemini_md: &Path) {
    if !gemini_md.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(gemini_md) else {
        return;
    };
    if !contents.contains("tokensave") {
        eprintln!("  GEMINI.md does not contain tokensave rules, skipping");
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
        std::fs::remove_file(gemini_md).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            gemini_md.display()
        );
    } else {
        std::fs::write(gemini_md, format!("{new_contents}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave rules from {}",
            gemini_md.display()
        );
    }
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
    let server = settings
        .get("mcpServers")
        .and_then(|v| v.get("tokensave"));

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
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_trusted {
        dc.pass("MCP server has trust: true (tools auto-approved)");
    } else {
        dc.warn("MCP server missing trust: true — Gemini will prompt for each tool call");
    }
}

/// Check GEMINI.md contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let gemini_md = home.join(".gemini").join("GEMINI.md");
    if gemini_md.exists() {
        let has_rules = std::fs::read_to_string(&gemini_md)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("GEMINI.md contains tokensave rules");
        } else {
            dc.fail("GEMINI.md missing tokensave rules — run `tokensave install --agent gemini`");
        }
    } else {
        dc.warn("~/.gemini/GEMINI.md does not exist");
    }
}
