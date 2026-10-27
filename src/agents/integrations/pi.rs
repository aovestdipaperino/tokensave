// Rust guideline compliant 2026-10-15
//! Pi coding agent integration.
//!
//! Handles registration of the tokensave MCP server and the tokensave rules
//! block inside Pi's agent directory. Per the Pi configuration docs, the
//! agent directory is `$PI_CODING_AGENT_DIR` when set, or `<home>/.pi/agent`
//! otherwise; the MCP config lives at `<agent-dir>/mcp.json` and the user
//! instructions at `<agent-dir>/AGENTS.md` (alongside `AGENTS.override.md`,
//! `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`). Pi uses the standard MCP JSON
//! shape, so the server entry matches the other JSON-based integrations
//! (Cursor, Cline): `{ "command", "args": ["serve"] }`, plus the `exposure`
//! field that makes the tools reachable at all — Pi's default for an MCP
//! server is `codemode`, which declares none of them to the model.
//!
//! Pi has both a user-level and a project-level configuration: the agent
//! directory holds `mcp.json` and `AGENTS.md`, while a project owns `.pi/mcp.json`
//! and the `AGENTS.md` in its root (Pi loads context files from the working
//! directory and its parents). Both scopes are supported, so `--local` targets
//! the project paths.
//!
//! Pi has no shell hooks, so the three things Claude Code gets from
//! `hook-pre-tool-use`, `hook-prompt-submit`, and `hook-stop` come from a
//! managed TypeScript extension (`pi_extension.ts`) written into
//! `<agent-dir>/extensions` or `<project>/.pi/extensions`. It only relays Pi's
//! tool calls to those subcommands, which keeps the classification in Rust.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::errors::Result;

use super::*;

/// Pi coding agent.
pub struct PiIntegration;

/// Resolves the Pi agent directory.
///
/// Honors `$PI_CODING_AGENT_DIR` when set, otherwise falls back to
/// `<home>/.pi/agent`. This is the directory every per-user Pi file lives
/// in (mcp.json, AGENTS.md, settings.json, ...), so both the MCP config and
/// the rules block derive from it.
fn pi_agent_dir(home: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    home.join(".pi").join("agent")
}

/// Returns the Pi MCP config path (`<agent-dir>/mcp.json`).
fn pi_config_path(home: &Path) -> PathBuf {
    pi_agent_dir(home).join("mcp.json")
}

/// Returns the path to Pi's global instructions file (`<agent-dir>/AGENTS.md`).
fn pi_rules_path(home: &Path) -> PathBuf {
    pi_agent_dir(home).join("AGENTS.md")
}

/// Returns the project-scoped Pi MCP config path (`<project>/.pi/mcp.json`).
///
/// Pi loads project MCP servers from the `.pi` directory of the working
/// directory, once the project is trusted.
fn pi_local_config_path(project_path: &Path) -> PathBuf {
    project_path.join(".pi").join("mcp.json")
}

/// Returns the project-scoped instructions file (`<project>/AGENTS.md`).
///
/// Pi loads context files from the working directory and its parents, so the
/// `AGENTS.md` in the project root is the file Pi reads for a project.
fn pi_local_rules_path(project_path: &Path) -> PathBuf {
    project_path.join("AGENTS.md")
}

/// Returns the Pi MCP config path the install scope targets.
fn pi_mcp_path(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => pi_config_path(&ctx.home),
        InstallScope::Local { project_path } => pi_local_config_path(project_path),
    }
}

/// Returns the instructions file the install scope targets.
fn pi_instructions_path(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => pi_rules_path(&ctx.home),
        InstallScope::Local { project_path } => pi_local_rules_path(project_path),
    }
}

/// Exposure Pi must apply to the tokensave server.
///
/// Pi's built-in MCP and `pi-mcp-adapter` both default a server to `codemode`,
/// which declares no tool for the model: `tools/list` is only reachable from a
/// `codemode` script or `tool_search`. Tokensave ships a curated core toolset
/// (11 tools plus `tokensave_more`), which is the size `direct` exists for, and
/// the rules the install writes name those tools directly.
const PI_SERVER_EXPOSURE: &str = "direct";

