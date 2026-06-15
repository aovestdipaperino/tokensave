//! AWS Kiro agent integration.
//!
//! Handles registration of the tokensave MCP server in Kiro's shared global
//! MCP config (`~/.kiro/settings/mcp.json`), adds global tokensave steering
//! (`~/.kiro/steering/tokensave.md`), and installs a tokensave-managed Kiro
//! agent selected as the default when doing so does not overwrite a user's
//! existing default-agent choice.
//!
//! User-owned Kiro agents remain user-managed. If `~/.kiro/agents/tokensave.json`
//! already exists and is not the file tokensave writes, install and uninstall
//! leave it untouched.

use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::{
    backup_and_write_json, backup_config_file, load_json_file, load_json_file_strict,
    safe_write_json_file, AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext,
    InstallScope,
};

/// Kiro agent.
pub struct KiroIntegration;

const PROMPT_MARKER: &str = "## Prefer tokensave MCP tools";
const PROMPT_END_MARKER: &str = "<!-- tokensave:kiro:end -->";
const KIRO_AGENT_NAME: &str = "tokensave";
const OWNED_AGENT_DESCRIPTION: &str =
    "Default Kiro agent with tokensave MCP tools and code-research guardrails.";
const KIRO_AGENT_ALL_TOOLS: &str = "*";
const KIRO_ALLOWED_BUILTIN_TOOLS: &str = "@builtin";
const KIRO_ALLOWED_TOKENSAVE_TOOLS: &str = "@tokensave";
const KIRO_PRE_TOOL_HOOK: &str = "hook-kiro-pre-tool-use";
const KIRO_PROMPT_HOOK: &str = "hook-kiro-prompt-submit";
const KIRO_POST_TOOL_HOOK: &str = "hook-kiro-post-tool-use";
const KIRO_SHORT_HOOK_TIMEOUT_MS: u64 = 5_000;
const KIRO_SYNC_HOOK_TIMEOUT_MS: u64 = 30_000;

fn kiro_home(home: &Path) -> PathBuf {
    if let Ok(kiro) = std::env::var("KIRO_HOME") {
        let kiro_path = PathBuf::from(&kiro);
        let is_real_home = super::home_dir().as_deref() == Some(home);
        if is_real_home || kiro_path.starts_with(home) {
            return kiro_path;
        }
    }
    home.join(".kiro")
}

/// The `.kiro` config root for this install: `~/.kiro` (or `$KIRO_HOME`) for a
/// global install, `<project>/.kiro` for a `--local` install.
fn kiro_base(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => kiro_home(&ctx.home),
        InstallScope::Local { project_path } => project_path.join(".kiro"),
    }
}

fn mcp_config_path_in(base: &Path) -> PathBuf {
    base.join("settings/mcp.json")
}

fn cli_config_path_in(base: &Path) -> PathBuf {
    base.join("settings/cli.json")
}

fn managed_agent_path_in(base: &Path) -> PathBuf {
    base.join("agents/tokensave.json")
}

fn steering_path_in(base: &Path) -> PathBuf {
    base.join("steering/tokensave.md")
}

fn mcp_config_path(home: &Path) -> PathBuf {
    mcp_config_path_in(&kiro_home(home))
}

fn cli_config_path(home: &Path) -> PathBuf {
    cli_config_path_in(&kiro_home(home))
}

fn managed_agent_path(home: &Path) -> PathBuf {
    managed_agent_path_in(&kiro_home(home))
}

fn steering_path(home: &Path) -> PathBuf {
    steering_path_in(&kiro_home(home))
}

fn workspace_mcp_config_path(project_path: &Path) -> PathBuf {
    project_path.join(".kiro/settings/mcp.json")
}

