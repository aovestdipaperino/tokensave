// Rust guideline compliant 2025-10-17
//! Claude Code agent integration.
//!
//! Handles registration of the tokensave MCP server in Claude Code's config
//! files (`~/.claude.json`, `~/.claude/settings.json`), tool permissions,
//! the `PreToolUse` hook, CLAUDE.md prompt rules, and health checks.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::{
    backup_and_write_json, backup_config_file, expected_tool_perms, load_json_file_strict,
    managed_rules_markdown, remove_legacy_rules_block, remove_managed_rules_file,
    safe_write_json_file, write_json_file, write_managed_rules_file, AgentIntegration,
    DoctorCounters, HealthcheckContext, InstallContext, InstallScope, RulesVariant,
};

/// Claude Code agent.
pub struct ClaudeIntegration;

impl AgentIntegration for ClaudeIntegration {
    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn id(&self) -> &'static str {
        "claude"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        match &ctx.scope {
            InstallScope::Global => install_global(ctx),
            InstallScope::Local { project_path } => install_local(ctx, project_path),
        }
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        match &ctx.scope {
            InstallScope::Global => uninstall_global(ctx),
            InstallScope::Local { project_path } => uninstall_local(project_path),
        }
        Ok(())
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mClaude Code integration\x1b[0m");
        doctor_check_claude_json(dc, &ctx.home);
        doctor_check_settings_json(dc, &ctx.home);
        doctor_check_claude_md(dc, &ctx.home);
        doctor_check_local_config(dc, &ctx.project_path);
    }

