// Rust guideline compliant 2026-07-02
//! Factory Droid agent integration.
//!
//! Handles registration of the tokensave MCP server in Factory Droid's MCP
//! config (`~/.factory/mcp.json` globally, `<project>/.factory/mcp.json` for
//! `--local`) under the `mcpServers.tokensave` key, prompt rules via
//! `AGENTS.md` (`~/.factory/AGENTS.md` globally, `<project>/AGENTS.md` for
//! `--local`), and the `PreToolUse` guardrail hook in Factory's `hooks`
//! object (`~/.factory/settings.json` globally, `<project>/.factory/settings.json`
//! for `--local` — the same file Factory's own `Stop`/`Notification`/
//! `PreToolUse`/`UserPromptSubmit` wrappers live in, per the live-config
//! verification in Factory's public hook docs; this is a separate
//! file from `mcp.json`). Droid blocks a tool call via **exit code 2 +
//! stderr** — the same mechanism as Kiro's `preToolUse` hook, not Claude
//! Code's stdout JSON decision.

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

/// Hook event name under `settings.json`'s `hooks` object that Droid fires
/// before a tool call executes.
const DROID_PRE_TOOL_EVENT: &str = "PreToolUse";

/// Regex matcher (Factory treats the value as a regex) covering Droid's two
/// tool names whose payloads the shared decision core can classify: `Execute`
/// (shell, a symbol-shaped `grep`/`rg`/`ag` on a code file) and `Grep`
/// (Droid's native content search, a symbol-shaped `pattern` on a code
/// target). Both redirect to `tokensave_search`/`tokensave_callers_for`, which
/// return a compact symbol list instead of raw match lines. The pattern is
/// anchored (`^(...)$`) so it can only match those exact tool names, never a
/// future tool whose name merely contains `Execute`/`Grep` as a substring;
/// anchored matching was verified live to still fire on both tools.
///
/// Deliberately excluded (see the PR doc for the full per-tool analysis):
/// `Read`/`LS`/`Glob` have only lossy tokensave equivalents and no way to
/// infer symbol intent from a bare path, and Droid's hook contract is a hard
/// block (exit 2 + stderr) with no confirmed soft-steer channel, so blocking
/// them would strand legitimate reads. `Task` always carries a typed
/// `subagent_type` on Droid, so the research classifier (which only fires on
/// Claude's `Explore` or an untyped research prompt) would never trigger.
/// Unmatched tools never reach our hook subprocess, the safest "fail open".
const DROID_HOOK_MATCHER: &str = "^(Execute|Grep)$";