impl AgentIntegration for KiroIntegration {
    fn name(&self) -> &'static str {
        "Kiro"
    }

    fn id(&self) -> &'static str {
        "kiro"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let base = kiro_base(ctx);
        std::fs::create_dir_all(&base).ok();

        let mcp_path = mcp_config_path_in(&base);
        install_mcp_server(&mcp_path, &ctx.tokensave_bin)?;

        let steering = steering_path_in(&base);
        install_steering_rules(&steering)?;

        let agent_path = managed_agent_path_in(&base);
        let owns_agent = install_managed_agent(&agent_path, &ctx.tokensave_bin, &steering)?;

        let cli_path = cli_config_path_in(&base);
        install_default_agent(&cli_path, owns_agent)?;

        eprintln!();
        eprintln!("Setup complete. Next steps:");
        eprintln!("  1. cd into your project and run: tokensave init");
        eprintln!("  2. Start a new Kiro session");
        eprintln!("     tokensave tools are now available through Kiro MCP");
        eprintln!(
            "     the tokensave Kiro agent includes hooks for delegation guardrails and sync"
        );
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let base = kiro_base(ctx);
        uninstall_mcp_server(&mcp_config_path_in(&base));
        remove_steering_rules(&steering_path_in(&base));
        let agent_path = managed_agent_path_in(&base);
        let owned_agent = is_owned_agent_file(&agent_path);
        uninstall_managed_agent(&agent_path);
        uninstall_default_agent(&cli_config_path_in(&base), &agent_path, owned_agent);

        eprintln!();
        eprintln!("Uninstall complete. Tokensave has been removed from Kiro.");
        eprintln!("Start a new Kiro session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        eprintln!("\n\x1b[1mKiro integration\x1b[0m");
        let global_server = doctor_check_mcp_config(dc, &ctx.home);
        doctor_check_workspace_mcp_override(
            dc,
            &ctx.home,
            &ctx.project_path,
            global_server.as_ref(),
        );
        doctor_check_steering(dc, &ctx.home);
        doctor_check_managed_agent(dc, &ctx.home);
        doctor_check_default_agent(dc, &ctx.home);
    }

    fn is_detected(&self, home: &Path) -> bool {
        kiro_home(home).is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(mcp_config_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let path = mcp_config_path(home);
        if !path.exists() {
            return false;
        }
        let json = load_json_file(&path);
        json.get("mcpServers")
            .and_then(|v| v.get("tokensave"))
            .is_some()
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

fn mcp_server_entry(tokensave_bin: &str) -> serde_json::Value {
    json!({
        "command": tokensave_bin,
        "args": ["serve"],
        "disabled": false
    })
}

fn hook_command(tokensave_bin: &str, subcommand: &str) -> String {
    format!("{tokensave_bin} {subcommand}")
}

fn file_resource_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = percent_encode_file_uri_path(&path);
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn percent_encode_file_uri_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0F) as usize] as char);
            }
        }
    }
    encoded
}

fn managed_agent_config(tokensave_bin: &str, steering_path: &Path) -> serde_json::Value {
    json!({
        "name": KIRO_AGENT_NAME,
        "description": OWNED_AGENT_DESCRIPTION,
        "includeMcpJson": true,
        "resources": [file_resource_uri(steering_path)],
        "tools": [KIRO_AGENT_ALL_TOOLS],
        "allowedTools": [KIRO_ALLOWED_BUILTIN_TOOLS, KIRO_ALLOWED_TOKENSAVE_TOOLS],
        "hooks": {
            "userPromptSubmit": [
                {
                    "command": hook_command(tokensave_bin, KIRO_PROMPT_HOOK),
                    "timeout_ms": KIRO_SHORT_HOOK_TIMEOUT_MS
                }
            ],
            "preToolUse": [
                {
                    "matcher": "delegate",
                    "command": hook_command(tokensave_bin, KIRO_PRE_TOOL_HOOK),
                    "timeout_ms": KIRO_SHORT_HOOK_TIMEOUT_MS
                },
                {
                    "matcher": "subagent",
                    "command": hook_command(tokensave_bin, KIRO_PRE_TOOL_HOOK),
                    "timeout_ms": KIRO_SHORT_HOOK_TIMEOUT_MS
                }
            ],
            "postToolUse": [
                {
                    "matcher": "fs_write",
                    "command": hook_command(tokensave_bin, KIRO_POST_TOOL_HOOK),
                    "timeout_ms": KIRO_SYNC_HOOK_TIMEOUT_MS
                }
            ]
        }
    })
}