    fn is_detected(&self, home: &Path) -> bool {
        claude_config_dir(home).is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(claude_json_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let claude_json = claude_json_path(home);
        if !claude_json.exists() {
            return false;
        }
        let json = super::load_json_file(&claude_json);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

/// Resolves Claude Code's user-level config directory (normally `~/.claude`).
///
/// Claude Code honors `CLAUDE_CONFIG_DIR` to relocate its entire user-level
/// config; when set, `settings.json`, `CLAUDE.md`, and `.claude.json` all live
/// under that directory rather than `~/.claude` / `~/.claude.json`. Writing to
/// the hardcoded home paths meant the install silently landed in a directory
/// Claude Code never reads (#191). A comma-separated value takes the first
/// entry. An empty value is treated as unset.
fn claude_config_dir(home: &Path) -> PathBuf {
    claude_config_dir_override().unwrap_or_else(|| home.join(".claude"))
}

/// Resolves the path to Claude Code's `.claude.json` (MCP registrations).
///
/// Normally `~/.claude.json`; under `CLAUDE_CONFIG_DIR` it moves to
/// `$CLAUDE_CONFIG_DIR/.claude.json` (#191).
fn claude_json_path(home: &Path) -> PathBuf {
    match claude_config_dir_override() {
        Some(dir) => dir.join(".claude.json"),
        None => home.join(".claude.json"),
    }
}

/// Path to the managed tokensave rules file. Claude Code auto-loads every
/// `.md` file under `~/.claude/rules/` at the same priority as `CLAUDE.md`
/// (recursively, no approval dialog for user-scope files — see
/// <https://code.claude.com/docs/en/memory>), so this needs no CLAUDE.md
/// edit at all. `claude_dir` is either the global config dir (from
/// [`claude_config_dir`]) or `<project>/.claude` for `--local`.
fn claude_rules_path(claude_dir: &Path) -> PathBuf {
    claude_dir.join("rules").join("tokensave.md")
}

/// The directory named by `CLAUDE_CONFIG_DIR`, or `None` when unset/empty.
fn claude_config_dir_override() -> Option<PathBuf> {
    let raw = std::env::var_os("CLAUDE_CONFIG_DIR")?;
    parse_config_dir_value(&raw.to_string_lossy())
}

/// Parses a raw `CLAUDE_CONFIG_DIR` value into the primary config directory.
///
/// Empty/whitespace → `None`. ccusage-style comma-separated lists are
/// unverified for Claude Code itself, so the primary, writable dir is taken as
/// the first entry.
fn parse_config_dir_value(raw: &str) -> Option<PathBuf> {
    let first = raw.split(',').next().unwrap_or("").trim();
    if first.is_empty() {
        None
    } else {
        Some(PathBuf::from(first))
    }
}

fn install_global(ctx: &InstallContext) -> Result<()> {
    let claude_dir = claude_config_dir(&ctx.home);
    let settings_path = claude_dir.join("settings.json");
    let claude_json_path = claude_json_path(&ctx.home);
    let claude_md_path = claude_dir.join("CLAUDE.md");

    install_mcp_server(&claude_json_path, &ctx.tokensave_bin)?;

    std::fs::create_dir_all(&claude_dir).ok();
    let mut settings = load_json_file_strict(&settings_path)?;
    install_migrate_old_mcp(&mut settings, &settings_path);
    install_hook(&mut settings, &ctx.tokensave_bin);
    install_permissions(
        &mut settings,
        &ctx.tool_permissions,
        ctx.force_permission_style,
    );
    write_json_file(&settings_path, &settings)?;

    // Migration: strip any pre-#256 inline block from CLAUDE.md, then write
    // the managed rules file Claude Code auto-loads from `rules/*.md`. A
    // migration failure (e.g. CLAUDE.md unreadable/unwritable) must fail the
    // install, not be swallowed — silently succeeding here would report
    // install as complete while stale rules text stays stuck in CLAUDE.md.
    uninstall_claude_md_rules(&claude_md_path)?;
    write_managed_rules_file(
        &claude_rules_path(&claude_dir),
        &managed_rules_markdown(RulesVariant::Claude),
    )?;
    install_clean_local_config();

    crate::agent_note!();
    crate::agent_note!("Setup complete. Next steps:");
    crate::agent_note!("  1. cd into your project and run: tokensave init");
    crate::agent_note!("  2. Start a new Claude Code session — tokensave tools are now available");
    Ok(())
}

fn install_local(ctx: &InstallContext, project: &Path) -> Result<()> {
    let claude_dir = project.join(".claude");
    let settings_path = claude_dir.join("settings.json");
    let mcp_json_path = project.join(".mcp.json");
    let claude_md_path = project.join("CLAUDE.md");

    // Project-scoped MCP server lives in ./.mcp.json (Claude Code's project file).
    install_mcp_server(&mcp_json_path, &ctx.tokensave_bin)?;

    std::fs::create_dir_all(&claude_dir).ok();
    let mut settings = load_json_file_strict(&settings_path)?;
    install_hook(&mut settings, &ctx.tokensave_bin);
    install_permissions(
        &mut settings,
        &ctx.tool_permissions,
        ctx.force_permission_style,
    );
    write_json_file(&settings_path, &settings)?;

    uninstall_claude_md_rules(&claude_md_path)?;
    write_managed_rules_file(
        &claude_rules_path(&claude_dir),
        &managed_rules_markdown(RulesVariant::Claude),
    )?;
    // NB: no install_clean_local_config() — that is the global-only cleanup.

    crate::agent_note!();
    crate::agent_note!(
        "Project setup complete (\x1b[1m{}\x1b[0m).",
        project.display()
    );
    crate::agent_note!("  Tokensave is registered for this project only (./.mcp.json).");
    crate::agent_note!("  Run: tokensave init   then start a new Claude Code session.");
    Ok(())
}

fn uninstall_global(ctx: &InstallContext) {
    let claude_dir = claude_config_dir(&ctx.home);
    let settings_path = claude_dir.join("settings.json");
    let claude_json_path = claude_json_path(&ctx.home);
    let claude_md_path = claude_dir.join("CLAUDE.md");

    uninstall_mcp_server(&claude_json_path);
    uninstall_settings(&settings_path);
    uninstall_claude_md_rules(&claude_md_path).ok(); // legacy pre-#256 installs, best-effort like the rest of uninstall
    remove_managed_rules_file(&claude_rules_path(&claude_dir));

    crate::agent_note!();
    crate::agent_note!("Uninstall complete. Tokensave has been removed from Claude Code.");
    crate::agent_note!("Start a new Claude Code session for changes to take effect.");
}

fn uninstall_local(project: &Path) {
    uninstall_mcp_server(&project.join(".mcp.json"));
    uninstall_settings(&project.join(".claude/settings.json"));
    uninstall_claude_md_rules(&project.join("CLAUDE.md")).ok(); // legacy pre-#256 installs, best-effort like the rest of uninstall
    remove_managed_rules_file(&claude_rules_path(&project.join(".claude")));

    crate::agent_note!();
    crate::agent_note!(
        "Removed tokensave from this project ({}).",
        project.display()
    );
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server in ~/.claude.json.
fn install_mcp_server(claude_json_path: &Path, tokensave_bin: &str) -> Result<()> {
    let backup = backup_config_file(claude_json_path)?;
    let mut claude_json = match load_json_file_strict(claude_json_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                crate::agent_note!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };

    let bin = crate::agents::preserve_mcp_command(
        claude_json.pointer("/mcpServers/tokensave/command"),
        tokensave_bin,
    );
    claude_json["mcpServers"]["tokensave"] = json!({
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(claude_json_path, &claude_json, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        claude_json_path.display()
    );
    Ok(())
}

/// Remove stale MCP server from old location in settings.json.
fn install_migrate_old_mcp(settings: &mut serde_json::Value, settings_path: &Path) {
    if let Some(servers) = settings
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    {
        if servers.remove("tokensave").is_some() {
            if servers.is_empty() {
                settings.as_object_mut().map(|o| o.remove("mcpServers"));
            }
            crate::agent_note!(
                "\x1b[32m✔\x1b[0m Removed tokensave MCP server from old location ({})",
                settings_path.display()
            );
        }
    }
}

/// Add all tokensave hooks (idempotent). Prints progress messages.
fn install_hook(settings: &mut serde_json::Value, tokensave_bin: &str) {
    install_hook_inner(settings, tokensave_bin, false);
}

/// Add all tokensave hooks silently (for post-upgrade migration).
fn install_hook_quiet(settings: &mut serde_json::Value, tokensave_bin: &str) {
    install_hook_inner(settings, tokensave_bin, true);
}

fn install_hook_inner(settings: &mut serde_json::Value, tokensave_bin: &str, quiet: bool) {
    install_single_hook(
        settings,
        "PreToolUse",
        tokensave_bin,
        "hook-pre-tool-use",
        expected_hook_matcher("PreToolUse"),
        quiet,
    );
    install_single_hook(
        settings,
        "UserPromptSubmit",
        tokensave_bin,
        "hook-prompt-submit",
        None,
        quiet,
    );
    install_single_hook(settings, "Stop", tokensave_bin, "hook-stop", None, quiet);
}

/// Install a single hook entry under `settings.hooks.<event>` (idempotent).
///
/// Writes the modern Claude Code shape `{type, command, args}`, where the exe
/// path is the entire `command` and the subcommand is the only entry in
/// `args`. This sidesteps Claude Code's whitespace-splitter so install paths
/// containing spaces work unchanged.
fn install_single_hook(
    settings: &mut serde_json::Value,
    event: &str,
    tokensave_bin: &str,
    subcommand: &str,
    matcher: Option<&str>,
    quiet: bool,
) {
    let hooks_arr = settings["hooks"][event]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let has_hook = hooks_arr
        .iter()
        .any(|h| hook_entry_command(h).is_some_and(|c| c.contains("tokensave")));

    if !has_hook {
        let mut new_hooks = hooks_arr;
        let mut entry = json!({
            "hooks": [{
                "type": "command",
                "command": tokensave_bin,
                "args": [subcommand],
            }]
        });
        if let Some(m) = matcher {
            entry["matcher"] = json!(m);
        }
        new_hooks.push(entry);
        settings["hooks"][event] = serde_json::Value::Array(new_hooks);
        if !quiet {
            crate::agent_note!("\x1b[32m✔\x1b[0m Added {event} hook");
        }
    } else if !quiet {
        crate::agent_note!("  {event} hook already present, skipping");
    }
}

/// Extract the `command` string from a hook event entry (the wrapper that
/// holds an `"hooks": [{...}]` array). Returns the first inner command.
fn hook_entry_command(entry: &serde_json::Value) -> Option<&str> {
    entry
        .get("hooks")?
        .as_array()?
        .iter()
        .find_map(|c| c.get("command").and_then(|v| v.as_str()))
}

/// Parse a hook inner-entry into `(bin, subcommand)`.
///
/// Accepts both the modern `{command, args: [subcmd]}` shape and the legacy
/// single-string `"bin subcmd"` shape (which is broken for paths with
/// spaces). The legacy variant is returned so callers can detect it and
/// rewrite, but the subcommand split is intentionally best-effort.
fn parse_hook_command(cmd_entry: &serde_json::Value) -> Option<(String, String)> {
    let command = cmd_entry.get("command")?.as_str()?;
    if let Some(args) = cmd_entry.get("args").and_then(|a| a.as_array()) {
        let sub = args.iter().find_map(|v| v.as_str()).unwrap_or("");
        return Some((command.to_string(), sub.to_string()));
    }
    // Legacy single-string shape — best-effort split on first space.
    let mut parts = command.splitn(2, char::is_whitespace);
    let bin = parts.next().unwrap_or("").to_string();
    let sub = parts.next().unwrap_or("").to_string();
    Some((bin, sub))
}

/// Find the first tokensave hook entry under an event and return
/// `(bin, subcommand, is_legacy_shape)`. `is_legacy_shape` is true when the
/// entry uses the broken single-string command shape and needs rewriting.
fn find_tokensave_hook(
    settings: &serde_json::Value,
    event: &str,
) -> Option<(String, String, bool)> {
    let arr = settings["hooks"][event].as_array()?;
    arr.iter().find_map(|wrapper| {
        let cmd_entry = wrapper.get("hooks")?.as_array()?.first()?;
        let raw_command = cmd_entry.get("command").and_then(|c| c.as_str())?;
        if !raw_command.contains("tokensave") {
            return None;
        }
        let (bin, sub) = parse_hook_command(cmd_entry)?;
        let is_legacy = cmd_entry.get("args").is_none();
        Some((bin, sub, is_legacy))
    })
}

/// True for any tokensave-owned permission entry: an individual tool grant,
/// the bare server-wide grant, or the compact wildcard. Shared by
/// `install_permissions` (to prune before re-adding, so switching between the
/// explicit and compact styles never leaves stale entries behind) and
/// `uninstall_permissions`.
fn is_tokensave_perm(s: &str) -> bool {
    s == "mcp__tokensave" || s.starts_with("mcp__tokensave__")
}

/// Add MCP tool permissions (idempotent). Any previously-installed
/// tokensave-owned entries are dropped first, so re-running install after
/// switching between the explicit per-tool list and the compact wildcard
/// (see `wildcard_permissions` in `UserConfig`) always leaves exactly the
/// entries in `tool_permissions` — never a mix of both styles.
///
/// `force_style` distinguishes an explicit `--wildcard-permissions` /
/// `--explicit-permissions` request from every default/silent path (flagless
/// `install`/`reinstall`, the silent reinstall-on-upgrade). When `false` and
/// the existing `allow` list already has a single entry that covers every
/// expected tool (a hand-written `mcp__tokensave__*`, bare `mcp__tokensave`,
/// or an anchored glob spanning all tools), that grant is left untouched
/// instead of being pruned and re-inflated into the explicit list — a user's
/// existing compact grant should survive a silent reinstall, not get
/// clobbered just because their config predates this feature.
fn install_permissions(
    settings: &mut serde_json::Value,
    tool_permissions: &[String],
    force_style: bool,
) {
    let existing: Vec<String> = settings["permissions"]["allow"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    if !force_style {
        let expected = expected_tool_perms();
        let has_compact_cover = existing.iter().any(|e| {
            let single = [e.as_str()];
            expected.iter().all(|p| perm_is_covered(p, &single))
        });
        if has_compact_cover {
            crate::agent_note!("\x1b[32m✔\x1b[0m Tool permissions already granted");
            return;
        }
    }

    let mut allow: Vec<String> = existing
        .into_iter()
        .filter(|e| !is_tokensave_perm(e))
        .collect();
    for tool in tool_permissions {
        if !allow.iter().any(|e| e == tool) {
            allow.push(tool.clone());
        }
    }
    allow.sort();
    allow.dedup();
    settings["permissions"]["allow"] =
        serde_json::Value::Array(allow.into_iter().map(serde_json::Value::String).collect());
    crate::agent_note!("\x1b[32m✔\x1b[0m Added tool permissions");
}

/// Clean up local project config (.mcp.json and settings.local.json).
fn install_clean_local_config() {
    let project_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let mcp_json_path = project_path.join(".mcp.json");
    if mcp_json_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&mcp_json_path) {
            if let Ok(mut mcp_val) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(servers) = mcp_val
                    .get_mut("mcpServers")
                    .and_then(|v| v.as_object_mut())
                {
                    if servers.remove("tokensave").is_some() {
                        if servers.is_empty() {
                            std::fs::remove_file(&mcp_json_path).ok();
                            crate::agent_note!(
                                "\x1b[32m✔\x1b[0m Removed local .mcp.json (using global config only)"
                            );
                        } else if backup_and_write_json(&mcp_json_path, &mcp_val) {
                            crate::agent_note!("\x1b[32m✔\x1b[0m Removed tokensave from local .mcp.json (using global config only)");
                        }
                    }
                }
            }
        }
    }

    let local_settings_path = project_path.join(".claude").join("settings.local.json");
    if local_settings_path.exists() {
        clean_local_settings_file(&project_path, &local_settings_path);
    }
}

/// Remove tokensave entries from a local settings.local.json file.
fn clean_local_settings_file(project_path: &Path, local_settings_path: &Path) {
    let Ok(contents) = std::fs::read_to_string(local_settings_path) else {
        return;
    };
    if !contents.contains("tokensave") {
        return;
    }
    let Ok(mut local_val) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let mut modified = false;

    if let Some(arr) = local_val
        .get_mut("enabledMcpjsonServers")
        .and_then(|v| v.as_array_mut())
    {
        let before = arr.len();
        arr.retain(|v| v.as_str() != Some("tokensave"));
        if arr.len() < before {
            modified = true;
        }
    }

    if let Some(servers) = local_val
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    {
        if servers.remove("tokensave").is_some() {
            modified = true;
            if servers.is_empty() {
                local_val.as_object_mut().map(|o| o.remove("mcpServers"));
            }
        }
    }

    if modified {
        clean_orphaned_local_mcp_keys(&mut local_val);
    }

    if !modified {
        return;
    }

    let is_empty = local_val.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        if std::fs::remove_file(local_settings_path).is_ok() {
            crate::agent_note!(
                "\x1b[32m✔\x1b[0m Removed {} (tokensave should only be in global config)",
                local_settings_path.display()
            );
            let claude_dir = project_path.join(".claude");
            std::fs::remove_dir(&claude_dir).ok();
        }
    } else if backup_and_write_json(local_settings_path, &local_val) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave entries from {} (should only be in global config)",
            local_settings_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.claude.json.
fn uninstall_mcp_server(claude_json_path: &Path) {
    if !claude_json_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(claude_json_path) else {
        return;
    };
    let Ok(mut claude_json) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = claude_json
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!("  No tokensave MCP server in ~/.claude.json, skipping");
        return;
    }
    if servers.is_empty() {
        claude_json.as_object_mut().map(|o| o.remove("mcpServers"));
    }
    let is_empty = claude_json
        .as_object()
        .is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(claude_json_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            claude_json_path.display()
        );
    } else if backup_and_write_json(claude_json_path, &claude_json) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            claude_json_path.display()
        );
    }
}

