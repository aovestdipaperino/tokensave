// Rust guideline compliant 2026-08-28
//! Oh My Pi (OMP) agent integration.
//!
//! Global installs ask OMP for its active agent directory with a bounded
//! `omp config path` subprocess. This deliberately delegates profile and
//! directory selection to OMP: exported `OMP_PROFILE` or compatible
//! `PI_PROFILE` selects a named profile, and OMP also honors `PI_CONFIG_DIR`
//! and `PI_CODING_AGENT_DIR`. A `--profile` flag passed to another OMP
//! process is not observable here, so a bare install otherwise targets OMP's
//! default profile. Project installs are deterministic and use `.omp/`
//! directly without invoking OMP.
//!
//! Tokensave installs OMP's native MCP configuration and advisory rules. It
//! does not install an OMP hook because no OMP-specific executable hook
//! contract has been proven for Tokensave.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::*;

const OMP_CONFIG_TIMEOUT: Duration = Duration::from_secs(10);

/// Oh My Pi coding agent.
pub struct OmpIntegration;

impl AgentIntegration for OmpIntegration {
    fn name(&self) -> &'static str {
        "Oh My Pi"
    }

    fn id(&self) -> &'static str {
        "omp"
    }

    fn supports_local(&self) -> bool {
        true
    }

    fn install(&self, ctx: &InstallContext) -> Result<()> {
        let (mcp_path, rules_path) = omp_paths(ctx)?;
        install_mcp_server(&mcp_path, &ctx.tokensave_bin)?;
        write_managed_rules_file(&rules_path, &rules_for_agent("omp")?).map(|_| ())?;

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new OMP session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        let (mcp_path, rules_path) = omp_paths(ctx)?;
        uninstall_mcp_server(&mcp_path);
        uninstall_prompt_rules(&rules_path);

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Oh My Pi.");
        crate::agent_note!("Start a new OMP session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mOh My Pi integration\x1b[0m");

        if self.is_detected(&ctx.home) {
            match resolve_omp_agent_dir() {
                Ok(agent_dir) => doctor_check_surfaces(
                    dc,
                    &agent_dir.join("mcp.json"),
                    &agent_dir.join("rules/tokensave.md"),
                    "global",
                ),
                Err(error) => dc.fail(&format!(
                    "could not resolve the active OMP profile with `omp config path`: {error}"
                )),
            }
        }

        let local_dir = ctx.project_path.join(".omp");
        if local_dir.is_dir() {
            doctor_check_surfaces(
                dc,
                &local_dir.join("mcp.json"),
                &local_dir.join("rules/tokensave.md"),
                "project-local",
            );
        }
    }

    fn is_detected(&self, home: &Path) -> bool {
        home.join(".omp").is_dir()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        let config = load_json_file(&default_omp_mcp_path(home));
        config
            .get("mcpServers")
            .and_then(|servers| servers.get("tokensave"))
            .is_some()
    }

    fn primary_config_path(&self, home: &Path) -> Option<PathBuf> {
        Some(default_omp_mcp_path(home))
    }
}

fn default_omp_mcp_path(home: &Path) -> PathBuf {
    home.join(".omp/agent/mcp.json")
}

fn omp_paths(ctx: &InstallContext) -> Result<(PathBuf, PathBuf)> {
    match &ctx.scope {
        InstallScope::Local { project_path } => Ok((
            project_path.join(".omp/mcp.json"),
            project_path.join(".omp/rules/tokensave.md"),
        )),
        InstallScope::Global => {
            let agent_dir = resolve_omp_agent_dir()?;
            Ok((
                agent_dir.join("mcp.json"),
                agent_dir.join("rules/tokensave.md"),
            ))
        }
    }
}

fn resolver_error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: format!("`omp config path` {}", message.into()),
    }
}

/// Ask OMP for the active agent directory without risking an unbounded hang.
fn resolve_omp_agent_dir() -> Result<PathBuf> {
    let mut child = Command::new("omp")
        .args(["config", "path"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| resolver_error(format!("could not be started: {error}")))?;

    let deadline = Instant::now() + OMP_CONFIG_TIMEOUT;
    let status = loop {
        match child
            .try_wait()
            .map_err(|error| resolver_error(format!("could not be awaited: {error}")))?
        {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(resolver_error("timed out after 10 seconds"));
            }
            None => thread::sleep(Duration::from_millis(25)),
        }
    };

    let mut stdout = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_end(&mut stdout)
            .map_err(|error| resolver_error(format!("stdout could not be read: {error}")))?;
    }

    if !status.success() {
        return Err(resolver_error(format!("failed with exit status {status}")));
    }

    let stdout = String::from_utf8(stdout)
        .map_err(|_| resolver_error("returned non-UTF-8 output instead of one path"))?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(resolver_error("returned an empty path"));
    }
    if trimmed.lines().count() != 1 {
        return Err(resolver_error(
            "returned multiple lines instead of one path",
        ));
    }

    Ok(PathBuf::from(trimmed))
}

