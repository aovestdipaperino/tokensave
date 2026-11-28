// Rust guideline compliant 2025-10-17
//! GitHub Copilot integration.
//!
//! Handles registration of the tokensave MCP server in:
//! - VS Code's dedicated `User/mcp.json` (1.102+, GA MCP, parsed as JSONC)
//!   under `servers.tokensave`. `install` never writes `settings.json`; a
//!   pre-#266 `mcp.servers.tokensave` entry there is read as a fallback by
//!   `doctor`/`has_tokensave` so those checks don't nag, and it's cleaned up
//!   on `uninstall`. Note that entry isn't inert: VS Code itself actively
//!   migrates it into its own MCP registry (making it the live
//!   registration) and strips the key back out of `settings.json` once
//!   done, so the fallback mostly only matters in the transient window
//!   before that migration runs (see issue #266).
//! - Copilot CLI's `~/.copilot/mcp-config.json` under `mcpServers.tokensave`
//! - `JetBrains` plugin's `~/.config/github-copilot/intellij/mcp.json` under
//!   `servers.tokensave` (only when the plugin's config dir exists)

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agents::uninstall::{self, UninstallOutput, UninstallPlan};
use crate::errors::{Result, TokenSaveError};

use super::*;

/// GitHub Copilot agent.
pub struct CopilotIntegration;

mod project_config {
    pub(super) const GITHUB_DIR: &str = ".github";
    pub(super) const MCP_FILE: &str = "mcp.json";
    pub(super) const OVERRIDE_FILE: &str = ".mcp.json";
    pub(super) const SERVERS_KEY: &str = "mcpServers";
    pub(super) const SERVER_NAME: &str = "tokensave";
}

