//! Cursor agent integration.
//!
//! Handles registration of the tokensave MCP server in Cursor's
//! `~/.cursor/mcp.json` and lifecycle hooks in `~/.cursor/hooks.json`.
//! Foreign hook entries (rtk, custom scripts) are preserved — only commands
//! containing `tokensave` are added or removed.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
};

/// Cursor agent.
pub struct CursorIntegration;

const CURSOR_PRE_TOOL_EVENT: &str = "preToolUse";
const CURSOR_PRE_TOOL_MATCHER: &str = "Grep|Shell";
const CURSOR_STOP_EVENT: &str = "stop";
const CURSOR_PROMPT_EVENT: &str = "beforeSubmitPrompt";
const HOOK_PRE_TOOL: &str = "hook-pre-tool-use";
const HOOK_STOP: &str = "hook-stop";
const HOOK_PROMPT: &str = "hook-prompt-submit";

impl AgentIntegration for CursorIntegration {
    fn name(&self) -> &'static str {
        "Cursor"
    }

    fn id(&self) -> &'static str {
        "cursor"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = ctx.base_dir().join(".cursor/mcp.json");

        if let Some(parent) = mcp_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let backup = backup_config_file(&mcp_path)?;
        let mut settings = match load_json_file_strict(&mcp_path) {
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
            &ctx.tokensave_bin,
        );
        settings["mcpServers"]["tokensave"] = json!({
            "command": bin,
            "args": ["serve"]
        });

        safe_write_json_file(&mcp_path, &settings, backup.as_deref())?;
        eprintln!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            mcp_path.display()
        );

        let hooks_path = cursor_hooks_path_for(ctx);
        install_hooks(&hooks_path, &ctx.tokensave_bin)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Restart Cursor — tokensave MCP tools and hooks are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = ctx.base_dir().join(".cursor/mcp.json");
        uninstall_mcp_server(&mcp_path);

        let hooks_path = cursor_hooks_path_for(ctx);
        uninstall_hooks(&hooks_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Cursor.");
        eprintln!("Restart Cursor for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mCursor integration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
        doctor_check_hooks(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".cursor").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".cursor/mcp.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        cursor_has_mcp(home)
    }
}

// ---------------------------------------------------------------------------
// Hook helpers
// ---------------------------------------------------------------------------

fn cursor_hooks_path_for(ctx: &InstallContext) -> PathBuf {
    ctx.base_dir().join(".cursor/hooks.json")
}

fn cursor_hooks_path(home: &Path) -> PathBuf {
    home.join(".cursor/hooks.json")
}

fn hook_command(tokensave_bin: &str, subcommand: &str) -> String {
    format!("{tokensave_bin} {subcommand}")
}

fn cursor_has_mcp(home: &Path) -> bool {
    let mcp_path = home.join(".cursor/mcp.json");
    if !mcp_path.exists() {
        return false;
    }
    load_json_file(&mcp_path)
        .get("mcpServers")
        .and_then(|v| v.get("tokensave"))
        .is_some()
}

fn is_tokensave_hook_entry(entry: &serde_json::Value) -> bool {
    entry
        .get("command")
        .and_then(|v| v.as_str())
        .is_some_and(|c| c.contains("tokensave"))
}

fn install_hooks(hooks_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = hooks_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(hooks_path)?;
    let mut config = if hooks_path.exists() {
        load_json_file_strict(hooks_path)?
    } else {
        json!({ "version": 1, "hooks": {} })
    };

    if config.get("version").is_none() {
        config["version"] = json!(1);
    }
    if config.get("hooks").is_none() {
        config["hooks"] = json!({});
    }

    let mut changed = false;
    changed |= upsert_hook_entry(
        &mut config,
        CURSOR_PRE_TOOL_EVENT,
        Some(CURSOR_PRE_TOOL_MATCHER),
        &hook_command(tokensave_bin, HOOK_PRE_TOOL),
    )?;
    changed |= upsert_hook_entry(
        &mut config,
        CURSOR_STOP_EVENT,
        None,
        &hook_command(tokensave_bin, HOOK_STOP),
    )?;
    changed |= upsert_hook_entry(
        &mut config,
        CURSOR_PROMPT_EVENT,
        None,
        &hook_command(tokensave_bin, HOOK_PROMPT),
    )?;

    if changed {
        safe_write_json_file(hooks_path, &config, backup.as_deref())?;
        eprintln!(
            "\x1b[32m✔\x1b[0m Added tokensave hooks to {}",
            hooks_path.display()
        );
    } else {
        eprintln!("  Cursor hooks already present, skipping");
    }
    Ok(())
}

