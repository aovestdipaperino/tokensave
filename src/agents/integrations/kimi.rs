// Rust guideline compliant 2025-10-17
//! Moonshot Kimi CLI agent integration.
//!
//! Registers the tokensave MCP server in Kimi's `~/.kimi/mcp.json`
//! (standard `mcpServers` JSON schema, same shape as Claude/Cursor) and
//! appends prompt rules to `~/.kimi/AGENTS.md`. Kimi has no hook system
//! and no per-tool auto-approval — approval is handled globally via
//! Kimi's YOLO / AFK modes.

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::*;

/// Moonshot Kimi CLI agent.
pub struct KimiIntegration;

impl AgentIntegration for KimiIntegration {
    fn name(&self) -> &'static str {
        "Kimi CLI"
    }

    fn id(&self) -> &'static str {
        "kimi"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let kimi_dir = ctx.home.join(".kimi");
        std::fs::create_dir_all(&kimi_dir).ok();

        let mcp_path = kimi_dir.join("mcp.json");
        install_mcp_server(&mcp_path, &ctx.tokensave_bin)?;

        let agents_md = kimi_dir.join("AGENTS.md");
        install_prompt_rules(&agents_md)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new Kimi session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let kimi_dir = ctx.home.join(".kimi");
        let mcp_path = kimi_dir.join("mcp.json");
        uninstall_mcp_server(&mcp_path);

        let agents_md = kimi_dir.join("AGENTS.md");
        uninstall_prompt_rules(&agents_md);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Kimi CLI.");
        crate::agent_note!("Start a new Kimi session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mKimi CLI integration\x1b[0m");
        let kimi_dir = ctx.home.join(".kimi");
        doctor_check_mcp(dc, &kimi_dir.join("mcp.json"));
        doctor_check_prompt(dc, &kimi_dir);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".kimi").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".kimi/mcp.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = home.join(".kimi/mcp.json");
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
// Install helpers
// ---------------------------------------------------------------------------

/// Register tokensave under `mcpServers` in `~/.kimi/mcp.json`.
fn install_mcp_server(mcp_path: &Path, tokensave_bin: &str) -> Result<()> {
    let backup = backup_config_file(mcp_path)?;
    let mut settings = match load_json_file_strict(mcp_path) {
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
        tokensave_bin,
    );
    settings["mcpServers"]["tokensave"] = json!({
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(mcp_path, &settings, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        mcp_path.display()
    );
    Ok(())
}

/// Write or refresh the tokensave rules block in AGENTS.md.
fn install_prompt_rules(agents_md: &Path) -> Result<()> {
    let body = rules_for_agent("kimi")?;
    write_rules_block(agents_md, "kimi", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove tokensave from `~/.kimi/mcp.json`.
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

/// Remove tokensave rules from AGENTS.md (both managed-block and
/// legacy heading-guarded forms).
fn uninstall_prompt_rules(agents_md: &Path) {
    remove_rules_block(agents_md).ok();
    remove_legacy_rules_block(agents_md, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check `~/.kimi/mcp.json` has tokensave registered.
fn doctor_check_mcp(dc: &mut DoctorCounters, mcp_path: &Path) {
    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent kimi` if you use Kimi CLI",
            mcp_path.display()
        ));
        return;
    }
    let settings = load_json_file(mcp_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));
    if server.and_then(|v| v.as_object()).is_some() {
        dc.pass(&format!("MCP server registered in {}", mcp_path.display()));
    } else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent kimi`",
            mcp_path.display()
        ));
    }
}

/// Check AGENTS.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, kimi_dir: &Path) {
    let agents_md = kimi_dir.join("AGENTS.md");
    check_shared_rules_block(dc, &agents_md, "kimi");
}
