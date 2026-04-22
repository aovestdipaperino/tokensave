// Rust guideline compliant 2025-10-17
//! Agent integration layer for CLI tools (Claude Code, OpenCode, Codex, etc.).
//!
//! Each supported agent implements the [`AgentIntegration`] trait which provides
//! `install`, `uninstall`, and `healthcheck` operations. The MCP server
//! itself is agent-agnostic; this module handles the per-agent config
//! plumbing (registering the MCP server, permissions, hooks, prompt rules).

pub mod antigravity;
pub mod claude;
pub mod cline;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod kilo;
pub mod opencode;
pub mod roo_code;
pub mod zed;

use std::path::{Path, PathBuf};

use crate::errors::Result;
use crate::errors::TokenSaveError;

pub use antigravity::AntigravityIntegration;
pub use claude::ClaudeIntegration;
pub use cline::ClineIntegration;
pub use codex::CodexIntegration;
pub use copilot::CopilotIntegration;
pub use cursor::CursorIntegration;
pub use gemini::GeminiIntegration;
pub use kilo::KiloIntegration;
pub use opencode::OpenCodeIntegration;
pub use roo_code::RooCodeIntegration;
pub use zed::ZedIntegration;

// ---------------------------------------------------------------------------
// AgentIntegration trait
// ---------------------------------------------------------------------------

/// A CLI agent that can be configured to use tokensave via MCP.
pub trait AgentIntegration {
    /// Human-readable name (e.g. "Claude Code").
    fn name(&self) -> &'static str;

    /// CLI identifier used in `--agent <id>` (e.g. "claude").
    fn id(&self) -> &'static str;

    /// Register MCP server, permissions, hooks, and prompt rules.
    fn install(&self, ctx: &InstallContext) -> Result<()>;

    /// Remove everything installed by [`AgentIntegration::install`].
    fn uninstall(&self, ctx: &InstallContext) -> Result<()>;

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
}

/// Context passed to [`AgentIntegration::install`] and [`AgentIntegration::uninstall`].
pub struct InstallContext {
    pub home: PathBuf,
    pub tokensave_bin: String,
    pub tool_permissions: &'static [&'static str],
}