/// Subcommand invoked by the installed hook.
const DROID_PRE_TOOL_HOOK: &str = "hook-droid-pre-tool-use";

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

        let settings_path = droid_settings_path_for(ctx);
        install_hook(&settings_path, &ctx.tokensave_bin)?;

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

        let settings_path = droid_settings_path_for(ctx);
        uninstall_hook(&settings_path);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Factory Droid.");
        eprintln!("Start a new droid session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mFactory Droid integration\x1b[0m");
        doctor_check_config(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
        doctor_check_hook(dc, &ctx.home);
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

/// Global Factory Droid settings path (`~/.factory/settings.json`) — the file
/// Factory reads its `hooks` object from.
fn droid_settings_path(home: &Path) -> PathBuf {
    home.join(".factory/settings.json")
}

/// settings.json path for this install: `~/.factory/settings.json` globally,
/// or `<project>/.factory/settings.json` for `--local` (same relative layout
/// as `mcp.json`).
fn droid_settings_path_for(ctx: &InstallContext) -> PathBuf {
    ctx.base_dir().join(".factory/settings.json")
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

    let bin = crate::agents::preserve_mcp_command(
        settings.pointer("/mcpServers/tokensave/command"),
        tokensave_bin,
    );
    settings["mcpServers"]["tokensave"] = json!({
        "type": "stdio",
        "command": bin,
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

/// Build the command string for a Droid hook entry: `<tokensave_bin> <subcommand>`
/// as a single string, matching the shape already confirmed live in
/// `~/.factory/settings.json` (Factory's own hook wrappers there use a bare
/// `command` string with no `args` array). This mirrors the Kiro adapter,
/// which makes the same choice for the same reason.
fn hook_command(tokensave_bin: &str, subcommand: &str) -> String {
    format!("{tokensave_bin} {subcommand}")
}

/// Extract the `command` string from a Droid hook wrapper entry (the object
/// holding `{"matcher": ..., "hooks": [{"type": "command", "command": ...}]}`).
/// Returns the first inner command.
fn droid_hook_entry_command(entry: &serde_json::Value) -> Option<&str> {
    entry
        .get("hooks")?
        .as_array()?
        .iter()
        .find_map(|c| c.get("command").and_then(|v| v.as_str()))
}

/// Is this `PreToolUse` array entry the tokensave guardrail hook? Matched on
/// the tokensave binary plus exact `hook-droid-pre-tool-use` subcommand rather
/// than the matcher value, so an older `"Execute"` install is migrated without
/// mistaking an unrelated wrapper path for our hook.
fn is_tokensave_droid_hook(entry: &serde_json::Value) -> bool {
    let Some(command) = droid_hook_entry_command(entry) else {
        return false;
    };
    let mut parts = command.split_whitespace();
    let binary_is_tokensave = parts
        .next()
        .and_then(|binary| Path::new(binary).file_stem())
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == "tokensave");
    binary_is_tokensave && parts.next() == Some(DROID_PRE_TOOL_HOOK) && parts.next().is_none()
}

/// Install the `PreToolUse` guardrail hook into `settings.json`'s `hooks`
/// object (idempotent), preserving every other key, including hook wrappers
/// Factory or other tools already installed (e.g. the owner's own
/// `PreToolUse` permission wrapper). Upserts the tokensave entry under the
/// current `^(Execute|Grep)$` matcher: a tokensave entry already carrying that
/// matcher is left untouched, while an entry written by an older tokensave
/// (identified by its `hook-droid-pre-tool-use` subcommand, not its matcher)
/// is migrated in place, so re-installing never leaves a duplicate.
fn install_hook(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

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

    let hooks_arr = settings["hooks"][DROID_PRE_TOOL_EVENT]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Already current: a tokensave entry exists *and* carries the current
    // matcher — nothing to do. A tokensave entry with a stale matcher (an
    // older `"Execute"`-only install) is not "current" and falls through to
    // the rebuild below, which drops it and writes the widened matcher.
    let already_current = hooks_arr.iter().any(|entry| {
        is_tokensave_droid_hook(entry)
            && entry.get("matcher").and_then(|v| v.as_str()) == Some(DROID_HOOK_MATCHER)
    });
    if already_current {
        eprintln!("  {DROID_PRE_TOOL_EVENT} hook already present, skipping");
        return Ok(());
    }

    // Rebuild: drop any existing tokensave entry (migrating a stale matcher in
    // place) while preserving every non-tokensave hook, then append the
    // current one.
    let mut new_hooks: Vec<serde_json::Value> = hooks_arr
        .into_iter()
        .filter(|entry| !is_tokensave_droid_hook(entry))
        .collect();
    new_hooks.push(json!({
        "matcher": DROID_HOOK_MATCHER,
        "hooks": [{
            "type": "command",
            "command": hook_command(tokensave_bin, DROID_PRE_TOOL_HOOK),
        }]
    }));
    settings["hooks"][DROID_PRE_TOOL_EVENT] = serde_json::Value::Array(new_hooks);

    safe_write_json_file(settings_path, &settings, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave PreToolUse hook to {}",
        settings_path.display()
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

/// Remove only tokensave's `PreToolUse` hook entry from `settings.json`,
/// preserving every other key — including hook wrappers Factory or other
/// tools installed under the same or other events.
fn uninstall_hook(settings_path: &Path) {
    if !settings_path.exists() {
        eprintln!("  {} not found, skipping", settings_path.display());
        return;
    }
    let Ok(contents) = std::fs::read_to_string(settings_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };

    let Some(arr) = settings["hooks"][DROID_PRE_TOOL_EVENT].as_array().cloned() else {
        eprintln!(
            "  No tokensave PreToolUse hook in {}, skipping",
            settings_path.display()
        );
        return;
    };

    let original_len = arr.len();
    let filtered: Vec<serde_json::Value> = arr
        .into_iter()
        .filter(|entry| !is_tokensave_droid_hook(entry))
        .collect();

    if filtered.len() == original_len {
        eprintln!(
            "  No tokensave PreToolUse hook in {}, skipping",
            settings_path.display()
        );
        return;
    }

    if filtered.is_empty() {
        if let Some(hooks) = settings.get_mut("hooks").and_then(|v| v.as_object_mut()) {
            hooks.remove(DROID_PRE_TOOL_EVENT);
            if hooks.is_empty() {
                settings.as_object_mut().map(|o| o.remove("hooks"));
            }
        }
    } else {
        settings["hooks"][DROID_PRE_TOOL_EVENT] = serde_json::Value::Array(filtered);
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
            "\x1b[32m✔\x1b[0m Removed tokensave PreToolUse hook from {}",
            settings_path.display()
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

/// Check settings.json has the tokensave `PreToolUse` guardrail hook.
fn doctor_check_hook(dc: &mut DoctorCounters, home: &Path) {
    let path = droid_settings_path(home);
    if !path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent droid` if you use Factory Droid",
            path.display()
        ));
        return;
    }

    let config = load_json_file(&path);
    let hook = config
        .get("hooks")
        .and_then(|v| v.get(DROID_PRE_TOOL_EVENT))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.iter().find(|entry| is_tokensave_droid_hook(entry)));

    let Some(hook) = hook else {
        dc.fail(&format!(
            "PreToolUse hook NOT installed in {} — run `tokensave install --agent droid`",
            path.display()
        ));
        return;
    };

    if hook.get("matcher").and_then(|v| v.as_str()) != Some(DROID_HOOK_MATCHER) {
        dc.fail(&format!(
            "PreToolUse hook matcher is outdated in {} — run `tokensave install --agent droid`",
            path.display()
        ));
        return;
    }

    dc.pass(&format!("PreToolUse hook installed in {}", path.display()));

    let command = droid_hook_entry_command(hook).unwrap_or("");
    let bin = command.split_whitespace().next().unwrap_or("");
    if bin.is_empty() {
        return;
    }
    if Path::new(bin).exists() {
        dc.pass(&format!("Hook binary exists: {bin}"));
    } else {
        dc.warn(&format!(
            "Hook binary not found: {bin} — run `tokensave install --agent droid`"
        ));
    }
}
