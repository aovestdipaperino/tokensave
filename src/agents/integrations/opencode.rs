// Rust guideline compliant 2025-10-17
//! `OpenCode` agent integration.
//!
//! Handles registration of the tokensave MCP server in `OpenCode`'s config
//! file (`$HOME/.config/opencode/opencode.json` or `$XDG_CONFIG_HOME/opencode/opencode.json`),
//! and prompt rules via `$HOME/.config/opencode/AGENTS.md`. `OpenCode` has no hook system or
//! declarative tool permissions — it uses interactive runtime approval.

use std::path::Path;

use serde_json::json;

use crate::errors::Result;

use super::*;
/// Filename of the managed rules file, colocated next to `opencode.json` and
/// referenced from its `"instructions"` array.
const INSTRUCTIONS_ENTRY: &str = "tokensave.md";

/// `OpenCode` agent.
pub struct OpenCodeIntegration;

impl AgentIntegration for OpenCodeIntegration {
    fn name(&self) -> &'static str {
        "OpenCode"
    }

    fn id(&self) -> &'static str {
        "opencode"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let config_path = opencode_config_path_for(ctx);
        let rules_path = opencode_rules_path(&config_path);
        let instructions_entry = instructions_entry_for(&ctx.scope, &rules_path);
        let v2 = opencode_is_v2();
        install_mcp_server(&config_path, &ctx.tokensave_bin, &instructions_entry, v2)?;

        // Resolved after the config write: on a fresh install that is what
        // creates the config directory, and the V2 rules block must land in
        // `<config-dir>/AGENTS.md`, the file OpenCode actually reads.
        let prompt_path = opencode_prompt_path_for(ctx);

        // Managed-block removal runs before the legacy strip: the strip is
        // heading-based and the managed body carries the same heading, so
        // stripping first would gut the block and strand its markers.
        remove_rules_block(&prompt_path).ok();
        // Migration: strip any pre-#256 inline block from AGENTS.md, then
        // write the rules for the detected OpenCode major. A migration
        // failure must fail the install rather than be swallowed — silently
        // succeeding here would report install as complete while stale rules
        // text stays stuck in AGENTS.md.
        uninstall_prompt_rules(&prompt_path)?;
        let rules = rules_for_agent("opencode")?;
        if v2 {
            // #689: V2 accepts the `instructions` array but never resolves
            // it; persistent instructions must live in AGENTS.md.
            write_rules_block(&prompt_path, "opencode", &rules).map(|_| ())?;
        } else {
            // V1 (or unknown): managed tokensave.md referenced from
            // opencode.json's "instructions".
            write_managed_rules_file(&rules_path, &rules).map(|_| ())?;
        }

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new OpenCode session — tokensave tools are now available");
        crate::agent_note!("  3. OpenCode will prompt for approval on first use of each tool");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let config_path = opencode_config_path_for(ctx);
        let rules_path = opencode_rules_path(&config_path);
        let instructions_entry = instructions_entry_for(&ctx.scope, &rules_path);
        uninstall_mcp_server(&config_path, &instructions_entry);
        // #689 V2 managed block before the legacy strip (same heading-gutting
        // order as install), both best-effort like the rest of uninstall.
        remove_rules_block(&opencode_prompt_path_for(ctx)).ok();
        uninstall_prompt_rules(&opencode_prompt_path_for(ctx)).ok(); // legacy pre-#256 installs
        remove_managed_rules_file(&rules_path);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from OpenCode.");
        crate::agent_note!("Start a new OpenCode session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mOpenCode integration\x1b[0m");
        let v2 = opencode_is_v2();
        doctor_check_config(dc, &ctx.home, v2);
        doctor_check_prompt(dc, &ctx.home, v2);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".config").join("opencode").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(opencode_config_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let config_path = opencode_config_path(home);
        if !config_path.exists() {
            return false;
        }
        let json = super::load_json_file(&config_path);
        json.get("mcp").and_then(|v| v.get("tokensave")).is_some()
            || json.pointer("/mcp/servers/tokensave").is_some()
    }
}

// ---------------------------------------------------------------------------
// Version detection (#689)
// ---------------------------------------------------------------------------

