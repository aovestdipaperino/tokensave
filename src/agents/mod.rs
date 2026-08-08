// Rust guideline compliant 2025-10-17
//! Agent integration layer for CLI tools (Claude Code, `OpenCode`, Codex, etc.).
//!
//! Each supported agent implements the [`AgentIntegration`] trait which provides
//! `install`, `uninstall`, and `healthcheck` operations. The MCP server
//! itself is agent-agnostic; this module handles the per-agent config
//! plumbing (registering the MCP server, permissions, hooks, prompt rules).

/// Set while a non-interactive caller (the silent reinstall-on-upgrade in
/// `main`) drives `install`, so the per-agent integrations stay quiet instead
/// of printing their full setup banner on every `init`/`sync` (#255).
static QUIET_INSTALL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Suppress (or re-enable) agent install progress output.
pub fn set_quiet_install(quiet: bool) {
    QUIET_INSTALL.store(quiet, std::sync::atomic::Ordering::Relaxed);
}

/// Whether agent install progress output is currently suppressed.
pub fn quiet_install() -> bool {
    QUIET_INSTALL.load(std::sync::atomic::Ordering::Relaxed)
}

/// Re-run install for every tracked agent so permissions, hooks, and MCP
/// config stay in sync after the binary changes version.
///
/// Two signals trigger a resync:
///   (a) `previous_version` (set by `tokensave upgrade` / `channel switch`
///       just before replacing the binary) differs from the running version
///       AND the transition is a minor/major bump. Patch bumps are no-ops:
///       we just advance `previous_version` and skip reinstall.
///   (b) Fallback for external upgrades (`brew upgrade`, `cargo install`):
///       the running version is newer than `last_installed_version`.
///
/// `install` is called once per tracked agent id and returns `false` when that
/// agent could not be updated. Version markers advance regardless of those
/// failures — see [`ResyncOutcome::failed`]. Returns the outcome; the caller is
/// responsible for persisting `config` and reporting failures.
pub fn resync_installed_agents<F>(
    config: &mut crate::user_config::UserConfig,
    running: &str,
    mut install: F,
) -> ResyncOutcome
where
    F: FnMut(&str) -> bool,
{
    let previous_version = if config.previous_version.is_empty() {
        "6.0.0".to_string()
    } else {
        config.previous_version.clone()
    };
    let upgrade_detected = previous_version != running;
    let transition_needs_reinstall = upgrade_detected
        && (crate::cloud::is_newer_minor_version(&previous_version, running)
            || crate::cloud::is_newer_minor_version(running, &previous_version));
    let external_upgrade_needs_reinstall = !upgrade_detected
        && (config.last_installed_version.is_empty()
            || crate::cloud::is_newer_version(&config.last_installed_version, running));
    let needs_reinstall = transition_needs_reinstall || external_upgrade_needs_reinstall;

    if config.installed_agents.is_empty() || running.is_empty() || !needs_reinstall {
        if upgrade_detected {
            // Patch-only bump (or nothing to reinstall) — advance the marker
            // so we don't keep re-checking on every subsequent startup.
            config.previous_version = running.to_string();
            return ResyncOutcome {
                changed: true,
                ran: false,
                failed: Vec::new(),
            };
        }
        return ResyncOutcome {
            changed: false,
            ran: false,
            failed: Vec::new(),
        };
    }

    let agents = config.installed_agents.clone();
    let failed: Vec<String> = agents.into_iter().filter(|id| !install(id)).collect();

    // Advance the markers even when some agents failed. A config path we can't
    // write (missing app, read-only location) fails identically on every run,
    // and gating the markers on full success re-ran this resync — banner output
    // included — on every single command (#255). Report the failures once
    // instead of retrying forever.
    config.last_installed_version = running.to_string();
    config.previous_version = running.to_string();
    ResyncOutcome {
        changed: true,
        ran: true,
        failed,
    }
}

/// Result of [`resync_installed_agents`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ResyncOutcome {
    /// Whether `config` was mutated and needs saving.
    pub changed: bool,
    /// Whether the per-agent install loop actually ran.
    pub ran: bool,
    /// Ids of agents whose install failed. Non-fatal.
    pub failed: Vec<String>,
}

/// `eprintln!` for agent install progress: silent under [`set_quiet_install`].
#[macro_export]
macro_rules! agent_note {
    ($($arg:tt)*) => {
        if !$crate::agents::quiet_install() {
            eprintln!($($arg)*);
        }
    };
}