impl AgentIntegration for CopilotIntegration {
    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }

    fn id(&self) -> &'static str {
        "copilot"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        if let InstallScope::Local { project_path } = &ctx.scope {
            return install_project_mcp(&project_mcp_path(project_path), &ctx.tokensave_bin);
        }
        let vscode_mcp_json_path = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("mcp.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");

        install_vscode_mcp_json_server(&vscode_mcp_json_path, &ctx.tokensave_bin)?;

        let insiders_mcp_json_path = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("mcp.json");
        if insiders_mcp_json_path
            .parent()
            .is_some_and(std::path::Path::exists)
        {
            install_vscode_mcp_json_server(&insiders_mcp_json_path, &ctx.tokensave_bin)?;
        }
        install_cli_mcp_server(&cli_settings_path, &ctx.tokensave_bin)?;

        // JetBrains plugin config lives under ~/.config/github-copilot/intellij.
        // Only install when ~/.config/github-copilot exists (the Copilot plugin
        // creates it on sign-in); otherwise we'd litter homes of non-JetBrains
        // users with an unused config tree.
        let jetbrains_dir = super::copilot_jetbrains_dir(&ctx.home);
        let jetbrains_detected = jetbrains_dir.parent().is_some_and(std::path::Path::exists);
        if jetbrains_detected {
            install_jetbrains_mcp_server(&jetbrains_dir.join("mcp.json"), &ctx.tokensave_bin)?;
        }

        // Install prompt rules
        let vscode_instructions = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md");
        install_prompt_rules(&vscode_instructions)?;
        let insiders_instructions = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md");
        if super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .exists()
        {
            install_prompt_rules(&insiders_instructions)?;
        }
        let cli_instructions = super::copilot_cli_dir(&ctx.home).join("copilot-instructions.md");
        install_prompt_rules(&cli_instructions)?;
        if jetbrains_detected {
            // JetBrains reads global instructions from this file, not from the
            // VS Code User/prompts location.
            install_prompt_rules(&jetbrains_dir.join("global-copilot-instructions.md"))?;
        }

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Restart VS Code and/or start a new Copilot CLI session");
        if jetbrains_detected {
            crate::agent_note!(
                "     For JetBrains IDEs, restart the IDE (MCP config is read at startup)"
            );
        }
        crate::agent_note!("     tokensave tools are now available in GitHub Copilot");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        if ctx.is_local() {
            let mut plan = UninstallPlan::for_scope(&ctx.scope);
            self.plan_uninstall(ctx, &mut plan)?;
            if plan.apply(UninstallOutput::default()).failed > 0 {
                return Err(TokenSaveError::Config {
                    message: "Could not remove project-local Copilot MCP registration".into(),
                });
            }
            return Ok(());
        }
        let vscode_settings_path = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("settings.json");
        let vscode_mcp_json_path = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("mcp.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");
        uninstall_vscode_mcp_json_server(&vscode_mcp_json_path);
        uninstall_vscode_mcp_server(&vscode_settings_path);
        let insiders_settings_path = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("settings.json");
        let insiders_mcp_json_path = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("mcp.json");
        uninstall_vscode_mcp_json_server(&insiders_mcp_json_path);
        uninstall_vscode_mcp_server(&insiders_settings_path);
        uninstall_cli_mcp_server(&cli_settings_path);
        let jetbrains_dir = super::copilot_jetbrains_dir(&ctx.home);
        uninstall_jetbrains_mcp_server(&jetbrains_dir.join("mcp.json"));

        let vscode_instructions = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md");
        uninstall_prompt_rules(&vscode_instructions);
        let insiders_instructions = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md");
        uninstall_prompt_rules(&insiders_instructions);
        let cli_instructions = super::copilot_cli_dir(&ctx.home).join("copilot-instructions.md");
        uninstall_prompt_rules(&cli_instructions);
        uninstall_prompt_rules(&jetbrains_dir.join("global-copilot-instructions.md"));

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from GitHub Copilot.");
        crate::agent_note!(
            "Restart VS Code, JetBrains IDEs, and/or start a new Copilot CLI session for changes to take effect."
        );
        Ok(())
    }

    fn plan_uninstall(
        &self,
        ctx: &InstallContext,
        plan: &mut crate::agents::uninstall::UninstallPlan,
    ) -> Result<()> {
        if let InstallScope::Local { project_path } = &ctx.scope {
            plan.json(
                "MCP server",
                &project_mcp_path(project_path),
                false,
                |config| {
                    let keys = if config.get(project_config::SERVERS_KEY).is_some() {
                        &[project_config::SERVERS_KEY][..]
                    } else {
                        &[]
                    };
                    uninstall::remove_mcp(config, keys)
                },
            );
            return Ok(());
        }
        let vscode_settings_path = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("settings.json");
        let vscode_mcp_json_path = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("mcp.json");
        let cli_settings_path = super::copilot_cli_dir(&ctx.home).join("mcp-config.json");
        plan.mcp(&vscode_mcp_json_path, &["servers"], true);
        plan.mcp(&vscode_settings_path, &["mcp", "servers"], true);
        let insiders_settings_path = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("settings.json");
        let insiders_mcp_json_path = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("mcp.json");
        plan.mcp(&insiders_mcp_json_path, &["servers"], true);
        plan.mcp(&insiders_settings_path, &["mcp", "servers"], true);
        plan.mcp(&cli_settings_path, &["mcpServers"], false);
        let jetbrains_dir = super::copilot_jetbrains_dir(&ctx.home);
        plan.mcp(&jetbrains_dir.join("mcp.json"), &["servers"], false);

        let vscode_instructions = super::vscode_data_dir(&ctx.home)
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md");
        plan.rules(
            &vscode_instructions,
            crate::agents::rules::LEGACY_RULES_MARKER,
            &[],
        );
        let insiders_instructions = super::vscode_insiders_data_dir(&ctx.home)
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md");
        plan.rules(
            &insiders_instructions,
            crate::agents::rules::LEGACY_RULES_MARKER,
            &[],
        );
        let cli_instructions = super::copilot_cli_dir(&ctx.home).join("copilot-instructions.md");
        plan.rules(
            &cli_instructions,
            crate::agents::rules::LEGACY_RULES_MARKER,
            &[],
        );
        plan.rules(
            &jetbrains_dir.join("global-copilot-instructions.md"),
            crate::agents::rules::LEGACY_RULES_MARKER,
            &[],
        );
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mGitHub Copilot integration\x1b[0m");
        let project_configured = self.healthcheck_project(dc, ctx);
        let mut configured = 0usize;
        for surface in copilot_surfaces(&ctx.home) {
            match surface.state() {
                SurfaceState::NotDetected => {
                    dc.info(&format!("{}: not detected, skipped", surface.label));
                }
                SurfaceState::NotConfigured => dc.info(&format!(
                    "{}: detected, tokensave not configured there (not checked) — \
                     `tokensave install --agent copilot` adds it",
                    surface.label
                )),
                state @ (SurfaceState::Configured | SurfaceState::Partial { .. }) => {
                    configured += 1;
                    surface.check_registration(dc);
                    check_shared_rules_block(dc, &surface.instructions, "copilot");
                    if let SurfaceState::Partial { registered } = state {
                        report_partial_surface(dc, &surface, registered);
                    }
                    if surface.kind == SurfaceKind::VsCode {
                        report_vscode_profiles(dc, &surface.root);
                    }
                }
            }
        }
        if configured == 0 && !project_configured {
            dc.fail(
                "tokensave is not configured for any GitHub Copilot surface (VS Code, \
                 VS Code Insiders, Copilot CLI, JetBrains) — run `tokensave install --agent copilot`",
            );
        }
    }

    fn healthcheck_project(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) -> bool {
        let path = project_mcp_path(&ctx.project_path);
        let config = match load_json_file_strict(&path) {
            Ok(config) => config,
            Err(error) => {
                dc.fail(&error.to_string());
                return false;
            }
        };
        if !project_server(&config).is_some_and(Value::is_object) {
            return false;
        }
        dc.pass(&format!(
            "Copilot project MCP server registered in {} (requires Copilot folder trust)",
            path.display()
        ));
        let override_path = ctx.project_path.join(project_config::OVERRIDE_FILE);
        if load_json_file_strict(&override_path)
            .is_ok_and(|config| project_server(&config).is_some())
        {
            dc.warn(&format!(
                "Copilot prefers the TokenSave entry in {}",
                override_path.display()
            ));
        }
        true
    }

    fn is_detected(&self, home: &Path) -> bool {
        super::vscode_data_dir(home).join("User").is_dir()
            || super::vscode_insiders_data_dir(home).join("User").is_dir()
            || super::copilot_cli_dir(home).is_dir()
            || super::copilot_jetbrains_dir(home)
                .parent()
                .is_some_and(Path::is_dir)
    }

    fn primary_config_path(&self, home: &Path) -> Option<std::path::PathBuf> {
        Some(super::vscode_data_dir(home).join("User").join("mcp.json"))
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let vscode_mcp_json_path = super::vscode_data_dir(home).join("User").join("mcp.json");
        let vscode_settings_path = super::vscode_data_dir(home)
            .join("User")
            .join("settings.json");
        let insiders_mcp_json_path = super::vscode_insiders_data_dir(home)
            .join("User")
            .join("mcp.json");
        let insiders_settings_path = super::vscode_insiders_data_dir(home)
            .join("User")
            .join("settings.json");
        let cli_settings_path = super::copilot_cli_dir(home).join("mcp-config.json");

        let vscode_mcp_json_has_tokensave = if vscode_mcp_json_path.exists() {
            let json = load_jsonc_file(&vscode_mcp_json_path);
            json.get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        // Legacy fallback: settings.json may still hold a pre-#266 registration.
        let vscode_has_tokensave = if vscode_settings_path.exists() {
            let json = load_jsonc_file(&vscode_settings_path);
            json.get("mcp")
                .and_then(|v| v.get("servers"))
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let insiders_mcp_json_has_tokensave = if insiders_mcp_json_path.exists() {
            let json = load_jsonc_file(&insiders_mcp_json_path);
            json.get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let insiders_has_tokensave = if insiders_settings_path.exists() {
            let json = load_jsonc_file(&insiders_settings_path);
            json.get("mcp")
                .and_then(|v| v.get("servers"))
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let cli_has_tokensave = if cli_settings_path.exists() {
            let json = load_json_file(&cli_settings_path);
            json.get("mcpServers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        let jetbrains_settings_path = super::copilot_jetbrains_dir(home).join("mcp.json");
        let jetbrains_has_tokensave = if jetbrains_settings_path.exists() {
            let json = load_json_file(&jetbrains_settings_path);
            json.get("servers")
                .and_then(|v| v.get("tokensave"))
                .is_some()
        } else {
            false
        };

        vscode_mcp_json_has_tokensave
            || vscode_has_tokensave
            || insiders_mcp_json_has_tokensave
            || insiders_has_tokensave
            || cli_has_tokensave
            || jetbrains_has_tokensave
    }
}

fn project_mcp_path(root: &Path) -> PathBuf {
    root.join(project_config::GITHUB_DIR)
        .join(project_config::MCP_FILE)
}

fn project_server(config: &Value) -> Option<&Value> {
    config
        .get(project_config::SERVERS_KEY)
        .unwrap_or(config)
        .get(project_config::SERVER_NAME)
}

fn install_project_mcp(path: &Path, tokensave_bin: &str) -> Result<()> {
    let mut config = load_json_file_strict(path)?;
    if config.as_object().is_some_and(serde_json::Map::is_empty) {
        config[project_config::SERVERS_KEY] = json!({});
    }
    let servers = if config.get(project_config::SERVERS_KEY).is_some() {
        &mut config[project_config::SERVERS_KEY]
    } else {
        &mut config
    };
    let malformed = || TokenSaveError::Config {
        message: format!("Expected an MCP configuration object in {}", path.display()),
    };
    let servers = servers.as_object_mut().ok_or_else(malformed)?;
    let mut entry = servers
        .get(project_config::SERVER_NAME)
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !entry.is_object() {
        return Err(malformed());
    }
    if let Some(command) = entry.get("command") {
        if !uninstall::is_tokensave_command(command.as_str()) {
            return Err(TokenSaveError::Config {
                message: format!(
                    "Existing TokenSave server name in {} uses a different command",
                    path.display()
                ),
            });
        }
    } else if !entry.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(malformed());
    }
    let binary = crate::agents::preserve_mcp_command(entry.get("command"), tokensave_bin);
    entry["type"] = json!("stdio");
    entry["command"] = json!(binary);
    entry["args"] = json!(["serve"]);
    if servers.get(project_config::SERVER_NAME) == Some(&entry) {
        return Ok(());
    }
    servers.insert(project_config::SERVER_NAME.into(), entry);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| TokenSaveError::Config {
            message: format!("Cannot create {}: {error}", parent.display()),
        })?;
    }
    let backup = backup_config_file(path)?;
    safe_write_json_file(path, &config, backup.as_deref())?;
    crate::agent_note!(
        "Registered Copilot project MCP in {}. Start Copilot in a trusted checkout.",
        path.display()
    );
    Ok(())
}

/// Register MCP server in VS Code's dedicated `User/mcp.json` (1.102+, GA
/// MCP). This is the authoritative user-level MCP config on modern VS Code —
/// the legacy `settings.json` `mcp.servers` key is migrated out of and no
/// longer read for MCP by current releases (issue #266).
///
/// Uses the same top-level `servers` shape as the `JetBrains` plugin's
/// `mcp.json`, but — unlike `JetBrains` — VS Code expects a `type` field.
fn install_vscode_mcp_json_server(mcp_json_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = mcp_json_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(mcp_json_path)?;
    let mut config = match load_jsonc_file_strict(mcp_json_path) {
        Ok(v) => v,
        Err(e) => {
            if let Some(ref b) = backup {
                crate::agent_note!("  Backup preserved at: {}", b.display());
            }
            return Err(e);
        }
    };
    let bin = crate::agents::preserve_mcp_command(
        config.pointer("/servers/tokensave/command"),
        tokensave_bin,
    );
    config["servers"]["tokensave"] = json!({
        "type": "stdio",
        "command": bin,
        "args": ["serve"]
    });

    safe_write_json_file(mcp_json_path, &config, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        mcp_json_path.display()
    );
    Ok(())
}

/// Register MCP server in Copilot CLI's ~/.copilot/mcp-config.json.
fn install_cli_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
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

/// Register MCP server in the `JetBrains` plugin's mcp.json.
///
/// The `JetBrains` plugin uses the VS Code `mcp.json` shape (top-level
/// `servers` key), not the CLI's `mcpServers` key. It rejects unknown
/// fields conservatively, so no `type` field is written.
fn install_jetbrains_mcp_server(settings_path: &Path, tokensave_bin: &str) -> Result<()> {
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
        settings.pointer("/servers/tokensave/command"),
        tokensave_bin,
    );
    settings["servers"]["tokensave"] = json!({
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

// ---------------------------------------------------------------------------
// Uninstall helpers
// ---------------------------------------------------------------------------

/// Remove MCP server entry from VS Code settings.json.
/// Does not delete the file even if the object becomes empty (other VS Code
/// settings may still exist).
fn uninstall_vscode_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        crate::agent_note!("  {} not found, skipping", settings_path.display());
        return;
    }

    let mut settings = load_jsonc_file(settings_path);

    // Remove mcpServers.tokensave
    let removed = settings
        .get_mut("mcp")
        .and_then(|mcp| mcp.get_mut("servers"))
        .and_then(|servers| servers.as_object_mut())
        .and_then(|map| map.remove("tokensave"))
        .is_some();

    if !removed {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }

    // Clean up empty "servers" object
    if let Some(mcp) = settings.get_mut("mcp") {
        let servers_empty = mcp
            .get("servers")
            .and_then(|v| v.as_object())
            .is_some_and(serde_json::Map::is_empty);
        if servers_empty {
            mcp.as_object_mut().map(|o| o.remove("servers"));
        }

        // Clean up empty "mcp" object
        let mcp_empty = settings
            .get("mcp")
            .and_then(|v| v.as_object())
            .is_some_and(serde_json::Map::is_empty);
        if mcp_empty {
            settings.as_object_mut().map(|o| o.remove("mcp"));
        }
    }

    // Always write back (never delete settings.json — it has other VS Code settings).
    // backup_and_write_json leaves a .bak so any mistake is recoverable (issue #63).
    if backup_and_write_json(settings_path, &settings) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            settings_path.display()
        );
    }
}

/// Remove MCP server entry from VS Code's `User/mcp.json`.
fn uninstall_vscode_mcp_json_server(mcp_json_path: &Path) {
    if !mcp_json_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(mcp_json_path) else {
        return;
    };
    let mut config = parse_jsonc(&contents);
    let Some(servers) = config.get_mut("servers").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_json_path.display()
        );
        return;
    }
    if servers.is_empty() {
        config.as_object_mut().map(|o| o.remove("servers"));
    }
    let is_empty = config.as_object().is_some_and(serde_json::Map::is_empty);
    if is_empty {
        std::fs::remove_file(mcp_json_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_json_path.display()
        );
    } else if backup_and_write_json(mcp_json_path, &config) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_json_path.display()
        );
    }
}