/// `OpenCode` major version. `TOKENSAVE_OPENCODE_VERSION` overrides the binary
/// probe (tests, hosts without the binary on PATH); otherwise ask
/// `opencode --version` ("opencode v2.0.28"). `None` when neither answers —
/// callers treat that as V1, the shape installs wrote before V2 existed.
///
/// shortcut: `output()` waits unbounded; `--version` is a trivial flag, and
/// the bounded-wait pattern (omp.rs) is only warranted for commands that do
/// real work. Revisit if a hung `opencode --version` is ever observed.
fn opencode_major_version() -> Option<u64> {
    if let Ok(v) = std::env::var("TOKENSAVE_OPENCODE_VERSION") {
        return parse_opencode_major(&v);
    }
    let out = std::process::Command::new("opencode")
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_opencode_major(&String::from_utf8_lossy(&out.stdout))
}

/// Extract the major version from `opencode --version` output or a bare
/// version string; `None` for empty or unparseable input.
fn parse_opencode_major(output: &str) -> Option<u64> {
    let digits = output
        .trim()
        .trim_start_matches(|c: char| !c.is_ascii_digit());
    digits.split('.').next()?.parse().ok()
}

/// True when the target `OpenCode` reads the native V2 config shape: servers
/// under `mcp.servers`, rules from `AGENTS.md` (the `instructions` array is
/// accepted but never resolved).
fn opencode_is_v2() -> bool {
    opencode_major_version().is_some_and(|major| major >= 2)
}

// ---------------------------------------------------------------------------
// Config path resolution
// ---------------------------------------------------------------------------

/// Returns the path to opencode config (global).
/// Prefers `$HOME/.config/opencode/opencode.json`. Falls back to
/// `$XDG_CONFIG_HOME/opencode/opencode.json` only when the XDG path
/// is under `home` (so tests with temp-dir homes are never polluted by
/// the real user's environment).
/// opencode.json path for this install: global config path, or
/// `<project>/opencode.json` for `--local`.
fn opencode_config_path_for(ctx: &InstallContext) -> std::path::PathBuf {
    match &ctx.scope {
        InstallScope::Global => opencode_config_path(&ctx.home),
        InstallScope::Local { project_path } => project_path.join("opencode.json"),
    }
}

/// AGENTS.md path for this install: global prompt path, or
/// `<project>/AGENTS.md` for `--local`.
fn opencode_prompt_path_for(ctx: &InstallContext) -> std::path::PathBuf {
    match &ctx.scope {
        InstallScope::Global => opencode_prompt_path(&ctx.home),
        InstallScope::Local { project_path } => project_path.join("AGENTS.md"),
    }
}

/// Path to the managed tokensave rules file, colocated with `opencode.json`
/// (`~/.config/opencode/tokensave.md` globally, `<project>/tokensave.md`
/// locally).
fn opencode_rules_path(config_path: &Path) -> std::path::PathBuf {
    config_path.parent().map_or_else(
        || std::path::PathBuf::from(INSTRUCTIONS_ENTRY),
        |p| p.join(INSTRUCTIONS_ENTRY),
    )
}

/// The value to store in / match against opencode.json's `"instructions"`
/// array for the managed rules file at `rules_path`.
///
/// `OpenCode` resolves a *relative* instruction entry by globbing upward from
/// the current project's working directory toward its worktree root —
/// nothing to do with where `opencode.json` itself lives (see
/// `Instruction.systemPaths`/`globUp` upstream). For a local project install
/// that's exactly right: `tokensave.md` sits at the project root, which is
/// the glob-up stop point, so a bare relative filename resolves correctly
/// and stays portable across machines/checkouts. For a *global* install,
/// though, the project being worked on has nothing to do with
/// `~/.config/opencode/`, so a bare "tokensave.md" there would silently
/// never match — the global entry must be the absolute path instead, which
/// `OpenCode` resolves relative to its own directory regardless of the
/// caller's cwd.
fn instructions_entry_for(scope: &InstallScope, rules_path: &Path) -> String {
    match scope {
        InstallScope::Global => rules_path.to_string_lossy().into_owned(),
        InstallScope::Local { .. } => INSTRUCTIONS_ENTRY.to_string(),
    }
}