/// One-line server summary Pi shows in `/mcp` and in its system prompt's
/// `mcp_servers` section.
const PI_SERVER_DESCRIPTION: &str =
    "Code-graph queries and edits for the indexed project: search, callers, impact, read.";

/// First line of the managed extension file, used to refuse overwriting a file
/// tokensave did not write.
const PI_EXTENSION_MARKER: &str = "// tokensave-managed extension";

/// The extension source, with `__TOKENSAVE_BIN__` standing in for the binary.
const PI_EXTENSION_SOURCE: &str = include_str!("pi_extension.ts");

/// Renders the managed extension for a given binary path.
fn pi_extension_source(tokensave_bin: &str) -> String {
    let bin = serde_json::Value::String(tokensave_bin.to_string()).to_string();
    PI_EXTENSION_SOURCE.replace("\"__TOKENSAVE_BIN__\"", &bin)
}

/// Returns the extensions directory the install scope targets.
///
/// Pi loads `*.ts` files from `<agent-dir>/extensions` for the user and from
/// `<project>/.pi/extensions` for a project.
fn pi_extensions_dir(ctx: &InstallContext) -> PathBuf {
    match &ctx.scope {
        InstallScope::Global => pi_agent_dir(&ctx.home).join("extensions"),
        InstallScope::Local { project_path } => project_path.join(".pi").join("extensions"),
    }
}

/// Returns the managed extension file path.
fn pi_extension_path(ctx: &InstallContext) -> PathBuf {
    pi_extensions_dir(ctx).join("tokensave.ts")
}

impl AgentIntegration for PiIntegration {
    fn name(&self) -> &'static str {
        "Pi"
    }

    fn id(&self) -> &'static str {
        "pi"
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = pi_mcp_path(ctx);

        if let Some(parent) = mcp_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let backup = backup_config_file(&mcp_path)?;
        let mut settings = match load_json_file_strict(&mcp_path) {
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
            &ctx.tokensave_bin,
        );
        settings["mcpServers"]["tokensave"] = json!({
            "command": bin,
            "args": ["serve"],
            "exposure": PI_SERVER_EXPOSURE,
            "description": PI_SERVER_DESCRIPTION,
        });

        safe_write_json_file(&mcp_path, &settings, backup.as_deref())?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
            mcp_path.display()
        );

        install_prompt_rules(ctx)?;
        install_guardrail_extension(ctx)?;
        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Restart Pi — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let mcp_path = pi_mcp_path(ctx);
        uninstall_mcp_server(&mcp_path);
        uninstall_prompt_rules(ctx);
        uninstall_guardrail_extension(ctx);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Pi.");
        crate::agent_note!("Restart Pi for changes to take effect.");
        Ok(())
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mPi integration — static configuration\x1b[0m");
        doctor_check_settings(dc, &ctx.home);
        doctor_check_prompt(dc, &ctx.home);
        doctor_check_local_config(dc, &ctx.project_path);
        doctor_check_shadowing(dc, &pi_agent_dir(&ctx.home));
        for ancestor in ctx.project_path.ancestors() {
            doctor_check_shadowing(dc, ancestor);
        }
        doctor_check_project_index(dc, &ctx.project_path);
        dc.warn(
            "Pi runtime UNVERIFIED: extension loading, declared tools, MCP readiness, adapter \
             discovery and effective tokensave_retrieval prompt section cannot be inspected by \
             this CLI. Restart/reload Pi, inspect its active tools and effective prompt, and \
             test a TokenSave query. Use the names/interfaces actually declared by your host.",
        );
        crate::agent_note!("\n\x1b[1mGuardrail extension\x1b[0m");
        doctor_check_extension(
            dc,
            &pi_config_path(&ctx.home),
            &pi_agent_dir(&ctx.home)
                .join("extensions")
                .join("tokensave.ts"),
            "tokensave install --agent pi",
        );
    }

    fn is_detected(&self, home: &Path) -> bool {
        let mcp_path = pi_config_path(home);
        mcp_path.exists() || mcp_path.parent().is_some_and(Path::is_dir)
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(pi_config_path(home))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let mcp_path = pi_config_path(home);
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
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove the tokensave MCP server entry from Pi's `mcp.json`.
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

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Check Pi's `mcp.json` has the tokensave MCP server registered.
fn doctor_check_settings(dc: &mut DoctorCounters, home: &Path) {
    let mcp_path = pi_config_path(home);

    if !mcp_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent pi` if you use Pi",
            mcp_path.display()
        ));
        return;
    }

    let settings = load_json_file(&mcp_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    match server.and_then(|v| v.as_object()) {
        Some(server) => {
            dc.pass(&format!("MCP server registered in {}", mcp_path.display()));
            if exposure_declares_tools(server) {
                dc.pass(&format!(
                    "Direct tool exposure configured (exposure: {PI_SERVER_EXPOSURE}); runtime unverified"
                ));
            } else {
                dc.fail(
                    "Direct MCP tool exposure is missing or unsupported — \
                     run `tokensave install --agent pi`, then restart/reload Pi",
                );
            }
        }
        None => dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent pi`",
            mcp_path.display()
        )),
    }
}