/// Register MCP server in ~/.kiro/settings/mcp.json.
fn install_mcp_server(path: &Path, tokensave_bin: &str) -> Result<()> {
    let backup = backup_config_file(path)?;
    let mut config = match load_json_file_strict(path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                eprintln!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };

    ensure_json_object(&config, path)?;
    ensure_child_object(&mut config, "mcpServers", path)?;
    config["mcpServers"]["tokensave"] = mcp_server_entry(tokensave_bin);

    safe_write_json_file(path, &config, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        path.display()
    );
    Ok(())
}

/// Create or refresh the tokensave-owned Kiro agent.
///
/// Returns true when tokensave owns the resulting agent file. A pre-existing
/// user-managed `tokensave.json` is preserved and returns false so the default
/// agent selector is not pointed at a file whose policy tokensave does not own.
fn install_managed_agent(path: &Path, tokensave_bin: &str, steering_path: &Path) -> Result<bool> {
    if path.exists() && !is_owned_agent_file(path) {
        eprintln!(
            "  {} already exists and is user-managed, leaving unchanged",
            path.display()
        );
        return Ok(false);
    }

    let backup = backup_config_file(path)?;
    let config = managed_agent_config(tokensave_bin, steering_path);
    safe_write_json_file(path, &config, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Wrote tokensave Kiro agent to {}",
        path.display()
    );
    Ok(true)
}

fn install_default_agent(path: &Path, owns_agent: bool) -> Result<()> {
    if !owns_agent {
        eprintln!(
            "  Skipping Kiro default-agent update because tokensave does not own the agent file"
        );
        return Ok(());
    }

    let backup = backup_config_file(path)?;
    let mut config = match load_json_file_strict(path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                eprintln!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };

    ensure_json_object(&config, path)?;
    ensure_child_object(&mut config, "chat", path)?;

    match config["chat"].get("defaultAgent") {
        Some(v) if v.as_str() == Some(KIRO_AGENT_NAME) => {
            eprintln!("  Kiro default agent already set to tokensave");
            return Ok(());
        }
        Some(v) if v.as_str().is_some_and(is_builtin_default_agent) => {}
        Some(v) if is_empty_default_agent(v) => {}
        None => {}
        Some(v) => {
            eprintln!(
                "  Kiro default agent is {}, leaving user choice unchanged",
                format_json_scalar(v)
            );
            return Ok(());
        }
    }

    config["chat"]["defaultAgent"] = json!(KIRO_AGENT_NAME);
    safe_write_json_file(path, &config, backup.as_deref())?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Set Kiro default agent in {}",
        path.display()
    );
    Ok(())
}

fn is_builtin_default_agent(agent: &str) -> bool {
    matches!(agent, "kiro_default" | "default")
}

fn is_empty_default_agent(value: &serde_json::Value) -> bool {
    value.is_null() || value.as_str() == Some("")
}

fn format_json_scalar(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), |s| format!("\"{s}\""))
}

fn ensure_json_object(config: &serde_json::Value, path: &Path) -> Result<()> {
    if config.is_object() {
        Ok(())
    } else {
        Err(TokenSaveError::Config {
            message: format!("{} must contain a JSON object", path.display()),
        })
    }
}

fn ensure_child_object(config: &mut serde_json::Value, key: &str, path: &Path) -> Result<()> {
    if config.get(key).is_none() {
        config[key] = json!({});
        return Ok(());
    }
    if config.get(key).is_some_and(serde_json::Value::is_object) {
        Ok(())
    } else {
        Err(TokenSaveError::Config {
            message: format!("{}.{} must be a JSON object", path.display(), key),
        })
    }
}

