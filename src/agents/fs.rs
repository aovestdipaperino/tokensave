// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

use crate::errors::TokenSaveError;
use std::path::{Path, PathBuf};

/// Load a JSON file, returning an empty object on missing/invalid.
/// Use this for **read-only** paths (healthcheck, `has_tokensave`, etc.).
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
pub fn load_json_file_strict(path: &Path) -> crate::errors::Result<serde_json::Value> {
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
pub fn backup_config_file(path: &Path) -> crate::errors::Result<Option<PathBuf>> {
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
) -> crate::errors::Result<()> {
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

    // 3. Skip a write that would not change a byte (#419). Reading `path`
    //    follows a symlink, so this compares against the same content the
    //    rename below would replace. Direct callers back up before calling in,
    //    so their `.bak` is already on disk by now — the two entry points in
    //    this module guard earlier and avoid it entirely.
    if std::fs::read_to_string(path).is_ok_and(|on_disk| on_disk == format!("{pretty}\n")) {
        return Ok(());
    }

    // 4. Resolve symlinks. If `path` (e.g. `~/.claude/settings.json`) is a
    //    symlink — common for dotfiles setups that track config in a repo and
    //    symlink it into place — `rename()` over `path` would delete the
    //    symlink and drop a plain file in its stead, silently detaching the
    //    live config from the dotfiles source. Write through the symlink to
    //    its real target instead, so the target gets updated and the symlink
    //    survives untouched. If the chain can't be resolved safely (cycle,
    //    unreadable link, pathological depth), bail out here rather than
    //    falling back to `path` — writing there would rename over the
    //    symlink itself, the exact destruction this function exists to
    //    prevent.
    let real_path = resolve_symlink_target(path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "cannot safely resolve symlink {}: {e}\n  \
             Refusing to write — the symlink was left untouched.",
            path.display()
        ),
    })?;

    // 5. Ensure parent dir
    if let Some(parent) = real_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TokenSaveError::Config {
            message: format!("cannot create directory {}: {e}", parent.display()),
        })?;
    }

    // 6. Write to a NEW sibling file — the original is never opened for
    //    writing, so an interrupted write or crash only affects the .new file.
    //    Staged next to `real_path` (not `path`) so the rename in step 7 stays
    //    on the same filesystem and remains atomic.
    let content = format!("{pretty}\n");
    let new_path = PathBuf::from(format!("{}.new", real_path.display()));
    if let Err(e) = std::fs::write(&new_path, &content) {
        std::fs::remove_file(&new_path).ok(); // clean up partial write
        return Err(TokenSaveError::Config {
            message: format!(
                "failed to write new config file {}: {e}",
                new_path.display()
            ),
        });
    }

    // 7. Atomic rename: new → real target.
    //    On POSIX, rename(2) atomically replaces the target.
    //    If this fails the original file is still intact.
    if let Err(e) = std::fs::rename(&new_path, &real_path) {
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
                real_path.display()
            ),
        });
    }

    Ok(())
}

/// Write `content` to `path` atomically, mirroring [`safe_write_json_file`]'s
/// symlink-safe temp-file-then-rename pattern for plain text: resolves a
/// symlinked `path` to its real target (so a dotfiles-managed symlink
/// survives instead of being replaced by a plain file), then writes to a
/// sibling `.new` file and renames it into place. A failure at either step —
/// e.g. disk exhaustion mid-write — leaves the original file untouched
/// instead of a partially-written, truncated one.
pub(crate) fn safe_write_text_file(path: &Path, content: &str) -> crate::errors::Result<()> {
    let real_path = resolve_symlink_target(path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "cannot safely resolve symlink {}: {e}\n  \
             Refusing to write — the symlink was left untouched.",
            path.display()
        ),
    })?;

    if let Some(parent) = real_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TokenSaveError::Config {
            message: format!("cannot create directory {}: {e}", parent.display()),
        })?;
    }

    let new_path = PathBuf::from(format!("{}.new", real_path.display()));
    if let Err(e) = std::fs::write(&new_path, content) {
        std::fs::remove_file(&new_path).ok(); // clean up partial write
        return Err(TokenSaveError::Config {
            message: format!("failed to write new file {}: {e}", new_path.display()),
        });
    }

    if let Err(e) = std::fs::rename(&new_path, &real_path) {
        std::fs::remove_file(&new_path).ok(); // clean up
        return Err(TokenSaveError::Config {
            message: format!(
                "failed to rename {} → {}: {e}\n  The original file was NOT modified.",
                new_path.display(),
                real_path.display()
            ),
        });
    }

    Ok(())
}

