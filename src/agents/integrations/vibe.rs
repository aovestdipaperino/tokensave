//! Mistral Vibe agent integration.
//!
//! Handles registration of the tokensave MCP server in Vibe's
//! `~/.vibe/config.toml` as a `[[mcp_servers]]` entry with stdio transport,
//! and prompt rules via `~/.vibe/prompts/cli.md`.

use std::io::Write;
use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::{
    check_shared_rules_block, remove_legacy_rules_block, remove_rules_block, rules_for_agent,
    write_rules_block, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    LEGACY_RULES_MARKER,
};

/// Mistral Vibe agent.
pub struct VibeIntegration;

/// Returns the Vibe home directory.
/// Respects `VIBE_HOME` only when it falls under `home` (so tests with
/// temp-dir homes are not polluted by the real user's environment).
fn vibe_home(home: &Path) -> std::path::PathBuf {
    if let Ok(vibe) = std::env::var("VIBE_HOME") {
        let vibe_path = std::path::PathBuf::from(&vibe);
        if vibe_path.starts_with(home) {
            return vibe_path;
        }
    }
    home.join(".vibe")
}

fn vibe_config_path(home: &Path) -> std::path::PathBuf {
    vibe_home(home).join("config.toml")
}

fn vibe_prompt_path(home: &Path) -> std::path::PathBuf {
    vibe_home(home).join("prompts/cli.md")
}

/// The TOML marker that identifies a tokensave MCP server entry.
const TOML_MARKER: &str = "name = \"tokensave\"";

impl AgentIntegration for VibeIntegration {
    fn name(&self) -> &'static str {
        "Mistral Vibe"
    }

    fn id(&self) -> &'static str {
        "vibe"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let vibe_dir = vibe_home(&ctx.home);
        std::fs::create_dir_all(&vibe_dir).ok();

        let config_path = vibe_config_path(&ctx.home);
        install_mcp_server(&config_path, &ctx.tokensave_bin)?;

        let prompt_dir = vibe_dir.join("prompts");
        std::fs::create_dir_all(&prompt_dir).ok();
        let prompt_path = vibe_prompt_path(&ctx.home);
        install_prompt_rules(&prompt_path)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new Vibe session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let config_path = vibe_config_path(&ctx.home);
        uninstall_mcp_server(&config_path);
        uninstall_prompt_rules(&vibe_prompt_path(&ctx.home));

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Mistral Vibe.");
        crate::agent_note!("Start a new Vibe session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mMistral Vibe integration\x1b[0m");
        doctor_check_config(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        vibe_home(home).is_dir()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let config_path = vibe_config_path(home);
        if !config_path.exists() {
            return false;
        }
        let contents = std::fs::read_to_string(&config_path).unwrap_or_default();
        contents.contains(TOML_MARKER)
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Append a `[[mcp_servers]]` entry for tokensave to `config.toml` (idempotent).
fn install_mcp_server(config_path: &Path, tokensave_bin: &str) -> Result<()> {
    let existing = if config_path.exists() {
        std::fs::read_to_string(config_path).unwrap_or_default()
    } else {
        String::new()
    };

    if existing.contains(TOML_MARKER) {
        crate::agent_note!(
            "  tokensave MCP server already registered in {}, skipping",
            config_path.display()
        );
        return Ok(());
    }

    let block = format!(
        "\n[[mcp_servers]]\n\
         name = \"tokensave\"\n\
         transport = \"stdio\"\n\
         command = \"{tokensave_bin}\"\n\
         args = [\"serve\"]\n"
    );

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(config_path)
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to open {}: {e}", config_path.display()),
        })?;
    f.write_all(block.as_bytes())
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to write {}: {e}", config_path.display()),
        })?;

    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Write or refresh the tokensave rules block in cli.md.
fn install_prompt_rules(prompt_path: &Path) -> Result<()> {
    let body = rules_for_agent("vibe")?;
    write_rules_block(prompt_path, "vibe", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove the tokensave `[[mcp_servers]]` block from `config.toml`.
fn uninstall_mcp_server(config_path: &Path) {
    if !config_path.exists() {
        crate::agent_note!("  {} not found, skipping", config_path.display());
        return;
    }

    let Ok(contents) = std::fs::read_to_string(config_path) else {
        return;
    };

    if !contents.contains(TOML_MARKER) {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            config_path.display()
        );
        return;
    }

    // Remove the [[mcp_servers]] block that contains name = "tokensave".
    // Strategy: split into lines, find the block, remove it.
    let lines: Vec<&str> = contents.lines().collect();
    let mut result: Vec<&str> = Vec::new();
    let mut skip = false;

    for line in &lines {
        if line.trim() == "[[mcp_servers]]" {
            // Peek ahead to see if this block is the tokensave one.
            // We'll collect the block and decide whether to keep it.
            skip = false;
        }

        if skip {
            // If we hit a new section header, stop skipping.
            let trimmed = line.trim();
            if trimmed.starts_with("[[") || (trimmed.starts_with('[') && !trimmed.starts_with("[["))
            {
                skip = false;
            } else {
                continue;
            }
        }

        if line.contains(TOML_MARKER) {
            // This line is inside the tokensave block — remove it and
            // the preceding [[mcp_servers]] header.
            // Pop the header we already pushed.
            while let Some(last) = result.last() {
                if last.trim() == "[[mcp_servers]]" {
                    result.pop();
                    break;
                }
                // Pop blank lines between header and this line
                if last.trim().is_empty() {
                    result.pop();
                } else {
                    break;
                }
            }
            skip = true;
            continue;
        }

        result.push(line);
    }

    // Trim trailing blank lines
    while result.last().is_some_and(|l| l.trim().is_empty()) {
        result.pop();
    }

    let new_contents = if result.is_empty() {
        String::new()
    } else {
        format!("{}\n", result.join("\n"))
    };

    if new_contents.trim().is_empty() {
        std::fs::remove_file(config_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            config_path.display()
        );
    } else {
        std::fs::write(config_path, &new_contents).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            config_path.display()
        );
    }
}

/// Remove tokensave rules from cli.md (both managed-block and
/// legacy heading-guarded forms).
fn uninstall_prompt_rules(prompt_path: &Path) {
    remove_rules_block(prompt_path).ok();
    remove_legacy_rules_block(prompt_path, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

fn doctor_check_config(dc: &mut DoctorCounters, home: &Path) {
    let config_path = vibe_config_path(home);

    if !config_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent vibe` if you use Mistral Vibe",
            config_path.display()
        ));
        return;
    }

    let contents = std::fs::read_to_string(&config_path).unwrap_or_default();
    if contents.contains(TOML_MARKER) {
        dc.pass(&format!(
            "MCP server registered in {}",
            config_path.display()
        ));
    } else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent vibe`",
            config_path.display()
        ));
    }
}

fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let prompt_path = vibe_prompt_path(home);
    check_shared_rules_block(dc, &prompt_path, "vibe");
}