/// Add tokensave's global steering resource for default Kiro sessions.
fn install_steering_rules(path: &Path) -> Result<()> {
    let existing = if path.exists() {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(PROMPT_MARKER) {
        if existing.contains(PROMPT_END_MARKER) {
            eprintln!("  Kiro steering already contains tokensave rules, skipping");
            return Ok(());
        }
        eprintln!(
            "  Kiro steering contains tokensave rules without an owned end marker, leaving unchanged"
        );
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TokenSaveError::Config {
            message: format!("failed to create {}: {e}", parent.display()),
        })?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| TokenSaveError::Config {
            message: format!("failed to open {}: {e}", path.display()),
        })?;
    let separator = if existing.trim().is_empty() {
        ""
    } else {
        "\n\n"
    };
    writeln!(f, "{}{}", separator, prompt_rules_text()).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write {}: {e}", path.display()),
    })?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Appended tokensave rules to {}",
        path.display()
    );
    Ok(())
}

fn prompt_rules_text() -> String {
    format!(
        "{}\n\n{}",
        prompt_rules_text_without_end_marker(),
        PROMPT_END_MARKER
    )
}

fn prompt_rules_text_without_end_marker() -> &'static str {
    "## Prefer tokensave MCP tools\n\n\
Before reading source files or scanning the codebase, use the tokensave MCP tools \
(`tokensave_context`, `tokensave_search`, `tokensave_callers`, `tokensave_callees`, \
`tokensave_impact`, `tokensave_node`, `tokensave_files`, `tokensave_affected`). \
They provide semantic results from a pre-built local knowledge graph and are faster \
than broad file reads.\n\n\
Do not use Kiro's `delegate` tool for codebase exploration, architecture mapping, \
call graph work, symbol lookup, or other code research until tokensave MCP tools \
have been tried. Delegation is still appropriate for long-running execution work \
such as builds, tests, generated reports, or independent implementation tasks.\n\n\
If a code analysis question cannot be fully answered by tokensave MCP tools, try \
querying the SQLite database directly at `.tokensave/tokensave.db` (tables: `nodes`, \
`edges`, `files`). Use SQL for structural queries that go beyond the MCP tools.\n\n\
If you discover a gap where an extractor, schema, or tokensave tool could answer a \
question natively, propose opening an issue at \
https://github.com/aovestdipaperino/tokensave. Remind the user to strip sensitive \
or proprietary code from the bug description before submitting."
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

fn uninstall_mcp_server(path: &Path) {
    if !path.exists() {
        eprintln!("  {} not found, skipping", path.display());
        return;
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = config.get_mut("mcpServers").and_then(|v| v.as_object_mut()) else {
        eprintln!("  No tokensave MCP server in {}, skipping", path.display());
        return;
    };
    if servers.remove("tokensave").is_none() {
        eprintln!("  No tokensave MCP server in {}, skipping", path.display());
        return;
    }
    if servers.is_empty() {
        config.as_object_mut().map(|o| o.remove("mcpServers"));
    }
    let is_empty = config.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(path).ok();
        eprintln!("\x1b[32m✔\x1b[0m Removed {} (was empty)", path.display());
    } else if backup_and_write_json(path, &config) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            path.display()
        );
    }
}

fn remove_steering_rules(path: &Path) {
    if !path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    if !contents.contains(PROMPT_MARKER) {
        eprintln!("  Kiro steering does not contain tokensave rules, skipping");
        return;
    }
    let Some(range) = tokensave_prompt_block_range(&contents) else {
        eprintln!(
            "  Kiro steering contains tokensave rules without an owned end marker; leaving unchanged"
        );
        return;
    };
    let mut new_contents = String::new();
    new_contents.push_str(contents[..range.start].trim_end());
    let remainder = &contents[range.end..];
    if !remainder.is_empty() {
        new_contents.push_str("\n\n");
        new_contents.push_str(remainder.trim_start());
    }
    let new_contents = new_contents.trim().to_string();
    if new_contents.is_empty() {
        std::fs::remove_file(path).ok();
        eprintln!("\x1b[32m✔\x1b[0m Removed {} (was empty)", path.display());
    } else {
        std::fs::write(path, format!("{new_contents}\n")).ok();
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave rules from {}",
            path.display()
        );
    }
}