/// `eprint!` for agent install progress: silent under [`set_quiet_install`].
#[macro_export]
macro_rules! agent_note_inline {
    ($($arg:tt)*) => {
        if !$crate::agents::quiet_install() {
            eprint!($($arg)*);
        }
    };
}

pub mod fs;
pub mod hooks;
pub mod integrations;
pub mod registry;
pub mod rules;
pub mod traits;

use std::path::{Path, PathBuf};

use crate::mcp::tools::get_tool_definitions;

pub use fs::*;
pub use hooks::*;
pub use integrations::*;
pub use registry::*;
pub use rules::*;
pub use traits::*;

/// Finds the tokensave binary path.
///
/// On Windows the returned path uses forward slashes so it can be safely
/// embedded in JSON hook commands without backslash-escaping issues.
pub fn which_tokensave() -> Option<String> {
    // Check the current executable first
    if let Ok(exe) = std::env::current_exe() {
        if exe
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("tokensave"))
        {
            let normalized = normalize_path_separators(&exe.to_string_lossy());
            // `current_exe()` resolves symlinks, so under Homebrew it points at the
            // version-pinned Cellar path (e.g. `.../Cellar/tokensave/6.4.2/bin/...`).
            // `brew upgrade`/`brew cleanup` later remove that path, breaking any hook
            // config that embedded it (#146). Prefer the version-stable `bin` symlink
            // when it exists.
            if let Some(stable) = homebrew_stable_path(&normalized) {
                if Path::new(&stable).exists() {
                    return Some(stable);
                }
            }
            return Some(normalized);
        }
    }
    // Fall back to PATH lookup
    let path_var = std::env::var("PATH").ok()?;
    let separator = if cfg!(windows) { ';' } else { ':' };
    let bin_name = if cfg!(windows) {
        "tokensave.exe"
    } else {
        "tokensave"
    };
    path_var.split(separator).find_map(|dir| {
        let candidate = PathBuf::from(dir).join(bin_name);
        candidate
            .exists()
            .then(|| normalize_path_separators(&candidate.to_string_lossy()))
    })
}

/// Warns when agent config will reference a disposable Cargo build artifact.
pub fn cargo_build_binary_warning(path: &str) -> Option<String> {
    let components: Vec<_> = Path::new(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();

    let is_profile = |value: &str| matches!(value, "debug" | "release");
    let cargo_build = components
        .windows(2)
        .any(|parts| parts[0] == "target" && is_profile(parts[1]))
        || components
            .windows(3)
            .any(|parts| parts[0] == "target" && is_profile(parts[2]));

    cargo_build.then(|| {
        format!(
            "\x1b[33mwarning:\x1b[0m agent config references Cargo build output:\n  \
             {path}\n  `cargo clean` or removing its worktree will break tokensave hooks and MCP \
             servers.\n  Re-run `tokensave install` from a stable `cargo install`, Homebrew, or \
             release binary."
        )
    })
}

/// Keeps the user's existing MCP command when it still resolves (issue #161).
///
/// Reinstalls used to overwrite whatever command the config held with this
/// install's absolute binary path, clobbering deliberate choices like a bare
/// `tokensave` resolved via `PATH` (portable across machines with different
/// install locations). If the previous command still resolves to a tokensave
/// binary, keep it verbatim; otherwise use `new_bin`.
pub fn preserve_mcp_command_str(previous: Option<&str>, new_bin: &str) -> String {
    match previous {
        Some(prev) if command_resolves_to_tokensave(prev) => prev.to_string(),
        _ => new_bin.to_string(),
    }
}

/// JSON variant of [`preserve_mcp_command_str`]: accepts the previous
/// command as either a string (`"command": "tokensave"`) or an array whose
/// first element is the binary (`"command": ["tokensave", "serve"]`).
pub fn preserve_mcp_command(previous: Option<&serde_json::Value>, new_bin: &str) -> String {
    let prev_str = previous.and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.as_str()),
        serde_json::Value::Array(a) => a.first().and_then(serde_json::Value::as_str),
        _ => None,
    });
    preserve_mcp_command_str(prev_str, new_bin)
}

/// True when `cmd` names a tokensave binary that exists: an absolute or
/// relative path that is on disk, or a bare name found on `PATH`.
fn command_resolves_to_tokensave(cmd: &str) -> bool {
    command_resolves_to_tokensave_in(cmd, std::env::var("PATH").ok().as_deref())
}

