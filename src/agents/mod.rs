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

// ---------------------------------------------------------------------------
// Git post-commit hook
// ---------------------------------------------------------------------------

/// The marker comment used to identify tokensave's section in a hook script.
const HOOK_MARKER: &str = "# tokensave: auto-sync";

/// The hook snippet appended to (or written as) the post-commit script.
fn post_commit_snippet(tokensave_bin: &str) -> String {
    format!(
        "{HOOK_MARKER}\n\
         {tokensave_bin} sync >/dev/null 2>&1 &\n"
    )
}

/// If a global git `post-commit` hook is not already set up for tokensave,
/// interactively asks the user whether to install one. Silently succeeds if
/// the hook is already present, if stdin is not a terminal, or if the user
/// declines.
pub fn offer_git_post_commit_hook(tokensave_bin: &str) {
    let Some(home) = home_dir() else { return };

    // Determine the global hooks directory by reading core.hooksPath from
    // the global gitconfig file(s). Falls back to ~/.config/git/hooks/.
    let hooks_dir = read_global_hooks_path(&home);

    let (hooks_dir, need_set_hookspath) = match hooks_dir {
        Some(dir) => (dir, false),
        None => (home.join(".config").join("git").join("hooks"), true),
    };

    let hook_path = hooks_dir.join("post-commit");

    // Check if already installed.
    if hook_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&hook_path) {
            if contents.contains(HOOK_MARKER) {
                eprintln!("  Global git post-commit hook already contains tokensave, skipping");
                return;
            }
        }
    }

    // Only prompt on a real terminal.
    if !atty_stdin() {
        return;
    }

    eprintln!();
    eprint!(
        "Install a global git post-commit hook to auto-run \x1b[1mtokensave sync\x1b[0m after each commit? [y/N] "
    );

    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return;
    }
    if !matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
        eprintln!("  Skipped git post-commit hook");
        return;
    }

    // Create the hooks directory if needed.
    if let Err(e) = std::fs::create_dir_all(&hooks_dir) {
        eprintln!("  \x1b[31m✘\x1b[0m Failed to create {}: {e}", hooks_dir.display());
        return;
    }

    // If no global hooksPath was configured, set it in ~/.gitconfig.
    if need_set_hookspath {
        let gitconfig_path = home.join(".gitconfig");
        if let Err(msg) = set_global_hooks_path(&gitconfig_path, &hooks_dir) {
            eprintln!("  \x1b[31m✘\x1b[0m {msg} — hook not installed");
            return;
        }
        eprintln!(
            "\x1b[32m✔\x1b[0m Set git core.hooksPath to {}",
            hooks_dir.display()
        );
    }

    // Append to or create the hook file.
    let snippet = post_commit_snippet(tokensave_bin);

    if hook_path.exists() {
        use std::io::Write;
        let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&hook_path) else {
            eprintln!(
                "  \x1b[31m✘\x1b[0m Failed to open {} for writing",
                hook_path.display()
            );
            return;
        };
        if write!(f, "\n{snippet}").is_err() {
            eprintln!("  \x1b[31m✘\x1b[0m Failed to write to {}", hook_path.display());
            return;
        }
    } else {
        let contents = format!("#!/bin/sh\n{snippet}");
        if std::fs::write(&hook_path, contents).is_err() {
            eprintln!(
                "  \x1b[31m✘\x1b[0m Failed to create {}",
                hook_path.display()
            );
            return;
        }
    }

    // Make executable (Unix).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755));
    }

    eprintln!(
        "\x1b[32m✔\x1b[0m Installed global git post-commit hook at {}",
        hook_path.display()
    );
}

