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

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::errors::{Result, TokenSaveError};

use super::*;

const OMP_CONFIG_TIMEOUT: Duration = Duration::from_secs(10);
const OMP_PROFILE_REGISTRY_VERSION: u32 = 1;
const OMP_PROFILE_REGISTRY_FILE: &str = "omp-profiles.json";
const OMP_PROFILE_REGISTRY_LOCK: &str = "omp-profiles.lock";

#[derive(Debug, Default, Deserialize, Serialize)]
struct OmpProfileRegistry {
    version: u32,
    agent_dirs: Vec<PathBuf>,
}

struct LockedOmpProfileRegistry {
    _lock: File,
    path: PathBuf,
    registry: OmpProfileRegistry,
}

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
        match &ctx.scope {
            InstallScope::Local { .. } => {
                let (mcp_path, rules_path) = omp_paths(ctx)?;
                install_omp_surfaces(&mcp_path, &rules_path, &ctx.tokensave_bin)?;
            }
            InstallScope::Global => install_global(ctx)?,
        }

        crate::agent_note!();
        crate::agent_note!("Setup complete. Next steps:");
        crate::agent_note!("  1. cd into your project and run: tokensave init");
        crate::agent_note!("  2. Start a new OMP session — tokensave tools are now available");
        Ok(())
    }

    fn uninstall(&self, ctx: &InstallContext) -> Result<()> {
        match &ctx.scope {
            InstallScope::Local { .. } => {
                let (mcp_path, rules_path) = omp_paths(ctx)?;
                uninstall_omp_surfaces(&mcp_path, &rules_path)?;
            }
            InstallScope::Global => uninstall_global(ctx)?,
        }

        crate::agent_note!();
        crate::agent_note!("Uninstall complete. Tokensave has been removed from Oh My Pi.");
        crate::agent_note!("Start a new OMP session for changes to take effect.");
        Ok(())
    }

    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext) {
        crate::agent_note!("\n\x1b[1mOh My Pi integration\x1b[0m");

        doctor_check_global_profiles(dc, &ctx.home);

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
        if home.join(".omp").is_dir() {
            return true;
        }
        if load_omp_profile_registry(home)
            .is_ok_and(|registry| registry.agent_dirs.iter().any(|path| path.is_dir()))
        {
            return true;
        }
        resolve_omp_agent_dir().is_ok()
    }

    fn has_tokensave(&self, home: &Path) -> bool {
        omp_candidate_dirs(home)
            .iter()
            .any(|agent_dir| omp_mcp_has_tokensave(&agent_dir.join("mcp.json")))
    }

    fn primary_config_path(&self, _home: &Path) -> Option<PathBuf> {
        resolve_omp_agent_dir()
            .ok()
            .map(|agent_dir| agent_dir.join("mcp.json"))
    }
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

impl LockedOmpProfileRegistry {
    fn load(home: &Path) -> Result<Self> {
        let state_dir = home.join(".tokensave");
        std::fs::create_dir_all(&state_dir).map_err(|error| {
            omp_registry_error(format!("could not create {}: {error}", state_dir.display()))
        })?;
        let lock_path = state_dir.join(OMP_PROFILE_REGISTRY_LOCK);
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(&lock_path).map_err(|error| {
            omp_registry_error(format!(
                "could not open lock {}: {error}",
                lock_path.display()
            ))
        })?;
        lock.lock_exclusive().map_err(|error| {
            omp_registry_error(format!("could not lock {}: {error}", lock_path.display()))
        })?;

        let path = state_dir.join(OMP_PROFILE_REGISTRY_FILE);
        let registry = load_omp_profile_registry_from(&path)?;
        Ok(Self {
            _lock: lock,
            path,
            registry,
        })
    }

    fn save(&mut self) -> Result<()> {
        normalize_agent_dirs(&mut self.registry.agent_dirs)?;
        if self.registry.agent_dirs.is_empty() {
            if self.path.exists() {
                std::fs::remove_file(&self.path).map_err(|error| {
                    omp_registry_error(format!("could not remove {}: {error}", self.path.display()))
                })?;
            }
            return Ok(());
        }
        self.registry.version = OMP_PROFILE_REGISTRY_VERSION;
        let value = serde_json::to_value(&self.registry)
            .map_err(|error| omp_registry_error(format!("could not be serialized: {error}")))?;
        safe_write_json_file(&self.path, &value, None)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).map_err(
                |error| {
                    omp_registry_error(format!(
                        "could not restrict permissions on {}: {error}",
                        self.path.display()
                    ))
                },
            )?;
        }
        Ok(())
    }
}

fn omp_registry_error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: format!("OMP profile registry {}", message.into()),
    }
}

fn load_omp_profile_registry(home: &Path) -> Result<OmpProfileRegistry> {
    load_omp_profile_registry_from(&home.join(".tokensave").join(OMP_PROFILE_REGISTRY_FILE))
}

