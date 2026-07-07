// Rust guideline compliant 2025-10-17
//! Qwen Code agent integration.
//!
//! Qwen Code is an open-source coding CLI forked from Gemini CLI, so it shares
//! Gemini's configuration shape: an MCP server registry in
//! `~/.qwen/settings.json` and prompt rules in `~/.qwen/QWEN.md`. Qwen Code has
//! no hook system; tool auto-approval is handled via the `trust: true` flag on
//! the MCP server entry.

use std::io::Write;
use std::path::Path;

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
};

/// Qwen Code agent.
pub struct QwenIntegration;

impl AgentIntegration for QwenIntegration {
    fn name(&self) -> &'static str {
        "Qwen Code"
    }

    fn id(&self) -> &'static str {
        "qwen"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let qwen_dir = ctx.base_dir().join(".qwen");
        std::fs::create_dir_all(&qwen_dir).ok();
        let settings_path = qwen_dir.join("settings.json");

        install_mcp_server(&settings_path, &ctx.tokensave_bin)?;

        let qwen_md = qwen_dir.join("QWEN.md");
        install_prompt_rules(&qwen_md)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Start a new Qwen Code session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let qwen_dir = ctx.base_dir().join(".qwen");
        let settings_path = qwen_dir.join("settings.json");

        uninstall_mcp_server(&settings_path);

        let qwen_md = qwen_dir.join("QWEN.md");
        uninstall_prompt_rules(&qwen_md);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Qwen Code.");
        eprintln!("Start a new Qwen Code session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mQwen Code integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".qwen").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".qwen/settings.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let settings = home.join(".qwen").join("settings.json");
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

/// Register MCP server in ~/.qwen/settings.json.
fn install_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
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
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// Append prompt rules to QWEN.md (idempotent).
fn install_prompt_rules(qwen_md: &Path) -> Result<()> {
    let marker = "## Prefer tokensave MCP tools";
    let existing = if qwen_md.exists() {
        std::fs::read_to_string(qwen_md).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(marker) {
        eprintln!("  QWEN.md already contains tokensave rules, skipping");
        return Ok(());
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(qwen_md)
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to open QWEN.md: {e}"),
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
        qwen_md.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.qwen/settings.json.
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
    let is_empty = settings.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(settings_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            settings_path.display()
        );
    } else if backup_and_write_json(settings_path, &settings) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            settings_path.display()
        );
    }
}

/// Remove tokensave rules from QWEN.md.
fn uninstall_prompt_rules(qwen_md: &Path) {
    if !qwen_md.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(qwen_md) else {
        return;
    };
    if !contents.contains("tokensave") {
        eprintln!("  QWEN.md does not contain tokensave rules, skipping");
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
        std::fs::remove_file(qwen_md).ok();
        eprintln!("\x1b[32m✔\x1b[0m Removed {} (was empty)", qwen_md.display());
    } else {
        std::fs::write(qwen_md, format!("{new_contents}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave rules from {}",
            qwen_md.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check settings.json has tokensave registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = home.join(".qwen").join("settings.json");
    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent qwen` if you use Qwen Code",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    let Some(server) = server.and_then(|v| v.as_object()) else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent qwen`",
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
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent qwen`");
    }

    // Check trust flag
    let is_trusted = server
        .get("trust")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if is_trusted {
        dc.pass("MCP server has trust: true (tools auto-approved)");
    } else {
        dc.warn("MCP server missing trust: true — Qwen Code will prompt for each tool call");
    }
}

/// Check QWEN.md contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let qwen_md = home.join(".qwen").join("QWEN.md");
    if qwen_md.exists() {
        let has_rules = std::fs::read_to_string(&qwen_md)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("QWEN.md contains tokensave rules");
        } else {
            dc.fail("QWEN.md missing tokensave rules — run `tokensave install --agent qwen`");
        }
    } else {
        dc.warn("~/.qwen/QWEN.md does not exist");
    }
}
