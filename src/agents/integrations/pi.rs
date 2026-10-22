// Rust guideline compliant 2026-10-15
//! Pi coding agent integration.
//!
//! Handles registration of the tokensave MCP server and the tokensave rules
//! block inside Pi's agent directory. Per the Pi configuration docs, the
//! agent directory is `$PI_CODING_AGENT_DIR` when set, or `<home>/.pi/agent`
//! otherwise; the MCP config lives at `<agent-dir>/mcp.json` and the user
//! instructions at `<agent-dir>/AGENTS.md` (alongside `AGENTS.override.md`,
//! `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`). Pi uses the standard MCP JSON
//! shape, so the server entry matches the other JSON-based integrations
//! (Cursor, Cline): `{ "command", "args": ["serve"] }`.
//!
//! Pi has both a user-level and a project-level configuration: the agent
//! directory holds `mcp.json` and `AGENTS.md`, while a project owns `.pi/mcp.json`
//! and the `AGENTS.md` in its root (Pi loads context files from the working
//! directory and its parents). Both scopes are supported, so `--local` targets
//! the project paths.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::*;

/// Pi coding agent.
pub struct PiIntegration;

/// Resolves the Pi agent directory.
///
/// Honors `$PI_CODING_AGENT_DIR` when set, otherwise falls back to
/// `<home>/.pi/agent`. This is the directory every per-user Pi file lives
/// in (mcp.json, AGENTS.md, settings.json, ...), so both the MCP config and
/// the rules block derive from it.
fn pi_agent_dir(home: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    home.join(".pi").join("agent")
}

/// Returns the Pi MCP config path (`<agent-dir>/mcp.json`).
fn pi_config_path(home: &Path) -> PathBuf {
    pi_agent_dir(home).join("mcp.json")
}

/// Returns the path to Pi's global instructions file (`<agent-dir>/AGENTS.md`).
fn pi_rules_path(home: &Path) -> PathBuf {
    pi_agent_dir(home).join("AGENTS.md")
}

/// Returns the project-scoped Pi MCP config path (`<project>/.pi/mcp.json`).
///
/// Pi loads project MCP servers from the `.pi` directory of the working
/// directory, once the project is trusted.
fn pi_local_config_path(project_path: &Path) -> PathBuf {
    project_path.join(".pi").join("mcp.json")
}

/// Returns the project-scoped instructions file (`<project>/AGENTS.md`).
///
/// Pi loads context files from the working directory and its parents, so the
/// `AGENTS.md` in the project root is the file Pi reads for a project.
fn pi_local_rules_path(project_path: &Path) -> PathBuf {
    project_path.join("AGENTS.md")
}

/// Returns the Pi MCP config path the install scope targets.
fn pi_mcp_path(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => pi_config_path(&ctx.home),
        InstallScope::Local { project_path } => pi_local_config_path(project_path),
    }
}

/// Returns the instructions file the install scope targets.
fn pi_instructions_path(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => pi_rules_path(&ctx.home),
        InstallScope::Local { project_path } => pi_local_rules_path(project_path),
    }
}

impl AgentIntegration for PiIntegration {
    fn name(&self) -> &'static str {
        "Pi"
    }

    fn id(&self) -> &'static str {
        "pi"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = pi_mcp_path(ctx);

        if let Some(parent) = mcp_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let backup = backup_config_file(&mcp_path)?;
        let mut settings = match load_json_file_strict(&mcp_path) {
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
            &ctx.tokensave_bin,
        );
        settings["mcpServers"]["tokensave"] = json!({
            "command": bin,
            "args": ["serve"]
        });

        safe_write_json_file(&mcp_path, &settings, backup.as_deref())?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            mcp_path.display()
        );

        install_prompt_rules(ctx)?;
        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Restart Pi — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = pi_mcp_path(ctx);
        uninstall_mcp_server(&mcp_path);
        uninstall_prompt_rules(ctx);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Pi.");
        crate::agent_note!("Restart Pi for changes to take effect.");
        Ok(())
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mPi integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
        doctor_check_local_config(dc, &ctx.project_path);
    }

    fn is_detected(&self, home: &Path) -> bool {
        let mcp_path = pi_config_path(home);
        mcp_path.exists() || mcp_path.parent().is_some_and(Path::is_dir)
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(pi_config_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = pi_config_path(home);
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
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove the tokensave MCP server entry from Pi's `mcp.json`.
fn uninstall_mcp_server(mcp_path: &Path) {
    if !mcp_path.exists() {
        crate::agent_note!("  {} not found, skipping", mcp_path.display());
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
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return;
    };

    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
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
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else if backup_and_write_json(mcp_path, &settings) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check Pi's `mcp.json` has the tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = pi_config_path(home);

    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent pi` if you use Pi",
            mcp_path.display()
        ));
        return;
    }

    let settings = load_json_file(&mcp_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    if server.and_then(|v| v.as_object()).is_some() {
        dc.pass(&format!("MCP server registered in {}", mcp_path.display()));
    } else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent pi`",
            mcp_path.display()
        ));
    }
}

/// Write or refresh the tokensave rules block in the AGENTS.md the install
/// scope targets.
fn install_prompt_rules(ctx: &InstallContext) -> Result<()> {
    let body = rules_for_agent("pi")?;
    write_rules_block(&pi_instructions_path(ctx), "pi", &body).map(|_| ())
}

/// Remove the tokensave rules block from the AGENTS.md the install scope
/// targets.
fn uninstall_prompt_rules(ctx: &InstallContext) {
    let rules_path = pi_instructions_path(ctx);
    remove_rules_block(&rules_path).ok();
    remove_legacy_rules_block(&rules_path, LEGACY_RULES_MARKER, &[]).ok();
}

/// Check Pi's AGENTS.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    check_shared_rules_block(dc, &pi_rules_path(home), "pi");
}

/// Check the project-scoped Pi configuration of the current project.
///
/// Only reported when the project actually owns a `.pi` directory: a project
/// without one has no project-scoped Pi configuration to complain about.
fn doctor_check_local_config(dc: &mut DoctorCounters, project_path: &Path) {
    if !project_path.join(".pi").is_dir() {
        return;
    }

    crate::agent_note!("\n\x1b[1mLocal config\x1b[0m");

    let mcp_path = pi_local_config_path(project_path);
    if mcp_path.exists() {
        let settings = load_json_file(&mcp_path);
        let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));
        if server.and_then(|v| v.as_object()).is_some() {
            dc.pass(&format!("MCP server registered in {}", mcp_path.display()));
        } else {
            dc.fail(&format!(
                "MCP server NOT registered in {} — run `tokensave install --agent pi --local`",
                mcp_path.display()
            ));
        }
    } else {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent pi --local` to register it for this project",
            mcp_path.display()
        ));
    }

    check_shared_rules_block(dc, &pi_local_rules_path(project_path), "pi");
}