fn opencode_config_path(home: &Path) -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg_path = std::path::PathBuf::from(&xdg);
        if xdg_path.starts_with(home) {
            return xdg_path.join("opencode/opencode.json");
        }
    }
    home.join(".config/opencode/opencode.json")
}

/// Returns the path to the global AGENTS.md prompt file.
fn opencode_prompt_path(home: &Path) -> std::path::PathBuf {
    let modern = home.join(".config/opencode/AGENTS.md");
    if modern.exists() || home.join(".config/opencode").exists() {
        return modern;
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg_path = std::path::PathBuf::from(&xdg);
        if xdg_path.starts_with(home) {
            let xdg_dir = xdg_path.join("opencode");
            if xdg_dir.exists() {
                return xdg_dir.join("AGENTS.md");
            }
        }
    }
    home.join("AGENTS.md")
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

/// Register MCP server in opencode.json.
///
/// Safety: creates a `.bak` backup before writing and restores it on any
/// error. Uses strict JSON parsing so an existing file with invalid syntax
/// is never silently replaced with an empty object.
fn install_mcp_server(
    config_path: &Path,
    tokensave_bin: &str,
    instructions_entry: &str,
    v2: bool,
) -> Result<()> {
    let backup = backup_config_file(config_path)?;
    let mut config = match load_json_file_strict(config_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                crate::agent_note!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };

    let bin = crate::agents::preserve_mcp_command(
        config
            .pointer("/mcp/servers/tokensave/command")
            .or_else(|| config.pointer("/mcp/tokensave/command")),
        tokensave_bin,
    );
    let mcp_server = json!({
        "type": "local",
        "command": [bin, "serve"]
    });
    // #689: V2 reads servers only under `mcp.servers` and never resolves the
    // `instructions` array. Write the shape the detected major reads, and
    // drop the tokensave entry from the other shape so a version change does
    // not leave a stale duplicate. An unknown version keeps the existing
    // shape (V2 config already using `mcp.servers` stays there).
    let use_servers = v2 || config.pointer("/mcp/servers").is_some();
    if use_servers {
        config["mcp"]["servers"]["tokensave"] = mcp_server;
        if let Some(mcp) = config.get_mut("mcp").and_then(|v| v.as_object_mut()) {
            mcp.remove("tokensave");
        }
    } else {
        config["mcp"]["tokensave"] = mcp_server;
        if let Some(servers) = config
            .pointer_mut("/mcp/servers")
            .and_then(|v| v.as_object_mut())
        {
            servers.remove("tokensave");
        }
    }
    if v2 {
        remove_instructions_entry(&mut config, instructions_entry);
    } else {
        add_instructions_entry(&mut config, instructions_entry);
    }

    safe_write_json_file(config_path, &config, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Add `entry` to the config's `"instructions"` array, creating it if absent
/// and skipping if already present (dedupe).
fn add_instructions_entry(config: &mut serde_json::Value, entry: &str) {
    let mut arr: Vec<String> = config["instructions"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !arr.iter().any(|e| e == entry) {
        arr.push(entry.to_string());
    }
    config["instructions"] =
        serde_json::Value::Array(arr.into_iter().map(serde_json::Value::String).collect());
}

/// Remove `entry` from the config's `"instructions"` array, dropping the key
/// entirely if that empties it. Returns whether anything was removed.
fn remove_instructions_entry(config: &mut serde_json::Value, entry: &str) -> bool {
    let removed = if let Some(arr) = config
        .get_mut("instructions")
        .and_then(|v| v.as_array_mut())
    {
        let before = arr.len();
        arr.retain(|v| v.as_str() != Some(entry));
        arr.len() < before
    } else {
        false
    };
    if removed && config["instructions"].as_array().is_some_and(Vec::is_empty) {
        if let Some(obj) = config.as_object_mut() {
            obj.remove("instructions");
        }
    }
    removed
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server (and its `"instructions"` entry) from opencode.json.
fn uninstall_mcp_server(config_path: &Path, instructions_entry: &str) {
    if !config_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(config_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };

    let mcp_removed_v1 = config
        .get_mut("mcp")
        .and_then(|v| v.as_object_mut())
        .is_some_and(|mcp| mcp.remove("tokensave").is_some());
    let mcp_removed_v2 = config
        .pointer_mut("/mcp/servers")
        .and_then(|v| v.as_object_mut())
        .is_some_and(|servers| servers.remove("tokensave").is_some());
    let mcp_removed = mcp_removed_v1 || mcp_removed_v2;
    if mcp_removed
        && config["mcp"]
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
    {
        if let Some(obj) = config.as_object_mut() {
            obj.remove("mcp");
        }
    }
    let instructions_removed = remove_instructions_entry(&mut config, instructions_entry);

    if !mcp_removed && !instructions_removed {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            config_path.display()
        );
        return;
    }

    let is_empty = config.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(config_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            config_path.display()
        );
    } else if backup_and_write_json(config_path, &config) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            config_path.display()
        );
    }
}

/// Remove the pre-#256 tokensave rules block from AGENTS.md, if present.
///
/// Thin wrapper around the shared [`remove_legacy_rules_block`] engine — see
/// that function for the backup/atomic-write/error-propagation contract.
/// `OpenCode`'s block has no sub-headings of its own to skip past.
fn uninstall_prompt_rules(prompt_path: &Path) -> Result<()> {
    remove_legacy_rules_block(prompt_path, "## Prefer tokensave MCP tools", &[])
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check opencode.json has tokensave registered, in the shape the detected
/// `OpenCode` major reads (#689: a V1-shaped entry on V2 is silently ignored,
/// so doctor must not call it healthy).
fn doctor_check_config(dc: &mut DoctorCounters, home: &Path, v2: bool) {
    let config_path = opencode_config_path(home);
    if !config_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent opencode` if you use OpenCode",
            config_path.display()
        ));
        return;
    }

    let config = load_json_file(&config_path);
    let mcp_entry = if v2 {
        config.pointer("/mcp/servers/tokensave")
    } else {
        config
            .pointer("/mcp/servers/tokensave")
            .or_else(|| config.pointer("/mcp/tokensave"))
    };
    let Some(mcp_entry) = mcp_entry else {
        if v2 && config.pointer("/mcp/tokensave").is_some() {
            dc.fail(
                "MCP server registered in the V1 shape (mcp.tokensave), which OpenCode 2 \
                 ignores — run `tokensave install --agent opencode` to rewrite it under \
                 mcp.servers",
            );
            return;
        }
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent opencode`",
            config_path.display()
        ));
        return;
    };
    if !mcp_entry.is_object() {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent opencode`",
            config_path.display()
        ));
        return;
    }
    dc.pass(&format!(
        "MCP server registered in {}",
        config_path.display()
    ));

    let command = mcp_entry["command"].as_array();
    let has_serve = command.is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent opencode`");
    }
}