/// [`command_resolves_to_tokensave`] with the `PATH` value injected so tests
/// don't have to mutate process-global environment.
fn command_resolves_to_tokensave_in(cmd: &str, path_var: Option<&str>) -> bool {
    let name_ok = Path::new(cmd)
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n.starts_with("tokensave"));
    if !name_ok {
        return false;
    }
    if cmd.contains('/') || cmd.contains('\\') {
        return Path::new(cmd).exists();
    }
    let Some(path_var) = path_var else {
        return false;
    };
    let separator = if cfg!(windows) { ';' } else { ':' };
    path_var.split(separator).any(|dir| {
        let base = PathBuf::from(dir).join(cmd);
        base.exists() || (cfg!(windows) && base.with_extension("exe").exists())
    })
}

/// Maps a Homebrew Cellar executable path to its version-stable `bin` symlink.
///
/// Homebrew installs the real binary under `<prefix>/Cellar/tokensave/<version>/bin/`
/// and exposes it on `PATH` via a stable `<prefix>/bin/tokensave` symlink. Embedding
/// the Cellar path in hook configs breaks on `brew upgrade`/`brew cleanup`; the `bin`
/// symlink always tracks the current version. Expects a forward-slash path. Returns
/// `None` for non-Cellar paths, leaving the caller to use the path as-is.
fn homebrew_stable_path(exe: &str) -> Option<String> {
    let (prefix, rest) = exe.split_once("/Cellar/tokensave/")?;
    let file = rest.rsplit('/').next()?;
    Some(format!("{prefix}/bin/{file}"))
}

#[cfg(test)]
mod which_tokensave_tests {
    use super::*;

    #[test]
    fn warns_for_cargo_profile_paths() {
        for path in [
            "/repo/target/debug/tokensave",
            "/repo/target/release/tokensave",
            "/repo/target/aarch64-apple-darwin/release/tokensave",
            "C:/repo/target/x86_64-pc-windows-msvc/debug/tokensave.exe",
        ] {
            let warning = cargo_build_binary_warning(path)
                .unwrap_or_else(|| panic!("expected warning for {path}"));
            assert!(warning.contains(path));
            assert!(warning.contains("cargo clean"));
            assert!(warning.contains("cargo install"));
        }
    }

    #[test]
    fn ignores_stable_and_near_miss_paths() {
        for path in [
            "/Users/me/.cargo/bin/tokensave",
            "/opt/homebrew/bin/tokensave",
            "/repo/mytarget/release/tokensave",
            "/repo/target/profile/tokensave",
            "/repo/target/foo/bar/release/tokensave",
        ] {
            assert_eq!(
                cargo_build_binary_warning(path),
                None,
                "unexpected warning for {path}"
            );
        }
    }

    // Regression for #146: hooks embedded a version-pinned Homebrew Cellar
    // path, which `brew upgrade`/`brew cleanup` later removes. The stable
    // `<prefix>/bin/tokensave` symlink survives upgrades and must be preferred.

    #[test]
    fn deversions_linuxbrew_cellar_path() {
        assert_eq!(
            homebrew_stable_path("/home/linuxbrew/.linuxbrew/Cellar/tokensave/6.4.2/bin/tokensave"),
            Some("/home/linuxbrew/.linuxbrew/bin/tokensave".to_string())
        );
    }

    #[test]
    fn deversions_macos_arm_cellar_path() {
        assert_eq!(
            homebrew_stable_path("/opt/homebrew/Cellar/tokensave/6.4.2/bin/tokensave"),
            Some("/opt/homebrew/bin/tokensave".to_string())
        );
    }

    #[test]
    fn ignores_non_cellar_cargo_path() {
        assert_eq!(homebrew_stable_path("/Users/me/.cargo/bin/tokensave"), None);
    }

    #[test]
    fn ignores_already_stable_bin_path() {
        assert_eq!(
            homebrew_stable_path("/home/linuxbrew/.linuxbrew/bin/tokensave"),
            None
        );
    }
}

pub fn tool_names() -> Vec<String> {
    get_tool_definitions()
        .iter()
        .map(|t| t.name.clone())
        .collect()
}