/// Reads `core.hooksPath` from the global gitconfig files.
///
/// Checks `~/.gitconfig` first, then `~/.config/git/config` (the XDG
/// location). Returns the resolved absolute path, or `None` if the key
/// is absent from both files.
fn read_global_hooks_path(home: &Path) -> Option<PathBuf> {
    let candidates = [
        home.join(".gitconfig"),
        home.join(".config").join("git").join("config"),
    ];
    for path in &candidates {
        if let Some(value) = parse_gitconfig_value(path, "core", "hookspath") {
            let expanded = expand_tilde(&value, home);
            let p = PathBuf::from(&expanded);
            if p.is_absolute() {
                return Some(p);
            }
            // Relative paths in gitconfig are relative to the home dir.
            return Some(home.join(p));
        }
    }
    None
}

/// Minimal gitconfig parser: finds the value of `key` under `[section]`.
///
/// Key matching is case-insensitive (git config keys are case-insensitive).
/// Handles `key = value`, `key=value`, and quoted values.
fn parse_gitconfig_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    let section_lower = section.to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();

    let mut in_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // Parse section header: [core], [core "subsection"], etc.
            let header = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim();
            let section_name = header.split_whitespace().next().unwrap_or("");
            in_section = section_name.eq_ignore_ascii_case(&section_lower);
            continue;
        }
        if !in_section {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        // Parse key = value
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().to_ascii_lowercase() == key_lower {
                let v = v.trim();
                // Strip surrounding quotes if present.
                let v = v
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(v);
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Appends `core.hooksPath` to the global gitconfig file, creating it if
/// necessary. Appends to an existing `[core]` section if one exists,
/// otherwise adds a new one at the end of the file.
fn set_global_hooks_path(gitconfig_path: &Path, hooks_dir: &Path) -> std::result::Result<(), String> {
    let hooks_str = hooks_dir.to_string_lossy();
    let contents = if gitconfig_path.exists() {
        std::fs::read_to_string(gitconfig_path)
            .map_err(|e| format!("Failed to read {}: {e}", gitconfig_path.display()))?
    } else {
        String::new()
    };

    let new_contents = insert_gitconfig_value(&contents, "core", "hooksPath", &hooks_str);

    if let Some(parent) = gitconfig_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(gitconfig_path, new_contents)
        .map_err(|e| format!("Failed to write {}: {e}", gitconfig_path.display()))?;
    Ok(())
}

/// Inserts `key = value` under `[section]` in gitconfig content.
/// If the section exists, appends the key after the last line of that section.
/// Otherwise appends a new section at the end.
fn insert_gitconfig_value(contents: &str, section: &str, key: &str, value: &str) -> String {
    let section_lower = section.to_ascii_lowercase();
    let lines: Vec<&str> = contents.lines().collect();
    let mut result = Vec::with_capacity(lines.len() + 3);
    let entry = format!("\t{key} = {value}");

    // Find the target section and the line index just before the next section.
    let mut section_end: Option<usize> = None;
    let mut in_section = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_section {
                // We've hit the next section — insert before it.
                section_end = Some(i);
                break;
            }
            let header = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim();
            let name = header.split_whitespace().next().unwrap_or("");
            if name.eq_ignore_ascii_case(&section_lower) {
                in_section = true;
            }
        }
    }
    if in_section && section_end.is_none() {
        // Section runs to end of file.
        section_end = Some(lines.len());
    }

    if let Some(insert_at) = section_end {
        for (i, line) in lines.iter().enumerate() {
            if i == insert_at {
                result.push(entry.as_str());
            }
            result.push(line);
        }
        // If inserting at end-of-file.
        if insert_at == lines.len() {
            result.push(&entry);
        }
    } else {
        // Section doesn't exist — append it.
        for line in &lines {
            result.push(line);
        }
        if !contents.is_empty() && !contents.ends_with('\n') {
            result.push("");
        }
        let section_header = format!("[{section}]");
        // We need to own these strings for the result.
        // Re-build as a String directly instead.
        let mut out = result.join("\n");
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&section_header);
        out.push('\n');
        out.push_str(&entry);
        out.push('\n');
        return out;
    }

    let mut out = result.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Expand a leading `~` to the given home directory.