/// Resolves `path` to the file it should actually be written to.
///
/// If `path` is not a symlink, returns it unchanged. If it is a symlink,
/// resolves the full chain to its real target via [`std::fs::canonicalize`].
/// `canonicalize` fails whenever any hop in the chain is dangling — including
/// a *multi-hop* chain where an intermediate link (not just the final one)
/// points at something that doesn't exist yet (e.g. a dotfiles repo cloned
/// but not yet fully materialized). In that case, walk the chain manually,
/// one `read_link` hop at a time, until reaching a path that is not itself a
/// symlink — that terminal path is where the write should land, so every
/// symlink in the chain survives untouched.
///
/// Returns `Err` on a cycle, an unreadable link, or a chain deeper than
/// [`MAX_SYMLINK_HOPS`] — deliberately *not* falling back to `path` in that
/// case. Falling back would make the caller write (and atomically rename)
/// straight onto the symlink itself, destroying it — the exact bug this
/// function exists to prevent, just reached through a different route.
pub(crate) fn resolve_symlink_target(path: &Path) -> std::result::Result<PathBuf, String> {
    let is_symlink = std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    if !is_symlink {
        return Ok(path.to_path_buf());
    }
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }
    walk_dangling_symlink_chain(path)
}

/// Matches the ELOOP hop limit most platforms enforce for real filesystem
/// symlink resolution — a generous ceiling for pathological (but acyclic)
/// chains, while `seen` below catches cycles long before this is reached.
const MAX_SYMLINK_HOPS: usize = 40;

/// Follows a symlink chain hop by hop via `read_link`, resolving each
/// relative target against its link's parent directory, until it reaches a
/// path that is not itself a symlink (this includes a path that doesn't
/// exist at all — the common "target not created yet" case, which is the
/// terminal write destination). Returns `Err` on a cycle, an unresolvable
/// hop, or exceeding [`MAX_SYMLINK_HOPS`].
fn walk_dangling_symlink_chain(path: &Path) -> std::result::Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    let mut seen = std::collections::HashSet::new();
    let mut hops = 0usize;
    loop {
        if !seen.insert(current.clone()) {
            return Err(format!(
                "symlink cycle detected at {} while resolving {}",
                current.display(),
                path.display()
            ));
        }
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                // The hop budget is checked *before following*, not before
                // checking terminality — so a chain of exactly
                // MAX_SYMLINK_HOPS symlinks that lands on a terminal path
                // still resolves; only a chain that needs one more hop past
                // the budget is rejected.
                if hops >= MAX_SYMLINK_HOPS {
                    return Err(format!(
                        "symlink chain from {} exceeds {MAX_SYMLINK_HOPS} hops",
                        path.display()
                    ));
                }
                hops += 1;
                let link_target = std::fs::read_link(&current)
                    .map_err(|e| format!("cannot read symlink {}: {e}", current.display()))?;
                current = if link_target.is_absolute() {
                    link_target
                } else {
                    current
                        .parent()
                        .ok_or_else(|| {
                            format!("symlink {} has no parent directory", current.display())
                        })?
                        .join(&link_target)
                };
            }
            // Not a symlink (regular file) or doesn't exist at all: terminal.
            _ => return Ok(current),
        }
    }
}

/// Render `value` exactly as [`safe_write_json_file`] would write it.
///
/// Kept next to the writer because the two must not drift: a mismatch here
/// makes [`json_file_is_current`] report a difference that does not exist,
/// and every no-op write comes back.
fn render_json(value: &serde_json::Value) -> Option<String> {
    serde_json::to_string_pretty(value)
        .ok()
        .map(|pretty| format!("{pretty}\n"))
}

/// True when `path` already holds byte-for-byte what writing `value` would
/// produce, so the write can be skipped entirely.
///
/// Issue #419: `tokensave gitignore` with no ACTION is documented as a query,
/// and it rewrote `~/.claude/settings.json` with byte-identical content —
/// advancing its mtime, dropping a fresh `.bak` beside it, and printing
/// `✔ Wrote …`. Nothing was lost that time, but rewriting the file that holds
/// the agent's permissions and hooks is not a neutral act: it races anything
/// else with the file open, and an interruption mid-write turns a no-op into a
/// truncated settings file. A user running a query has quiesced nothing,
/// because they had no reason to expect a write.
///
/// Comparison is on the rendered bytes rather than parsed JSON, deliberately.
/// Writing normalises key order, indentation and the trailing newline, so
/// re-rendering a semantically equal file can still change it on disk — and a
/// formatting-only rewrite is a real change to a file the user may track in
/// git. Only an exact match is a no-op.
///
/// Returns `false` when the file is missing or unreadable: the caller should
/// go ahead and write, and find out from the write itself.
pub fn json_file_is_current(path: &Path, value: &serde_json::Value) -> bool {
    let Some(rendered) = render_json(value) else {
        return false;
    };
    std::fs::read_to_string(path).is_ok_and(|on_disk| on_disk == rendered)
}