/// Context passed to [`AgentIntegration::healthcheck`].
pub struct HealthcheckContext {
    pub home: PathBuf,
    pub project_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Returns the agent matching `id`, or an error if unknown.
pub fn get_integration(id: &str) -> Result<Box<dyn AgentIntegration>> {
    match id {
        "claude" => Ok(Box::new(ClaudeIntegration)),
        "opencode" => Ok(Box::new(OpenCodeIntegration)),
        "codex" => Ok(Box::new(CodexIntegration)),
        "gemini" => Ok(Box::new(GeminiIntegration)),
        "copilot" => Ok(Box::new(CopilotIntegration)),
        "cursor" => Ok(Box::new(CursorIntegration)),
        "zed" => Ok(Box::new(ZedIntegration)),
        "cline" => Ok(Box::new(ClineIntegration)),
        "roo-code" => Ok(Box::new(RooCodeIntegration)),
        "antigravity" => Ok(Box::new(AntigravityIntegration)),
        "kilo" => Ok(Box::new(KiloIntegration)),
        _ => Err(TokenSaveError::Config {
            message: format!(
                "unknown agent: \"{id}\". Available agents: {}",
                available_integrations().join(", ")
            ),
        }),
    }
}

/// Returns all registered agents.
pub fn all_integrations() -> Vec<Box<dyn AgentIntegration>> {
    vec![
        Box::new(ClaudeIntegration),
        Box::new(OpenCodeIntegration),
        Box::new(CodexIntegration),
        Box::new(GeminiIntegration),
        Box::new(CopilotIntegration),
        Box::new(CursorIntegration),
        Box::new(ZedIntegration),
        Box::new(ClineIntegration),
        Box::new(RooCodeIntegration),
        Box::new(AntigravityIntegration),
        Box::new(KiloIntegration),
    ]
}

/// Returns the CLI identifiers of all registered agents (for help text).
pub fn available_integrations() -> Vec<&'static str> {
    vec![
        "claude",
        "opencode",
        "codex",
        "gemini",
        "copilot",
        "cursor",
        "zed",
        "cline",
        "roo-code",
        "antigravity",
        "kilo",
    ]
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

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Load a JSON file, returning an empty object on missing/invalid.
/// Use this for **read-only** paths (healthcheck, has_tokensave, etc.).
/// For install/edit paths, use [`load_json_file_strict`] instead.
pub fn load_json_file(path: &Path) -> serde_json::Value {
    if path.exists() {
        let contents = std::fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&contents).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

/// Load a JSON file for **editing**. Unlike [`load_json_file`], this returns
/// an error if the file exists but cannot be parsed, preventing silent data
/// loss when the modified value is written back.
///
/// # Error conditions
/// - File exists but is not readable (permissions, I/O error).
/// - File exists and has content but contains invalid JSON.
///
/// Returns `Ok(json!({}))` only when the file does not exist or is empty,
/// which is safe for creating a new config from scratch.
pub fn load_json_file_strict(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let contents = std::fs::read_to_string(path).map_err(|e| TokenSaveError::Config {
        message: format!("cannot read {}: {e}", path.display()),
    })?;
    if contents.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&contents).map_err(|e| TokenSaveError::Config {
        message: format!(
            "cannot parse {} as JSON: {e}\n  \
             Hint: fix the JSON syntax manually and re-run the command,\n  \
             or delete the file to start fresh",
            path.display()
        ),
    })
}

/// Create a backup copy of a config file before modifying it.
///
/// The backup itself is written atomically: content is first written to a
/// staging file (`.bak.new`), then renamed to `.bak`. This ensures the
/// `.bak` file is never half-written even if the process is killed.
///
/// Returns `Ok(Some(backup_path))` when a backup was created, or `Ok(None)`
/// when the file did not exist (nothing to back up).
///
/// # Error conditions
/// - File exists but cannot be read (permissions, I/O error).
/// - Staging file cannot be written (disk full, permissions).
/// - Staging file cannot be renamed to `.bak` (cross-device, permissions).
pub fn backup_config_file(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let backup_path = PathBuf::from(format!("{}.bak", path.display()));
    let staging_path = PathBuf::from(format!("{}.bak.new", path.display()));

    // Read original content
    let content = std::fs::read(path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to read {} for backup: {e}\n  \
             Hint: check file permissions",
            path.display()
        ),
    })?;

    // Write to staging file
    std::fs::write(&staging_path, &content).map_err(|e| {
        std::fs::remove_file(&staging_path).ok();
        TokenSaveError::Config {
            message: format!(
                "failed to write backup staging file {}: {e}\n  \
                 Hint: check available disk space and permissions",
                staging_path.display()
            ),
        }
    })?;

    // Atomic rename staging → .bak
    std::fs::rename(&staging_path, &backup_path).map_err(|e| {
        std::fs::remove_file(&staging_path).ok();
        TokenSaveError::Config {
            message: format!(
                "failed to create backup {}: {e}\n  \
                 Hint: check file permissions",
                backup_path.display()
            ),
        }
    })?;

    Ok(Some(backup_path))
}

/// Restore a config file from its backup. Prints instructions for manual
/// recovery if the restore itself fails.
pub fn restore_config_backup(original: &Path, backup: &Path) {
    match std::fs::copy(backup, original) {
        Ok(_) => {
            eprintln!(
                "\x1b[33m⚠\x1b[0m  Restored {} from backup",
                original.display()
            );
        }
        Err(e) => {
            eprintln!(
                "\x1b[31m✗\x1b[0m Failed to auto-restore {} from backup: {e}",
                original.display()
            );
            eprintln!(
                "  Manual recovery: cp '{}' '{}'",
                backup.display(),
                original.display()
            );
        }
    }
}