fn install_mcp_server(mcp_path: &Path, tokensave_bin: &str) -> Result<()> {
    if let Some(parent) = mcp_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let backup = backup_config_file(mcp_path)?;
    let mut config = match load_json_file_strict(mcp_path) {
        Ok(value) => value,
        Err(error) => {
            if let Some(ref backup_path) = backup {
                crate::agent_note!("  Backup preserved at: {}", backup_path.display());
            }
            return Err(error);
        }
    };
    let command = crate::agents::preserve_mcp_command(
        config.pointer("/mcpServers/tokensave/command"),
        tokensave_bin,
    );
    config["mcpServers"]["tokensave"] = json!({
        "command": command,
        "args": ["serve"]
    });

    safe_write_json_file(mcp_path, &config, backup.as_deref())?;
    crate::agent_note!(
        "\x1b[32m✔\x1b[0m Added tokensave MCP server to {}",
        mcp_path.display()
    );
    Ok(())
}

fn uninstall_mcp_server(mcp_path: &Path) {
    if !mcp_path.exists() {
        crate::agent_note!("  {} not found, skipping", mcp_path.display());
        return;
    }

    let Ok(contents) = std::fs::read_to_string(mcp_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return;
    };
    let Some(servers) = config
        .get_mut("mcpServers")
        .and_then(serde_json::Value::as_object_mut)
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

    let is_empty = config.as_object().is_some_and(|object| {
        object.iter().all(|(key, value)| {
            key == "mcpServers" && value.as_object().is_some_and(serde_json::Map::is_empty)
        })
    });
    if is_empty {
        std::fs::remove_file(mcp_path).ok();
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else if backup_and_write_json(mcp_path, &config) {
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
}

fn uninstall_prompt_rules(rules_path: &Path) {
    let Ok(contents) = std::fs::read_to_string(rules_path) else {
        return;
    };
    if !contents.contains(OMP_RULES_MARKER) {
        crate::agent_note!(
            "  {} does not contain the OMP ownership marker, skipping",
            rules_path.display()
        );
        return;
    }
    remove_managed_rules_file(rules_path);
}

fn doctor_check_surfaces(dc: &mut DoctorCounters, mcp_path: &Path, rules_path: &Path, scope: &str) {
    doctor_check_mcp(dc, mcp_path, scope);
    if rules_path.exists() {
        check_managed_rules_file(dc, rules_path, "omp");
    } else {
        dc.warn(&format!(
            "{scope} OMP rules not found at {} — run `tokensave install --agent omp{}`",
            rules_path.display(),
            if scope == "project-local" {
                " --local"
            } else {
                ""
            }
        ));
    }
}

fn doctor_check_mcp(dc: &mut DoctorCounters, mcp_path: &Path, scope: &str) {
    if !mcp_path.exists() {
        dc.warn(&format!(
            "{scope} OMP MCP config not found at {} — run `tokensave install --agent omp{}`",
            mcp_path.display(),
            if scope == "project-local" {
                " --local"
            } else {
                ""
            }
        ));
        return;
    }

    let config = load_json_file(mcp_path);
    let Some(server) = config
        .get("mcpServers")
        .and_then(|servers| servers.get("tokensave"))
        .and_then(serde_json::Value::as_object)
    else {
        dc.fail(&format!(
            "tokensave MCP server is missing or malformed in {}",
            mcp_path.display()
        ));
        return;
    };

    let command_ok = server
        .get("command")
        .and_then(serde_json::Value::as_str)
        .and_then(|command| Path::new(command).file_name())
        .is_some_and(|name| name == "tokensave");
    if command_ok {
        dc.pass(&format!(
            "tokensave MCP command is valid in {}",
            mcp_path.display()
        ));
    } else {
        dc.fail(&format!(
            "tokensave MCP command is missing or invalid in {} — run `tokensave install --agent omp`",
            mcp_path.display()
        ));
    }

    let args_ok = server
        .get("args")
        .is_some_and(|args| args == &json!(["serve"]));
    if args_ok {
        dc.pass(&format!(
            "tokensave MCP args are current in {}",
            mcp_path.display()
        ));
    } else {
        dc.fail(&format!(
            "tokensave MCP args are not exactly [\"serve\"] in {} — run `tokensave install --agent omp`",
            mcp_path.display()
        ));
    }
}
