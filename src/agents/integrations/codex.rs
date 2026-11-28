// Rust guideline compliant 2025-10-17
//! `OpenAI` Codex CLI agent integration.
//!
//! Handles registration of the tokensave MCP server in Codex's config
//! file (`~/.codex/config.toml`), per-tool auto-approval settings,
//! the PreToolUse shell hook, and prompt rules via `AGENTS.md`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};

use super::*;

/// Keys and defaults of a Codex `[mcp_servers.<name>]` table (#587).
mod codex_config {
    /// Seconds Codex waits for a server to finish `initialize` and
    /// `tools/list`; slower servers are dropped and their tools never appear.
    pub const STARTUP_TIMEOUT_SEC_KEY: &str = "startup_timeout_sec";
    /// Older spelling of the same timeout, in milliseconds.
    pub const STARTUP_TIMEOUT_MS_KEY: &str = "startup_timeout_ms";
    /// Codex's own default when neither key is set.
    pub const DEFAULT_STARTUP_TIMEOUT_SECS: u64 = 10;
    /// What `install` writes: room for a first index or a schema migration
    /// before `serve` answers `initialize`.
    pub const INSTALLED_STARTUP_TIMEOUT_SECS: i64 = 60;
    /// `false` keeps Codex from starting the server at all.
    pub const ENABLED_KEY: &str = "enabled";
    /// Allow list: when present, only these tools are exposed.
    pub const ENABLED_TOOLS_KEY: &str = "enabled_tools";
    /// Deny list: these tools are hidden.
    pub const DISABLED_TOOLS_KEY: &str = "disabled_tools";
    /// Extra environment for the server process.
    pub const ENV_KEY: &str = "env";
    /// Extra time the doctor probe allows past the timeout, to tell "slow"
    /// from "hung".
    pub const PROBE_GRACE_SECS: u64 = 10;
    /// File stem of a command the doctor probe is willing to launch.
    pub const TOKENSAVE_COMMAND_STEM: &str = "tokensave";
    pub const HOOKS_FILE: &str = "hooks.json";
    pub const HOOKS_KEY: &str = "hooks";
    pub const PRE_TOOL_USE: &str = "PreToolUse";
    pub const SHELL_MATCHER: &str = "^(Bash|shell_command)$";
    pub const SHELL_HOOK: &str = "hook-pre-tool-use-codex";
}

/// `OpenAI` Codex CLI agent.
pub struct CodexIntegration;