fn load_omp_profile_registry_from(path: &Path) -> Result<OmpProfileRegistry> {
    if !path.exists() {
        return Ok(OmpProfileRegistry {
            version: OMP_PROFILE_REGISTRY_VERSION,
            agent_dirs: Vec::new(),
        });
    }
    let contents = std::fs::read_to_string(path).map_err(|error| {
        omp_registry_error(format!("could not read {}: {error}", path.display()))
    })?;
    let mut registry: OmpProfileRegistry = serde_json::from_str(&contents).map_err(|error| {
        omp_registry_error(format!("at {} is malformed: {error}", path.display()))
    })?;
    if registry.version != OMP_PROFILE_REGISTRY_VERSION {
        return Err(omp_registry_error(format!(
            "at {} has unsupported version {}",
            path.display(),
            registry.version
        )));
    }
    normalize_agent_dirs(&mut registry.agent_dirs)?;
    Ok(registry)
}

fn normalize_agent_dirs(agent_dirs: &mut Vec<PathBuf>) -> Result<()> {
    if let Some(path) = agent_dirs.iter().find(|path| !path.is_absolute()) {
        return Err(omp_registry_error(format!(
            "contains a non-absolute agent directory: {}",
            path.display()
        )));
    }
    agent_dirs.sort();
    agent_dirs.dedup();
    Ok(())
}

fn omp_candidate_dirs(home: &Path) -> Vec<PathBuf> {
    let mut paths = load_omp_profile_registry(home)
        .map(|registry| registry.agent_dirs)
        .unwrap_or_default();
    if let Ok(active) = resolve_omp_agent_dir() {
        paths.push(active);
    }
    paths.push(home.join(".omp/agent"));
    let _ = normalize_agent_dirs(&mut paths);
    paths
}

fn omp_mcp_has_tokensave(mcp_path: &Path) -> bool {
    let config = load_json_file(mcp_path);
    config
        .get("mcpServers")
        .and_then(|servers| servers.get("tokensave"))
        .is_some()
}

fn install_global(ctx: &InstallContext) -> Result<()> {
    let active = resolve_omp_agent_dir()?;
    let mut locked = LockedOmpProfileRegistry::load(&ctx.home)?;
    locked
        .registry
        .agent_dirs
        .retain(|path| path == &active || path.is_dir());
    locked.registry.agent_dirs.push(active.clone());
    normalize_agent_dirs(&mut locked.registry.agent_dirs)?;
    locked.save()?;

    let rules = rules_for_agent("omp")?;
    let mut targets = vec![active.clone()];
    targets.extend(
        locked
            .registry
            .agent_dirs
            .iter()
            .filter(|path| *path != &active)
            .cloned(),
    );
    let mut failures = Vec::new();
    for agent_dir in &targets {
        if let Err(error) = install_omp_surfaces_with_rules(
            &agent_dir.join("mcp.json"),
            &agent_dir.join("rules/tokensave.md"),
            &ctx.tokensave_bin,
            &rules,
        ) {
            failures.push(format!("{}: {error}", agent_dir.display()));
        }
    }
    if !failures.is_empty() {
        return Err(omp_profile_failures("install", &failures));
    }
    Ok(())
}