fn upsert_hook_entry(
    config: &mut serde_json::Value,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) -> Result<bool> {
    let hooks =
        config["hooks"]
            .as_object_mut()
            .ok_or_else(|| crate::errors::TokenSaveError::Config {
                message: "hooks.json hooks field is not an object".to_string(),
            })?;

    let arr = hooks.entry(event.to_string()).or_insert_with(|| json!([]));
    let Some(entries) = arr.as_array_mut() else {
        return Err(crate::errors::TokenSaveError::Config {
            message: format!("hooks.{event} is not an array"),
        });
    };

    let subcommand = command.split_whitespace().last().unwrap_or("");
    for entry in entries.iter_mut() {
        let Some(cmd) = entry.get("command").and_then(|v| v.as_str()) else {
            continue;
        };
        if !cmd.contains("tokensave") || !cmd.contains(subcommand) {
            continue;
        }
        let matcher_ok =
            matcher.is_none_or(|m| entry.get("matcher").and_then(|v| v.as_str()) == Some(m));
        if cmd == command && matcher_ok {
            return Ok(false);
        }
        entry["command"] = json!(command);
        if let Some(m) = matcher {
            entry["matcher"] = json!(m);
        } else {
            entry.as_object_mut().and_then(|o| o.remove("matcher"));
        }
        return Ok(true);
    }

    let mut entry = json!({ "command": command });
    if let Some(m) = matcher {
        entry["matcher"] = json!(m);
    }
    entries.push(entry);
    Ok(true)
}

fn uninstall_hooks(hooks_path: &Path) {
    if !hooks_path.exists() {
        eprintln!("  {} not found, skipping", hooks_path.display());
        return;
    }

    let Ok(mut config) = load_json_file_strict(hooks_path) else {
        eprintln!("  {} is not valid JSON, skipping", hooks_path.display());
        return;
    };

    let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut()) else {
        return;
    };

    let mut removed_any = false;
    for (_event, entries_val) in hooks.iter_mut() {
        let Some(entries) = entries_val.as_array_mut() else {
            continue;
        };
        let before = entries.len();
        entries.retain(|entry| !is_tokensave_hook_entry(entry));
        if entries.len() < before {
            removed_any = true;
        }
    }

    if !removed_any {
        eprintln!("  No tokensave hooks in {}, skipping", hooks_path.display());
        return;
    }

    hooks.retain(|_, entries_val| {
        entries_val
            .as_array()
            .is_none_or(|entries| !entries.is_empty())
    });

    let hooks_empty = config
        .get("hooks")
        .and_then(|v| v.as_object())
        .is_some_and(serde_json::Map::is_empty);
    let only_version = config.as_object().is_some_and(|o| {
        o.len() <= 2 && o.contains_key("version") && (o.len() == 1 || hooks_empty)
    });

    if only_version {
        std::fs::remove_file(hooks_path).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty after tokensave uninstall)",
            hooks_path.display()
        );
    } else if backup_and_write_json(hooks_path, &config) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave hooks from {}",
            hooks_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server entry from ~/.cursor/mcp.json.
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

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check ~/.cursor/mcp.json has tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = home.join(".cursor/mcp.json");

    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent cursor` if you use Cursor",
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
            "MCP server NOT registered in {} — run `tokensave install --agent cursor`",
            mcp_path.display()
        ));
    }
}

/// Check ~/.cursor/hooks.json has tokensave lifecycle hooks.
fn doctor_check_hooks(dc: &mut DoctorCounters, home: &Path) {
    let hooks_path = cursor_hooks_path(home);
    if !hooks_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent cursor` to add hooks",
            hooks_path.display()
        ));
        return;
    }

    let Ok(config) = load_json_file_strict(&hooks_path) else {
        dc.fail(&format!(
            "{} is not valid JSON — fix syntax and re-run install",
            hooks_path.display()
        ));
        return;
    };

    let Some(hooks) = config.get("hooks") else {
        dc.fail("hooks.json missing hooks object");
        return;
    };

    let pre_ok = hooks
        .get(CURSOR_PRE_TOOL_EVENT)
        .and_then(|v| v.as_array())
        .is_some_and(|arr| {
            arr.iter().any(|e| {
                is_tokensave_hook_entry(e)
                    && e.get("matcher").and_then(|v| v.as_str()) == Some(CURSOR_PRE_TOOL_MATCHER)
            })
        });
    if pre_ok {
        dc.pass("preToolUse hook configured (Grep|Shell)");
    } else {
        dc.fail("tokensave preToolUse hook missing or wrong matcher in hooks.json");
    }

    for (event, label) in [
        (CURSOR_STOP_EVENT, "stop"),
        (CURSOR_PROMPT_EVENT, "beforeSubmitPrompt"),
    ] {
        let ok = hooks
            .get(event)
            .and_then(|v| v.as_array())
            .is_some_and(|arr| arr.iter().any(is_tokensave_hook_entry));
        if ok {
            dc.pass(&format!("{label} hook configured"));
        } else {
            dc.warn(&format!("tokensave {label} hook not found in hooks.json"));
        }
    }
}