fn uninstall_managed_agent(path: &Path) {
    if !path.exists() {
        return;
    }
    if !is_owned_agent_file(path) {
        eprintln!("  {} is user-managed, leaving unchanged", path.display());
        return;
    }
    if std::fs::remove_file(path).is_ok() {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave Kiro agent from {}",
            path.display()
        );
        if let Some(parent) = path.parent() {
            std::fs::remove_dir(parent).ok();
        }
    }
}

fn uninstall_default_agent(path: &Path, agent_path: &Path, owned_agent: bool) {
    if !path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    if config
        .get("chat")
        .and_then(|v| v.get("defaultAgent"))
        .and_then(serde_json::Value::as_str)
        != Some(KIRO_AGENT_NAME)
    {
        return;
    }
    if agent_path.exists() && !owned_agent {
        eprintln!(
            "  Kiro default agent points at a user-managed tokensave agent, leaving unchanged"
        );
        return;
    }

    let Some(chat) = config.get_mut("chat").and_then(|v| v.as_object_mut()) else {
        return;
    };
    chat.remove("defaultAgent");
    if chat.is_empty() {
        config.as_object_mut().map(|o| o.remove("chat"));
    }

    let is_empty = config.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(path).ok();
        eprintln!("\x1b[32m✔\x1b[0m Removed {} (was empty)", path.display());
    } else if backup_and_write_json(path, &config) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Removed tokensave Kiro default agent from {}",
            path.display()
        );
    }
}

fn is_owned_agent_file(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let config = load_json_file(path);
    is_owned_agent_config(&config)
}

fn is_owned_agent_config(config: &serde_json::Value) -> bool {
    config.get("name").and_then(serde_json::Value::as_str) == Some(KIRO_AGENT_NAME)
        && config
            .get("description")
            .and_then(serde_json::Value::as_str)
            == Some(OWNED_AGENT_DESCRIPTION)
}

fn tokensave_prompt_block_range(contents: &str) -> Option<Range<usize>> {
    let start = contents.find(PROMPT_MARKER)?;
    let end_marker = contents[start..].find(PROMPT_END_MARKER)?;
    let end = start + end_marker + PROMPT_END_MARKER.len();
    Some(start..end)
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

fn doctor_check_mcp_config(dc: &mut DoctorCounters, home: &Path) -> Option<serde_json::Value> {
    let path = mcp_config_path(home);
    if !path.exists() {
        dc.warn(&format!(
            "{} not found -- run `tokensave install --agent kiro` if you use Kiro",
            path.display()
        ));
        return None;
    }

    let config = load_json_file(&path);
    let server = config.get("mcpServers").and_then(|v| v.get("tokensave"));

    let Some(server_value) = server else {
        dc.fail(&format!(
            "MCP server NOT registered in {} -- run `tokensave install --agent kiro`",
            path.display()
        ));
        return None;
    };
    let Some(server) = server_value.as_object() else {
        dc.fail(&format!(
            "MCP server in {} is not an object -- run `tokensave install --agent kiro`",
            path.display()
        ));
        return None;
    };
    dc.pass(&format!("MCP server registered in {}", path.display()));

    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" -- run `tokensave install --agent kiro`");
    }

    let disabled = server
        .get("disabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if disabled {
        dc.fail("MCP server is disabled -- run `tokensave install --agent kiro`");
    } else {
        dc.pass("MCP server is enabled");
    }

    Some(server_value.clone())
}