fn uninstall_global(ctx: &InstallContext) -> Result<()> {
    let mut locked = LockedOmpProfileRegistry::load(&ctx.home)?;
    let active = resolve_omp_agent_dir();
    let mut targets = locked.registry.agent_dirs.clone();
    targets.push(ctx.home.join(".omp/agent"));
    if let Ok(path) = &active {
        targets.push(path.clone());
    }
    normalize_agent_dirs(&mut targets)?;
    let had_known_target = targets.iter().any(|path| path.is_dir());

    let mut failures = Vec::new();
    let mut retry = Vec::new();
    for agent_dir in targets.iter().filter(|path| path.is_dir()) {
        if let Err(error) = uninstall_omp_surfaces(
            &agent_dir.join("mcp.json"),
            &agent_dir.join("rules/tokensave.md"),
        ) {
            failures.push(format!("{}: {error}", agent_dir.display()));
            retry.push(agent_dir.clone());
        }
    }

    locked.registry.agent_dirs = retry;
    locked.save()?;
    if !failures.is_empty() {
        return Err(omp_profile_failures("uninstall", &failures));
    }
    match active {
        Ok(_) => Ok(()),
        Err(error) if had_known_target => {
            crate::agent_note!(
                "  Active OMP profile unavailable; removed tokensave from recorded profiles: {error}"
            );
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn omp_profile_failures(action: &str, failures: &[String]) -> TokenSaveError {
    TokenSaveError::Config {
        message: format!(
            "OMP {action} was incomplete for {} profile(s): {}",
            failures.len(),
            failures.join("; ")
        ),
    }
}

fn install_omp_surfaces(mcp_path: &Path, rules_path: &Path, tokensave_bin: &str) -> Result<()> {
    let rules = rules_for_agent("omp")?;
    install_omp_surfaces_with_rules(mcp_path, rules_path, tokensave_bin, &rules)
}

fn install_omp_surfaces_with_rules(
    mcp_path: &Path,
    rules_path: &Path,
    tokensave_bin: &str,
    rules: &str,
) -> Result<()> {
    install_mcp_server(mcp_path, tokensave_bin)?;
    write_managed_rules_file(rules_path, rules).map(|_| ())
}

fn uninstall_omp_surfaces(mcp_path: &Path, rules_path: &Path) -> Result<()> {
    uninstall_mcp_server(mcp_path)?;
    uninstall_prompt_rules(rules_path)
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

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(resolver_error("returned a non-absolute path"));
    }
    Ok(path)
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

fn uninstall_mcp_server(mcp_path: &Path) -> Result<()> {
    if !mcp_path.exists() {
        crate::agent_note!("  {} not found, skipping", mcp_path.display());
        return Ok(());
    }

    let contents = std::fs::read_to_string(mcp_path).map_err(|error| TokenSaveError::Config {
        message: format!(
            "failed to read OMP MCP config {}: {error}",
            mcp_path.display()
        ),
    })?;
    let mut config = serde_json::from_str::<serde_json::Value>(&contents).map_err(|error| {
        TokenSaveError::Config {
            message: format!(
                "failed to parse OMP MCP config {} during uninstall: {error}",
                mcp_path.display()
            ),
        }
    })?;
    let Some(servers) = config
        .get_mut("mcpServers")
        .and_then(serde_json::Value::as_object_mut)
    else {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return Ok(());
    };
    if servers.remove("tokensave").is_none() {
        crate::agent_note!(
            "  No tokensave MCP server in {}, skipping",
            mcp_path.display()
        );
        return Ok(());
    }

    let is_empty = config.as_object().is_some_and(|object| {
        object.iter().all(|(key, value)| {
            key == "mcpServers" && value.as_object().is_some_and(serde_json::Map::is_empty)
        })
    });
    if is_empty {
        std::fs::remove_file(mcp_path).map_err(|error| TokenSaveError::Config {
            message: format!(
                "failed to remove empty OMP MCP config {}: {error}",
                mcp_path.display()
            ),
        })?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed {} (was empty)",
            mcp_path.display()
        );
    } else {
        let backup = backup_config_file(mcp_path)?;
        safe_write_json_file(mcp_path, &config, backup.as_deref())?;
        crate::agent_note!(
            "\x1b[32m✔\x1b[0m Removed tokensave MCP server from {}",
            mcp_path.display()
        );
    }
    Ok(())
}

fn uninstall_prompt_rules(rules_path: &Path) -> Result<()> {
    let contents = match std::fs::read_to_string(rules_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(TokenSaveError::Config {
                message: format!(
                    "failed to read OMP rules {} during uninstall: {error}",
                    rules_path.display()
                ),
            });
        }
    };
    if !contents.contains(OMP_RULES_MARKER) {
        crate::agent_note!(
            "  {} does not contain the OMP ownership marker, skipping",
            rules_path.display()
        );
        return Ok(());
    }
    let real_path = resolve_symlink_target(rules_path).map_err(|error| TokenSaveError::Config {
        message: format!(
            "failed to resolve OMP rules {} during uninstall: {error}",
            rules_path.display()
        ),
    })?;
    remove_managed_rules_file(rules_path);
    if real_path.exists() {
        return Err(TokenSaveError::Config {
            message: format!(
                "failed to remove OMP rules {} during uninstall",
                real_path.display()
            ),
        });
    }
    Ok(())
}

fn doctor_check_global_profiles(dc: &mut DoctorCounters, home: &Path) {
    let registry = match load_omp_profile_registry(home) {
        Ok(registry) => registry,
        Err(error) => {
            dc.fail(&error.to_string());
            OmpProfileRegistry::default()
        }
    };
    let mut targets = registry.agent_dirs;
    if home.join(".omp/agent").is_dir() {
        targets.push(home.join(".omp/agent"));
    }
    match resolve_omp_agent_dir() {
        Ok(active) => targets.push(active),
        Err(_) if targets.is_empty() && !home.join(".omp").is_dir() => return,
        Err(error) if targets.is_empty() => dc.fail(&format!(
            "could not resolve the active OMP profile with `omp config path`: {error}"
        )),
        Err(error) => dc.warn(&format!(
            "could not resolve the active OMP profile with `omp config path`; checking recorded profiles only: {error}"
        )),
    }
    let _ = normalize_agent_dirs(&mut targets);

    for agent_dir in &targets {
        doctor_check_surfaces(
            dc,
            &agent_dir.join("mcp.json"),
            &agent_dir.join("rules/tokensave.md"),
            "global",
        );
    }
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