impl AgentIntegration for CodexIntegration {
    fn name(&self) -> &'static str {
        "Codex CLI"
    }

    fn id(&self) -> &'static str {
        "codex"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let codex_dir = ctx.home.join(".codex");
        std::fs::create_dir_all(&codex_dir).ok();
        let config_path = codex_dir.join("config.toml");

        install_mcp_server(&config_path, &ctx.tokensave_bin, ctx)?;
        install_shell_hook(
            &codex_dir.join(codex_config::HOOKS_FILE),
            &ctx.tokensave_bin,
        )?;

        let agents_md = codex_dir.join("AGENTS.md");
        install_prompt_rules(&agents_md)?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new Codex session — tokensave tools are now available");
        crate::agent_note!("  3. Use /hooks in Codex to review and trust the TokenSave shell hook");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");

        if has_user_approvals(&config_path, ctx) {
            crate::agent_note!("  MCP entry kept because it contains user-managed tool approvals.");
        } else {
            uninstall_mcp_server(&config_path)?;
        }

        let agents_md = codex_dir.join("AGENTS.md");
        uninstall_prompt_rules(&agents_md);
        let hooks_path = codex_dir.join(codex_config::HOOKS_FILE);
        let mut hooks = load_json_file_strict(&hooks_path)?;
        if remove_shell_hook(&mut hooks) {
            write_json_file(&hooks_path, &hooks)?;
        }

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Codex CLI.");
        crate::agent_note!("Start a new Codex session for changes to take effect.");
        Ok(())
    }

    fn plan_uninstall(
        &self,
        ctx: &InstallContext,
        plan: &mut crate::agents::uninstall::UninstallPlan,
    ) -> Result<()> {
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");

        if has_user_approvals(&config_path, ctx) {
            plan.skipped("MCP server", "contains user-managed tool approvals");
        } else {
            plan.toml_mcp(&config_path);
        }

        let agents_md = codex_dir.join("AGENTS.md");
        plan.rules(&agents_md, crate::agents::rules::LEGACY_RULES_MARKER, &[]);
        plan.json(
            "Codex shell hook",
            &codex_dir.join(codex_config::HOOKS_FILE),
            false,
            remove_shell_hook,
        );
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mCodex CLI integration\x1b[0m");
        let codex_dir = ctx.home.join(".codex");
        let config_path = codex_dir.join("config.toml");
        doctor_check_config(dc, &config_path, permissions_management_enabled(&ctx.home));
        if let Some(server) = load_toml_file(&config_path).ok().and_then(|config| {
            config
                .get("mcp_servers")
                .and_then(|servers| servers.get("tokensave"))
                .and_then(toml::Value::as_table)
                .cloned()
        }) {
            doctor_check_launch(dc, &server, &ctx.project_path);
        }
        doctor_check_shell_hook(dc, &codex_dir.join(codex_config::HOOKS_FILE));
        doctor_check_prompt(dc, &codex_dir);
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".codex").is_dir()
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(home.join(".codex").join("config.toml"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let config = home.join(".codex").join("config.toml");
        if !config.exists() {
            return false;
        }
        // If the file is unparseable, conservatively report "not installed"
        // so the caller treats it like a fresh install path.
        super::load_toml_file(&config).is_ok_and(|toml| {
            toml.get("mcp_servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        })
    }
}

// ---------------------------------------------------------------------------
// Install helpers
// ---------------------------------------------------------------------------

fn has_user_approvals(path: &Path, ctx: &InstallContext) -> bool {
    !permissions_management_enabled(&ctx.home)
        && load_toml_file(path).is_ok_and(|config| {
            config
                .get("mcp_servers")
                .and_then(|servers| servers.get("tokensave"))
                .and_then(|server| server.get("tools"))
                .is_some()
        })
}

/// Register MCP server and auto-approve tools in ~/.codex/config.toml.
///
/// With `manage_permissions` false (#1067) the per-tool approvals are the
/// user's: an existing `tools` table is carried over unchanged and none is
/// created.
fn install_mcp_server(config_path: &Path, tokensave_bin: &str, ctx: &InstallContext) -> Result<()> {
    let manage_permissions = permissions_management_enabled(&ctx.home);
    let mut config = load_toml_file(config_path)?;

    // Ensure [mcp_servers.tokensave] exists
    let table = config
        .as_table_mut()
        .ok_or_else(|| TokenSaveError::Config {
            message: "config.toml is not a TOML table".to_string(),
        })?;

    let servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| TokenSaveError::Config {
            message: "mcp_servers is not a table in config.toml".to_string(),
        })?;

    let bin = crate::agents::preserve_mcp_command_str(
        servers
            .get("tokensave")
            .and_then(|t| t.get("command"))
            .and_then(toml::Value::as_str),
        tokensave_bin,
    );
    let mut server_table = servers
        .get("tokensave")
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default();
    server_table.insert("command".to_string(), toml::Value::String(bin));
    server_table.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("serve".to_string())]),
    );
    // Codex drops a server that has not listed its tools within 10 s, and a
    // first index or schema migration can take longer (#587). Keep any
    // timeout the user chose.
    if !server_table.contains_key(codex_config::STARTUP_TIMEOUT_SEC_KEY)
        && !server_table.contains_key(codex_config::STARTUP_TIMEOUT_MS_KEY)
    {
        server_table.insert(
            codex_config::STARTUP_TIMEOUT_SEC_KEY.to_string(),
            toml::Value::Integer(codex_config::INSTALLED_STARTUP_TIMEOUT_SECS),
        );
    }

    if manage_permissions {
        let tools = server_table
            .entry("tools")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(tools) = tools.as_table_mut() {
            if ctx.force_permission_style {
                for policy in tools
                    .iter_mut()
                    .filter_map(|(_, value)| value.as_table_mut())
                {
                    if policy.get("approval_mode").and_then(toml::Value::as_str) == Some("auto") {
                        policy.remove("approval_mode");
                    }
                }
            }
            for name in crate::agents::approved_tool_names(&ctx.tool_permissions) {
                let policy = tools
                    .entry(name)
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()));
                if let Some(policy) = policy.as_table_mut() {
                    policy
                        .entry("approval_mode")
                        .or_insert_with(|| toml::Value::String("auto".to_string()));
                }
            }
        }
    } else {
        crate::agent_note!("{PERMISSIONS_USER_MANAGED_NOTE}");
    }

    servers.insert("tokensave".to_string(), toml::Value::Table(server_table));

    write_toml_file(config_path, &config)?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        config_path.display()
    );
    Ok(())
}