fn doctor_check_workspace_mcp_override(
    dc: &mut DoctorCounters,
    home: &Path,
    project_path: &Path,
    global_server: Option<&serde_json::Value>,
) {
    let path = workspace_mcp_config_path(project_path);
    if path == mcp_config_path(home) {
        return;
    }
    if !path.exists() {
        dc.pass("No workspace Kiro MCP tokensave override");
        return;
    }

    let config = load_json_file(&path);
    let server = config.get("mcpServers").and_then(|v| v.get("tokensave"));
    let Some(server_value) = server else {
        dc.pass("No workspace Kiro MCP tokensave override");
        return;
    };
    let Some(server) = server_value.as_object() else {
        dc.fail(&format!(
            "Workspace Kiro MCP tokensave entry in {} is not an object and shadows the global install",
            path.display()
        ));
        return;
    };

    let mut compatible = true;
    let disabled = server
        .get("disabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if disabled {
        dc.fail(&format!(
            "Workspace Kiro MCP tokensave entry in {} is disabled and shadows the global install",
            path.display()
        ));
        compatible = false;
    }

    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if !has_serve {
        dc.fail(&format!(
            "Workspace Kiro MCP tokensave entry in {} is missing \"serve\" and shadows the global install",
            path.display()
        ));
        compatible = false;
    }

    if let Some(global_server) = global_server {
        let workspace_command = server.get("command").and_then(|v| v.as_str());
        let global_command = global_server.get("command").and_then(|v| v.as_str());
        if workspace_command != global_command {
            dc.fail(&format!(
                "Workspace Kiro MCP tokensave command in {} differs from the global install",
                path.display()
            ));
            compatible = false;
        }
    }

    if compatible {
        dc.pass(&format!(
            "Workspace Kiro MCP tokensave override in {} is compatible",
            path.display()
        ));
    }
}

fn doctor_check_steering(dc: &mut DoctorCounters, home: &Path) {
    let path = steering_path(home);
    if !path.exists() {
        dc.warn("~/.kiro/steering/tokensave.md does not exist");
        return;
    }
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    if !contents.contains(PROMPT_MARKER) {
        dc.fail(
            "Kiro global tokensave.md missing tokensave rules -- run `tokensave install --agent kiro`",
        );
    } else if tokensave_prompt_block_range(&contents).is_none() {
        dc.fail(
            "Kiro global tokensave.md contains tokensave rules without an owned end marker -- remove the stale block and run `tokensave install --agent kiro`",
        );
    } else {
        dc.pass("Kiro global tokensave.md contains tokensave rules");
    }
}

fn doctor_check_managed_agent(dc: &mut DoctorCounters, home: &Path) {
    let path = managed_agent_path(home);
    if !path.exists() {
        dc.fail(&format!(
            "Kiro tokensave agent NOT installed at {} -- run `tokensave install --agent kiro`",
            path.display()
        ));
        return;
    }

    let config = load_json_file(&path);
    if !is_owned_agent_config(&config) {
        dc.warn(&format!(
            "{} is user-managed; tokensave hooks were not installed there",
            path.display()
        ));
        return;
    }

    dc.pass(&format!("Kiro tokensave agent: {}", path.display()));

    if config
        .get("includeMcpJson")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        dc.pass("Kiro tokensave agent includes global/workspace MCP config");
    } else {
        dc.fail("Kiro tokensave agent missing includeMcpJson=true -- run `tokensave install --agent kiro`");
    }

    doctor_check_agent_tools(dc, &config);
    doctor_check_agent_allowed_tools(dc, &config);

    let expected_resource = file_resource_uri(&steering_path(home));
    if config
        .get("resources")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| {
            arr.iter()
                .any(|v| v.as_str() == Some(expected_resource.as_str()))
        })
    {
        dc.pass("Kiro tokensave agent loads global steering as a resource");
    } else {
        dc.fail(
            "Kiro tokensave agent missing global steering resource -- run `tokensave install --agent kiro`",
        );
    }

    doctor_check_agent_hook(
        dc,
        &config,
        "userPromptSubmit",
        None,
        KIRO_PROMPT_HOOK,
        KIRO_SHORT_HOOK_TIMEOUT_MS,
    );
    doctor_check_agent_hook(
        dc,
        &config,
        "preToolUse",
        Some("delegate"),
        KIRO_PRE_TOOL_HOOK,
        KIRO_SHORT_HOOK_TIMEOUT_MS,
    );
    doctor_check_agent_hook(
        dc,
        &config,
        "preToolUse",
        Some("subagent"),
        KIRO_PRE_TOOL_HOOK,
        KIRO_SHORT_HOOK_TIMEOUT_MS,
    );
    doctor_check_agent_hook(
        dc,
        &config,
        "postToolUse",
        Some("fs_write"),
        KIRO_POST_TOOL_HOOK,
        KIRO_SYNC_HOOK_TIMEOUT_MS,
    );
}