/// Write a JSON value to a file via atomic rename.
///
/// The caller is responsible for creating the backup via
/// [`backup_config_file`] before loading the config. Pass the backup path
/// here so that it can be mentioned in error messages and used for restore
/// if the rename somehow leaves the target in a bad state.
///
/// # Strategy
///
/// 1. Serialize → validate → write to a **new** sibling file (`.new`).
///    The original file is never opened for writing.
/// 2. `rename(new, original)` — on POSIX this is an atomic replace.
///    The old content disappears in a single syscall; there is no window
///    where the file is half-written.
/// 3. If rename fails (e.g. cross-device mount), the `.new` file is
///    cleaned up and the original is left **untouched**. No copy fallback
///    is attempted because copy is non-atomic and can leave the target
///    corrupted on interruption.
///
/// # Error conditions
/// - Serialization failure (should not happen with well-formed Values).
/// - Re-parse validation failure (internal bug).
/// - Cannot create parent directory.
/// - Cannot write the `.new` file (permissions, disk full).
/// - Cannot rename `.new` → target (cross-device, permissions).
///
/// In every error case the original file remains intact.
pub fn safe_write_json_file(
    path: &Path,
    value: &serde_json::Value,
    backup: Option<&Path>,
) -> Result<()> {
    // 1. Serialize
    let pretty = serde_json::to_string_pretty(value).map_err(|e| TokenSaveError::Config {
        message: format!("failed to serialize JSON for {}: {e}", path.display()),
    })?;

    // 2. Re-parse to verify the serialized output is valid JSON
    if serde_json::from_str::<serde_json::Value>(&pretty).is_err() {
        return Err(TokenSaveError::Config {
            message: format!(
                "internal error: serialized JSON for {} failed re-parse validation.\n  \
                 This is a bug in tokensave — please report it.",
                path.display()
            ),
        });
    }

    // 3. Ensure parent dir
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TokenSaveError::Config {
            message: format!("cannot create directory {}: {e}", parent.display()),
        })?;
    }

    // 4. Write to a NEW sibling file — the original is never opened for
    //    writing, so an interrupted write or crash only affects the .new file.
    let content = format!("{pretty}\n");
    let new_path = PathBuf::from(format!("{}.new", path.display()));
    if let Err(e) = std::fs::write(&new_path, &content) {
        std::fs::remove_file(&new_path).ok(); // clean up partial write
        return Err(TokenSaveError::Config {
            message: format!(
                "failed to write new config file {}: {e}",
                new_path.display()
            ),
        });
    }

    // 5. Atomic rename: new → original.
    //    On POSIX, rename(2) atomically replaces the target.
    //    If this fails the original file is still intact.
    if let Err(e) = std::fs::rename(&new_path, path) {
        std::fs::remove_file(&new_path).ok(); // clean up
        let hint = if let Some(b) = backup {
            format!(
                "\n  Backup is at: {}\n  \
                 The original file was NOT modified.",
                b.display()
            )
        } else {
            "\n  The original file was NOT modified.".to_string()
        };
        return Err(TokenSaveError::Config {
            message: format!(
                "failed to rename {} → {}: {e}{hint}",
                new_path.display(),
                path.display()
            ),
        });
    }

    Ok(())
}

/// Write a JSON value to a file with pretty formatting.
/// Creates a backup, writes atomically, and restores on failure.
pub fn write_json_file(path: &Path, value: &serde_json::Value) -> Result<()> {
    let backup = backup_config_file(path)?;
    safe_write_json_file(path, value, backup.as_deref())?;
    eprintln!("\x1b[32m✔\x1b[0m Wrote {}", path.display());
    Ok(())
}

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
            return Some(normalize_path_separators(&exe.to_string_lossy()));
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

/// Replace backslashes with forward slashes so paths work in JSON/shell
/// contexts on Windows. No-op on Unix where paths already use `/`.
fn normalize_path_separators(path: &str) -> String {
    path.replace('\\', "/")
}

