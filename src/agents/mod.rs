// Rust guideline compliant 2025-10-17
//! Agent integration layer for CLI tools (Claude Code, OpenCode, Codex, etc.).
//!
//! Each supported agent implements the [`Agent`] trait which provides
//! `install`, `uninstall`, and `healthcheck` operations. The MCP server
//! itself is agent-agnostic; this module handles the per-agent config
//! plumbing (registering the MCP server, permissions, hooks, prompt rules).

pub mod claude;
pub mod codex;
pub mod opencode;

use std::path::{Path, PathBuf};

use crate::errors::Result;
use crate::errors::TokenSaveError;

pub use claude::ClaudeAgent;
pub use codex::CodexAgent;
pub use opencode::OpenCodeAgent;

// ---------------------------------------------------------------------------
// Agent trait
// ---------------------------------------------------------------------------

/// A CLI agent that can be configured to use tokensave via MCP.
pub trait Agent {
    /// Human-readable name (e.g. "Claude Code").
    fn name(&self) -> &'static str;

    /// CLI identifier used in `--agent <id>` (e.g. "claude").
    fn id(&self) -> &'static str;

    /// Register MCP server, permissions, hooks, and prompt rules.
    fn install(&self, ctx: &InstallContext) -> Result<()>;

    /// Remove everything installed by [`Agent::install`].
    fn uninstall(&self, ctx: &InstallContext) -> Result<()>;

    /// Verify installation health (replaces agent-specific doctor checks).
    fn healthcheck(&self, dc: &mut DoctorCounters, ctx: &HealthcheckContext);
}

/// Context passed to [`Agent::install`] and [`Agent::uninstall`].
pub struct InstallContext {
    pub home: PathBuf,
    pub tokensave_bin: String,
    pub tool_permissions: &'static [&'static str],
}

/// Context passed to [`Agent::healthcheck`].
pub struct HealthcheckContext {
    pub home: PathBuf,
    pub project_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Returns the agent matching `id`, or an error if unknown.
pub fn get_agent(id: &str) -> Result<Box<dyn Agent>> {
    match id {
        "claude" => Ok(Box::new(ClaudeAgent)),
        "opencode" => Ok(Box::new(OpenCodeAgent)),
        "codex" => Ok(Box::new(CodexAgent)),
        _ => Err(TokenSaveError::Config {
            message: format!(
                "unknown agent: \"{id}\". Available agents: {}",
                available_agents().join(", ")
            ),
        }),
    }
}

/// Returns all registered agents.
pub fn all_agents() -> Vec<Box<dyn Agent>> {
    vec![
        Box::new(ClaudeAgent),
        Box::new(OpenCodeAgent),
        Box::new(CodexAgent),
    ]
}

/// Returns the CLI identifiers of all registered agents (for help text).
pub fn available_agents() -> Vec<&'static str> {
    vec!["claude", "opencode", "codex"]
}

// ---------------------------------------------------------------------------
// DoctorCounters
// ---------------------------------------------------------------------------

/// Diagnostic counters for doctor checks.
pub struct DoctorCounters {
    pub issues: u32,
    pub warnings: u32,
}

impl DoctorCounters {
    pub fn new() -> Self {
        Self { issues: 0, warnings: 0 }
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

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Load a JSON file, returning an empty object on missing/invalid.
pub fn load_json_file(path: &Path) -> serde_json::Value {
    if path.exists() {
        let contents = std::fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&contents).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

/// Write a JSON value to a file with pretty formatting.
pub fn write_json_file(path: &Path, value: &serde_json::Value) -> Result<()> {
    let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(path, format!("{pretty}\n")).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write {}: {e}", path.display()),
    })?;
    eprintln!("\x1b[32m✔\x1b[0m Wrote {}", path.display());
    Ok(())
}

/// Finds the tokensave binary path.
pub fn which_tokensave() -> Option<String> {
    // Check the current executable first
    if let Ok(exe) = std::env::current_exe() {
        if exe
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("tokensave"))
        {
            return Some(exe.to_string_lossy().to_string());
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
        candidate.exists().then(|| candidate.to_string_lossy().to_string())
    })
}

/// Returns the user's home directory, cross-platform.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Load a TOML file, returning an empty table on missing/invalid.
pub fn load_toml_file(path: &Path) -> toml::Value {
    if path.exists() {
        let contents = std::fs::read_to_string(path).unwrap_or_default();
        contents
            .parse::<toml::Value>()
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::Value::Table(toml::map::Map::new())
    }
}

/// Write a TOML value to a file.
pub fn write_toml_file(path: &Path, value: &toml::Value) -> Result<()> {
    let contents =
        toml::to_string_pretty(value).unwrap_or_else(|_| String::new());
    std::fs::write(path, contents).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write {}: {e}", path.display()),
    })?;
    eprintln!("\x1b[32m✔\x1b[0m Wrote {}", path.display());
    Ok(())
}

/// Bare MCP tool names (without any agent-specific prefix).
pub const TOOL_NAMES: &[&str] = &[
    "tokensave_affected",
    "tokensave_callees",
    "tokensave_callers",
    "tokensave_changelog",
    "tokensave_circular",
    "tokensave_complexity",
    "tokensave_context",
    "tokensave_coupling",
    "tokensave_dead_code",
    "tokensave_diff_context",
    "tokensave_distribution",
    "tokensave_doc_coverage",
    "tokensave_files",
    "tokensave_god_class",
    "tokensave_hotspots",
    "tokensave_impact",
    "tokensave_inheritance_depth",
    "tokensave_largest",
    "tokensave_module_api",
    "tokensave_node",
    "tokensave_rank",
    "tokensave_recursion",
    "tokensave_rename_preview",
    "tokensave_search",
    "tokensave_similar",
    "tokensave_status",
    "tokensave_unused_imports",
];

/// Expected MCP tool permissions for the current version (Claude Code format).
pub const EXPECTED_TOOL_PERMS: &[&str] = &[
    "mcp__tokensave__tokensave_affected",
    "mcp__tokensave__tokensave_callees",
    "mcp__tokensave__tokensave_callers",
    "mcp__tokensave__tokensave_changelog",
    "mcp__tokensave__tokensave_circular",
    "mcp__tokensave__tokensave_complexity",
    "mcp__tokensave__tokensave_context",
    "mcp__tokensave__tokensave_coupling",
    "mcp__tokensave__tokensave_dead_code",
    "mcp__tokensave__tokensave_diff_context",
    "mcp__tokensave__tokensave_distribution",
    "mcp__tokensave__tokensave_doc_coverage",
    "mcp__tokensave__tokensave_files",
    "mcp__tokensave__tokensave_god_class",
    "mcp__tokensave__tokensave_hotspots",
    "mcp__tokensave__tokensave_impact",
    "mcp__tokensave__tokensave_inheritance_depth",
    "mcp__tokensave__tokensave_largest",
    "mcp__tokensave__tokensave_module_api",
    "mcp__tokensave__tokensave_node",
    "mcp__tokensave__tokensave_rank",
    "mcp__tokensave__tokensave_recursion",
    "mcp__tokensave__tokensave_rename_preview",
    "mcp__tokensave__tokensave_search",
    "mcp__tokensave__tokensave_similar",
    "mcp__tokensave__tokensave_status",
    "mcp__tokensave__tokensave_unused_imports",
];