/// Write or refresh the tokensave rules block in AGENTS.md.
fn install_prompt_rules(agents_md: &Path) -> Result<()> {
    let body = rules_for_agent("codex")?;
    write_rules_block(agents_md, "codex", &body).map(|_| ())
}

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server from ~/.codex/config.toml.
fn uninstall_mcp_server(config_path: &Path) -> Result<()> {
    if !config_path.exists() {
        return Ok(());
    }
    let mut config = load_toml_file(config_path)?;
    let Some(table) = config.as_table_mut() else {
        return Ok(());
    };
    let Some(servers) = table.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) else {
        return Ok(());
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            config_path.display()
        );
        return Ok(());
    }
    if servers.is_empty() {
        table.remove("mcp_servers");
    }
    if table.is_empty() {
        std::fs::remove_file(config_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            config_path.display()
        );
    } else {
        write_toml_file(config_path, &config)?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            config_path.display()
        );
    }
    Ok(())
}

/// Remove tokensave rules from AGENTS.md (both managed-block and legacy
/// heading-guarded forms).
fn uninstall_prompt_rules(agents_md: &Path) {
    remove_rules_block(agents_md).ok();
    remove_legacy_rules_block(agents_md, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check config.toml has tokensave registered.
fn doctor_check_config(dc: &mut DoctorCounters, config_path: &Path, manage_permissions: bool) {
    if !config_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent codex` if you use Codex CLI",
            config_path.display()
        ));
        return;
    }

    let config = match load_toml_file(config_path) {
        Ok(c) => c,
        Err(e) => {
            dc.fail(&format!("{e}"));
            return;
        }
    };
    let has_server = config
        .get("mcp_servers")
        .and_then(|v| v.get("tokensave"))
        .and_then(|v| v.as_table())
        .is_some();

    if !has_server {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent codex`",
            config_path.display()
        ));
        return;
    }
    dc.pass(&format!(
        "MCP server registered in {}",
        config_path.display()
    ));

    if !manage_permissions {
        dc.pass(PERMISSIONS_USER_MANAGED_DOCTOR);
        return;
    }

    // Check read-only auto-approvals and report retained write policy.
    let tools = config
        .get("mcp_servers")
        .and_then(|v| v.get("tokensave"))
        .and_then(|v| v.get("tools"))
        .and_then(|v| v.as_table());

    if tools.is_some_and(|tools| {
        tools.iter().any(|(name, policy)| {
            crate::mcp::tools::tool_capability(name)
                != Some(crate::mcp::tools::ToolCapability::ReadOnly)
                && policy.get("approval_mode").and_then(toml::Value::as_str) == Some("auto")
        })
    }) {
        dc.warn("Existing Codex write approvals kept. Use tokensave install --agent codex --reset-tool-approvals to restore write prompts.");
    }

    let auto_count = tools.map_or(0, |t| {
        t.iter()
            .filter(|(name, v)| {
                crate::mcp::tools::tool_capability(name)
                    == Some(crate::mcp::tools::ToolCapability::ReadOnly)
                    && v.get("approval_mode").and_then(|m| m.as_str()) == Some("auto")
            })
            .count()
    });

    let tools = crate::agents::read_only_tool_names();
    let tools_len = tools.len();
    if auto_count >= tools_len {
        dc.pass(&format!("All {tools_len} tools set to auto-approve"));
    } else if auto_count > 0 {
        dc.warn(&format!(
            "{auto_count}/{tools_len} tools auto-approved — run `tokensave install --agent codex` to update"
        ));
    } else {
        dc.warn("No tools auto-approved — Codex will prompt for each tool call");
    }
}