pub fn read_only_tool_names() -> Vec<String> {
    get_tool_definitions()
        .iter()
        .filter(|t| {
            t.annotations
                .as_ref()
                .and_then(|annotations| annotations.get("readOnlyHint"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .map(|t| t.name.clone())
        .collect()
}

pub fn expected_tool_perms() -> Vec<String> {
    get_tool_definitions()
        .iter()
        .map(|t| format!("mcp__tokensave__{}", t.name))
        .collect()
}

/// The single compact permission entry that grants Claude Code all tokensave
/// tools at once, as an alternative to enumerating every tool individually.
/// Both this wildcard form and the bare `mcp__tokensave` form are fully
/// honored by Claude Code as allow rules; this is the one tokensave writes
/// when the compact style is requested.
pub const TOKENSAVE_WILDCARD_PERM: &str = "mcp__tokensave__*";

/// Tool permissions to install for Claude Code: either the single compact
/// wildcard entry, or the full explicit per-tool list, depending on
/// `wildcard`. See [`TOKENSAVE_WILDCARD_PERM`] and [`expected_tool_perms`].
pub fn install_tool_perms(wildcard: bool) -> Vec<String> {
    if wildcard {
        vec![TOKENSAVE_WILDCARD_PERM.to_string()]
    } else {
        expected_tool_perms()
    }
}

/// Regression tests for #255: the silent reinstall-on-upgrade printed every
/// agent's setup banner on every `init`/`sync`, forever.
#[cfg(test)]
mod resync_tests {
    use super::*;
    use crate::user_config::UserConfig;

    /// A config that looks like a fresh external upgrade (`brew upgrade`):
    /// two tracked agents and a stale `last_installed_version`.
    fn upgraded_config() -> UserConfig {
        UserConfig {
            installed_agents: vec!["claude".to_string(), "copilot".to_string()],
            last_installed_version: "7.3.0".to_string(),
            previous_version: "7.4.0".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn resync_runs_once_then_stops_when_every_agent_succeeds() {
        let mut config = upgraded_config();
        let mut calls = 0;
        let first = resync_installed_agents(&mut config, "7.4.0", |_| {
            calls += 1;
            true
        });
        assert!(first.ran);
        assert_eq!(calls, 2);
        assert!(first.failed.is_empty());

        // Second invocation at the same version must not reinstall again.
        let second = resync_installed_agents(&mut config, "7.4.0", |_| {
            calls += 1;
            true
        });
        assert!(!second.ran, "resync repeated at an unchanged version");
        assert_eq!(calls, 2, "install ran again on the second invocation");
    }

    /// The core of #255: one agent that can never be written (missing app,
    /// read-only path) must not pin the version markers and re-trigger the
    /// whole resync — banner output included — on every subsequent command.
    #[test]
    fn failing_agent_does_not_retrigger_resync_forever() {
        let mut config = upgraded_config();
        let calls = std::cell::Cell::new(0);
        // "copilot" always fails, exactly as an unwritable VS Code settings
        // path would.
        let mut installer = |id: &str| {
            calls.set(calls.get() + 1);
            id != "copilot"
        };

        let first = resync_installed_agents(&mut config, "7.4.0", &mut installer);
        assert!(first.ran);
        assert_eq!(first.failed, vec!["copilot".to_string()]);
        assert!(first.changed, "markers must advance despite the failure");
        assert_eq!(config.last_installed_version, "7.4.0");
        assert_eq!(config.previous_version, "7.4.0");
        assert_eq!(calls.get(), 2);

        // Every later run at the same version is a no-op.
        for _ in 0..3 {
            let again = resync_installed_agents(&mut config, "7.4.0", &mut installer);
            assert!(!again.ran, "a failing agent re-triggered the resync (#255)");
            assert!(again.failed.is_empty());
        }
        assert_eq!(
            calls.get(),
            2,
            "install re-ran after a permanent failure (#255)"
        );
    }

    #[test]
    fn patch_bump_advances_marker_without_reinstalling() {
        let mut config = upgraded_config();
        config.last_installed_version = "7.4.0".to_string();
        config.previous_version = "7.4.1".to_string();
        let mut calls = 0;
        let outcome = resync_installed_agents(&mut config, "7.4.2", |_| {
            calls += 1;
            true
        });
        assert!(!outcome.ran, "patch bump should not reinstall");
        assert_eq!(calls, 0);
        assert!(outcome.changed);
        assert_eq!(config.previous_version, "7.4.2");
    }

    #[test]
    fn no_tracked_agents_is_a_no_op() {
        let mut config = UserConfig {
            previous_version: "7.4.2".to_string(),
            ..Default::default()
        };
        let mut calls = 0;
        let outcome = resync_installed_agents(&mut config, "7.4.2", |_| {
            calls += 1;
            true
        });
        assert_eq!(outcome, ResyncOutcome::default());
        assert_eq!(calls, 0);
    }

    #[test]
    fn quiet_install_suppresses_agent_progress_output() {
        set_quiet_install(true);
        assert!(quiet_install());
        set_quiet_install(false);
        assert!(!quiet_install());
    }
}
