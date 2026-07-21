// Rust guideline compliant 2026-07-02
//! `AugmentCode` (`auggie`) agent integration.
//!
//! Handles registration of the tokensave MCP server in Augment's settings
//! (`~/.augment/settings.json` globally, `<project>/.augment/settings.json`
//! for `--local`) under the `mcpServers.tokensave` key, and prompt rules via
//! a dedicated `~/.augment/rules/tokensave.md` file (`<project>/.augment/rules/
//! tokensave.md` for `--local`). Augment discovers rules as individual
//! `*.md`/`*.mdx` files under `.augment/rules/`, unlike the single shared
//! `AGENTS.md` file some other agents append to, so a dedicated file is both
//! the natural fit and the simplest thing to remove cleanly on uninstall.
//! Workspace-scoped rules default to manual attachment unless frontmatter
//! says otherwise, so the file carries `type: always_apply`; user-level
//! rules are always treated as "always apply" regardless of frontmatter.
//! Augment also supports Claude-style `hooks` in `settings.json`, but wiring
//! those is left to a follow-up PR (needs its own I/O protocol + subcommands).

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
};

/// `AugmentCode` (`auggie`) agent.
pub struct AugmentIntegration;

impl AgentIntegration for AugmentIntegration {
    fn name(&self) -> &'static str {
        "AugmentCode"
    }

    fn id(&self) -> &'static str {
        "auggie"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let settings_path = augment_settings_path_for(ctx);
        install_mcp_server(&settings_path, &ctx.tokensave_bin)?;

        let rules_path = augment_rules_path_for(ctx);
        install_prompt_rules(&rules_path)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new auggie session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let settings_path = augment_settings_path_for(ctx);
        uninstall_mcp_server(&settings_path);

        let rules_path = augment_rules_path_for(ctx);
        uninstall_prompt_rules(&rules_path);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from AugmentCode.");
        crate::agent_note!("Start a new auggie session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mAugmentCode integration\x1b[0m");
        doctor_check_config(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".augment").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(augment_settings_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let settings_path = augment_settings_path(home);
        if !settings_path.exists() {
            return false;
        }
        let json = load_json_file(&settings_path);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Config path resolution
// ---------------------------------------------------------------------------

/// Global Augment settings path (`~/.augment/settings.json`).
fn augment_settings_path(home: &Path) -> PathBuf {
    home.join(".augment/settings.json")
}

/// settings.json path for this install: `~/.augment/settings.json` globally,
/// or `<project>/.augment/settings.json` for `--local` (same relative
/// layout — matches `auggie mcp add --project`).
fn augment_settings_path_for(ctx: &InstallContext) -> PathBuf {
    ctx.base_dir().join(".augment/settings.json")
}

/// Rules file path for this install: `~/.augment/rules/tokensave.md`
/// globally, or `<project>/.augment/rules/tokensave.md` for `--local` (same
/// relative layout).
fn augment_rules_path_for(ctx: &InstallContext) -> PathBuf {
    ctx.base_dir().join(".augment/rules/tokensave.md")
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server in settings.json.
///
/// Safety: creates a `.bak` backup before writing and restores it on any
/// error. Uses strict JSON parsing so an existing file with invalid syntax
/// is never silently replaced with an empty object. `settings.json` also
/// holds unrelated keys (`model`, `hooks`, `indexingAllowDirs`, `vimMode`,
/// …) — only `mcpServers.tokensave` is ever touched.
fn install_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(settings_path)?;
    let mut settings = match load_json_file_strict(settings_path) {
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
        "type": "stdio",
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        settings_path.display()
    );
    Ok(())
}

/// The tokensave rules file content, written as a dedicated Augment rule.
fn prompt_rules_content() -> String {
    "---\ntype: always_apply\n---\n\n\
     ## Prefer tokensave MCP tools\n\n\
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
        .to_string()
}

/// Write the dedicated tokensave rules file (idempotent — overwriting with
/// the same content is a no-op in effect).
fn install_prompt_rules(rules_path: &Path) -> Result<()> {
    if let Some(parent) = rules_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(rules_path, prompt_rules_content()).map_err(|e| {
        crate::errors::TokenSaveError::Config {
            message: format!("failed to write {}: {e}", rules_path.display()),
        }
    })?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Wrote tokensave rules to {}",
        rules_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove the tokensave MCP server entry from settings.json.
fn uninstall_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        crate::agent_note!("  {} not found, skipping", settings_path.display());
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
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    };

    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }

    let is_empty = settings.as_object().is_some_and(|o| {
        o.iter()
            .all(|(k, v)| k == "mcpServers" && v.as_object().is_some_and(serde_json::Map::is_empty))
    });

    if is_empty {
        std::fs::remove_file(settings_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            settings_path.display()
        );
    } else if backup_and_write_json(settings_path, &settings) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            settings_path.display()
        );
    }
}

/// Remove the dedicated tokensave rules file, but only if it still looks
/// like the file tokensave wrote (guards against deleting a user-authored
/// `tokensave.md` rule that happens to share the name).
fn uninstall_prompt_rules(rules_path: &Path) {
    if !rules_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(rules_path) else {
        return;
    };
    if !contents.contains("tokensave") {
        crate::agent_note!(
            "  {} does not contain tokensave rules, skipping",
            rules_path.display()
        );
        return;
    }
    std::fs::remove_file(rules_path).ok();
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Removed tokensave rules at {}",
        rules_path.display()
    );
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check settings.json has tokensave registered.
fn doctor_check_config(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = augment_settings_path(home);
    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent auggie` if you use AugmentCode",
            settings_path.display()
        ));
        return;
    }

    let config = load_json_file(&settings_path);
    let mcp_entry = config.get("mcpServers").and_then(|v| v.get("tokensave"));
    if mcp_entry.and_then(|v| v.as_object()).is_none() {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent auggie`",
            settings_path.display()
        ));
        return;
    }
    dc.pass(&format!(
        "MCP server registered in {}",
        settings_path.display()
    ));

    let args = mcp_entry
        .and_then(|v| v.get("args"))
        .and_then(|v| v.as_array());
    let has_serve = args.is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent auggie`");
    }
}

/// Check the dedicated rules file exists and contains tokensave rules.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    let rules_path = home.join(".augment/rules/tokensave.md");
    if rules_path.exists() {
        let has_rules = std::fs::read_to_string(&rules_path)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass(&format!(
                "{} contains tokensave rules",
                rules_path.display()
            ));
        } else {
            dc.fail(&format!(
                "{} missing tokensave rules — run `tokensave install --agent auggie`",
                rules_path.display()
            ));
        }
    } else {
        dc.warn(&format!("{} does not exist", rules_path.display()));
    }
}