/// Resolve an MCP `command` the way a process launcher would: a path is
/// taken as written, a bare name is looked up on `PATH`.
fn resolve_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 || path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }
    let search = std::env::var_os("PATH")?;
    std::env::split_paths(&search).find_map(|dir| {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(windows) {
            let exe = candidate.with_extension(std::env::consts::EXE_EXTENSION);
            if exe.is_file() {
                return Some(exe);
            }
        }
        None
    })
}

/// The startup timeout Codex applies to this server, and whether the config
/// sets it explicitly.
fn startup_timeout(server: &toml::Table) -> (Duration, bool) {
    let as_secs = |v: &toml::Value| {
        v.as_float()
            .or_else(|| v.as_integer().map(|i| i as f64))
            .filter(|s| s.is_finite() && *s > 0.0)
    };
    if let Some(secs) = server
        .get(codex_config::STARTUP_TIMEOUT_SEC_KEY)
        .and_then(as_secs)
    {
        return (Duration::from_secs_f64(secs), true);
    }
    if let Some(ms) = server
        .get(codex_config::STARTUP_TIMEOUT_MS_KEY)
        .and_then(as_secs)
    {
        return (Duration::from_secs_f64(ms / 1000.0), true);
    }
    (
        Duration::from_secs(codex_config::DEFAULT_STARTUP_TIMEOUT_SECS),
        false,
    )
}