/// Reports whether a server requests the managed direct exposure mode.
fn exposure_declares_tools(server: &serde_json::Map<String, serde_json::Value>) -> bool {
    server.get("exposure").and_then(|v| v.as_str()) == Some(PI_SERVER_EXPOSURE)
}

/// Write the guardrail extension, or refresh it when it is ours.
///
/// A file at that path that tokensave did not write is left alone: it is the
/// user's extension, and replacing it would silently delete their behaviour.
fn install_guardrail_extension(ctx: &InstallContext) -> Result<()> {
    let path = pi_extension_path(ctx);
    let body = pi_extension_source(&ctx.tokensave_bin);

    if let Ok(existing) = std::fs::read_to_string(&path) {
        if existing == body {
            return Ok(());
        }
        if !existing.starts_with(PI_EXTENSION_MARKER) {
            crate::agent_note!(
                "  {} exists and was not written by tokensave, leaving it unchanged",
                path.display()
            );
            return Ok(());
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, &body)?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added search guardrail extension to {}",
        path.display()
    );
    Ok(())
}

/// Remove the guardrail extension, and its directory when tokensave created
/// the only file in it.
fn uninstall_guardrail_extension(ctx: &InstallContext) {
    let path = pi_extension_path(ctx);
    let is_ours = std::fs::read_to_string(&path).is_ok_and(|c| c.starts_with(PI_EXTENSION_MARKER));
    if is_ours && std::fs::remove_file(&path).is_ok() {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed guardrail extension {}",
            path.display()
        );
        let dir = pi_extensions_dir(ctx);
        let is_empty = std::fs::read_dir(&dir).is_ok_and(|mut e| e.next().is_none());
        if is_empty {
            std::fs::remove_dir(&dir).ok();
        }
    }
}

/// Check the guardrail extension that belongs to `mcp_path` is installed and
/// current, comparing it against the binary that server entry runs.
///
/// `install_command` is the command that repairs it, which differs between the
/// global and the project surface.
fn doctor_check_extension(
    dc: &mut DoctorCounters,
    mcp_path: &Path,
    extension_path: &Path,
    install_command: &str,
) {
    let server = load_json_file(mcp_path);
    let tokensave_bin = server
        .pointer("/mcpServers/tokensave/command")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let Ok(contents) = std::fs::read_to_string(extension_path) else {
        dc.warn(&format!(
            "{} not found — install the search guardrail with `{install_command}`",
            extension_path.display()
        ));
        return;
    };
    if tokensave_bin.is_empty() {
        dc.warn("cannot verify the guardrail extension without a registered binary");
    } else if contents == pi_extension_source(tokensave_bin) {
        dc.pass(&format!(
            "Search guardrail installed ({})",
            extension_path.display()
        ));
    } else {
        dc.fail(&format!(
            "Search guardrail at {} is stale — reinstall it with `{install_command}`",
            extension_path.display()
        ));
    }
}