/// Remove hook, permissions, and stale MCP from settings.json.
fn uninstall_settings(settings_path: &Path) {
    if !settings_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(settings_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let mut modified = false;

    modified |= uninstall_stale_mcp(&mut settings);
    modified |= uninstall_hook(&mut settings);
    modified |= uninstall_permissions(&mut settings);

    if modified && backup_and_write_json(settings_path, &settings) {
        crate::agent_note!("\x1b[32m✔\x1b[0m Wrote {}", settings_path.display());
    }
}

/// Remove stale MCP server from settings.json. Returns true if modified.
fn uninstall_stale_mcp(settings: &mut serde_json::Value) -> bool {
    if let Some(servers) = settings
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    {
        if servers.remove("tokensave").is_some() {
            if servers.is_empty() {
                settings.as_object_mut().map(|o| o.remove("mcpServers"));
            }
            crate::agent_note!(
                "\x1b[32m✔\x1b[0m Removed stale tokensave MCP server from settings.json"
            );
            return true;
        }
    }
    false
}

/// Remove all tokensave hooks. Returns true if modified.
fn uninstall_hook(settings: &mut serde_json::Value) -> bool {
    let mut modified = false;
    for event in &["PreToolUse", "UserPromptSubmit", "Stop"] {
        modified |= uninstall_single_hook(settings, event);
    }
    modified
}

/// Remove tokensave entries from a single hook event. Returns true if modified.
fn uninstall_single_hook(settings: &mut serde_json::Value, event: &str) -> bool {
    let Some(arr) = settings["hooks"][event].as_array().cloned() else {
        return false;
    };
    let filtered: Vec<serde_json::Value> = arr
        .into_iter()
        .filter(|h| {
            !h.get("hooks")
                .and_then(|a| a.as_array())
                .is_some_and(|arr| {
                    arr.iter().any(|entry| {
                        entry
                            .get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|c| c.contains("tokensave"))
                    })
                })
        })
        .collect();
    if filtered.len()
        >= settings["hooks"][event]
            .as_array()
            .map_or(0, std::vec::Vec::len)
    {
        return false;
    }
    if filtered.is_empty() {
        if let Some(hooks) = settings.get_mut("hooks").and_then(|v| v.as_object_mut()) {
            hooks.remove(event);
            if hooks.is_empty() {
                settings.as_object_mut().map(|o| o.remove("hooks"));
            }
        }
    } else {
        settings["hooks"][event] = serde_json::Value::Array(filtered);
    }
    crate::agent_note!("\x1b[32m✔\x1b[0m Removed {event} hook");
    true
}

/// Remove tokensave tool permissions. Returns true if modified.
fn uninstall_permissions(settings: &mut serde_json::Value) -> bool {
    let Some(arr) = settings["permissions"]["allow"].as_array().cloned() else {
        return false;
    };
    let filtered: Vec<serde_json::Value> = arr
        .into_iter()
        .filter(|v| !v.as_str().is_some_and(is_tokensave_perm))
        .collect();
    if filtered.len()
        >= settings["permissions"]["allow"]
            .as_array()
            .map_or(0, std::vec::Vec::len)
    {
        return false;
    }
    if filtered.is_empty() {
        if let Some(perms) = settings
            .get_mut("permissions")
            .and_then(|v| v.as_object_mut())
        {
            perms.remove("allow");
            if perms.is_empty() {
                settings.as_object_mut().map(|o| o.remove("permissions"));
            }
        }
    } else {
        settings["permissions"]["allow"] = serde_json::Value::Array(filtered);
    }
    crate::agent_note!("\x1b[32m✔\x1b[0m Removed tokensave tool permissions");
    true
}