/// Returns the user's home directory, cross-platform.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Strip `//` line comments, `/* */` block comments, and trailing commas
/// before `}` / `]` from a JSONC string, then parse with `serde_json`.
/// Falls back to `serde_json::json!({})` on any parse failure.
pub fn parse_jsonc(input: &str) -> serde_json::Value {
    let stripped = strip_jsonc_comments(input);
    serde_json::from_str(&stripped).unwrap_or_else(|_| serde_json::json!({}))
}

/// Internal helper: removes JSONC comments and trailing commas.
fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        // Handle string literals (skip comment stripping inside strings).
        if in_string {
            if chars[i] == '\\' && i + 1 < len {
                out.push(chars[i]);
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if chars[i] == '"' {
                in_string = false;
            }
            out.push(chars[i]);
            i += 1;
            continue;
        }

        // Start of string.
        if chars[i] == '"' {
            in_string = true;
            out.push(chars[i]);
            i += 1;
            continue;
        }

        // Line comment `//`.
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            // Skip until newline.
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment `/* ... */`.
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2; // consume `*/`
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    // Remove trailing commas before `}` or `]`.
    // Simple regex-free approach: repeatedly collapse ", <whitespace> }" patterns.
    remove_trailing_commas(&out)
}

/// Removes trailing commas that appear immediately before `}` or `]` (with
/// optional whitespace/newlines in between).
fn remove_trailing_commas(input: &str) -> String {
    // We scan for comma, optional whitespace, then `}` or `]`.
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = Vec::with_capacity(len);
    let mut i = 0;

    while i < len {
        if bytes[i] == b',' {
            // Peek ahead past whitespace.
            let mut j = i + 1;
            while j < len
                && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\n' || bytes[j] == b'\r')
            {
                j += 1;
            }
            if j < len && (bytes[j] == b'}' || bytes[j] == b']') {
                // Skip the comma; whitespace will be included normally.
                i += 1;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

/// Read a file and parse it as JSONC. Falls back to `json!({})` if the file
/// is missing, unreadable, or unparseable.
/// Use this for **read-only** paths. For install/edit paths, use
/// [`load_jsonc_file_strict`] instead.
pub fn load_jsonc_file(path: &Path) -> serde_json::Value {
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return serde_json::json!({}),
    };
    parse_jsonc(&contents)
}

/// Load a JSONC file for **editing**. Unlike [`load_jsonc_file`], this returns
/// an error if the file exists but cannot be parsed after comment stripping,
/// preventing silent data loss when the modified value is written back.
///
/// # Error conditions
/// - File exists but is not readable (permissions, I/O error).
/// - File exists and has content but contains invalid JSONC.
///
/// Returns `Ok(json!({}))` only when the file does not exist or is empty.
pub fn load_jsonc_file_strict(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let contents = std::fs::read_to_string(path).map_err(|e| TokenSaveError::Config {
        message: format!("cannot read {}: {e}", path.display()),
    })?;
    if contents.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let stripped = strip_jsonc_comments(&contents);
    serde_json::from_str(&stripped).map_err(|e| TokenSaveError::Config {
        message: format!(
            "cannot parse {} as JSONC: {e}\n  \
             Hint: fix the JSON syntax manually and re-run the command,\n  \
             or delete the file to start fresh",
            path.display()
        ),
    })
}

/// Returns the VS Code user data directory, platform-specific.
pub fn vscode_data_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config/Code")
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let appdata_path = PathBuf::from(&appdata);
            if appdata_path.starts_with(home) {
                return appdata_path.join("Code");
            }
        }
        home.join("AppData/Roaming/Code")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        home.join(".config/Code")
    }
}

/// Returns the GitHub Copilot CLI config directory.
pub fn copilot_cli_dir(home: &Path) -> PathBuf {
    home.join(".copilot")
}

