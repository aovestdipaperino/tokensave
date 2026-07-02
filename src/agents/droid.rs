// Rust guideline compliant 2026-07-02
//! Factory Droid agent integration.
//!
//! Handles registration of the tokensave MCP server in Factory Droid's MCP
//! config (`~/.factory/mcp.json` globally, `<project>/.factory/mcp.json` for
//! `--local`) under the `mcpServers.tokensave` key, and prompt rules via
//! `AGENTS.md` (`~/.factory/AGENTS.md` globally, `<project>/AGENTS.md` for
//! `--local`). Factory Droid has no hook system or declarative tool
//! permissions — it uses interactive runtime approval.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    InstallScope,
};

/// Factory Droid agent.
pub struct DroidIntegration;

impl AgentIntegration for DroidIntegration {
    fn name(&self) -> &'static str {
        "Factory Droid"
    }

    fn id(&self) -> &'static str {
        "droid"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = droid_mcp_path_for(ctx);
        install_mcp_server(&mcp_path, &ctx.tokensave_bin)?;

        let agents_md = droid_agents_md_for(ctx);
        install_prompt_rules(&agents_md)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Start a new droid session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = droid_mcp_path_for(ctx);
        uninstall_mcp_server(&mcp_path);

        let agents_md = droid_agents_md_for(ctx);
        uninstall_prompt_rules(&agents_md);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Factory Droid.");
        eprintln!("Start a new droid session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mFactory Droid integration\x1b[0m");
        doctor_check_config(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".factory").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(droid_mcp_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = droid_mcp_path(home);
        if !mcp_path.exists() {
            return false;
        }
        let json = load_json_file(&mcp_path);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Config path resolution
// ---------------------------------------------------------------------------

/// Global Factory Droid MCP config path (`~/.factory/mcp.json`).
fn droid_mcp_path(home: &Path) -> PathBuf {
    home.join(".factory/mcp.json")
}

/// mcp.json path for this install: `~/.factory/mcp.json` globally, or
/// `<project>/.factory/mcp.json` for `--local` (same relative layout).
fn droid_mcp_path_for(ctx: &InstallContext) -> PathBuf {
    ctx.base_dir().join(".factory/mcp.json")
}

/// AGENTS.md path for this install: `~/.factory/AGENTS.md` globally, or
/// `<project>/AGENTS.md` for `--local` (Factory reads a repo-root AGENTS.md).
fn droid_agents_md_for(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => ctx.home.join(".factory/AGENTS.md"),
        InstallScope::Local { project_path } => project_path.join("AGENTS.md"),
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server in mcp.json.
///
/// Safety: creates a `.bak` backup before writing and restores it on any
/// error. Uses strict JSON parsing so an existing file with invalid syntax
/// is never silently replaced with an empty object.
fn install_mcp_server(mcp_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = mcp_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(mcp_path)?;
    let mut settings = match load_json_file_strict(mcp_path) {
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
        "args": ["serve"]
    });

    safe_write_json_file(mcp_path, &settings, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        mcp_path.display()
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
    if let Some(parent) = agents_md.parent() {
        std::fs::create_dir_all(parent).ok();
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
        agents_md.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove the tokensave MCP server entry from mcp.json.
fn uninstall_mcp_server(mcp_path: &Path) {
    if !mcp_path.exists() {
        eprintln!("  {} not found, skipping", mcp_path.display());
        return;
    }

    let Ok(contents) = std::fs::read_to_string(mcp_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };

    let Some(servers) = settings
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    else {
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return;
    };

    if servers.remove("tokensave").is_none() {
        eprintln!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return;
    }

    let is_empty = settings.as_object().is_some_and(|o| {
        o.iter()
            .all(|(k, v)| k == "mcpServers" && v.as_object().is_some_and(serde_json::Map::is_empty))
    });

    if is_empty {
        std::fs::remove_file(mcp_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else if backup_and_write_json(mcp_path, &settings) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
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

/// Check mcp.json has tokensave registered.
fn doctor_check_config(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = droid_mcp_path(home);
    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent droid` if you use Factory Droid",
            mcp_path.display()
        ));
        return;
    }

    let config = load_json_file(&mcp_path);
    let mcp_entry = config.get("mcpServers").and_then(|v| v.get("tokensave"));
    if mcp_entry.and_then(|v| v.as_object()).is_none() {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent droid`",
            mcp_path.display()
        ));
        return;
    }
    dc.pass(&format!("MCP server registered in {}", mcp_path.display()));

    let args = mcp_entry
        .and_then(|v| v.get("args"))
        .and_then(|v| v.as_array());
    let has_serve = args.is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent droid`");
    }
}

/// Check AGENTS.md contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let agents_md = home.join(".factory/AGENTS.md");
    if agents_md.exists() {
        let has_rules = std::fs::read_to_string(&agents_md)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("AGENTS.md contains tokensave rules");
        } else {
            dc.fail("AGENTS.md missing tokensave rules — run `tokensave install --agent droid`");
        }
    } else {
        dc.warn("AGENTS.md does not exist");
    }
}
