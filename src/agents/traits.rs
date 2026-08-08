// ---------------------------------------------------------------------------
// AgentIntegration trait
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};

/// A CLI agent that can be configured to use tokensave via MCP.
pub trait AgentIntegration {
    /// Human-readable name (e.g. "Claude Code").
    fn name(&self) -> &'static str;

    /// CLI identifier used in `--agent <id>` (e.g. "claude").
    fn id(&self) -> &'static str;

    /// Register MCP server, permissions, hooks, and prompt rules.
    fn install(&self, ctx: &InstallContext) -> crate::errors::Result<()>;

    /// Remove everything installed by [`AgentIntegration::install`].
    fn uninstall(&self, ctx: &InstallContext) -> crate::errors::Result<()>;

    /// Verify installation health (replaces agent-specific doctor checks).
    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext);

    /// Returns true if this agent appears to be installed on the system
    /// (its config directory exists).
    fn is_detected(&self, _home: &Path) -> bool {
        false
    }

    /// Returns true if tokensave MCP server is already registered in this
    /// agent's config. Used for migration backfill.
    fn has_tokensave(&self, _home: &Path) -> bool {
        false
    }

    /// True if this agent has a project-scoped config that `--local` can
    /// target. Default false; supporting integrations override to true.
    fn supports_local(&self) -> bool {
        false
    }

    /// The single config file this agent rewrites on install / uninstall, if
    /// any. Returning `Some(path)` lets tests (and any future external tool)
    /// ask the integration for its own path instead of re-deriving it via
    /// `#[cfg(target_os = ...)]`, which is how the v4.3.15 zed regression
    /// test silently disagreed with the Windows install path. Implementors
    /// should return the same path the install helper writes to, including
    /// any platform-conditional branching. Returning `None` means "no single
    /// primary config" (e.g. an append-only TOML file with no rewrite path).
    fn primary_config_path(&self, _home: &Path) -> Option<PathBuf> {
        None
    }
}

/// Where an install writes its configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum InstallScope {
    /// User-level config under `$HOME` (default).
    Global,
    /// Project-level config rooted at `project_path` (`--local`).
    Local { project_path: PathBuf },
}

/// Context passed to [`AgentIntegration::install`] and [`AgentIntegration::uninstall`].
pub struct InstallContext {
    pub home: PathBuf,
    pub tokensave_bin: String,
    pub tool_permissions: Vec<String>,
    pub scope: InstallScope,
    /// Whether the caller explicitly requested a permission style this run
    /// (`--wildcard-permissions` / `--explicit-permissions`). `false` on
    /// every default/silent path (flagless `install`/`reinstall`, the
    /// silent reinstall-on-upgrade). Used by the Claude integration: when
    /// `false`, an existing covering grant the user already has (e.g. a
    /// hand-written `mcp__tokensave__*`) is left untouched instead of being
    /// churned back into the explicit per-tool list; when `true`, the
    /// requested style is written exactly, tearing down the other style.
    pub force_permission_style: bool,
}

impl InstallContext {
    /// True when this is a project-scoped (`--local`) install.
    pub fn is_local(&self) -> bool {
        matches!(self.scope, InstallScope::Local { .. })
    }

    /// Directory that agent config paths are rooted at: the user's home for
    /// global installs, the project directory for `--local`. Use this only for
    /// agents whose project-scoped path is the same relative path as the
    /// global one (e.g. `.cursor/mcp.json`). Agents whose layout differs must
    /// match on `scope` directly.
    pub fn base_dir(&self) -> &Path {
        match &self.scope {
            InstallScope::Global => &self.home,
            InstallScope::Local { project_path } => project_path,
        }
    }
}

/// Context passed to [`AgentIntegration::healthcheck`].
pub struct HealthcheckContext {
    pub home: PathBuf,
    pub project_path: PathBuf,
}

// ---------------------------------------------------------------------------
// DoctorCounters
// ---------------------------------------------------------------------------

/// Diagnostic counters for doctor checks.
#[derive(Default)]
pub struct DoctorCounters {
    pub issues: u32,
    pub warnings: u32,
}

impl DoctorCounters {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn pass(&self, msg: &str) {
        eprintln!("  \x1b[32m✔\x1b[0m {msg}");
    }
    pub fn fail(&mut self, msg: &str) {
        eprintln!("  \x1b[31m✘\x1b[0m {msg}");
        self.issues += 1;
    }
    pub fn warn(&mut self, msg: &str) {
        eprintln!("  \x1b[33m!\x1b[0m {msg}");
        self.warnings += 1;
    }
    pub fn info(&self, msg: &str) {
        eprintln!("    {msg}");
    }
}