/// Backfill `installed_agents` for users upgrading from older versions.
/// Scans all agents and checks if tokensave is already configured.
pub fn migrate_installed_agents(home: &Path, config: &mut crate::user_config::UserConfig) {
    if !config.installed_agents.is_empty() {
        return; // already populated
    }
    let mut found = Vec::new();
    for ag in all_integrations() {
        if ag.has_tokensave(home) {
            found.push(ag.id().to_string());
        }
    }
    if !found.is_empty() {
        config.installed_agents = found;
        config.save();
    }
}

/// Interactively pick which agents to install/uninstall.
///
/// - 0 detected agents → returns an error.
/// - 1 detected and not already installed → returns it directly (no prompt).
/// - Otherwise → asks a Y/n question for each detected agent.
///
/// Returns `(to_install, to_uninstall)`.
pub fn pick_integrations_interactive(
    home: &Path,
    installed: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    let detected: Vec<Box<dyn AgentIntegration>> = all_integrations()
        .into_iter()
        .filter(|ag| ag.is_detected(home))
        .collect();

    if detected.is_empty() {
        return Err(TokenSaveError::Config {
            message: "No supported agents detected on this system".to_string(),
        });
    }

    // Fast path: exactly one detected agent and it isn't installed yet.
    if detected.len() == 1 && !installed.contains(&detected[0].id().to_string()) {
        let id = detected[0].id().to_string();
        return Ok((vec![id], vec![]));
    }

    let mut to_install = Vec::new();
    let mut to_uninstall = Vec::new();

    for ag in &detected {
        let id = ag.id().to_string();
        let already = installed.contains(&id);
        if already {
            eprint!("Keep tokensave for {}? [Y/n] ", ag.name());
        } else {
            eprint!("Install tokensave for {}? [Y/n] ", ag.name());
        }

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to read input: {e}"),
            })?;
        let answer = input.trim().to_lowercase();
        let yes = answer.is_empty() || answer == "y" || answer == "yes";

        if yes && !already {
            to_install.push(id);
        } else if !yes && already {
            to_uninstall.push(id);
        }
    }

    Ok((to_install, to_uninstall))
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
    let contents = toml::to_string_pretty(value).unwrap_or_else(|_| String::new());
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
    let bin = tokensave_bin.replace('\\', "/");
    format!(
        "{HOOK_MARKER}\n\
         {bin} sync >/dev/null 2>&1 &\n"
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
        eprintln!(
            "  \x1b[31m✘\x1b[0m Failed to create {}: {e}",
            hooks_dir.display()
        );
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
            eprintln!(
                "  \x1b[31m✘\x1b[0m Failed to write to {}",
                hook_path.display()
            );
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
fn set_global_hooks_path(
    gitconfig_path: &Path,
    hooks_dir: &Path,
) -> std::result::Result<(), String> {
    let hooks_str = hooks_dir.to_string_lossy().replace('\\', "/");
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
        return home.join(rest).to_string_lossy().replace('\\', "/");
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
#[allow(clippy::unwrap_used)]
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
            if !in_section {
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim().to_ascii_lowercase() == key_lower {
                    let v = v.trim();
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod jsonc_tests {
    use super::*;

    #[test]
    fn parse_jsonc_plain_json() {
        let input = r#"{"key": "value", "num": 42}"#;
        let v = parse_jsonc(input);
        assert_eq!(v["key"], "value");
        assert_eq!(v["num"], 42);
    }

    #[test]
    fn parse_jsonc_line_comment() {
        let input = "{\n  // this is a comment\n  \"key\": \"val\"\n}";
        let v = parse_jsonc(input);
        assert_eq!(v["key"], "val");
    }

    #[test]
    fn parse_jsonc_block_comment() {
        let input = "{ /* block comment */ \"key\": \"val\" }";
        let v = parse_jsonc(input);
        assert_eq!(v["key"], "val");
    }

    #[test]
    fn parse_jsonc_trailing_comma_object() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let v = parse_jsonc(input);
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn parse_jsonc_trailing_comma_array() {
        let input = r#"{"items": [1, 2, 3,]}"#;
        let v = parse_jsonc(input);
        assert_eq!(v["items"][2], 3);
    }

    #[test]
    fn parse_jsonc_combined() {
        let input = "{\n  // comment\n  \"x\": /* inline */ 99,\n}";
        let v = parse_jsonc(input);
        assert_eq!(v["x"], 99);
    }

    #[test]
    fn parse_jsonc_url_in_string_not_stripped() {
        // A URL containing `//` inside a string must NOT be treated as a comment.
        let input = r#"{"url": "https://example.com/path"}"#;
        let v = parse_jsonc(input);
        assert_eq!(v["url"], "https://example.com/path");
    }

    #[test]
    fn parse_jsonc_invalid_falls_back_to_empty() {
        let input = "not valid json at all !!!";
        let v = parse_jsonc(input);
        assert_eq!(v, serde_json::json!({}));
    }

    #[test]
    fn parse_jsonc_empty_string() {
        let v = parse_jsonc("");
        assert_eq!(v, serde_json::json!({}));
    }

    #[test]
    fn parse_jsonc_trailing_comma_with_whitespace() {
        let input = "{\n  \"a\": 1  ,\n}";
        let v = parse_jsonc(input);
        assert_eq!(v["a"], 1);
    }
}

// ---------------------------------------------------------------------------
// Regression tests for safe config backup / load / write
// ---------------------------------------------------------------------------
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod safe_config_tests {
    use super::*;
    use std::fs;

    /// Create a temp directory that is cleaned up on drop.
    fn tmpdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    // ----- backup_config_file -----

    #[test]
    fn backup_returns_none_when_file_missing() {
        let dir = tmpdir();
        let path = dir.path().join("nonexistent.json");
        let result = backup_config_file(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn backup_creates_bak_with_identical_content() {
        let dir = tmpdir();
        let path = dir.path().join("config.json");
        let original = r#"{"existing": "data", "nested": {"key": 1}}"#;
        fs::write(&path, original).unwrap();

        let backup = backup_config_file(&path)
            .unwrap()
            .expect("should create backup");
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        // Original is untouched
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn backup_staging_file_is_cleaned_up() {
        let dir = tmpdir();
        let path = dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        backup_config_file(&path).unwrap();

        let staging = dir.path().join("config.json.bak.new");
        assert!(!staging.exists(), ".bak.new staging file should be removed");
    }

    // ----- load_json_file_strict -----

    #[test]
    fn strict_load_returns_empty_for_missing_file() {
        let dir = tmpdir();
        let path = dir.path().join("nope.json");
        let val = load_json_file_strict(&path).unwrap();
        assert_eq!(val, serde_json::json!({}));
    }

    #[test]
    fn strict_load_returns_empty_for_blank_file() {
        let dir = tmpdir();
        let path = dir.path().join("empty.json");
        fs::write(&path, "   \n  ").unwrap();
        let val = load_json_file_strict(&path).unwrap();
        assert_eq!(val, serde_json::json!({}));
    }

    #[test]
    fn strict_load_parses_valid_json() {
        let dir = tmpdir();
        let path = dir.path().join("valid.json");
        fs::write(&path, r#"{"hello": "world", "n": 42}"#).unwrap();
        let val = load_json_file_strict(&path).unwrap();
        assert_eq!(val["hello"], "world");
        assert_eq!(val["n"], 42);
    }

    #[test]
    fn strict_load_errors_on_invalid_json() {
        let dir = tmpdir();
        let path = dir.path().join("bad.json");
        fs::write(&path, "not json {{{").unwrap();
        let err = load_json_file_strict(&path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cannot parse"), "error: {msg}");
        assert!(
            msg.contains("bad.json"),
            "error should mention filename: {msg}"
        );
    }

    #[test]
    fn strict_load_errors_on_truncated_json() {
        let dir = tmpdir();
        let path = dir.path().join("trunc.json");
        fs::write(&path, r#"{"key": "value", "incomplete"#).unwrap();
        assert!(load_json_file_strict(&path).is_err());
    }

    // ----- load_jsonc_file_strict -----

    #[test]
    fn strict_jsonc_load_returns_empty_for_missing() {
        let dir = tmpdir();
        let path = dir.path().join("nope.jsonc");
        let val = load_jsonc_file_strict(&path).unwrap();
        assert_eq!(val, serde_json::json!({}));
    }

    #[test]
    fn strict_jsonc_load_parses_valid_jsonc() {
        let dir = tmpdir();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            "{\n  // comment\n  \"key\": \"val\",\n  /* block */ \"n\": 1,\n}",
        )
        .unwrap();
        let val = load_jsonc_file_strict(&path).unwrap();
        assert_eq!(val["key"], "val");
        assert_eq!(val["n"], 1);
    }

    #[test]
    fn strict_jsonc_load_errors_on_garbage() {
        let dir = tmpdir();
        let path = dir.path().join("garbage.json");
        fs::write(&path, "totally not json or jsonc !!!").unwrap();
        let err = load_jsonc_file_strict(&path).unwrap_err();
        assert!(err.to_string().contains("cannot parse"));
    }

    // ----- safe_write_json_file -----

    #[test]
    fn safe_write_creates_file_from_scratch() {
        let dir = tmpdir();
        let path = dir.path().join("new.json");
        let value = serde_json::json!({"created": true});
        safe_write_json_file(&path, &value, None).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(parsed["created"], true);
    }

    #[test]
    fn safe_write_replaces_existing_file_atomically() {
        let dir = tmpdir();
        let path = dir.path().join("existing.json");
        fs::write(&path, r#"{"old": true}"#).unwrap();

        let value = serde_json::json!({"new": true});
        safe_write_json_file(&path, &value, None).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed["new"], true);
        assert!(parsed.get("old").is_none());
    }

    #[test]
    fn safe_write_cleans_up_new_file_on_success() {
        let dir = tmpdir();
        let path = dir.path().join("config.json");
        safe_write_json_file(&path, &serde_json::json!({}), None).unwrap();

        let new_path = dir.path().join("config.json.new");
        assert!(!new_path.exists(), ".new staging file should be removed");
    }

    #[test]
    fn safe_write_creates_parent_dirs() {
        let dir = tmpdir();
        let path = dir.path().join("deep").join("nested").join("config.json");
        safe_write_json_file(&path, &serde_json::json!({"deep": true}), None).unwrap();
        assert!(path.exists());
    }

    // ----- write_json_file (convenience wrapper) -----

    #[test]
    fn write_json_file_creates_backup_automatically() {
        let dir = tmpdir();
        let path = dir.path().join("auto.json");
        fs::write(&path, r#"{"original": true}"#).unwrap();

        write_json_file(&path, &serde_json::json!({"updated": true})).unwrap();

        // .bak should exist with original content
        let bak = dir.path().join("auto.json.bak");
        assert!(bak.exists());
        let backup_content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&bak).unwrap()).unwrap();
        assert_eq!(backup_content["original"], true);
    }

    // ----- THE KEY REGRESSION TEST -----
    // This is the exact bug the fix addresses: load_json_file silently
    // returned {} on parse failure, and the install wrote {} + tokensave
    // back, destroying the user's config.

    #[test]
    fn invalid_json_is_never_silently_replaced() {
        let dir = tmpdir();
        let path = dir.path().join("opencode.json");
        // Simulate a file that serde_json can't parse (e.g. has trailing commas
        // that the non-strict loader would silently drop).
        let corrupted =
            r#"{"mcp": {"other_server": {"url": "http://example.com"},}, "theme": "dark",}"#;
        fs::write(&path, corrupted).unwrap();

        // The strict loader must refuse to parse this.
        let err = load_json_file_strict(&path);
        assert!(err.is_err(), "strict loader must reject invalid JSON");

        // The original file must be completely untouched.
        assert_eq!(fs::read_to_string(&path).unwrap(), corrupted);

        // Contrast: the old non-strict loader silently returns {} — this
        // is the exact behavior that destroyed configs.
        let old_style = load_json_file(&path);
        assert_eq!(
            old_style,
            serde_json::json!({}),
            "non-strict loader returns empty"
        );
    }

    #[test]
    fn full_install_cycle_preserves_existing_config() {
        // Simulate the full install cycle: backup → strict load → mutate → safe write.
        // Existing keys must be preserved.
        let dir = tmpdir();
        let path = dir.path().join("config.json");
        let original = serde_json::json!({
            "theme": "dark",
            "mcp": {
                "existing_server": {"url": "http://localhost:8080"}
            },
            "other_setting": [1, 2, 3]
        });
        fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        // Simulate install
        let backup = backup_config_file(&path).unwrap();
        let mut config = load_json_file_strict(&path).unwrap();
        config["mcp"]["tokensave"] = serde_json::json!({
            "type": "local",
            "command": ["tokensave", "serve"]
        });
        safe_write_json_file(&path, &config, backup.as_deref()).unwrap();

        // Verify
        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        // Tokensave was added
        assert!(result["mcp"]["tokensave"].is_object());
        // Existing keys survived
        assert_eq!(result["theme"], "dark");
        assert_eq!(
            result["mcp"]["existing_server"]["url"],
            "http://localhost:8080"
        );
        assert_eq!(result["other_setting"], serde_json::json!([1, 2, 3]));

        // Backup exists with original content
        let bak_content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(backup.unwrap()).unwrap()).unwrap();
        assert!(bak_content.get("tokensave").is_none());
        assert_eq!(bak_content["theme"], "dark");
    }

    #[test]
    fn full_install_cycle_aborts_on_corrupt_file() {
        // If the existing config is corrupt, the install must fail without
        // touching the file. This is the core regression test.
        let dir = tmpdir();
        let path = dir.path().join("config.json");
        let corrupt_content = "{ this is not valid json at all }}}";
        fs::write(&path, corrupt_content).unwrap();

        // Backup succeeds (it just copies bytes)
        let backup = backup_config_file(&path).unwrap();
        assert!(backup.is_some());

        // Strict load fails
        let err = load_json_file_strict(&path);
        assert!(err.is_err());

        // Original file is byte-for-byte unchanged
        assert_eq!(fs::read_to_string(&path).unwrap(), corrupt_content);
        // Backup also has the same content
        assert_eq!(
            fs::read_to_string(backup.unwrap()).unwrap(),
            corrupt_content
        );
    }

    #[test]
    fn safe_write_output_is_valid_json() {
        // Verify the written file is always parseable JSON (round-trip).
        let dir = tmpdir();
        let path = dir.path().join("roundtrip.json");
        let value = serde_json::json!({
            "unicode": "héllo wörld 🦀",
            "nested": {"deep": {"array": [1, null, true, "str"]}},
            "empty_obj": {},
            "empty_arr": []
        });

        safe_write_json_file(&path, &value, None).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        let reparsed: serde_json::Value =
            serde_json::from_str(&raw).expect("written file must be valid JSON");
        assert_eq!(reparsed, value);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod path_normalize_tests {
    use super::*;

    #[test]
    fn normalizes_windows_backslashes() {
        assert_eq!(
            normalize_path_separators(r"C:\Users\dev\scoop\shims\tokensave.exe"),
            "C:/Users/dev/scoop/shims/tokensave.exe"
        );
    }

    #[test]
    fn leaves_unix_paths_unchanged() {
        assert_eq!(
            normalize_path_separators("/usr/local/bin/tokensave"),
            "/usr/local/bin/tokensave"
        );
    }
}