/// Write a JSON value to a file with pretty formatting.
/// Creates a backup, writes atomically, and restores on failure.
///
/// A write that would not change a byte is skipped: no backup, no rename, no
/// `✔ Wrote` line (#419). The guard sits here rather than inside
/// [`safe_write_json_file`] because the backup is taken first, and a spurious
/// `.bak` was part of what #419 reported.
pub fn write_json_file(path: &Path, value: &serde_json::Value) -> crate::errors::Result<()> {
    if json_file_is_current(path, value) {
        return Ok(());
    }
    let backup = backup_config_file(path)?;
    safe_write_json_file(path, value, backup.as_deref())?;
    // Agent-install progress, so it honours quiet mode like every other line
    // the silent upgrade resync suppresses. A raw `eprintln!` here is what let
    // that resync print a bare `✔ Wrote ~/.claude/settings.json` from under a
    // command the user had run as a query (#419).
    crate::agent_note!("\x1b[32m✔\x1b[0m Wrote {}", path.display());
    Ok(())
}

/// Best-effort "back up and write" for uninstall paths.
///
/// Mirrors the install pattern (`backup_config_file` then
/// `safe_write_json_file`) but swallows errors so the rest of the uninstall
/// can continue. Returns `true` when the new content reached disk.
///
/// Issue #63: every config rewrite must leave a `.bak` so the user can
/// recover if anything goes wrong.
pub fn backup_and_write_json(path: &Path, value: &serde_json::Value) -> bool {
    // Nothing to write means nothing reached disk, so this returns `false` and
    // callers keep quiet — every caller already gates its `✔ Wrote` message on
    // the return value (#419).
    if json_file_is_current(path, value) {
        return false;
    }
    let backup = backup_config_file(path).ok().flatten();
    safe_write_json_file(path, value, backup.as_deref()).is_ok()
}

/// Replace backslashes with forward slashes so paths work in JSON/shell
/// contexts on Windows. No-op on Unix where paths already use `/`.
pub(crate) fn normalize_path_separators(path: &str) -> String {
    path.replace('\\', "/")
}

/// Returns the user's home directory, cross-platform.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Expand a leading `~` to the given home directory.
pub(crate) fn expand_tilde(s: &str, home: &Path) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        return home.join(rest).to_string_lossy().replace('\\', "/");
    }
    if s == "~" {
        return home.to_string_lossy().to_string();
    }
    s.to_string()
}

/// Strip `//` line comments, `/* */` block comments, and trailing commas
/// before `}` / `]` from a JSONC string, then parse with `serde_json`.
/// Falls back to `serde_json::json!({})` on any parse failure.
pub fn parse_jsonc(input: &str) -> serde_json::Value {
    let stripped = strip_jsonc_comments(input);
    serde_json::from_str(&stripped).unwrap_or_else(|_| serde_json::json!({}))
}

/// Read a file and parse it as JSONC. Falls back to `json!({})` if the file
/// is missing, unreadable, or unparseable.
/// Use this for **read-only** paths. For install/edit paths, use
/// [`load_jsonc_file_strict`] instead.
pub fn load_jsonc_file(path: &Path) -> serde_json::Value {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return serde_json::json!({});
    };
    parse_jsonc(&contents)
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

/// Load a JSONC file for **editing**. Unlike [`load_jsonc_file`], this returns
/// an error if the file exists but cannot be parsed after comment stripping,
/// preventing silent data loss when the modified value is written back.
///
/// # Error conditions
/// - File exists but is not readable (permissions, I/O error).
/// - File exists and has content but contains invalid JSONC.
///
/// Returns `Ok(json!({}))` only when the file does not exist or is empty.
pub fn load_jsonc_file_strict(path: &Path) -> crate::errors::Result<serde_json::Value> {
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

/// Returns the platform-specific VS Code Insiders data directory.
pub fn vscode_insiders_data_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code - Insiders")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config/Code - Insiders")
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let appdata_path = PathBuf::from(&appdata);
            if appdata_path.starts_with(home) {
                return appdata_path.join("Code - Insiders");
            }
        }
        home.join("AppData/Roaming/Code - Insiders")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        home.join(".config/Code - Insiders")
    }
}