/// Write or refresh the tokensave rules block in the AGENTS.md the install
/// scope targets.
fn install_prompt_rules(ctx: &InstallContext) -> Result<()> {
    let body = rules_for_agent("pi")?;
    write_rules_block(&pi_instructions_path(ctx), "pi", &body).map(|_| ())
}

/// Remove the tokensave rules block from the AGENTS.md the install scope
/// targets.
fn uninstall_prompt_rules(ctx: &InstallContext) {
    let rules_path = pi_instructions_path(ctx);
    remove_rules_block(&rules_path).ok();
    remove_legacy_rules_block(&rules_path, LEGACY_RULES_MARKER, &[]).ok();
}

/// Check Pi's AGENTS.md contains the up-to-date tokensave rules block.
fn doctor_check_prompt(dc: &mut DoctorCounters, home: &Path) {
    check_shared_rules_block(dc, &pi_rules_path(home), "pi");
}

/// Check the project-scoped Pi configuration of the current project.
///
/// Only reported when the project actually owns a `.pi` directory: a project
/// without one has no project-scoped Pi configuration to complain about.
fn doctor_check_local_config(dc: &mut DoctorCounters, project_path: &Path) {
    if !project_path.join(".pi").is_dir() {
        dc.pass("Local Pi configuration absent; only global configuration applies");
        return;
    }

    crate::agent_note!("\n\x1b[1mLocal config\x1b[0m");

    let mcp_path = pi_local_config_path(project_path);
    if mcp_path.exists() {
        let settings = load_json_file(&mcp_path);
        let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));
        match server.and_then(|v| v.as_object()) {
            Some(server) => {
                dc.pass(&format!("MCP server registered in {}", mcp_path.display()));
                if exposure_declares_tools(server) {
                    dc.pass("Local direct tool exposure configured; runtime unverified");
                } else {
                    dc.fail(&format!(
                        "Direct MCP tool exposure is missing or unsupported in {} — run \
                         `tokensave install --agent pi --local`",
                        mcp_path.display()
                    ));
                }
            }
            None => dc.fail(&format!(
                "MCP server NOT registered in {} — run `tokensave install --agent pi --local`",
                mcp_path.display()
            )),
        }
    } else {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent pi --local` to register it for this project",
            mcp_path.display()
        ));
    }

    check_shared_rules_block(dc, &pi_local_rules_path(project_path), "pi");
    doctor_check_extension(
        dc,
        &mcp_path,
        &project_path
            .join(".pi")
            .join("extensions")
            .join("tokensave.ts"),
        "tokensave install --agent pi --local",
    );
}

/// Pi chooses override context before AGENTS.md in each directory.
/// Report a possibility, since runtime context discovery may be customized.
fn doctor_check_shadowing(dc: &mut DoctorCounters, directory: &Path) {
    let override_path = directory.join("AGENTS.override.md");
    if override_path.is_file() {
        dc.warn(&format!(
            "{} may shadow AGENTS.md, including TokenSave rules — review the override's \
             guidance and Pi's effective prompt; reinstall does not change user overrides",
            override_path.display()
        ));
    }
}

/// Metadata-only discovery: never open/migrate a database or trigger indexing.
fn doctor_check_project_index(dc: &mut DoctorCounters, project_path: &Path) {
    match crate::config::discover_project(project_path) {
        crate::config::ProjectDiscovery::Initialized(root) => dc.pass(&format!(
            "Project index file found at {} — schema, coverage, freshness and runtime \
             usability UNVERIFIED; use `tokensave sync` if refresh is needed",
            root.join(".tokensave").join("tokensave.db").display()
        )),
        crate::config::ProjectDiscovery::Incomplete(root) => dc.warn(&format!(
            "Project configuration at {} has no index — run `tokensave init` in that project",
            root.display()
        )),
        crate::config::ProjectDiscovery::Uninitialized { .. } => dc.warn(
            "No project index found — run `tokensave init` in the intended project; \
             native tools remain necessary until indexed retrieval is available",
        ),
    }
}