/// Remove MCP server entry from Copilot CLI's ~/.copilot/mcp-config.json.
fn uninstall_cli_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
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
        return;
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }
    if servers.is_empty() {
        settings.as_object_mut().map(|o| o.remove("mcpServers"));
    }
    let is_empty = settings.as_object().is_some_and(serde_json::Map::is_empty);
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

/// Remove MCP server entry from the `JetBrains` plugin's mcp.json.
fn uninstall_jetbrains_mcp_server(settings_path: &Path) {
    if !settings_path.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(settings_path) else {
        return;
    };
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = settings.get_mut("servers").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            settings_path.display()
        );
        return;
    }
    if servers.is_empty() {
        settings.as_object_mut().map(|o| o.remove("servers"));
    }
    let is_empty = settings.as_object().is_some_and(serde_json::Map::is_empty);
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

// ---------------------------------------------------------------------------
// Prompt rules helpers
// ---------------------------------------------------------------------------

/// Write or refresh the tokensave rules block in a copilot-instructions.md file.
fn install_prompt_rules(instructions_path: &Path) -> Result<()> {
    let body = rules_for_agent("copilot")?;
    write_rules_block(instructions_path, "copilot", &body).map(|_| ())
}

/// Remove tokensave rules from a copilot-instructions.md file (both
/// managed-block and legacy heading-guarded forms).
fn uninstall_prompt_rules(instructions_path: &Path) {
    remove_rules_block(instructions_path).ok();
    remove_legacy_rules_block(instructions_path, LEGACY_RULES_MARKER, &[]).ok();
}

