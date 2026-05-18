//! AWS Kiro agent integration.
//!
//! Handles registration of the tokensave MCP server in Kiro's shared global
//! MCP config (`~/.kiro/settings/mcp.json`), adds global AGENTS.md steering
//! (`~/.kiro/steering/AGENTS.md`), and installs a tokensave-managed Kiro
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
    read_only_tool_names, safe_write_json_file, tool_names, AgentIntegration, DoctorCounters,
    HealthcheckContext, InstallContext,
};

/// Kiro agent.
pub struct KiroIntegration;

const PROMPT_MARKER: &str = "## Prefer tokensave MCP tools";
const PROMPT_END_MARKER: &str = "<!-- tokensave:kiro:end -->";
const KIRO_AGENT_NAME: &str = "tokensave";
const OWNED_AGENT_DESCRIPTION: &str =
    "Default Kiro agent with tokensave MCP tools and code-research guardrails.";
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

fn mcp_config_path(home: &Path) -> PathBuf {
    kiro_home(home).join("settings/mcp.json")
}

fn cli_config_path(home: &Path) -> PathBuf {
    kiro_home(home).join("settings/cli.json")
}

fn managed_agent_path(home: &Path) -> PathBuf {
    kiro_home(home).join("agents/tokensave.json")
}

fn steering_path(home: &Path) -> PathBuf {
    kiro_home(home).join("steering/AGENTS.md")
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

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        std::fs::create_dir_all(kiro_home(&ctx.home)).ok();

        let mcp_path = mcp_config_path(&ctx.home);
        install_mcp_server(&mcp_path, &ctx.tokensave_bin)?;

        let steering = steering_path(&ctx.home);
        install_prompt_rules(&steering)?;

        let agent_path = managed_agent_path(&ctx.home);
        let owns_agent = install_managed_agent(&agent_path, &ctx.tokensave_bin)?;

        let cli_path = cli_config_path(&ctx.home);
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
        uninstall_mcp_server(&mcp_config_path(&ctx.home));
        uninstall_prompt_rules(&steering_path(&ctx.home));
        let agent_path = managed_agent_path(&ctx.home);
        let owned_agent = is_owned_agent_file(&agent_path);
        uninstall_managed_agent(&agent_path);
        uninstall_default_agent(&cli_config_path(&ctx.home), &agent_path, owned_agent);

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
        "disabled": false,
        "autoApprove": read_only_tool_names()
    })
}

fn mutating_tool_names() -> Vec<String> {
    let read_only: std::collections::HashSet<String> = read_only_tool_names().into_iter().collect();
    tool_names()
        .into_iter()
        .filter(|name| !read_only.contains(name))
        .collect()
}

fn hook_command(tokensave_bin: &str, subcommand: &str) -> String {
    format!("{tokensave_bin} {subcommand}")
}