/// Returns the GitHub Copilot CLI config directory.
pub fn copilot_cli_dir(home: &Path) -> PathBuf {
    home.join(".copilot")
}

/// Returns the GitHub Copilot `JetBrains` plugin config directory.
///
/// The `JetBrains` plugin stores its MCP config (`mcp.json`) and global
/// instructions under `~/.config/github-copilot/intellij` on macOS and
/// Linux (XDG-style even on macOS), and under
/// `%LOCALAPPDATA%\github-copilot\intellij` on Windows.
pub fn copilot_jetbrains_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let localappdata_path = PathBuf::from(&localappdata);
            if localappdata_path.starts_with(home) {
                return localappdata_path.join("github-copilot/intellij");
            }
        }
        home.join("AppData/Local/github-copilot/intellij")
    }
    #[cfg(not(target_os = "windows"))]
    {
        home.join(".config/github-copilot/intellij")
    }
}

/// Load a TOML file as a document.
///
/// Returns an empty table when the file does not exist. When the file exists
/// but cannot be parsed as a TOML document, returns a [`TokenSaveError::Config`]
/// so callers do not silently overwrite the user's data (see issue #63).
pub fn load_toml_file(path: &Path) -> crate::errors::Result<toml::Value> {
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let contents = std::fs::read_to_string(path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    if contents.trim().is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    // NOTE: `str.parse::<toml::Value>()` parses a single TOML value in toml v1,
    // not a document — using it here would treat any well-formed config.toml as
    // unparseable and silently drop its contents. Use `toml::from_str` instead.
    let table: toml::Table = toml::from_str(&contents).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to parse {} as TOML: {e}. Refusing to overwrite — fix the file or remove it manually.",
            path.display()
        ),
    })?;
    Ok(toml::Value::Table(table))
}

/// Copy `path` to `<path>.bak` if it exists. Used before overwriting a user
/// config so an unexpected change is recoverable (issue #63).
fn backup_file(path: &Path) -> crate::errors::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut backup = path.as_os_str().to_owned();
    backup.push(".bak");
    let backup = std::path::PathBuf::from(backup);
    std::fs::copy(path, &backup).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to back up {} to {}: {e}",
            path.display(),
            backup.display()
        ),
    })?;
    eprintln!(
        "\x1b[32m✔\x1b[0m Backed up {} to {}",
        path.display(),
        backup.display()
    );
    Ok(())
}