/// Check the rules surface the detected major actually reads: the managed
/// tokensave.md wired into opencode.json's `"instructions"` for V1 (issue
/// #256), or the managed AGENTS.md block for V2 (#689).
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path, v2: bool) {
    if v2 {
        check_shared_rules_block(dc, &opencode_prompt_path(home), "opencode");
        return;
    }
    let config_path = opencode_config_path(home);
    let rules_path = opencode_rules_path(&config_path);
    check_managed_rules_file(dc, &rules_path, "opencode");

    let config = load_json_file(&config_path);
    let expected_entry = instructions_entry_for(&InstallScope::Global, &rules_path);
    let wired = config["instructions"].as_array().is_some_and(|arr| {
        arr.iter()
            .any(|v| v.as_str() == Some(expected_entry.as_str()))
    });
    if wired {
        dc.pass("opencode.json \"instructions\" includes tokensave.md");
    } else {
        dc.fail(
            "opencode.json \"instructions\" missing tokensave.md — run `tokensave install --agent opencode`",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::parse_opencode_major;

    #[test]
    fn parses_version_command_output() {
        assert_eq!(parse_opencode_major("opencode v2.0.28\n"), Some(2));
        assert_eq!(parse_opencode_major("1.14.20"), Some(1));
        assert_eq!(parse_opencode_major("v10.0"), Some(10));
    }

    #[test]
    fn rejects_unparseable_output() {
        assert_eq!(parse_opencode_major(""), None);
        assert_eq!(parse_opencode_major("opencode"), None);
        assert_eq!(parse_opencode_major("not-a-version"), None);
    }
}