fn doctor_check_agent_tools(dc: &mut DoctorCounters, config: &serde_json::Value) {
    if json_array_contains_str(config, "tools", KIRO_AGENT_ALL_TOOLS) {
        dc.pass("Kiro tokensave agent exposes all configured tools");
    } else {
        dc.warn(
            "Kiro tokensave agent tools list is not permissive -- run `tokensave install --agent kiro`",
        );
    }
}

fn doctor_check_agent_allowed_tools(dc: &mut DoctorCounters, config: &serde_json::Value) {
    let required = [KIRO_ALLOWED_BUILTIN_TOOLS, KIRO_ALLOWED_TOKENSAVE_TOOLS];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|tool| !json_array_contains_str(config, "allowedTools", tool))
        .collect();

    if missing.is_empty() {
        dc.pass("Kiro tokensave agent pre-approves built-in and tokensave tools");
    } else {
        dc.warn(
            "Kiro tokensave agent allowedTools is not permissive -- run `tokensave install --agent kiro`",
        );
        for tool in missing {
            dc.info(&format!("missing allowedTools entry: {tool}"));
        }
    }
}

fn json_array_contains_str(config: &serde_json::Value, field: &str, expected: &str) -> bool {
    config
        .get(field)
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(expected)))
}

fn doctor_check_agent_hook(
    dc: &mut DoctorCounters,
    config: &serde_json::Value,
    event: &str,
    matcher: Option<&str>,
    subcommand: &str,
    timeout_ms: u64,
) {
    let hook = find_agent_hook(config, event, matcher, subcommand);
    let Some(hook) = hook else {
        let matcher_label = matcher.map_or(String::new(), |m| format!(" ({m})"));
        dc.fail(&format!(
            "Kiro {event}{matcher_label} hook missing {subcommand} -- run `tokensave install --agent kiro`"
        ));
        return;
    };

    let timeout_ok = hook.get("timeout_ms").and_then(serde_json::Value::as_u64) == Some(timeout_ms);
    if timeout_ok {
        let matcher_label = matcher.map_or(String::new(), |m| format!(" ({m})"));
        dc.pass(&format!("Kiro {event}{matcher_label} hook installed"));
    } else {
        dc.warn(&format!(
            "Kiro {event} hook timeout differs from tokensave default -- run `tokensave install --agent kiro` to update"
        ));
    }
}

fn find_agent_hook<'a>(
    config: &'a serde_json::Value,
    event: &str,
    matcher: Option<&str>,
    subcommand: &str,
) -> Option<&'a serde_json::Value> {
    config
        .get("hooks")
        .and_then(|v| v.get(event))
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|hook| {
            let matcher_ok = match matcher {
                Some(expected) => {
                    hook.get("matcher").and_then(serde_json::Value::as_str) == Some(expected)
                }
                None => hook.get("matcher").is_none(),
            };
            matcher_ok
                && hook
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|cmd| cmd.split_whitespace().any(|part| part == subcommand))
        })
}

fn doctor_check_default_agent(dc: &mut DoctorCounters, home: &Path) {
    let path = cli_config_path(home);
    if !path.exists() {
        dc.fail(&format!(
            "{} not found -- run `tokensave install --agent kiro`",
            path.display()
        ));
        return;
    }

    let config = load_json_file(&path);
    let default_agent = config
        .get("chat")
        .and_then(|v| v.get("defaultAgent"))
        .and_then(serde_json::Value::as_str);

    match default_agent {
        Some(KIRO_AGENT_NAME) => dc.pass("Kiro default agent is tokensave"),
        Some(agent) if is_builtin_default_agent(agent) => dc.warn(
            "Kiro default agent is still the built-in default -- run `tokensave install --agent kiro`",
        ),
        Some(agent) => dc.warn(&format!(
            "Kiro default agent is \"{agent}\"; tokensave hooks run only when the tokensave agent is selected"
        )),
        None => dc.warn(
            "Kiro default agent is not set; tokensave hooks run only when the tokensave agent is selected",
        ),
    }
}