/// Write a TOML value to a file, backing up any existing file first.
pub fn write_toml_file(path: &Path, value: &toml::Value) -> crate::errors::Result<()> {
    backup_file(path)?;
    let contents = toml::to_string_pretty(value).unwrap_or_else(|_| String::new());
    std::fs::write(path, contents).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write {}: {e}", path.display()),
    })?;
    crate::agent_note!("\x1b[32m✔\x1b[0m Wrote {}", path.display());
    Ok(())
}

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
    use crate::agents::fs::MAX_SYMLINK_HOPS;
    use crate::agents::*;
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

    // ----- symlink handling (dotfiles use case, issue: settings.json
    //       symlinked into a dotfiles repo was replaced by a plain file) -----

    #[test]
    #[cfg(unix)]
    fn safe_write_through_symlink_preserves_link_and_updates_target() {
        use std::os::unix::fs::symlink;

        let dir = tmpdir();
        let target = dir.path().join("real_target.json");
        fs::write(&target, r#"{"old": true}"#).unwrap();

        let link = dir.path().join("settings.json");
        symlink(&target, &link).unwrap();

        safe_write_json_file(&link, &serde_json::json!({"new": true}), None).unwrap();

        // The symlink itself must still be a symlink pointing at the target.
        let meta = fs::symlink_metadata(&link).unwrap();
        assert!(
            meta.file_type().is_symlink(),
            "link.json should remain a symlink"
        );
        assert_eq!(fs::read_link(&link).unwrap(), target);

        // The real target must contain the new content.
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(parsed["new"], true);
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_symlink_target_in_other_dir() {
        use std::os::unix::fs::symlink;

        let link_dir = tmpdir();
        let target_dir = tmpdir();
        let target = target_dir.path().join("dotfiles_settings.json");
        fs::write(&target, r#"{"old": true}"#).unwrap();

        let link = link_dir.path().join("settings.json");
        symlink(&target, &link).unwrap();

        safe_write_json_file(&link, &serde_json::json!({"new": true}), None).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(parsed["new"], true);

        // No leftover staging file next to the symlink or the target.
        assert!(!target_dir
            .path()
            .join("dotfiles_settings.json.new")
            .exists());
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_broken_symlink_creates_target() {
        use std::os::unix::fs::symlink;

        let dir = tmpdir();
        let target = dir.path().join("not_yet_created.json");
        let link = dir.path().join("settings.json");
        symlink(&target, &link).unwrap(); // target does not exist yet

        safe_write_json_file(&link, &serde_json::json!({"created": true}), None).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(parsed["created"], true);
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_multi_hop_dangling_chain_preserves_every_hop() {
        use std::os::unix::fs::symlink;

        let dir = tmpdir();
        let final_target = dir.path().join("final_target.json"); // never created
        let intermediate = dir.path().join("intermediate.json");
        let config = dir.path().join("config.json");

        symlink(&final_target, &intermediate).unwrap(); // intermediate -> missing final_target
        symlink(&intermediate, &config).unwrap(); // config -> intermediate

        safe_write_json_file(&config, &serde_json::json!({"created": true}), None).unwrap();

        // Every hop in the chain must survive as a symlink — only the
        // terminal (previously missing) target becomes a regular file.
        assert!(
            fs::symlink_metadata(&config)
                .unwrap()
                .file_type()
                .is_symlink(),
            "config.json should remain a symlink"
        );
        assert_eq!(fs::read_link(&config).unwrap(), intermediate);
        assert!(
            fs::symlink_metadata(&intermediate)
                .unwrap()
                .file_type()
                .is_symlink(),
            "intermediate.json should remain a symlink, not be replaced by a regular file"
        );
        assert_eq!(fs::read_link(&intermediate).unwrap(), final_target);

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&final_target).unwrap()).unwrap();
        assert_eq!(parsed["created"], true);
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_cyclic_symlink_fails_safely_without_touching_links() {
        use std::os::unix::fs::symlink;

        let dir = tmpdir();
        let a = dir.path().join("a.json");
        let b = dir.path().join("b.json");
        symlink(&b, &a).unwrap(); // a -> b
        symlink(&a, &b).unwrap(); // b -> a (cycle)

        // Must terminate (cycle-detected) rather than hang, and must refuse
        // to write rather than fall back to renaming over `a` itself — that
        // fallback would silently destroy the symlink it's meant to protect.
        let result = safe_write_json_file(&a, &serde_json::json!({"x": true}), None);
        assert!(result.is_err(), "a cyclic symlink must be rejected");

        assert!(
            fs::symlink_metadata(&a).unwrap().file_type().is_symlink(),
            "a.json must remain untouched after a failed resolution"
        );
        assert!(
            fs::symlink_metadata(&b).unwrap().file_type().is_symlink(),
            "b.json must remain untouched after a failed resolution"
        );
        assert_eq!(fs::read_link(&a).unwrap(), b);
        assert_eq!(fs::read_link(&b).unwrap(), a);
    }

    /// Builds a chain of `hops` distinct (non-cyclic) symlinks:
    /// `hop_0 -> hop_1 -> ... -> hop_{hops-1} -> hop_final_missing` (never
    /// created). Returns `(entry_path, final_missing_target)`.
    #[cfg(unix)]
    fn build_dangling_chain(dir: &Path, hops: usize) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::symlink;

        let final_target = dir.join("hop_final_missing.json");
        let mut prev = final_target.clone();
        for i in (0..hops).rev() {
            let link = dir.join(format!("hop_{i}.json"));
            symlink(&prev, &link).unwrap();
            prev = link;
        }
        (prev, final_target)
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_chain_of_exactly_max_hops_succeeds() {
        let dir = tmpdir();
        // A chain of exactly MAX_SYMLINK_HOPS symlinks landing on a terminal
        // (missing) target must still resolve — the hop budget bounds how
        // many links are *followed*, not how many are merely inspected, so
        // the terminal check after the last followed hop must still run.
        let (entry, final_target) = build_dangling_chain(dir.path(), MAX_SYMLINK_HOPS);

        safe_write_json_file(&entry, &serde_json::json!({"x": true}), None).unwrap();

        assert!(
            fs::symlink_metadata(&entry)
                .unwrap()
                .file_type()
                .is_symlink(),
            "entry point must remain a symlink"
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&final_target).unwrap()).unwrap();
        assert_eq!(parsed["x"], true);
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_chain_one_hop_past_max_fails_safely() {
        let dir = tmpdir();
        // One hop past the budget must still be rejected, and must not fall
        // back to writing over the entry point.
        let (entry, _final_target) = build_dangling_chain(dir.path(), MAX_SYMLINK_HOPS + 1);

        let result = safe_write_json_file(&entry, &serde_json::json!({"x": true}), None);
        assert!(
            result.is_err(),
            "a chain one hop past the budget must be rejected"
        );
        assert!(
            fs::symlink_metadata(&entry)
                .unwrap()
                .file_type()
                .is_symlink(),
            "entry point must remain untouched after a failed resolution"
        );
    }

    #[test]
    #[cfg(unix)]
    fn safe_write_through_excessively_long_dangling_chain_fails_safely() {
        let dir = tmpdir();
        // Well past the limit — also must be rejected rather than falling
        // back to writing over the entry point.
        let (entry, _final_target) = build_dangling_chain(dir.path(), 50);

        let result = safe_write_json_file(&entry, &serde_json::json!({"x": true}), None);
        assert!(
            result.is_err(),
            "an overlong dangling chain must be rejected"
        );
        assert!(
            fs::symlink_metadata(&entry)
                .unwrap()
                .file_type()
                .is_symlink(),
            "entry point must remain untouched after a failed resolution"
        );
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

    // ----- remove_managed_rules_file (issue #256 follow-up: symlink safety) -----

    #[test]
    #[cfg(unix)]
    fn remove_managed_rules_file_preserves_symlink_removes_target() {
        use std::os::unix::fs::symlink;

        let link_dir = tmpdir();
        let target_dir = tmpdir();
        let target = target_dir.path().join("dotfiles_tokensave.md");
        fs::write(&target, "old rules").unwrap();

        let link = link_dir.path().join("tokensave.md");
        symlink(&target, &link).unwrap();

        remove_managed_rules_file(&link);

        // The symlink itself must survive — a dotfiles setup that symlinks
        // this file into a repo must not have that symlink silently deleted.
        let meta = fs::symlink_metadata(&link).unwrap();
        assert!(meta.file_type().is_symlink(), "symlink must be preserved");
        assert_eq!(fs::read_link(&link).unwrap(), target);

        // The generated content it pointed at must actually be gone —
        // otherwise uninstall did not really remove the rules.
        assert!(!target.exists(), "target content must be removed");
    }

    #[test]
    fn remove_managed_rules_file_removes_plain_file_and_prunes_empty_dir() {
        let dir = tmpdir();
        let rules_dir = dir.path().join("rules");
        fs::create_dir_all(&rules_dir).unwrap();
        let path = rules_dir.join("tokensave.md");
        fs::write(&path, "rules").unwrap();

        remove_managed_rules_file(&path);

        assert!(!path.exists());
        assert!(!rules_dir.exists(), "now-empty rules/ dir should be pruned");
    }

    // ----- remove_legacy_rules_block (issue #256 follow-up: adjacent
    //       headings, atomic writes, error propagation) -----

    const CLAUDE_MARKER: &str = "## MANDATORY: No Explore Agents When Tokensave Is Available";
    const CLAUDE_SUBHEADING: &str =
        "## When you spawn an Explore agent in a tokensave-enabled project";

    #[test]
    fn remove_legacy_rules_block_noop_when_file_missing() {
        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn remove_legacy_rules_block_noop_when_marker_absent() {
        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        fs::write(&path, "# My notes\n\nNothing to do with tokensave here.\n").unwrap();

        remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# My notes\n\nNothing to do with tokensave here.\n"
        );
    }

    #[test]
    fn remove_legacy_rules_block_preserves_adjacent_user_heading_mentioning_tokensave() {
        // Regression: the old heuristic decided a sub-heading belonged to
        // our block by checking `heading_line.contains("tokensave")`, so a
        // user heading that merely mentions tokensave right after our block
        // was swallowed as if it were part of it. Matching by the exact
        // known sub-heading text instead must leave it alone.
        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        fs::write(
            &path,
            format!(
                "# My notes\n\n\
                 Some custom content.\n\n\
                 {CLAUDE_MARKER}\n\n\
                 legacy body text\n\n\
                 {CLAUDE_SUBHEADING}\n\n\
                 legacy sub-body text\n\n\
                 ## My tokensave workflow notes\n\n\
                 This is MY content about tokensave, not the installed block.\n"
            ),
        )
        .unwrap();

        remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(!result.contains(CLAUDE_MARKER));
        assert!(!result.contains(CLAUDE_SUBHEADING));
        assert!(result.contains("Some custom content."));
        assert!(
            result.contains("## My tokensave workflow notes"),
            "adjacent user heading must survive: {result}"
        );
        assert!(result.contains("This is MY content about tokensave, not the installed block."));
    }

    #[test]
    fn remove_legacy_rules_block_leaves_only_custom_content_when_block_is_appended_at_eof() {
        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        fs::write(
            &path,
            format!(
                "# My notes\n\nSome custom content.\n\n{CLAUDE_MARKER}\n\nlegacy body\n\n{CLAUDE_SUBHEADING}\n\nlegacy sub-body\n"
            ),
        )
            .unwrap();

        remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# My notes\n\nSome custom content.\n"
        );
    }

    #[test]
    fn remove_legacy_rules_block_removes_file_when_migration_leaves_it_empty() {
        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        fs::write(
            &path,
            format!("{CLAUDE_MARKER}\n\nlegacy body\n\n{CLAUDE_SUBHEADING}\n\nlegacy sub-body\n"),
        )
        .unwrap();

        remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();

        assert!(!path.exists(), "file with nothing left should be removed");
    }

    #[test]
    #[cfg(unix)]
    fn remove_legacy_rules_block_preserves_symlink_when_migration_empties_file() {
        use std::os::unix::fs::symlink;

        let link_dir = tmpdir();
        let target_dir = tmpdir();
        let target = target_dir.path().join("dotfiles_claude_md");
        fs::write(&target, format!("{CLAUDE_MARKER}\n\nlegacy body\n")).unwrap();

        let link = link_dir.path().join("CLAUDE.md");
        symlink(&target, &link).unwrap();

        remove_legacy_rules_block(&link, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();

        // The symlink itself must survive a dotfiles-managed setup.
        let meta = fs::symlink_metadata(&link).unwrap();
        assert!(meta.file_type().is_symlink(), "symlink must be preserved");
        assert_eq!(fs::read_link(&link).unwrap(), target);

        // The obsolete legacy content it pointed at must actually be gone.
        assert!(!target.exists(), "legacy target content must be removed");
    }

    #[test]
    fn remove_legacy_rules_block_creates_backup() {
        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        let original = format!("Custom.\n\n{CLAUDE_MARKER}\n\nlegacy body\n");
        fs::write(&path, &original).unwrap();

        remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]).unwrap();

        let backup = dir.path().join("CLAUDE.md.bak");
        assert!(backup.exists(), "migration must leave a recoverable backup");
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    }

    #[test]
    #[cfg(unix)]
    fn remove_legacy_rules_block_write_failure_returns_err_and_leaves_file_untouched() {
        // Make the directory read-only so backup_config_file's staging write
        // (CLAUDE.md.bak.new) fails before safe_write_text_file ever runs,
        // simulating a permission error / read-only filesystem mid-migration.
        // The caller (install) must see this as an Err, not a silent success
        // with stale content left in CLAUDE.md.
        use std::os::unix::fs::PermissionsExt;

        let dir = tmpdir();
        let path = dir.path().join("CLAUDE.md");
        let original = format!("Custom.\n\n{CLAUDE_MARKER}\n\nlegacy body\n");
        fs::write(&path, &original).unwrap();

        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();

        let result = remove_legacy_rules_block(&path, CLAUDE_MARKER, &[CLAUDE_SUBHEADING]);

        // Restore permissions before any assertion can panic/early-return,
        // so the tempdir can still be cleaned up on drop.
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();

        assert!(result.is_err(), "write failure must surface as Err");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            original,
            "CLAUDE.md must be left untouched when migration fails partway"
        );
    }

    #[test]
    fn safe_write_text_file_rename_failure_leaves_original_untouched() {
        // Target is a directory, not a file: the `.new` staging write next to
        // it succeeds (parent dir is writable), but the final rename onto a
        // directory fails. This isolates the rename-failure branch itself,
        // uncovered by the migration test above (which only reaches the
        // earlier backup-write failure).
        let dir = tmpdir();
        let path = dir.path().join("target");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("existing.txt"), "keep me").unwrap();

        let result = safe_write_text_file(&path, "new content");

        assert!(result.is_err(), "rename onto a directory must fail");
        assert!(
            path.is_dir(),
            "original directory must survive a failed rename"
        );
        assert_eq!(
            fs::read_to_string(path.join("existing.txt")).unwrap(),
            "keep me"
        );
        assert!(
            !dir.path().join("target.new").exists(),
            "the failed .new staging file must be cleaned up"
        );
    }

    // -----------------------------------------------------------------------
    // #419: a write that would not change a byte is not a write
    // -----------------------------------------------------------------------

    /// Content already on disk, rendered the way the writer renders it.
    fn seed_current(dir: &std::path::Path, value: &serde_json::Value) -> std::path::PathBuf {
        let path = dir.join("settings.json");
        let pretty = serde_json::to_string_pretty(value).unwrap();
        fs::write(&path, format!("{pretty}\n")).unwrap();
        path
    }

    #[test]
    fn json_file_is_current_matches_only_exact_bytes() {
        let dir = tmpdir();
        let value = serde_json::json!({"hooks": {"a": 1}, "permissions": ["x"]});
        let path = seed_current(dir.path(), &value);
        assert!(json_file_is_current(&path, &value));

        // Semantically equal but formatted differently is NOT current: writing
        // would reformat the file, and that is a real change to it.
        fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
        assert!(
            !json_file_is_current(&path, &value),
            "compact JSON differs from what the writer produces"
        );

        // Missing trailing newline is the easy regression to ship by accident.
        let pretty = serde_json::to_string_pretty(&value).unwrap();
        fs::write(&path, &pretty).unwrap();
        assert!(
            !json_file_is_current(&path, &value),
            "the writer appends a trailing newline"
        );

        // A missing file is not current, and must not be reported as such.
        assert!(!json_file_is_current(
            &dir.path().join("absent.json"),
            &value
        ));
    }

    #[test]
    fn write_json_file_with_identical_content_touches_nothing() {
        let dir = tmpdir();
        let value = serde_json::json!({"hooks": {"PreToolUse": []}});
        let path = seed_current(dir.path(), &value);
        let before = fs::metadata(&path).unwrap().modified().unwrap();

        write_json_file(&path, &value).unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            before,
            "mtime must not advance when no byte changes"
        );
        assert!(
            !dir.path().join("settings.json.bak").exists(),
            "#419: a no-op write must not drop a .bak beside the settings file"
        );
    }

    #[test]
    fn backup_and_write_json_reports_false_and_writes_nothing_when_current() {
        let dir = tmpdir();
        let value = serde_json::json!({"mcpServers": {"tokensave": {"command": "ts"}}});
        let path = seed_current(dir.path(), &value);
        let before = fs::metadata(&path).unwrap().modified().unwrap();

        assert!(
            !backup_and_write_json(&path, &value),
            "nothing reached disk, so callers must not print '✔ Wrote'"
        );
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
        assert!(!dir.path().join("settings.json.bak").exists());
    }

    #[test]
    fn a_real_change_still_writes_and_still_backs_up() {
        // The control. Without this, the three tests above would pass just as
        // well against a writer that had stopped writing altogether.
        let dir = tmpdir();
        let original = serde_json::json!({"hooks": {"a": 1}});
        let path = seed_current(dir.path(), &original);

        let updated = serde_json::json!({"hooks": {"a": 2}});
        assert!(backup_and_write_json(&path, &updated));

        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk["hooks"]["a"], 2);

        let bak = dir.path().join("settings.json.bak");
        assert!(bak.exists(), "a real write must still leave a .bak (#63)");
        let backed_up: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&bak).unwrap()).unwrap();
        assert_eq!(backed_up["hooks"]["a"], 1);
    }

    #[test]
    fn safe_write_json_file_skips_the_rename_when_current() {
        // The inner guard, for the call sites that back up themselves and call
        // straight through. Their .bak is already written by this point, but
        // the file itself must be left alone.
        let dir = tmpdir();
        let value = serde_json::json!({"x": [1, 2, 3]});
        let path = seed_current(dir.path(), &value);
        let before = fs::metadata(&path).unwrap().modified().unwrap();

        safe_write_json_file(&path, &value, None).unwrap();

        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
        assert!(
            !dir.path().join("settings.json.new").exists(),
            "the staging file must not be left behind"
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod path_normalize_tests {
    use crate::agents::normalize_path_separators;

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod install_scope_tests {
    use crate::agents::InstallContext;
    use crate::agents::InstallScope;
    use std::path::PathBuf;

    #[test]
    fn install_context_base_dir_follows_scope() {
        let home = PathBuf::from("/home/user");
        let proj = PathBuf::from("/work/proj");

        let global = InstallContext {
            home: home.clone(),
            tokensave_bin: "tokensave".into(),
            tool_permissions: vec![],
            scope: InstallScope::Global,
            force_permission_style: false,
        };
        assert_eq!(global.base_dir(), home.as_path());
        assert!(!global.is_local());

        let local = InstallContext {
            home: home.clone(),
            tokensave_bin: "tokensave".into(),
            tool_permissions: vec![],
            scope: InstallScope::Local {
                project_path: proj.clone(),
            },
            force_permission_style: false,
        };
        assert_eq!(local.base_dir(), proj.as_path());
        assert!(local.is_local());
    }
}