/// Remove the pre-#256 tokensave rules block from CLAUDE.md, if present.
///
/// Thin wrapper around the shared [`remove_legacy_rules_block`] engine,
/// supplying Claude's marker heading and its one known sub-heading (so the
/// end-of-block search doesn't stop early on it) — see that function for the
/// backup/atomic-write/error-propagation contract.
fn uninstall_claude_md_rules(claude_md_path: &Path) -> Result<()> {
    remove_legacy_rules_block(
        claude_md_path,
        "## MANDATORY: No Explore Agents When Tokensave Is Available",
        &["## When you spawn an Explore agent in a tokensave-enabled project"],
    )
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check ~/.claude.json MCP server registration.
fn doctor_check_claude_json(dc: &mut DoctorCounters, home: &Path) {
    let claude_json_path = claude_json_path(home);
    if !claude_json_path.exists() {
        dc.fail("~/.claude.json not found — run `tokensave install`");
        return;
    }
    let claude_json_ok = std::fs::read_to_string(&claude_json_path)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok());

    let Some(claude_json) = claude_json_ok else {
        dc.fail("Could not parse ~/.claude.json");
        return;
    };

    dc.pass(&format!(
        "Global MCP config: {}",
        claude_json_path.display()
    ));

    let mcp_entry = &claude_json["mcpServers"]["tokensave"];
    if !mcp_entry.is_object() {
        dc.fail("MCP server NOT registered in ~/.claude.json — run `tokensave install`");
        return;
    }
    dc.pass("MCP server registered in ~/.claude.json");
    doctor_check_mcp_binary(dc, mcp_entry);

    let args_ok = mcp_entry["args"]
        .as_array()
        .is_some_and(|a| a.first().and_then(|v| v.as_str()) == Some("serve"));
    if args_ok {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install`");
    }
}

/// Validate MCP binary path and match against current executable.
fn doctor_check_mcp_binary(dc: &mut DoctorCounters, mcp_entry: &serde_json::Value) {
    let Some(mcp_cmd) = mcp_entry["command"].as_str() else {
        dc.fail("MCP server entry missing \"command\" field — run `tokensave install`");
        return;
    };
    let mcp_bin = Path::new(mcp_cmd);
    if !mcp_bin.exists() {
        dc.fail(&format!(
            "MCP binary not found: {mcp_cmd} — run `tokensave install`"
        ));
        return;
    }
    dc.pass(&format!("MCP binary exists: {mcp_cmd}"));

    if let Ok(current_exe) = std::env::current_exe() {
        let current = current_exe.canonicalize().unwrap_or(current_exe);
        let registered = mcp_bin.canonicalize().unwrap_or(mcp_bin.to_path_buf());
        if current == registered {
            dc.pass("MCP binary matches current executable");
        } else {
            dc.warn(&format!(
                "MCP binary differs from current executable\n\
                 \x1b[33m      registered:\x1b[0m {mcp_cmd}\n\
                 \x1b[33m      running:\x1b[0m   {}",
                current.display()
            ));
        }
    }
}

/// Check ~/.claude/settings.json for hook, permissions, and stale entries.
/// Auto-repairs missing hooks when a tokensave binary can be determined.
fn doctor_check_settings_json(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = claude_config_dir(home).join("settings.json");

    // Check for stale MCP server in old location
    if settings_path.exists() {
        if let Some(settings) = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        {
            if settings["mcpServers"]["tokensave"].is_object() {
                dc.warn("Stale MCP server entry in ~/.claude/settings.json — run `tokensave install` to migrate");
            }
        }
    }

    if !settings_path.exists() {
        dc.fail("~/.claude/settings.json not found — run `tokensave install`");
        return;
    }

    let settings_ok = std::fs::read_to_string(&settings_path)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok());

    let Some(settings) = settings_ok else {
        dc.fail("Could not parse settings.json");
        return;
    };

    dc.pass(&format!("Settings: {}", settings_path.display()));
    doctor_check_hook(dc, &settings);
    doctor_fix_hooks(dc, &settings_path, &settings);
    doctor_check_permissions(dc, &settings);
}

/// Expected subcommand for each hook event.
fn expected_hook_subcommand(event: &str) -> &'static str {
    match event {
        "PreToolUse" => "hook-pre-tool-use",
        "UserPromptSubmit" => "hook-prompt-submit",
        "Stop" => "hook-stop",
        _ => unreachable!("unexpected hook event: {event}"),
    }
}

/// Expected hook matcher for each event, or `None` when the event is unmatched.
///
/// `PreToolUse` runs for `Agent`, `Grep`, and `Bash` — the latter two redirect
/// symbol-shaped greps to `tokensave_search` / `tokensave_signature_search` /
/// `tokensave_callers`.
fn expected_hook_matcher(event: &str) -> Option<&'static str> {
    match event {
        "PreToolUse" => Some("Agent|Grep|Bash"),
        _ => None,
    }
}

/// Find the matcher string currently installed on a tokensave hook entry.
/// Returns `None` if the hook isn't installed or has no matcher field.
fn find_tokensave_hook_matcher(settings: &serde_json::Value, event: &str) -> Option<String> {
    let arr = settings["hooks"][event].as_array()?;
    arr.iter().find_map(|wrapper| {
        let cmd = wrapper.get("hooks")?.as_array()?.first()?;
        cmd.get("command")
            .and_then(|c| c.as_str())?
            .contains("tokensave")
            .then_some(())?;
        wrapper
            .get("matcher")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}

/// Check all tokensave hooks in settings.
fn doctor_check_hook(dc: &mut DoctorCounters, settings: &serde_json::Value) {
    for event in &["PreToolUse", "UserPromptSubmit", "Stop"] {
        doctor_check_single_hook(dc, settings, event);
    }
}

/// Check a single hook event for a tokensave entry.
/// Validates that the subcommand is correct for this event.
fn doctor_check_single_hook(dc: &mut DoctorCounters, settings: &serde_json::Value, event: &str) {
    let Some((bin, sub, is_legacy)) = find_tokensave_hook(settings, event) else {
        dc.fail(&format!("{event} hook NOT installed"));
        return;
    };

    let expected_sub = expected_hook_subcommand(event);
    if is_legacy {
        dc.fail(&format!(
            "{event} hook uses legacy single-string shape (breaks on paths with spaces) — will be auto-repaired"
        ));
        return;
    }
    if sub != expected_sub {
        dc.fail(&format!(
            "{event} hook has wrong subcommand: \"{sub}\" (expected \"{expected_sub}\")"
        ));
        return;
    }

    if let Some(expected_matcher) = expected_hook_matcher(event) {
        let actual = find_tokensave_hook_matcher(settings, event);
        if actual.as_deref() != Some(expected_matcher) {
            dc.fail(&format!(
                "{event} hook has stale matcher: {:?} (expected \"{expected_matcher}\") — \
                 will be auto-repaired so the redirect catches Grep and Bash too",
                actual.unwrap_or_default()
            ));
            return;
        }
    }

    dc.pass(&format!("{event} hook installed"));

    if Path::new(&bin).exists() {
        dc.pass(&format!("Hook binary exists: {bin}"));
    } else {
        dc.fail(&format!(
            "Hook binary not found: {bin} — run `tokensave install`"
        ));
    }
}

/// Auto-repair missing or misconfigured hooks. Only touches hooks that are
/// actually wrong — correctly configured hooks are left untouched.
///
/// Bin resolution per event:
/// - missing → use `current_exe()`
/// - legacy single-string shape → use `current_exe()` (the embedded path
///   cannot be parsed unambiguously when it contains spaces — issue #81)
/// - modern shape with wrong subcommand → reuse the existing bin
fn doctor_fix_hooks(dc: &mut DoctorCounters, settings_path: &Path, settings: &serde_json::Value) {
    let current_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from));

    let mut settings = settings.clone();
    let mut repaired = false;

    for event in &["PreToolUse", "UserPromptSubmit", "Stop"] {
        let expected_sub = expected_hook_subcommand(event);
        let expected_matcher = expected_hook_matcher(event);

        let current = find_tokensave_hook(&settings, event);
        let matcher_ok = expected_matcher
            .is_none_or(|m| find_tokensave_hook_matcher(&settings, event).as_deref() == Some(m));
        let correct = current
            .as_ref()
            .is_some_and(|(_, s, legacy)| !*legacy && s == expected_sub)
            && matcher_ok;
        if correct {
            continue;
        }

        let bin = match &current {
            // Modern shape with wrong subcommand or stale matcher: keep user's bin path.
            Some((b, _, false)) => Some(b.clone()),
            // Legacy shape or missing: only repair if we know our own path.
            _ => current_exe.clone(),
        };
        let Some(bin) = bin else {
            continue;
        };

        if current.is_some() {
            uninstall_single_hook(&mut settings, event);
        }
        install_single_hook(
            &mut settings,
            event,
            &bin,
            expected_sub,
            expected_matcher,
            true,
        );
        repaired = true;
    }

    if repaired {
        if backup_and_write_json(settings_path, &settings) {
            dc.pass("Auto-repaired hook(s)");
        } else {
            dc.fail("Could not write settings.json to repair hooks");
        }
    }
}

/// Check tool permissions and detect stale ones.
fn doctor_check_permissions(dc: &mut DoctorCounters, settings: &serde_json::Value) {
    let installed: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let expected = expected_tool_perms();
    let missing: Vec<&String> = expected
        .iter()
        .filter(|p| !perm_is_covered(p, &installed))
        .collect();

    if missing.is_empty() {
        dc.pass(&format!("All {} tool permissions granted", expected.len()));
    } else {
        dc.fail(&format!(
            "{} tool permission(s) missing — run `tokensave install`",
            missing.len()
        ));
        for perm in &missing {
            dc.info(&format!("missing: {perm}"));
        }
    }

    // A covering grant (bare "mcp__tokensave" or a wildcard/glob anchored on
    // "mcp__tokensave__") is a deliberate compact grant, not a stale leftover
    // from an older version — only flag entries that look like individually
    // installed tool permissions no longer in the expected set.
    let stale: Vec<&&str> = installed
        .iter()
        .filter(|p| {
            p.starts_with("mcp__tokensave__")
                && !p.ends_with('*')
                && !expected.contains(&p.to_string())
        })
        .collect();
    if !stale.is_empty() {
        dc.warn(&format!(
            "{} stale permission(s) from older version (harmless)",
            stale.len()
        ));
    }
}

/// Check the managed tokensave rules file exists (issue #256: rules live in
/// `~/.claude/rules/tokensave.md`, not appended to the user's CLAUDE.md).
fn doctor_check_claude_md(dc: &mut DoctorCounters, home: &Path) {
    let rules_path = claude_rules_path(&claude_config_dir(home));
    if rules_path.exists() {
        let has_rules = std::fs::read_to_string(&rules_path)
            .unwrap_or_default()
            .contains("tokensave");
        if has_rules {
            dc.pass("rules/tokensave.md contains tokensave rules");
        } else {
            dc.fail("rules/tokensave.md missing tokensave rules — run `tokensave install`");
        }
    } else {
        dc.fail("~/.claude/rules/tokensave.md does not exist — run `tokensave install`");
    }
}

/// Clean up local project config (.mcp.json and settings.local.json).
fn doctor_check_local_config(dc: &mut DoctorCounters, project_path: &Path) {
    crate::agent_note!("\n\x1b[1mLocal config\x1b[0m");
    let mut local_cleaned = false;

    let mcp_json_path = project_path.join(".mcp.json");
    if mcp_json_path.exists() {
        local_cleaned |= doctor_check_local_mcp_json(dc, &mcp_json_path);
    }

    let local_settings_path = project_path.join(".claude").join("settings.local.json");
    if local_settings_path.exists() {
        local_cleaned |= doctor_check_local_settings(dc, &local_settings_path);
    }

    if !local_cleaned && !mcp_json_path.exists() && !local_settings_path.exists() {
        dc.pass("No project-local MCP config (using global install)");
    } else if local_cleaned {
        dc.pass("Project-local tokensave install is valid");
    }
}

/// Report a local .mcp.json that registers tokensave. With --local now a
/// supported install mode, this is a valid state — never remove it. Returns
/// true if a tokensave entry was found (so the caller can adjust messaging).
fn doctor_check_local_mcp_json(dc: &mut DoctorCounters, mcp_json_path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(mcp_json_path) else {
        return false;
    };
    let Ok(mcp_val) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    if !mcp_val["mcpServers"]["tokensave"].is_object() {
        dc.pass("No tokensave in .mcp.json");
        return false;
    }
    dc.pass(&format!(
        "Local (project-scoped) tokensave install detected in {}",
        mcp_json_path.display()
    ));
    true
}

/// Report a local settings.local.json that references tokensave. Valid under
/// --local; never removed by doctor. Returns true if tokensave was found.
fn doctor_check_local_settings(dc: &mut DoctorCounters, local_settings_path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(local_settings_path) else {
        return false;
    };
    if !contents.contains("tokensave") {
        dc.pass("No tokensave in .claude/settings.local.json");
        return false;
    }
    dc.pass(&format!(
        "Local (project-scoped) tokensave config detected in {}",
        local_settings_path.display()
    ));
    true
}

// ---------------------------------------------------------------------------
// Shared local helpers
// ---------------------------------------------------------------------------

/// Clean up orphaned MCP-related keys in a local settings JSON value.
fn clean_orphaned_local_mcp_keys(local_val: &mut serde_json::Value) {
    let no_local_servers = local_val
        .get("enabledMcpjsonServers")
        .and_then(|v| v.as_array())
        .is_some_and(std::vec::Vec::is_empty)
        && local_val
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .is_none_or(serde_json::Map::is_empty);
    if no_local_servers {
        local_val
            .as_object_mut()
            .map(|o| o.remove("enableAllProjectMcpServers"));
        local_val
            .as_object_mut()
            .map(|o| o.remove("enabledMcpjsonServers"));
    }
}

/// Best-effort check: warn if `install` needs re-running.
/// Reads ~/.claude/settings.json and compares installed permissions
/// against what the current version expects. Silent on any error.
///
/// Also silently backfills any missing hooks (post-upgrade migration)
/// and normalizes Windows backslash paths in hook commands — both in the
/// user-level settings and in the current project's `.claude/settings.json`
/// / `.claude/settings.local.json`, so broken project-scope hooks self-heal.
pub fn check_install_stale() {
    let Some(home) = super::home_dir() else {
        return;
    };

    // --- user-level settings: permissions warning + hook backfill ---
    let user_settings_path = claude_config_dir(&home).join("settings.json");
    if let Ok(contents) = std::fs::read_to_string(&user_settings_path) {
        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&contents) {
            warn_missing_permissions(&settings);
        }
    }
    normalize_and_backfill_settings_file(&user_settings_path);

    // --- project-level settings: hook backfill only ---
    // Fixes issue #38: a project opened with pre-fix backslash paths in
    // .claude/settings.json never self-healed because we only scanned the
    // user-level file. Scanning the cwd covers the common case of Claude
    // Code invoking a project-scoped hook.
    if let Ok(cwd) = std::env::current_dir() {
        let project_claude = cwd.join(".claude");
        normalize_and_backfill_settings_file(&project_claude.join("settings.json"));
        normalize_and_backfill_settings_file(&project_claude.join("settings.local.json"));
    }
}

/// True if `perm` (an expected `mcp__tokensave__<tool>` string) is granted by
/// any entry in `installed`. Mirrors Claude Code's *allow-rule* matching: an
/// exact tool name, the bare server grant, or a glob anchored after the
/// literal `mcp__tokensave__` prefix. Unanchored globs (`mcp__*`, `*`) are
/// deliberately NOT accepted — Claude Code skips them in `allow` rules (see
/// docs: <https://code.claude.com/docs/en/permissions#tool-name-wildcards>), so
/// honoring them here would hide a real "tools not granted" state.
fn perm_is_covered(perm: &str, installed: &[&str]) -> bool {
    installed.iter().any(|e| {
        *e == perm                       // exact tool grant
            || *e == "mcp__tokensave"    // bare server-wide grant
            || *e == "mcp__tokensave__*" // full-server wildcard
            // anchored partial glob, e.g. "mcp__tokensave__tokensave_*"
            || e.strip_suffix('*').is_some_and(|pfx| {
                pfx.starts_with("mcp__tokensave__") && perm.starts_with(pfx)
            })
    })
}

/// Emit a warning if the current tokensave version expects tool permissions
/// that aren't present in `settings`.
fn warn_missing_permissions(settings: &serde_json::Value) {
    let installed: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let expected = expected_tool_perms();
    let missing_count = expected
        .iter()
        .filter(|p| !perm_is_covered(p, &installed))
        .count();

    if missing_count > 0 {
        crate::agent_note!(
            "\x1b[33mwarning: {missing_count} new tokensave tool(s) not yet permitted. Run `tokensave reinstall` to update permissions.\x1b[0m"
        );
    }
}

/// Load `path`, normalize any backslashed tokensave hook commands, backfill
/// missing hook events, and write back if anything changed. Silent on any
/// error (missing file, unparseable JSON, write failure). Safe no-op when
/// no tokensave hook is present in the file.
fn normalize_and_backfill_settings_file(path: &Path) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    // Only touch files that already reference tokensave — don't accidentally
    // rewrite unrelated project settings just because tokensave ran in cwd.
    let Some(bin) = extract_tokensave_bin_from_hooks(&settings) else {
        return;
    };
    let before = serde_json::to_string(&settings).unwrap_or_default();
    normalize_hook_command_paths(&mut settings);
    install_hook_quiet(&mut settings, &bin);
    let after = serde_json::to_string(&settings).unwrap_or_default();
    if before != after {
        backup_and_write_json(path, &settings);
    }
}

/// Rewrite any tokensave hook command containing a backslash to use forward
/// slashes. Fixes pre-v4.0.x Windows installs where backslashed paths got
/// mangled by `bash -c` (e.g. `C:\Users\...` → `C:Users...` — see issue #38).
/// Only touches commands that mention `tokensave` so unrelated hooks are left
/// alone.
fn normalize_hook_command_paths(settings: &mut serde_json::Value) {
    let Some(hooks) = settings.get_mut("hooks").and_then(|v| v.as_object_mut()) else {
        return;
    };
    for entries in hooks.values_mut() {
        let Some(arr) = entries.as_array_mut() else {
            continue;
        };
        for entry in arr.iter_mut() {
            let Some(cmds) = entry.get_mut("hooks").and_then(|a| a.as_array_mut()) else {
                continue;
            };
            for cmd in cmds.iter_mut() {
                let Some(command_val) = cmd.get_mut("command") else {
                    continue;
                };
                let Some(command) = command_val.as_str() else {
                    continue;
                };
                if command.contains("tokensave") && command.contains('\\') {
                    *command_val = serde_json::Value::String(command.replace('\\', "/"));
                }
            }
        }
    }
}

/// Extracts the tokensave binary path from any existing hook command.
///
/// Scans all hook events for a command containing "tokensave" and returns
/// the binary path. Handles both the modern `{command, args}` shape and the
/// legacy single-string shape. Returns `None` if no tokensave hook is found.
fn extract_tokensave_bin_from_hooks(settings: &serde_json::Value) -> Option<String> {
    let hooks = settings.get("hooks")?.as_object()?;
    for entries in hooks.values() {
        let Some(arr) = entries.as_array() else {
            continue;
        };
        for entry in arr {
            let Some(cmds) = entry.get("hooks").and_then(|a| a.as_array()) else {
                continue;
            };
            for cmd in cmds {
                let Some(raw) = cmd.get("command").and_then(|c| c.as_str()) else {
                    continue;
                };
                if !raw.contains("tokensave") {
                    continue;
                }
                let bin = if cmd.get("args").is_some() {
                    raw.to_string()
                } else {
                    raw.split_whitespace().next().unwrap_or(raw).to_string()
                };
                return Some(bin.replace('\\', "/"));
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::{install_tool_perms, TOKENSAVE_WILDCARD_PERM};
    use super::*;
    use serde_json::json;

    #[test]
    fn config_dir_override_parsing() {
        assert_eq!(parse_config_dir_value(""), None);
        assert_eq!(parse_config_dir_value("   "), None);
        assert_eq!(
            parse_config_dir_value("/home/me/.claude-work"),
            Some(PathBuf::from("/home/me/.claude-work"))
        );
        // Whitespace trimmed; comma-separated takes the first entry.
        assert_eq!(
            parse_config_dir_value("  /a/b  "),
            Some(PathBuf::from("/a/b"))
        );
        assert_eq!(
            parse_config_dir_value("/first,/second"),
            Some(PathBuf::from("/first"))
        );
    }

    /// Build a settings value with the three tokensave hooks installed
    /// (modern `{command, args}` shape).
    fn settings_with_all_hooks(bin: &str) -> serde_json::Value {
        json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Agent|Grep|Bash",
                    "hooks": [{ "type": "command", "command": bin, "args": ["hook-pre-tool-use"] }]
                }],
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": bin, "args": ["hook-prompt-submit"] }]
                }],
                "Stop": [{
                    "hooks": [{ "type": "command", "command": bin, "args": ["hook-stop"] }]
                }]
            },
            "permissions": {
                "allow": ["mcp__tokensave__search", "mcp__tokensave__lookup"]
            }
        })
    }

    /// Build a settings value with the legacy single-string command shape
    /// (broken for paths with spaces — used to test migration/repair).
    fn settings_with_legacy_hooks(bin: &str) -> serde_json::Value {
        json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Agent|Grep|Bash",
                    "hooks": [{ "type": "command", "command": format!("{bin} hook-pre-tool-use") }]
                }],
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": format!("{bin} hook-prompt-submit") }]
                }],
                "Stop": [{
                    "hooks": [{ "type": "command", "command": format!("{bin} hook-stop") }]
                }]
            }
        })
    }

    // -----------------------------------------------------------------------
    // Uninstall tests
    // -----------------------------------------------------------------------

    #[test]
    fn uninstall_hook_removes_all_three_events() {
        let mut settings = settings_with_all_hooks("/usr/bin/tokensave");
        let modified = uninstall_hook(&mut settings);
        assert!(modified);
        // All three hook events should be gone.
        assert!(
            settings.get("hooks").is_none() || settings["hooks"].as_object().unwrap().is_empty()
        );
    }

    #[test]
    fn uninstall_hook_removes_user_prompt_submit() {
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "tokensave hook-prompt-submit" }]
                }]
            }
        });
        let modified = uninstall_single_hook(&mut settings, "UserPromptSubmit");
        assert!(modified);
        assert!(
            settings.get("hooks").is_none(),
            "hooks key should be removed when empty"
        );
    }

    #[test]
    fn uninstall_preserves_non_tokensave_hooks() {
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "hooks": [{ "type": "command", "command": "tokensave hook-prompt-submit" }]
                    },
                    {
                        "hooks": [{ "type": "command", "command": "other-tool do-something" }]
                    }
                ],
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "afplay /System/Library/Sounds/Submarine.aiff" }]
                }]
            }
        });
        uninstall_hook(&mut settings);
        // The non-tokensave UserPromptSubmit entry should survive.
        let arr = settings["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("other-tool"));
        // The Stop event (no tokensave) should survive.
        assert!(settings["hooks"]["Stop"].is_array());
    }

    #[test]
    fn uninstall_noop_when_no_hooks() {
        let mut settings = json!({ "permissions": { "allow": [] } });
        let modified = uninstall_hook(&mut settings);
        assert!(!modified);
    }

    #[test]
    fn uninstall_permissions_removes_tokensave_entries() {
        let mut settings = json!({
            "permissions": {
                "allow": [
                    "Bash",
                    "mcp__tokensave__search",
                    "mcp__tokensave__lookup",
                    "Read"
                ]
            }
        });
        let modified = uninstall_permissions(&mut settings);
        assert!(modified);
        let remaining: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(remaining, vec!["Bash", "Read"]);
    }

    // -----------------------------------------------------------------------
    // Install tests
    // -----------------------------------------------------------------------

    #[test]
    fn install_adds_all_three_hooks() {
        let mut settings = json!({});
        install_hook(&mut settings, "/usr/bin/tokensave");
        assert!(settings["hooks"]["PreToolUse"].is_array());
        assert!(settings["hooks"]["UserPromptSubmit"].is_array());
        assert!(settings["hooks"]["Stop"].is_array());
    }

    #[test]
    fn install_is_idempotent() {
        let mut settings = json!({});
        install_hook(&mut settings, "/usr/bin/tokensave");
        let snapshot = settings.clone();
        install_hook(&mut settings, "/usr/bin/tokensave");
        assert_eq!(settings, snapshot, "second install should be a no-op");
    }

    #[test]
    fn install_preserves_existing_hooks() {
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "other-tool" }]
                }]
            }
        });
        install_hook(&mut settings, "/usr/bin/tokensave");
        // Should have both entries in UserPromptSubmit.
        let arr = settings["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    /// Regression for issue #81: paths with spaces must not be concatenated
    /// into the `command` field — Claude Code whitespace-splits it.
    #[test]
    fn install_uses_args_array_for_paths_with_spaces() {
        let bin = "C:/Path With Spaces/tokensave.exe";
        let mut settings = json!({});
        install_hook(&mut settings, bin);

        for (event, expected_sub) in [
            ("PreToolUse", "hook-pre-tool-use"),
            ("UserPromptSubmit", "hook-prompt-submit"),
            ("Stop", "hook-stop"),
        ] {
            let inner = &settings["hooks"][event][0]["hooks"][0];
            assert_eq!(
                inner["command"].as_str().unwrap(),
                bin,
                "{event}: command must be the exe path alone — no concatenated subcommand"
            );
            assert_eq!(
                inner["args"].as_array().unwrap(),
                &vec![json!(expected_sub)],
                "{event}: subcommand must live in args[]"
            );
        }
    }

    #[test]
    fn install_is_idempotent_for_legacy_shape() {
        // A legacy single-string install must not get a second entry added —
        // the doctor is what rewrites it, not a re-run of install.
        let mut settings = settings_with_legacy_hooks("/usr/bin/tokensave");
        let before = settings.clone();
        install_hook(&mut settings, "/usr/bin/tokensave");
        assert_eq!(settings, before);
    }

    // -----------------------------------------------------------------------
    // doctor_fix_hooks tests (issue #81)
    // -----------------------------------------------------------------------

    /// Issue #81: legacy single-string shape with a path-with-spaces cannot
    /// be parsed unambiguously. Repair must rewrite to the modern `args`
    /// shape using `current_exe()` (the binary that's actually running),
    /// not a whitespace-split of the legacy command. This is what breaks
    /// the doctor → install loop.
    #[test]
    fn doctor_repairs_legacy_shape_to_args_array() {
        let legacy_bin = "C:/Path With Spaces/tokensave.exe";
        let settings_dir = tempfile::tempdir().unwrap();
        let settings_path = settings_dir.path().join("settings.json");
        let settings = settings_with_legacy_hooks(legacy_bin);
        std::fs::write(&settings_path, serde_json::to_string(&settings).unwrap()).unwrap();

        let mut dc = DoctorCounters::default();
        doctor_fix_hooks(&mut dc, &settings_path, &settings);

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let expected_bin = std::env::current_exe()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        for (event, expected_sub) in [
            ("PreToolUse", "hook-pre-tool-use"),
            ("UserPromptSubmit", "hook-prompt-submit"),
            ("Stop", "hook-stop"),
        ] {
            let inner = &after["hooks"][event][0]["hooks"][0];
            assert_eq!(
                inner["command"].as_str().unwrap(),
                expected_bin,
                "{event}: must use current_exe (legacy path cannot be parsed safely)"
            );
            assert_eq!(
                inner["args"].as_array().unwrap(),
                &vec![json!(expected_sub)],
                "{event}: subcommand must move into args[]"
            );
            assert!(
                !inner["command"].as_str().unwrap().contains(expected_sub),
                "{event}: subcommand must not be embedded in the command string"
            );
        }
    }

    #[test]
    fn doctor_is_noop_on_correctly_installed_hooks() {
        let bin = "/usr/bin/tokensave";
        let settings_dir = tempfile::tempdir().unwrap();
        let settings_path = settings_dir.path().join("settings.json");
        let settings = settings_with_all_hooks(bin);
        std::fs::write(&settings_path, serde_json::to_string(&settings).unwrap()).unwrap();

        let mut dc = DoctorCounters::default();
        doctor_fix_hooks(&mut dc, &settings_path, &settings);

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(after, settings);
    }

    // -----------------------------------------------------------------------
    // extract_tokensave_bin_from_hooks tests
    // -----------------------------------------------------------------------

    #[test]
    fn extract_bin_from_any_hook_event() {
        let settings = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "/opt/bin/tokensave hook-stop" }]
                }]
            }
        });
        assert_eq!(
            extract_tokensave_bin_from_hooks(&settings),
            Some("/opt/bin/tokensave".to_string())
        );
    }

    #[test]
    fn extract_bin_returns_none_without_hooks() {
        let settings = json!({ "permissions": {} });
        assert_eq!(extract_tokensave_bin_from_hooks(&settings), None);
    }

    #[test]
    fn extract_bin_normalizes_windows_backslashes() {
        let settings = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "C:\\Users\\dev\\scoop\\shims\\tokensave.exe hook-prompt-submit" }]
                }]
            }
        });
        assert_eq!(
            extract_tokensave_bin_from_hooks(&settings),
            Some("C:/Users/dev/scoop/shims/tokensave.exe".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // normalize_hook_command_paths tests (issue #38)
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_rewrites_backslashed_tokensave_commands() {
        let mut settings = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "C:\\Users\\alkam\\scoop\\apps\\tokensave\\current\\tokensave.exe hook-stop"
                    }]
                }]
            }
        });
        normalize_hook_command_paths(&mut settings);
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"][0]["command"]
                .as_str()
                .unwrap(),
            "C:/Users/alkam/scoop/apps/tokensave/current/tokensave.exe hook-stop"
        );
    }

    #[test]
    fn normalize_leaves_non_tokensave_hooks_alone() {
        let mut settings = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "C:\\Windows\\System32\\other.exe --flag"
                    }]
                }]
            }
        });
        let before = settings.clone();
        normalize_hook_command_paths(&mut settings);
        assert_eq!(settings, before);
    }

    #[test]
    fn normalize_is_noop_when_already_forward_slashed() {
        let mut settings = settings_with_all_hooks("C:/Users/dev/scoop/shims/tokensave.exe");
        let before = settings.clone();
        normalize_hook_command_paths(&mut settings);
        assert_eq!(settings, before);
    }

    #[test]
    fn normalize_and_backfill_rewrites_project_settings_file() {
        use std::io::Write as _;
        // `tempfile::TempDir` gives a per-test unique path; the previous
        // PID + nanos scheme collided when the two `normalize_and_backfill_*`
        // tests ran in parallel under coarse-resolution clocks.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let contents = r#"{
  "hooks": {
    "Stop": [{
      "hooks": [{ "type": "command", "command": "C:\\Users\\u\\tokensave.exe hook-stop" }]
    }]
  }
}
"#;
        std::fs::File::create(&path)
            .unwrap()
            .write_all(contents.as_bytes())
            .unwrap();

        normalize_and_backfill_settings_file(&path);

        let after = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(
            parsed["hooks"]["Stop"][0]["hooks"][0]["command"]
                .as_str()
                .unwrap(),
            "C:/Users/u/tokensave.exe hook-stop"
        );
        // All three events should now be present (backfill).
        assert!(parsed["hooks"]["PreToolUse"].is_array());
        assert!(parsed["hooks"]["UserPromptSubmit"].is_array());
    }

    #[test]
    fn normalize_and_backfill_skips_file_without_tokensave_hook() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let contents = r#"{"permissions": {"allow": ["Bash"]}}
"#;
        std::fs::File::create(&path)
            .unwrap()
            .write_all(contents.as_bytes())
            .unwrap();

        normalize_and_backfill_settings_file(&path);

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, contents,
            "file without tokensave hook must be untouched"
        );
    }

    // -----------------------------------------------------------------------
    // Doctor check tests
    // -----------------------------------------------------------------------

    #[test]
    fn doctor_detects_missing_user_prompt_submit() {
        let mut dc = DoctorCounters::new();
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "hooks": [{ "type": "command", "command": "tokensave hook-pre-tool-use" }]
                }]
            }
        });
        doctor_check_single_hook(&mut dc, &settings, "UserPromptSubmit");
        assert!(dc.issues > 0, "should report missing UserPromptSubmit hook");
    }

    #[test]
    fn doctor_passes_when_user_prompt_submit_present() {
        let mut dc = DoctorCounters::new();
        let bin = std::env::current_exe()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let settings = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": bin,
                        "args": ["hook-prompt-submit"],
                    }]
                }]
            }
        });
        doctor_check_single_hook(&mut dc, &settings, "UserPromptSubmit");
        assert_eq!(
            dc.issues, 0,
            "should pass when UserPromptSubmit hook is present"
        );
    }

    #[test]
    fn doctor_detects_wrong_subcommand() {
        let mut dc = DoctorCounters::new();
        let settings = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "tokensave invalidcommand" }]
                }]
            }
        });
        doctor_check_single_hook(&mut dc, &settings, "UserPromptSubmit");
        assert!(dc.issues > 0, "should report wrong subcommand");
    }

    #[test]
    fn doctor_detects_wrong_subcommand_on_stop() {
        let mut dc = DoctorCounters::new();
        let settings = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "tokensave hook-pre-tool-use" }]
                }]
            }
        });
        doctor_check_single_hook(&mut dc, &settings, "Stop");
        assert!(dc.issues > 0, "should report wrong subcommand for Stop");
    }

    #[test]
    fn doctor_detects_missing_subcommand() {
        let mut dc = DoctorCounters::new();
        let settings = json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "tokensave" }]
                }]
            }
        });
        doctor_check_single_hook(&mut dc, &settings, "UserPromptSubmit");
        assert!(dc.issues > 0, "should report missing subcommand");
    }

    // -----------------------------------------------------------------------
    // Doctor fix tests
    // -----------------------------------------------------------------------

    #[test]
    fn doctor_fix_adds_missing_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        // Start with only Stop hook.
        let settings = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "/usr/bin/tokensave hook-stop" }]
                }]
            }
        });
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let mut dc = DoctorCounters::new();
        doctor_fix_hooks(&mut dc, &settings_path, &settings);

        // Re-read and verify all three hooks are present.
        let fixed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(fixed["hooks"]["PreToolUse"].is_array());
        assert!(fixed["hooks"]["UserPromptSubmit"].is_array());
        assert!(fixed["hooks"]["Stop"].is_array());
    }

    #[test]
    fn doctor_fix_replaces_wrong_subcommand() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        // Modern shape with a wrong subcommand on UserPromptSubmit.
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Agent|Grep|Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/tokensave",
                        "args": ["hook-pre-tool-use"],
                    }]
                }],
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/tokensave",
                        "args": ["invalidcommand"],
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/tokensave",
                        "args": ["hook-stop"],
                    }]
                }]
            }
        });
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let mut dc = DoctorCounters::new();
        doctor_fix_hooks(&mut dc, &settings_path, &settings);

        let fixed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let inner = &fixed["hooks"]["UserPromptSubmit"][0]["hooks"][0];
        assert_eq!(
            inner["args"].as_array().unwrap(),
            &vec![json!("hook-prompt-submit")],
            "should have correct subcommand in args[]"
        );
        // Should keep the original bin path on a modern-shape repair.
        assert_eq!(inner["command"].as_str().unwrap(), "/usr/bin/tokensave");
    }

    #[test]
    fn doctor_fix_noop_when_all_present() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let settings = settings_with_all_hooks("/usr/bin/tokensave");
        let pretty = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&settings_path, &pretty).unwrap();

        let mut dc = DoctorCounters::new();
        doctor_fix_hooks(&mut dc, &settings_path, &settings);

        // File should be unchanged.
        let after = std::fs::read_to_string(&settings_path).unwrap();
        assert_eq!(
            after, pretty,
            "should not modify file when all hooks present"
        );
    }

    #[test]
    fn doctor_fix_upgrades_stale_pretool_matcher() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        // Old-shape: matcher is just "Agent" (pre-Grep/Bash redirect).
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Agent",
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/tokensave",
                        "args": ["hook-pre-tool-use"],
                    }]
                }],
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/tokensave",
                        "args": ["hook-prompt-submit"],
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/tokensave",
                        "args": ["hook-stop"],
                    }]
                }]
            }
        });
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let mut dc = DoctorCounters::new();
        doctor_fix_hooks(&mut dc, &settings_path, &settings);

        let fixed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(
            fixed["hooks"]["PreToolUse"][0]["matcher"].as_str(),
            Some("Agent|Grep|Bash"),
            "stale Agent-only matcher must be upgraded to Agent|Grep|Bash"
        );
        // Bin path preserved across the matcher repair.
        assert_eq!(
            fixed["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str(),
            Some("/usr/bin/tokensave")
        );
    }

    // -----------------------------------------------------------------------
    // Wildcard/compact permission recognition (`perm_is_covered`)
    // -----------------------------------------------------------------------

    #[test]
    fn perm_is_covered_matches_exact_tool() {
        let installed = ["mcp__tokensave__tokensave_search"];
        assert!(perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &installed
        ));
    }

    #[test]
    fn perm_is_covered_matches_bare_server_grant() {
        let installed = ["mcp__tokensave"];
        assert!(perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &installed
        ));
    }

    #[test]
    fn perm_is_covered_matches_full_wildcard() {
        let installed = ["mcp__tokensave__*"];
        assert!(perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &installed
        ));
    }

    #[test]
    fn perm_is_covered_matches_anchored_partial_glob() {
        let installed = ["mcp__tokensave__tokensave_*"];
        assert!(perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &installed
        ));
        // A glob anchored on a different prefix must not match.
        assert!(!perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &["mcp__tokensave__other_*"]
        ));
    }

    #[test]
    fn perm_is_covered_rejects_unanchored_mcp_star() {
        // Claude Code documents "mcp__*" as skipped-with-a-warning in `allow`
        // rules — it does NOT grant anything. Treating it as covering here
        // would hide a real "tools not granted" state, so it must not match.
        let installed = ["mcp__*"];
        assert!(!perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &installed
        ));
    }

    #[test]
    fn perm_is_covered_rejects_unrelated_entries() {
        let installed = ["Bash", "Read", "mcp__other_server__tool"];
        assert!(!perm_is_covered(
            "mcp__tokensave__tokensave_search",
            &installed
        ));
    }

    // -----------------------------------------------------------------------
    // doctor_check_permissions / warn_missing_permissions recognition
    // -----------------------------------------------------------------------

    #[test]
    fn doctor_passes_with_full_wildcard_grant() {
        let settings = json!({ "permissions": { "allow": ["mcp__tokensave__*"] } });
        let mut dc = DoctorCounters::new();
        doctor_check_permissions(&mut dc, &settings);
        assert_eq!(
            dc.issues, 0,
            "wildcard grant should satisfy all permissions"
        );
        assert_eq!(
            dc.warnings, 0,
            "wildcard grant must not be reported as a stale leftover"
        );
    }

    #[test]
    fn doctor_passes_with_bare_server_grant() {
        let settings = json!({ "permissions": { "allow": ["mcp__tokensave"] } });
        let mut dc = DoctorCounters::new();
        doctor_check_permissions(&mut dc, &settings);
        assert_eq!(
            dc.issues, 0,
            "bare server grant should satisfy all permissions"
        );
    }

    #[test]
    fn doctor_still_fails_with_only_unanchored_mcp_star() {
        // Guard against a false negative: Claude Code doesn't honor "mcp__*"
        // as an allow rule, so this must still be reported as missing.
        let settings = json!({ "permissions": { "allow": ["mcp__*"] } });
        let mut dc = DoctorCounters::new();
        doctor_check_permissions(&mut dc, &settings);
        assert!(
            dc.issues > 0,
            "unanchored mcp__* must not be treated as covering the tools"
        );
    }

    #[test]
    fn doctor_still_fails_when_permissions_missing() {
        let settings = json!({ "permissions": { "allow": ["Bash"] } });
        let mut dc = DoctorCounters::new();
        doctor_check_permissions(&mut dc, &settings);
        assert!(dc.issues > 0);
    }

    // -----------------------------------------------------------------------
    // Opt-in compact install (`install_tool_perms`, prune-then-add)
    // -----------------------------------------------------------------------

    #[test]
    fn install_tool_perms_wildcard_is_single_entry() {
        assert_eq!(
            install_tool_perms(true),
            vec![TOKENSAVE_WILDCARD_PERM.to_string()]
        );
    }

    #[test]
    fn install_tool_perms_explicit_is_full_list() {
        assert_eq!(install_tool_perms(false), expected_tool_perms());
    }

    #[test]
    fn install_permissions_writes_wildcard_entry() {
        let mut settings = json!({});
        // Represents an explicit `--wildcard-permissions` request.
        install_permissions(&mut settings, &install_tool_perms(true), true);
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(allow, vec![TOKENSAVE_WILDCARD_PERM]);
    }

    #[test]
    fn install_permissions_switching_to_wildcard_prunes_explicit_list() {
        let mut settings = json!({});
        // A plain, flagless install writes the (default) explicit list.
        install_permissions(&mut settings, &expected_tool_perms(), false);
        // Sanity check the explicit list was actually written.
        assert!(
            settings["permissions"]["allow"].as_array().unwrap().len() > 1,
            "explicit install should have written more than one entry"
        );

        // Represents an explicit `--wildcard-permissions` request.
        install_permissions(&mut settings, &install_tool_perms(true), true);
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(
            allow,
            vec![TOKENSAVE_WILDCARD_PERM],
            "switching to wildcard must prune the stale explicit entries"
        );
    }

    #[test]
    fn install_permissions_switching_to_explicit_prunes_wildcard() {
        let mut settings = json!({});
        // Represents an explicit `--wildcard-permissions` request.
        install_permissions(&mut settings, &install_tool_perms(true), true);

        // Represents an explicit `--explicit-permissions` request: force_style
        // must be `true` here, since a flagless call would instead preserve
        // the existing wildcard (see `install_permissions_default_preserves_*`).
        install_permissions(&mut settings, &expected_tool_perms(), true);
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(
            !allow.contains(&TOKENSAVE_WILDCARD_PERM),
            "switching to explicit must prune the stale wildcard entry"
        );
        for perm in expected_tool_perms() {
            assert!(allow.contains(&perm.as_str()));
        }
    }

    #[test]
    fn install_permissions_default_preserves_existing_wildcard() {
        let mut settings = json!({ "permissions": { "allow": [TOKENSAVE_WILDCARD_PERM] } });
        // A flagless reinstall (e.g. the silent reinstall-on-upgrade) must
        // leave a user's existing compact grant untouched rather than
        // pruning it back to the 80+ explicit entries.
        install_permissions(&mut settings, &expected_tool_perms(), false);
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(allow, vec![TOKENSAVE_WILDCARD_PERM]);
    }

    #[test]
    fn install_permissions_default_preserves_existing_bare_grant() {
        let mut settings = json!({ "permissions": { "allow": ["mcp__tokensave"] } });
        install_permissions(&mut settings, &expected_tool_perms(), false);
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(allow, vec!["mcp__tokensave"]);
    }

    #[test]
    fn install_permissions_preserves_non_tokensave_entries_across_style_switch() {
        let mut settings = json!({ "permissions": { "allow": ["Bash", "Read"] } });
        install_permissions(&mut settings, &expected_tool_perms(), false);
        install_permissions(&mut settings, &install_tool_perms(true), true);
        let allow: Vec<&str> = settings["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(allow.contains(&"Bash"));
        assert!(allow.contains(&"Read"));
        assert!(allow.contains(&TOKENSAVE_WILDCARD_PERM));
    }
}