fn expand_tilde(s: &str, home: &Path) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        return home.join(rest).to_string_lossy().to_string();
    }
    if s == "~" {
        return home.to_string_lossy().to_string();
    }
    s.to_string()
}

/// Returns true if stdin is connected to a terminal.
fn atty_stdin() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod git_hook_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_hookspath_basic() {
        let config = "[core]\n\thooksPath = /home/user/.git-hooks\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            Some("/home/user/.git-hooks".to_string())
        );
    }

    #[test]
    fn parse_hookspath_quoted() {
        let config = "[core]\n\thooksPath = \"/home/user/my hooks\"\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            Some("/home/user/my hooks".to_string())
        );
    }

    #[test]
    fn parse_hookspath_case_insensitive() {
        let config = "[Core]\n\tHooksPath = /tmp/hooks\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            Some("/tmp/hooks".to_string())
        );
    }

    #[test]
    fn parse_hookspath_missing() {
        let config = "[core]\n\tautocrlf = true\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            None
        );
    }

    #[test]
    fn parse_hookspath_wrong_section() {
        let config = "[user]\n\thooksPath = /nope\n[core]\n\tautocrlf = true\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            None
        );
    }

    #[test]
    fn insert_into_existing_section() {
        let config = "[user]\n\tname = Test\n[core]\n\tautocrlf = true\n";
        let result = insert_gitconfig_value(config, "core", "hooksPath", "/tmp/hooks");
        assert!(result.contains("\thooksPath = /tmp/hooks"));
        assert!(result.contains("[core]"));
        assert!(result.contains("autocrlf = true"));
    }

    #[test]
    fn insert_new_section() {
        let config = "[user]\n\tname = Test\n";
        let result = insert_gitconfig_value(config, "core", "hooksPath", "/tmp/hooks");
        assert!(result.contains("[core]\n\thooksPath = /tmp/hooks"));
    }

    #[test]
    fn insert_into_empty_file() {
        let result = insert_gitconfig_value("", "core", "hooksPath", "/tmp/hooks");
        assert!(result.contains("[core]\n\thooksPath = /tmp/hooks"));
    }

    #[test]
    fn insert_before_next_section() {
        let config = "[core]\n\tautocrlf = true\n[user]\n\tname = Test\n";
        let result = insert_gitconfig_value(config, "core", "hooksPath", "/tmp/hooks");
        // hooksPath should appear after autocrlf but before [user]
        let hooks_pos = result.find("hooksPath").unwrap();
        let user_pos = result.find("[user]").unwrap();
        let autocrlf_pos = result.find("autocrlf").unwrap();
        assert!(hooks_pos > autocrlf_pos);
        assert!(hooks_pos < user_pos);
    }

    #[test]
    fn expand_tilde_with_slash() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde("~/hooks", home), "/home/test/hooks");
    }

    #[test]
    fn expand_tilde_bare() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde("~", home), "/home/test");
    }

    #[test]
    fn expand_tilde_no_tilde() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde("/abs/path", home), "/abs/path");
    }

    /// Helper: parse from a string directly (avoids file I/O in tests).
    fn parse_gitconfig_value_from_str(contents: &str, section: &str, key: &str) -> Option<String> {
        let section_lower = section.to_ascii_lowercase();
        let key_lower = key.to_ascii_lowercase();
        let mut in_section = false;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                let header = trimmed
                    .trim_start_matches('[')
                    .split(']')
                    .next()
                    .unwrap_or("")
                    .trim();
                let section_name = header.split_whitespace().next().unwrap_or("");
                in_section = section_name.eq_ignore_ascii_case(&section_lower);
                continue;
            }
            if !in_section { continue; }
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim().to_ascii_lowercase() == key_lower {
                    let v = v.trim();
                    let v = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(v);
                    return Some(v.to_string());
                }
            }
        }
        None
    }
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