/// Read-only core tools a Codex tool filter hides, by name.
fn filtered_core_tools(server: &toml::Table) -> Vec<String> {
    let names = |key: &str| -> Option<Vec<String>> {
        server.get(key).and_then(toml::Value::as_array).map(|list| {
            list.iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
    };
    let enabled = names(codex_config::ENABLED_TOOLS_KEY);
    let disabled = names(codex_config::DISABLED_TOOLS_KEY).unwrap_or_default();
    crate::agents::read_only_tool_names()
        .into_iter()
        .filter(|name| {
            disabled.contains(name) || enabled.as_ref().is_some_and(|list| !list.contains(name))
        })
        .collect()
}

/// Check what decides whether Codex can start the server and see its tools
/// (#587): `enabled`, the command, `args`, the startup timeout and tool
/// filters, then a live handshake when that is safe to run.
fn doctor_check_launch(dc: &mut DoctorCounters, server: &toml::Table, project_path: &Path) {
    if server
        .get(codex_config::ENABLED_KEY)
        .and_then(toml::Value::as_bool)
        == Some(false)
    {
        dc.fail(
            "tokensave MCP server is disabled (enabled = false) in config.toml — Codex will \
             not start it; remove the line or set it to true",
        );
        return;
    }

    let args: Vec<String> = server
        .get("args")
        .and_then(toml::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if args.iter().any(|a| a == "serve") {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent codex`");
    }

    let Some(command) = server.get("command").and_then(toml::Value::as_str) else {
        dc.fail("MCP server entry has no command — run `tokensave install --agent codex`");
        return;
    };
    let Some(resolved) = resolve_command(command) else {
        dc.fail(&format!(
            "MCP command not found: {command} — Codex cannot start the server; \
             run `tokensave install --agent codex`"
        ));
        return;
    };
    dc.pass(&format!("MCP command resolves to {}", resolved.display()));

    let (timeout, explicit) = startup_timeout(server);
    if explicit {
        dc.pass(&format!(
            "Codex startup timeout: {:.0} s",
            timeout.as_secs_f64()
        ));
    } else {
        dc.warn(&format!(
            "{key} not set: Codex drops servers that take over {default} s to list their \
             tools, and a first index or schema migration can take longer — run \
             `tokensave install --agent codex` to set {key} = {installed}",
            key = codex_config::STARTUP_TIMEOUT_SEC_KEY,
            default = codex_config::DEFAULT_STARTUP_TIMEOUT_SECS,
            installed = codex_config::INSTALLED_STARTUP_TIMEOUT_SECS,
        ));
    }

    let hidden = filtered_core_tools(server);
    if !hidden.is_empty() {
        dc.warn(&format!(
            "{} / {} in config.toml hide {} tokensave tool(s) from Codex: {}",
            codex_config::ENABLED_TOOLS_KEY,
            codex_config::DISABLED_TOOLS_KEY,
            hidden.len(),
            hidden.join(", ")
        ));
    }

    if dc.read_only {
        dc.info("Live MCP startup probe skipped in read-only mode");
    } else {
        doctor_probe_handshake(dc, &resolved, &args, server, project_path, timeout);
    }
}

/// Launch the configured server once and time `initialize` + `tools/list`
/// against Codex's startup timeout (#587).
///
/// Runs only for a tokensave executable and from an indexed project: the
/// server it starts is the one Codex would start here, and it neither
/// indexes a new repository nor runs a command doctor does not recognize.
fn doctor_probe_handshake(
    dc: &mut DoctorCounters,
    command: &Path,
    args: &[String],
    server: &toml::Table,
    project_path: &Path,
    timeout: Duration,
) {
    let is_tokensave = command
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(codex_config::TOKENSAVE_COMMAND_STEM));
    if !is_tokensave {
        dc.info("Startup probe skipped: the MCP command is not a tokensave executable");
        return;
    }
    if !crate::tokensave::TokenSave::is_initialized(project_path) {
        dc.info(
            "Startup probe skipped: run `tokensave doctor --agent codex` from an indexed \
             project to time the server's startup",
        );
        return;
    }
    let envs: Vec<(String, String)> = server
        .get(codex_config::ENV_KEY)
        .and_then(toml::Value::as_table)
        .map(|env| {
            env.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let deadline = timeout + Duration::from_secs(codex_config::PROBE_GRACE_SECS);
    match crate::agents::mcp_probe::probe_stdio_server(command, args, project_path, &envs, deadline)
    {
        Ok(timings) if timings.tool_count == 0 => dc
            .fail("Server started but listed no tools — check `tokensave serve` output for errors"),
        Ok(timings) if timings.total > timeout => dc.fail(&format!(
            "Server listed {} tools in {:.1} s, over Codex's {:.0} s startup timeout — Codex \
             starts without tokensave tools; raise {} in config.toml",
            timings.tool_count,
            timings.total.as_secs_f64(),
            timeout.as_secs_f64(),
            codex_config::STARTUP_TIMEOUT_SEC_KEY
        )),
        Ok(timings) => dc.pass(&format!(
            "Server answered initialize in {:.2} s and listed {} tools in {:.2} s \
             (startup timeout {:.0} s)",
            timings.initialize.as_secs_f64(),
            timings.tool_count,
            timings.total.as_secs_f64(),
            timeout.as_secs_f64()
        )),
        Err(e) => dc.fail(&format!(
            "Startup probe failed: {e}. Codex would start without tokensave tools; \
             run `{} serve` in this project to see its error output",
            command.display()
        )),
    }
}

/// Check AGENTS.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, codex_dir: &Path) {
    let agents_md = codex_dir.join("AGENTS.md");
    check_shared_rules_block(dc, &agents_md, "codex");
}

fn is_shell_hook(handler: &Value) -> bool {
    handler["type"] == "command"
        && handler["command"]
            .as_str()
            .and_then(|command| command.rsplit_once(' '))
            .is_some_and(|(binary, subcommand)| {
                subcommand == codex_config::SHELL_HOOK
                    && Path::new(binary.trim_start_matches("& ").trim_matches(['\'', '"']))
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| {
                            stem.eq_ignore_ascii_case(codex_config::TOKENSAVE_COMMAND_STEM)
                        })
            })
}

fn remove_shell_hook(config: &mut Value) -> bool {
    let Some(groups) = config
        .get_mut(codex_config::HOOKS_KEY)
        .and_then(|hooks| hooks.get_mut(codex_config::PRE_TOOL_USE))
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let mut changed = false;
    groups.retain_mut(|group| {
        let Some(handlers) = group
            .get_mut(codex_config::HOOKS_KEY)
            .and_then(Value::as_array_mut)
        else {
            return true;
        };
        let before = handlers.len();
        handlers.retain(|handler| !is_shell_hook(handler));
        let removed = handlers.len() != before;
        changed |= removed;
        !removed || !handlers.is_empty()
    });
    changed
}

fn install_shell_hook(path: &Path, binary: &str) -> Result<()> {
    let mut config = load_json_file_strict(path)?;
    remove_shell_hook(&mut config);
    let invalid = || TokenSaveError::Config {
        message: format!("Invalid Codex hook configuration in {}", path.display()),
    };
    let hooks = config
        .as_object_mut()
        .ok_or_else(invalid)?
        .entry(codex_config::HOOKS_KEY)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(invalid)?;
    let groups = hooks
        .entry(codex_config::PRE_TOOL_USE)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(invalid)?;
    let quoted = if cfg!(windows) {
        format!("& '{}'", binary.replace('\'', "''"))
    } else {
        format!("'{}'", binary.replace('\'', "'\\''"))
    };
    let command = format!("{quoted} {}", codex_config::SHELL_HOOK);
    groups.push(json!({
        "matcher": codex_config::SHELL_MATCHER,
        "hooks": [{"type": "command", "command": command}]
    }));
    write_json_file(path, &config)
}

fn doctor_check_shell_hook(dc: &mut DoctorCounters, path: &Path) {
    match load_json_file_strict(path) {
        Ok(config) => {
            let configured = config[codex_config::HOOKS_KEY][codex_config::PRE_TOOL_USE]
                .as_array()
                .into_iter()
                .flatten()
                .any(|group| {
                    group["matcher"] == codex_config::SHELL_MATCHER
                        && group[codex_config::HOOKS_KEY]
                            .as_array()
                            .is_some_and(|handlers| handlers.iter().any(is_shell_hook))
                });
            if configured {
                dc.pass("Codex PreToolUse shell hook configured");
                dc.info("Use /hooks in Codex to confirm it is enabled and trusted");
            } else {
                dc.warn("Codex PreToolUse shell hook missing; run tokensave install --agent codex");
            }
        }
        Err(error) => dc.fail(&format!("Codex hook configuration: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_hook_install_is_idempotent_and_removal_preserves_other_handlers() -> Result<()> {
        let dir = tempfile::tempdir().map_err(|error| TokenSaveError::Config {
            message: error.to_string(),
        })?;
        let path = dir.path().join(codex_config::HOOKS_FILE);
        let original = json!({
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type":"command","command":"other"}]}],
                "Stop": [{"hooks": [{"type":"command","command":"other-stop"}]}]
            }
        });
        write_json_file(&path, &original)?;
        install_shell_hook(&path, "path with spaces/tokensave")?;
        let first = load_json_file_strict(&path)?;
        install_shell_hook(&path, "path with spaces/tokensave")?;
        let mut second = load_json_file_strict(&path)?;
        assert_eq!(first, second);
        assert!(remove_shell_hook(&mut second));
        assert_eq!(second, original);
        Ok(())
    }
}