fn managed_agent_config(tokensave_bin: &str) -> serde_json::Value {
    json!({
        "name": KIRO_AGENT_NAME,
        "description": OWNED_AGENT_DESCRIPTION,
        "includeMcpJson": true,
        "prompt": "file://../steering/AGENTS.md",
        "tools": ["*"],
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
fn install_managed_agent(path: &Path, tokensave_bin: &str) -> Result<bool> {
    if path.exists() && !is_owned_agent_file(path) {
        eprintln!(
            "  {} already exists and is user-managed, leaving unchanged",
            path.display()
        );
        return Ok(false);
    }

    let backup = backup_config_file(path)?;
    let config = managed_agent_config(tokensave_bin);
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

/// Append global AGENTS.md steering for default Kiro sessions.
fn install_prompt_rules(path: &Path) -> Result<()> {
    let existing = if path.exists() {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };
    if existing.contains(PROMPT_MARKER) {
        if existing.contains(PROMPT_END_MARKER) {
            eprintln!("  Kiro AGENTS.md already contains tokensave rules, skipping");
            return Ok(());
        }
        if let Some(range) = legacy_prompt_block_range(&existing) {
            let mut updated = existing;
            updated.replace_range(range, &prompt_rules_text());
            std::fs::write(path, updated).map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {}: {e}", path.display()),
            })?;
            eprintln!(
                "\x1b[32m✔\x1b[0m Updated tokensave rules in {}",
                path.display()
            );
        } else {
            eprintln!(
                "  Kiro AGENTS.md contains legacy or edited tokensave rules without an end marker, leaving unchanged"
            );
        }
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
    write!(f, "\n{}\n", prompt_rules_text()).map_err(|e| TokenSaveError::Config {
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

fn uninstall_prompt_rules(path: &Path) {
    if !path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    if !contents.contains(PROMPT_MARKER) {
        eprintln!("  Kiro AGENTS.md does not contain tokensave rules, skipping");
        return;
    }
    let Some(range) = tokensave_prompt_block_range(&contents) else {
        eprintln!(
            "  Kiro AGENTS.md contains tokensave rules without an owned end marker; leaving unchanged"
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
    if let Some(end_marker) = contents[start..].find(PROMPT_END_MARKER) {
        let end = start + end_marker + PROMPT_END_MARKER.len();
        return Some(start..end);
    }
    legacy_prompt_block_range(contents)
}

fn legacy_prompt_block_range(contents: &str) -> Option<Range<usize>> {
    let start = contents.find(PROMPT_MARKER)?;
    let after_marker = start + PROMPT_MARKER.len();
    let end = contents[after_marker..]
        .find("\n## ")
        .map_or(contents.len(), |pos| after_marker + pos);
    let candidate = &contents[start..end];
    if candidate.trim() == prompt_rules_text_without_end_marker() {
        Some(start..end)
    } else {
        None
    }
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

    let expected = read_only_tool_names();
    let approved_count = server
        .get("autoApprove")
        .and_then(|v| v.as_array())
        .map_or(0, |arr| {
            expected
                .iter()
                .filter(|name| arr.iter().any(|v| v.as_str() == Some(name.as_str())))
                .count()
        });
    if approved_count >= expected.len() {
        dc.pass(&format!(
            "All {} read-only tokensave tools auto-approved",
            expected.len()
        ));
    } else {
        dc.warn(&format!(
            "{approved_count}/{} read-only tokensave tools auto-approved -- run `tokensave install --agent kiro` to update",
            expected.len()
        ));
    }

    let auto_approve = server.get("autoApprove").and_then(|v| v.as_array());
    if let Some(auto_approve) = auto_approve {
        let has_broad = auto_approve.iter().any(|v| v.as_str() == Some("*"));
        let mutating_approved: Vec<String> = mutating_tool_names()
            .into_iter()
            .filter(|name| {
                auto_approve
                    .iter()
                    .any(|v| v.as_str() == Some(name.as_str()))
            })
            .collect();
        if has_broad || !mutating_approved.is_empty() {
            dc.warn(
                "Kiro MCP autoApprove includes mutating tokensave tools -- run `tokensave install --agent kiro` to restore read-only defaults",
            );
        } else {
            dc.pass("Kiro MCP autoApprove excludes mutating tokensave tools");
        }
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

    let expected = read_only_tool_names();
    let approved_count = server
        .get("autoApprove")
        .and_then(|v| v.as_array())
        .map_or(0, |arr| {
            expected
                .iter()
                .filter(|name| arr.iter().any(|v| v.as_str() == Some(name.as_str())))
                .count()
        });
    if approved_count < expected.len() {
        dc.warn(&format!(
            "Workspace Kiro MCP tokensave entry auto-approves {approved_count}/{} read-only tools and shadows the global install",
            expected.len()
        ));
    }

    if let Some(auto_approve) = server.get("autoApprove").and_then(|v| v.as_array()) {
        let has_broad = auto_approve.iter().any(|v| v.as_str() == Some("*"));
        let mutating_approved = mutating_tool_names().into_iter().any(|name| {
            auto_approve
                .iter()
                .any(|v| v.as_str() == Some(name.as_str()))
        });
        if has_broad || mutating_approved {
            dc.warn(
                "Workspace Kiro MCP tokensave entry auto-approves mutating tools and shadows the global install",
            );
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
        dc.warn("~/.kiro/steering/AGENTS.md does not exist");
        return;
    }
    let has_rules = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .contains(PROMPT_MARKER);
    if has_rules {
        dc.pass("Kiro global AGENTS.md contains tokensave rules");
    } else {
        dc.fail(
            "Kiro global AGENTS.md missing tokensave rules -- run `tokensave install --agent kiro`",
        );
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

    if config
        .get("tools")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("*")))
    {
        dc.pass("Kiro tokensave agent keeps default tool access");
    } else {
        dc.warn("Kiro tokensave agent tools list may not include all default Kiro tools");
    }

    if config.get("prompt").and_then(serde_json::Value::as_str)
        == Some("file://../steering/AGENTS.md")
    {
        dc.pass("Kiro tokensave agent references global steering");
    } else {
        dc.fail(
            "Kiro tokensave agent prompt should reference global steering -- run `tokensave install --agent kiro`",
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