// ---------------------------------------------------------------------------
// Healthcheck helpers
// ---------------------------------------------------------------------------

/// Report whether an MCP server entry's `args` include `"serve"`.
fn report_serve_arg(dc: &mut DoctorCounters, server: &serde_json::Map<String, serde_json::Value>) {
    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent copilot`");
    }
}

/// Check VS Code (or VS Code Insiders) has the tokensave MCP server
/// registered in the modern `User/mcp.json`, falling back to the legacy
/// `settings.json` `mcp.servers` entry (issue #266). Passes if either is
/// valid; warns if both are present since that's a stale duplicate.
fn doctor_check_vscode_settings(dc: &mut DoctorCounters, vscode_dir: &Path, label: &str) {
    let mcp_json_path = vscode_dir.join("User").join("mcp.json");
    let settings_path = vscode_dir.join("User").join("settings.json");

    if !mcp_json_path.exists() && !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use GitHub Copilot in {label}",
            mcp_json_path.display()
        ));
        return;
    }

    let mcp_json_server = if mcp_json_path.exists() {
        load_jsonc_file(&mcp_json_path)
            .get("servers")
            .and_then(|v| v.get("tokensave"))
            .and_then(serde_json::Value::as_object)
            .cloned()
    } else {
        None
    };

    let legacy_server = if settings_path.exists() {
        load_jsonc_file(&settings_path)
            .get("mcp")
            .and_then(|v| v.get("servers"))
            .and_then(|v| v.get("tokensave"))
            .and_then(serde_json::Value::as_object)
            .cloned()
    } else {
        None
    };

    match (&mcp_json_server, &legacy_server) {
        (Some(server), _) => {
            dc.pass(&format!(
                "MCP server registered in {}",
                mcp_json_path.display()
            ));
            report_serve_arg(dc, server);
            if legacy_server.is_some() {
                dc.warn(&format!(
                    "tokensave is also registered in legacy {} — modern VS Code migrates this into its own MCP registry and clears the key automatically, so it's likely just a stale leftover; you can remove mcp.servers.tokensave there to avoid confusion",
                    settings_path.display()
                ));
            }
        }
        (None, Some(server)) => {
            dc.pass(&format!(
                "MCP server registered in legacy {}",
                settings_path.display()
            ));
            report_serve_arg(dc, server);
        }
        (None, None) => {
            dc.fail(&format!(
                "MCP server NOT registered in {} — run `tokensave install --agent copilot`",
                mcp_json_path.display()
            ));
        }
    }
}

/// Check the `JetBrains` plugin's mcp.json has tokensave MCP server registered.
fn doctor_check_jetbrains_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = super::copilot_jetbrains_dir(home).join("mcp.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use GitHub Copilot in JetBrains IDEs",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("servers").and_then(|v| v.get("tokensave"));

    let Some(server) = server.and_then(|v| v.as_object()) else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent copilot`",
            settings_path.display()
        ));
        return;
    };
    dc.pass(&format!(
        "MCP server registered in {}",
        settings_path.display()
    ));

    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent copilot`");
    }
}

/// Check Copilot CLI mcp-config.json has tokensave MCP server registered.
fn doctor_check_cli_settings(dc: &mut DoctorCounters, home: &Path) {
    let settings_path = super::copilot_cli_dir(home).join("mcp-config.json");

    if !settings_path.exists() {
        dc.warn(&format!(
            "{} not found — run `tokensave install --agent copilot` if you use Copilot CLI",
            settings_path.display()
        ));
        return;
    }

    let settings = load_json_file(&settings_path);
    let server = settings.get("mcpServers").and_then(|v| v.get("tokensave"));

    let Some(server) = server.and_then(|v| v.as_object()) else {
        dc.fail(&format!(
            "MCP server NOT registered in {} — run `tokensave install --agent copilot`",
            settings_path.display()
        ));
        return;
    };
    dc.pass(&format!(
        "MCP server registered in {}",
        settings_path.display()
    ));

    let has_serve = server
        .get("args")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("serve")));
    if has_serve {
        dc.pass("MCP server args include \"serve\"");
    } else {
        dc.fail("MCP server args missing \"serve\" — run `tokensave install --agent copilot`");
    }
}

// ---------------------------------------------------------------------------
// Per-surface status (#1117)
// ---------------------------------------------------------------------------

/// Which Copilot client a surface belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceKind {
    VsCode,
    Cli,
    JetBrains,
}

/// One place GitHub Copilot reads tokensave from.
struct CopilotSurface {
    label: &'static str,
    kind: SurfaceKind,
    /// Directory whose presence means this client is on the machine (VS Code's
    /// `User` directory, `~/.copilot`, the `JetBrains` plugin's config root).
    root: PathBuf,
    /// VS Code data directory, for the registration check of a VS Code surface.
    data_dir: PathBuf,
    instructions: PathBuf,
    home_dir: PathBuf,
}

/// What `doctor` found for one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceState {
    /// The client is not on this machine.
    NotDetected,
    /// The client is here, but neither tokensave's MCP entry nor its
    /// instructions are: a CLI-only install, say. Not applicable, not counted.
    NotConfigured,
    /// MCP entry and instructions both present.
    Configured,
    /// Only one of the two is present: an interrupted install, or a
    /// registration removed afterwards. `registered` says which one is there.
    Partial { registered: bool },
}

impl CopilotSurface {
    fn mcp_registered(&self) -> bool {
        let home = self.home_dir.as_path();
        let server_in = |path: &Path, pointer: &str| {
            path.exists() && load_jsonc_file(path).pointer(pointer).is_some()
        };
        match self.kind {
            SurfaceKind::VsCode => {
                let user = self.data_dir.join("User");
                server_in(&user.join("mcp.json"), "/servers/tokensave")
                    || server_in(&user.join("settings.json"), "/mcp/servers/tokensave")
            }
            SurfaceKind::Cli => server_in(
                &super::copilot_cli_dir(home).join("mcp-config.json"),
                "/mcpServers/tokensave",
            ),
            SurfaceKind::JetBrains => server_in(
                &super::copilot_jetbrains_dir(home).join("mcp.json"),
                "/servers/tokensave",
            ),
        }
    }

    fn state(&self) -> SurfaceState {
        let registered = self.mcp_registered();
        let rules = has_rules_block(&self.instructions);
        match (registered, rules) {
            (true, true) => SurfaceState::Configured,
            (true, false) | (false, true) => SurfaceState::Partial { registered },
            (false, false) if self.root.is_dir() => SurfaceState::NotConfigured,
            (false, false) => SurfaceState::NotDetected,
        }
    }

    fn check_registration(&self, dc: &mut DoctorCounters) {
        let home = self.home_dir.as_path();
        match self.kind {
            SurfaceKind::VsCode => doctor_check_vscode_settings(dc, &self.data_dir, self.label),
            SurfaceKind::Cli => doctor_check_cli_settings(dc, home),
            SurfaceKind::JetBrains => doctor_check_jetbrains_settings(dc, home),
        }
    }
}

/// Every Copilot surface `install` can write, in report order.
fn copilot_surfaces(home: &Path) -> Vec<CopilotSurface> {
    let vscode = |label, data_dir: PathBuf| CopilotSurface {
        label,
        kind: SurfaceKind::VsCode,
        root: data_dir.join("User"),
        instructions: data_dir
            .join("User")
            .join("prompts")
            .join("copilot-instructions.md"),
        data_dir,
        home_dir: home.to_path_buf(),
    };
    let jetbrains_dir = super::copilot_jetbrains_dir(home);
    vec![
        vscode("VS Code", super::vscode_data_dir(home)),
        vscode("VS Code Insiders", super::vscode_insiders_data_dir(home)),
        CopilotSurface {
            label: "Copilot CLI",
            kind: SurfaceKind::Cli,
            root: super::copilot_cli_dir(home),
            data_dir: PathBuf::new(),
            instructions: super::copilot_cli_dir(home).join("copilot-instructions.md"),
            home_dir: home.to_path_buf(),
        },
        CopilotSurface {
            label: "JetBrains",
            kind: SurfaceKind::JetBrains,
            // `install` treats the plugin as present when this parent exists.
            root: jetbrains_dir
                .parent()
                .map_or_else(|| jetbrains_dir.clone(), Path::to_path_buf),
            data_dir: PathBuf::new(),
            instructions: jetbrains_dir.join("global-copilot-instructions.md"),
            home_dir: home.to_path_buf(),
        },
    ]
}

/// Explain a half-configured surface. The common VS Code case (#1117): the
/// instructions are current but `User/mcp.json` lost the server after install.
fn report_partial_surface(dc: &mut DoctorCounters, surface: &CopilotSurface, registered: bool) {
    if registered {
        dc.info(&format!(
            "{}: MCP server registered but tokensave instructions missing — \
             `tokensave install --agent copilot` restores them",
            surface.label
        ));
        return;
    }
    dc.info(&format!(
        "{}: tokensave instructions are installed but the MCP server is not, so the \
         registration was removed after install (manual edit, reset, or a sync \
         service replacing the MCP config) — `tokensave install --agent copilot` restores it",
        surface.label
    ));
    if surface.kind == SurfaceKind::VsCode {
        dc.info(
            "  VS Code Settings Sync can replace User/mcp.json from another machine; \
             if it does, exclude MCP servers from sync or register tokensave on that machine too",
        );
    }
}

/// Report VS Code profiles (#1117). A profile that does not share the default
/// profile's settings keeps its own `mcp.json`, so a registration in
/// `User/mcp.json` does not reach it. Informational: whether a profile shares
/// settings is not recorded where tokensave can read it reliably.
fn report_vscode_profiles(dc: &mut DoctorCounters, user_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(user_dir.join("profiles")) else {
        return;
    };
    let mut total = 0usize;
    let mut registered = 0usize;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        total += 1;
        let mcp_json = entry.path().join("mcp.json");
        if mcp_json.exists()
            && load_jsonc_file(&mcp_json)
                .pointer("/servers/tokensave")
                .is_some()
        {
            registered += 1;
        }
    }
    if total == 0 {
        return;
    }
    dc.info(&format!(
        "VS Code has {total} custom profile(s); tokensave is registered in {registered} of \
         their own mcp.json files. A profile that does not share the default profile's \
         settings needs its own registration (MCP: Add Server in that profile)."
    ));
}
