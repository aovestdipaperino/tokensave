//! User-level configuration stored at `~/.tokensave/config.toml`, with
//! frequently-changing machine-local state split out into
//! `~/.tokensave/state.toml`.
//!
//! `UserConfig` itself stays a single flat struct (unchanged public API) so
//! every call site can keep treating it as one value. Internally, `load()`
//! and `save()` fan the fields out across the two on-disk files via the
//! private `ConfigFile`/`StateFile` view structs below: `config.toml` holds
//! stable, dotfile-friendly preferences; `state.toml` holds volatile cached
//! values and timestamps that would otherwise churn a version-controlled
//! `config.toml` on almost every run.
//!
//! All fields have defaults so a missing file or missing fields are handled
//! gracefully. Unknown fields are silently ignored for forward compatibility.
//! Existing single-file installs migrate transparently: `load()` reads any
//! legacy state values still present in `config.toml`, and the next `save()`
//! writes them out to `state.toml` and drops them from `config.toml`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// User-level tokensave configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct UserConfig {
    /// Whether to upload pending tokens to the worldwide counter.
    #[serde(default = "default_true")]
    pub upload_enabled: bool,

    /// Tokens accumulated locally, not yet uploaded.
    #[serde(default)]
    pub pending_upload: u64,

    /// UNIX timestamp of last successful upload.
    #[serde(default)]
    pub last_upload_at: i64,

    /// Cached worldwide total from last fetch.
    #[serde(default)]
    pub last_worldwide_total: u64,

    /// UNIX timestamp of last worldwide total fetch.
    #[serde(default)]
    pub last_worldwide_fetch_at: i64,

    /// UNIX timestamp of last flush attempt (success or failure).
    #[serde(default)]
    pub last_flush_attempt_at: i64,

    /// Cached latest version from GitHub releases.
    #[serde(default)]
    pub cached_latest_version: String,

    /// UNIX timestamp of last version check.
    #[serde(default)]
    pub last_version_check_at: i64,

    /// UNIX timestamp of last version-update warning shown to the user.
    #[serde(default)]
    pub last_version_warning_at: i64,

    /// Agent integrations that have been installed (e.g. `["claude", "gemini"]`).
    #[serde(default)]
    pub installed_agents: Vec<String>,

    /// Debounce duration for the embedded MCP file watcher (e.g. "2s", "15s", "1m").
    #[serde(default = "default_watcher_debounce", alias = "daemon_debounce")]
    pub watcher_debounce: String,

    /// Cached country flags from the worldwide counter.
    #[serde(default)]
    pub cached_country_flags: Vec<String>,

    /// UNIX timestamp of last country flags fetch.
    #[serde(default)]
    pub last_flags_fetch_at: i64,

    /// UNIX timestamp of last `LiteLLM` pricing fetch.
    #[serde(default)]
    pub last_pricing_fetch_at: i64,

    /// Version that last ran `install` or `reinstall`. Used to trigger a
    /// silent reinstall when the binary is upgraded.
    #[serde(default)]
    pub last_installed_version: String,

    /// Version of the *previously running* tokensave binary, recorded by
    /// `tokensave upgrade` / `channel switch` just before the binary is
    /// replaced. The *new* binary reads this on startup and decides whether
    /// reinstall is required for the transition (patch-only bumps are
    /// no-ops; minor/major bumps re-register agents). Always updated to the
    /// running version after the decision is made.
    #[serde(default)]
    pub previous_version: String,

    /// Per-file extraction timeout in seconds. The worker is killed and
    /// the file is recorded in `SyncResult.skipped_paths` if a single
    /// file's extraction takes longer. Bounds the worst case from any
    /// pathological grammar / input combo.
    #[serde(default = "default_extraction_timeout_secs")]
    pub extraction_timeout_secs: u64,

    /// When true, `install`/`reinstall` grant Claude Code tokensave tools via
    /// a single compact `mcp__tokensave__*` entry in `permissions.allow`
    /// instead of enumerating every tool individually. Both forms are fully
    /// honored by Claude Code; this only affects what gets written. Defaults
    /// to `false` (explicit per-tool list) for continuity with existing
    /// installs. Overridable per-invocation with `--wildcard-permissions` /
    /// `--explicit-permissions`.
    #[serde(default)]
    pub wildcard_permissions: bool,
}

fn default_true() -> bool {
    true
}

fn default_watcher_debounce() -> String {
    "2s".to_string()
}

fn default_extraction_timeout_secs() -> u64 {
    60
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            upload_enabled: true,
            pending_upload: 0,
            last_upload_at: 0,
            last_worldwide_total: 0,
            last_worldwide_fetch_at: 0,
            last_flush_attempt_at: 0,
            cached_latest_version: String::new(),
            last_version_check_at: 0,
            last_version_warning_at: 0,
            installed_agents: Vec::new(),
            watcher_debounce: default_watcher_debounce(),
            cached_country_flags: Vec::new(),
            last_flags_fetch_at: 0,
            last_pricing_fetch_at: 0,
            last_installed_version: String::new(),
            previous_version: String::new(),
            extraction_timeout_secs: default_extraction_timeout_secs(),
            wildcard_permissions: false,
        }
    }
}

/// Stable, dotfile-friendly fields persisted to `config.toml`.
#[derive(Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default = "default_true")]
    upload_enabled: bool,
    #[serde(default = "default_watcher_debounce", alias = "daemon_debounce")]
    watcher_debounce: String,
    #[serde(default = "default_extraction_timeout_secs")]
    extraction_timeout_secs: u64,
    #[serde(default)]
    wildcard_permissions: bool,
}

/// Volatile, machine-local fields persisted to `state.toml`.
#[derive(Serialize, Deserialize, Default)]
struct StateFile {
    #[serde(default)]
    pending_upload: u64,
    #[serde(default)]
    last_upload_at: i64,
    #[serde(default)]
    last_worldwide_total: u64,
    #[serde(default)]
    last_worldwide_fetch_at: i64,
    #[serde(default)]
    last_flush_attempt_at: i64,
    #[serde(default)]
    cached_latest_version: String,
    #[serde(default)]
    last_version_check_at: i64,
    #[serde(default)]
    last_version_warning_at: i64,
    #[serde(default)]
    cached_country_flags: Vec<String>,
    #[serde(default)]
    last_flags_fetch_at: i64,
    #[serde(default)]
    last_pricing_fetch_at: i64,
    #[serde(default)]
    last_installed_version: String,
    #[serde(default)]
    previous_version: String,
    #[serde(default)]
    installed_agents: Vec<String>,
}

/// Overlays the state fields of `StateFile` onto an in-memory `UserConfig`.
/// State-file values win, since `config` may still be carrying legacy state
/// values recovered from an old mixed `config.toml`.
fn apply_state(config: &mut UserConfig, state: StateFile) {
    config.pending_upload = state.pending_upload;
    config.last_upload_at = state.last_upload_at;
    config.last_worldwide_total = state.last_worldwide_total;
    config.last_worldwide_fetch_at = state.last_worldwide_fetch_at;
    config.last_flush_attempt_at = state.last_flush_attempt_at;
    config.cached_latest_version = state.cached_latest_version;
    config.last_version_check_at = state.last_version_check_at;
    config.last_version_warning_at = state.last_version_warning_at;
    config.cached_country_flags = state.cached_country_flags;
    config.last_flags_fetch_at = state.last_flags_fetch_at;
    config.last_pricing_fetch_at = state.last_pricing_fetch_at;
    config.last_installed_version = state.last_installed_version;
    config.previous_version = state.previous_version;
    config.installed_agents = state.installed_agents;
}

/// Per-process counter mixed into temp file names so two `write_atomic` calls
/// racing on the same thread-pool (e.g. background MCP work and a shutdown
/// path both saving `UserConfig` around the same time) never pick the same
/// temp path: a bare PID is shared by every call in the process.
static TMP_FILE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Serializes `UserConfig::save()` calls within this process so the
/// state.toml write and the config.toml write of one call are never
/// interleaved with those of another: without this, two same-process savers
/// (e.g. background MCP work racing a shutdown save) could each write one
/// half of the pair, and a subsequent `load()` would combine one caller's
/// state with the other's config into a value neither of them saved. Does
/// not, and cannot, order writes from a second tokensave *process*.
static SAVE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Upper bound on symlink hops `resolve_write_target` will follow through a
/// dangling chain before giving up. Generous for any real dotfiles setup
/// (a handful of hops at most) while still bounding a cyclic chain so
/// resolution always terminates.
const MAX_SYMLINK_HOPS: u32 = 40;

/// Resolves the real file `write_atomic` should write to, so it never
/// renames over a symlink and replaces the link itself.
///
/// `canonicalize()` handles the common case (an existing target, possibly
/// through nested symlinks) but errors as soon as any hop doesn't exist —
/// which is true both for a first save (no file, no link) and for a
/// *dangling* chain (one or more links exist, but the file at the end of the
/// chain doesn't yet). Those cases must be told apart: falling back to the
/// path as given would make the rename replace a real link in the chain,
/// breaking a dotfiles setup where the repo copy hasn't been created yet.
/// `symlink_metadata` (which does not follow the link) tells them apart; for
/// each symlink hop we resolve its target via `read_link`, relative to that
/// hop's own parent directory, and keep walking until a hop is not itself a
/// symlink (dangling chain) or `canonicalize` succeeds (the chain turned out
/// to lead to a real file after all), bounded by `MAX_SYMLINK_HOPS` in case
/// the chain cycles back on itself.
///
/// Returns `None` if `MAX_SYMLINK_HOPS` is exhausted: since a straight-line
/// dangling chain always terminates via one of the early returns below (a
/// missing or non-symlink final hop), running out of hops only happens for a
/// cyclic chain, where every hop resolved is itself a live symlink. There is
/// no path in that case that's safe to hand back — whichever link in the
/// cycle we returned, `write_atomic` would rename over it and break it — so
/// the caller must fail the save instead of guessing.
fn resolve_write_target(path: &std::path::Path) -> Option<PathBuf> {
    if let Ok(canon) = std::fs::canonicalize(path) {
        return Some(canon);
    }
    let mut current = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_HOPS {
        let Ok(meta) = std::fs::symlink_metadata(&current) else {
            return Some(current);
        };
        if !meta.file_type().is_symlink() {
            return Some(current);
        }
        let Ok(link_target) = std::fs::read_link(&current) else {
            return Some(current);
        };
        current = if link_target.is_absolute() {
            link_target
        } else {
            current
                .parent()
                .map(|parent| parent.join(&link_target))
                .unwrap_or(link_target)
        };
        if let Ok(canon) = std::fs::canonicalize(&current) {
            return Some(canon);
        }
    }
    None
}

/// Replaces `target` with `tmp_path`, for the Windows fallback where
/// `rename` returns `AlreadyExists` instead of replacing an existing
/// destination the way POSIX `rename` does.
///
/// Moves the existing `target` aside to a backup path *first*, rather than
/// deleting it and only then attempting the real rename: if that second
/// rename then fails (full disk, permissions, an antivirus lock, another
/// process holding the file open) the previous approach left `target`
/// deleted for good, losing `state.toml`'s `pending_upload` and installed-
/// agent bookkeeping. Here, a failed replacement restores the backup back to
/// `target` instead, so `target` is never left missing.
fn replace_via_backup(tmp_path: &std::path::Path, target: &std::path::Path, unique: u64) -> bool {
    let backup = target.with_extension(format!("bak.{}.{unique}", std::process::id()));
    if std::fs::rename(target, &backup).is_err() {
        let _ = std::fs::remove_file(tmp_path);
        return false;
    }
    if std::fs::rename(tmp_path, target).is_ok() {
        let _ = std::fs::remove_file(&backup);
        true
    } else {
        let _ = std::fs::rename(&backup, target);
        let _ = std::fs::remove_file(tmp_path);
        false
    }
}

/// Sets `tmp_path`'s permission bits to match `target`'s existing mode before
/// it gets renamed into place, so a save never silently changes the mode a
/// user (or a dotfiles repo) already set on the destination. Falls back to a
/// restrictive `0o600` when `target` doesn't exist yet (first save, or a
/// dangling symlink chain whose end hasn't been created) so a freshly
/// created `config.toml`/`state.toml` is never accidentally world-readable.
///
/// Unix-only: `std::fs::write` creates new files using the process umask
/// (commonly `0o644`), so without this the destination's mode would silently
/// widen to the umask default on every save, undoing e.g. a dotfiles-managed
/// `config.toml` intentionally locked to `0o600`. Windows has no umask
/// equivalent here — new files inherit the parent directory's ACLs and a
/// replacing `rename` doesn't reset them — so there is nothing to fix there.
/// Ownership (uid/gid) is left alone: the temp file is already created by the
/// same user in the same user-owned directory, and changing ownership to
/// someone else would need privileges this process doesn't have.
#[cfg(unix)]
fn set_tmp_file_mode(tmp_path: &std::path::Path, target: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(target).map_or(0o600, |m| m.permissions().mode() & 0o777);
    std::fs::set_permissions(tmp_path, std::fs::Permissions::from_mode(mode)).is_ok()
}

/// Writes `contents` to `path` via a same-directory temp file plus rename,
/// so a crash or a full disk mid-write can't leave `path` truncated or
/// corrupt. The temp file name mixes the process ID with a per-process
/// counter so it is unique per invocation, not just per process.
fn write_atomic(path: &std::path::Path, contents: &str) -> bool {
    // Resolve symlinks (including dangling ones) so we write through them
    // rather than replacing the link with a regular file. Dotfile setups
    // symlink config.toml into a repo; a plain rename-over would detach that
    // symlink and leave the repo copy stale. The temp file is placed next to
    // the *resolved* target so the rename stays on one filesystem (atomic).
    // A cyclic symlink chain has no safe target to resolve to; fail rather
    // than rename over a live link in the cycle.
    let Some(target) = resolve_write_target(path) else {
        return false;
    };
    let unique = TMP_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = target.with_extension(format!("tmp.{}.{unique}", std::process::id()));
    if std::fs::write(&tmp_path, contents).is_err() {
        return false;
    }
    // Set the temp file's mode to match the target's existing mode (or a
    // restrictive default for a new file) before the rename below makes it
    // live, so the destination's permissions are never silently reset to the
    // umask default. Bail out rather than rename a file whose mode we
    // couldn't pin down deliberately.
    #[cfg(unix)]
    if !set_tmp_file_mode(&tmp_path, &target) {
        let _ = std::fs::remove_file(&tmp_path);
        return false;
    }
    match std::fs::rename(&tmp_path, &target) {
        Ok(()) => true,
        // Unlike POSIX rename, Windows' `rename` fails with `AlreadyExists`
        // instead of replacing an existing destination, so every save after
        // the first would otherwise fail here. Scoped to that exact condition
        // so any other rename failure (permissions, full disk, destination is
        // a directory) is reported as a failed save instead of being papered
        // over.
        Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::AlreadyExists => {
            replace_via_backup(&tmp_path, &target, unique)
        }
        Err(_) => {
            let _ = std::fs::remove_file(&tmp_path);
            false
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_HOME_OVERRIDE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Points `config_path()`/`state_path()` at a test-controlled directory for
/// the current thread only, so tests can run in parallel without racing over
/// process-global state (e.g. the `HOME` environment variable).
#[cfg(test)]
fn set_test_home_dir(dir: Option<PathBuf>) {
    TEST_HOME_OVERRIDE.with(|cell| *cell.borrow_mut() = dir);
}

/// Returns the `~/.tokensave` directory, or a test-injected override.
fn tokensave_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Some(dir) = TEST_HOME_OVERRIDE.with(|cell| cell.borrow().clone()) {
            return Some(dir);
        }
    }
    dirs::home_dir().map(|h| h.join(".tokensave"))
}

/// Returns the path to the config file: `~/.tokensave/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    tokensave_dir().map(|d| d.join("config.toml"))
}

/// Returns the path to the machine-local state file: `~/.tokensave/state.toml`.
pub fn state_path() -> Option<PathBuf> {
    tokensave_dir().map(|d| d.join("state.toml"))
}

/// Moves an unparseable `state.toml` aside rather than leaving it where a
/// later `save()` would overwrite it in place. Best-effort: if the rename
/// itself fails there is nothing more to do without risking the save path.
/// The backup name mixes a timestamp, PID, and the shared temp-file counter
/// so repeated corruption (or concurrent callers hitting it at once) never
/// collide on the same backup path.
fn preserve_corrupt_state_file(state_path: &std::path::Path) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let unique = TMP_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let backup = state_path.with_extension(format!(
        "toml.corrupt.{timestamp}.{}.{unique}",
        std::process::id()
    ));
    let _ = std::fs::rename(state_path, backup);
}

impl UserConfig {
    /// Loads the config from `~/.tokensave/config.toml`, overlaying any
    /// machine-local state found in `~/.tokensave/state.toml`. Returns
    /// defaults if both files are missing or unreadable.
    ///
    /// Legacy installs that still have state fields mixed into a single
    /// `config.toml` (pre-split) recover those values here since `config.toml`
    /// is parsed into the full `UserConfig`; the next `save()` writes them out
    /// to `state.toml` and strips them from `config.toml`.
    ///
    /// Reads both files under `SAVE_LOCK` (see `save()`'s doc comment) so a
    /// concurrent same-process `save()` can't land between the config.toml
    /// read and the state.toml read — without this, `load()` could return a
    /// config.toml snapshot from before a save paired with a state.toml
    /// snapshot from after it, mixing two callers' writes into one object
    /// that neither of them saved.
    pub fn load() -> Self {
        let _guard = SAVE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut base: Self = config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|contents| toml::from_str(&contents).unwrap_or_default())
            .unwrap_or_default();

        if let Some(state_p) = state_path() {
            if let Ok(contents) = std::fs::read_to_string(&state_p) {
                match toml::from_str::<StateFile>(&contents) {
                    Ok(state) => apply_state(&mut base, state),
                    // state.toml exists but doesn't parse (manual edit,
                    // filesystem corruption, a future incompatible format).
                    // `base` falls back to defaults for the fields it can't
                    // recover; without preserving the original file, the very
                    // next routine save() would overwrite it in place with
                    // those defaults, permanently losing pending_upload,
                    // installed_agents, and every cached timestamp. Move it
                    // aside instead so the original content survives on disk
                    // for manual recovery even after later saves write a
                    // fresh state.toml at the original path.
                    Err(_) => preserve_corrupt_state_file(&state_p),
                }
            }
        }

        base
    }

    /// Saves the config to `~/.tokensave/config.toml` and `~/.tokensave/state.toml`.
    /// Best-effort. Returns true only if both files were saved successfully.
    ///
    /// The two files are written under `SAVE_LOCK`, so same-process callers
    /// (e.g. background MCP work racing a shutdown save) never interleave
    /// their writes into a config.toml from one caller paired with a
    /// state.toml from another. This does not protect against a *second
    /// tokensave process* saving concurrently — there is no cross-process
    /// file lock, so racing processes can still each write one half of the
    /// pair, matching the existing best-effort persistence model.
    pub fn save(&self) -> bool {
        let Some(config_path) = config_path() else {
            return false;
        };
        let Some(state_path) = state_path() else {
            return false;
        };
        if let Some(parent) = config_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return false;
            }
        }

        let config_file = ConfigFile {
            upload_enabled: self.upload_enabled,
            watcher_debounce: self.watcher_debounce.clone(),
            extraction_timeout_secs: self.extraction_timeout_secs,
            wildcard_permissions: self.wildcard_permissions,
        };
        let state_file = StateFile {
            pending_upload: self.pending_upload,
            last_upload_at: self.last_upload_at,
            last_worldwide_total: self.last_worldwide_total,
            last_worldwide_fetch_at: self.last_worldwide_fetch_at,
            last_flush_attempt_at: self.last_flush_attempt_at,
            cached_latest_version: self.cached_latest_version.clone(),
            last_version_check_at: self.last_version_check_at,
            last_version_warning_at: self.last_version_warning_at,
            cached_country_flags: self.cached_country_flags.clone(),
            last_flags_fetch_at: self.last_flags_fetch_at,
            last_pricing_fetch_at: self.last_pricing_fetch_at,
            last_installed_version: self.last_installed_version.clone(),
            previous_version: self.previous_version.clone(),
            installed_agents: self.installed_agents.clone(),
        };

        let Ok(config_contents) = toml::to_string_pretty(&config_file) else {
            return false;
        };
        let Ok(state_contents) = toml::to_string_pretty(&state_file) else {
            return false;
        };

        // Serialize the pair of writes against other same-process saves (see
        // SAVE_LOCK doc comment) so no other thread's save can land between
        // the state.toml and config.toml writes below.
        let _guard = SAVE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Write state.toml first: if it fails, config.toml is left untouched
        // (still holding any legacy state fields recovered from a pre-split
        // install), so a failed save never loses state that only lived in
        // config.toml a moment ago.
        write_atomic(&state_path, &state_contents) && write_atomic(&config_path, &config_contents)
    }

    /// Returns true if this is a fresh config (file did not exist before).
    pub fn is_fresh() -> bool {
        config_path().is_none_or(|p| !p.exists())
    }
}

/// Parse a human-readable duration string like "15s" or "1m" into a Duration.
pub fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.trim()
            .parse::<u64>()
            .ok()
            .map(std::time::Duration::from_secs)
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.trim()
            .parse::<u64>()
            .ok()
            .map(|m| std::time::Duration::from_secs(m * 60))
    } else {
        s.parse::<u64>().ok().map(std::time::Duration::from_secs)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::duration_suboptimal_units
)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("15s"), Some(Duration::from_secs(15)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration(" 5s "), Some(Duration::from_secs(5)));
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration("1m"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration("2m"), Some(Duration::from_secs(120)));
    }

    #[test]
    fn parse_duration_bare_number() {
        assert_eq!(parse_duration("10"), Some(Duration::from_secs(10)));
    }

    #[test]
    fn parse_duration_invalid() {
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("1h"), None);
    }

    struct TestHome {
        _dir: tempfile::TempDir,
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            set_test_home_dir(None);
        }
    }

    fn test_home() -> TestHome {
        let dir = tempfile::tempdir().expect("tempdir");
        set_test_home_dir(Some(dir.path().to_path_buf()));
        TestHome { _dir: dir }
    }

    fn sample_config() -> UserConfig {
        UserConfig {
            upload_enabled: false,
            pending_upload: 42,
            last_upload_at: 100,
            last_worldwide_total: 999,
            last_worldwide_fetch_at: 200,
            last_flush_attempt_at: 300,
            cached_latest_version: "1.2.3".to_string(),
            last_version_check_at: 400,
            last_version_warning_at: 500,
            installed_agents: vec!["claude".to_string(), "cursor".to_string()],
            watcher_debounce: "5s".to_string(),
            cached_country_flags: vec!["us".to_string(), "cz".to_string()],
            last_flags_fetch_at: 600,
            last_pricing_fetch_at: 700,
            last_installed_version: "1.2.2".to_string(),
            previous_version: "1.2.1".to_string(),
            extraction_timeout_secs: 30,
            wildcard_permissions: true,
        }
    }

    #[test]
    fn round_trip_save_and_load() {
        let _home = test_home();
        let config = sample_config();
        assert!(config.save());

        let loaded = UserConfig::load();
        assert_eq!(loaded.upload_enabled, config.upload_enabled);
        assert_eq!(loaded.pending_upload, config.pending_upload);
        assert_eq!(loaded.last_upload_at, config.last_upload_at);
        assert_eq!(loaded.last_worldwide_total, config.last_worldwide_total);
        assert_eq!(
            loaded.last_worldwide_fetch_at,
            config.last_worldwide_fetch_at
        );
        assert_eq!(loaded.last_flush_attempt_at, config.last_flush_attempt_at);
        assert_eq!(loaded.cached_latest_version, config.cached_latest_version);
        assert_eq!(loaded.last_version_check_at, config.last_version_check_at);
        assert_eq!(
            loaded.last_version_warning_at,
            config.last_version_warning_at
        );
        assert_eq!(loaded.installed_agents, config.installed_agents);
        assert_eq!(loaded.watcher_debounce, config.watcher_debounce);
        assert_eq!(loaded.cached_country_flags, config.cached_country_flags);
        assert_eq!(loaded.last_flags_fetch_at, config.last_flags_fetch_at);
        assert_eq!(loaded.last_pricing_fetch_at, config.last_pricing_fetch_at);
        assert_eq!(loaded.last_installed_version, config.last_installed_version);
        assert_eq!(loaded.previous_version, config.previous_version);
        assert_eq!(
            loaded.extraction_timeout_secs,
            config.extraction_timeout_secs
        );
        assert_eq!(loaded.wildcard_permissions, config.wildcard_permissions);
    }

    #[test]
    fn save_splits_fields_across_files() {
        let _home = test_home();
        sample_config().save();

        let config_contents =
            std::fs::read_to_string(config_path().expect("config path")).expect("read config");
        assert!(config_contents.contains("upload_enabled"));
        assert!(!config_contents.contains("cached_latest_version"));
        assert!(!config_contents.contains("pending_upload"));

        let state_contents =
            std::fs::read_to_string(state_path().expect("state path")).expect("read state");
        assert!(state_contents.contains("pending_upload"));
        assert!(state_contents.contains("cached_latest_version"));
        assert!(!state_contents.contains("wildcard_permissions"));
    }

    #[test]
    fn save_twice_overwrites_previous_files() {
        let _home = test_home();
        let mut config = sample_config();
        assert!(config.save());

        config.pending_upload = 4321;
        config.wildcard_permissions = false;
        assert!(config.save());

        let loaded = UserConfig::load();
        assert_eq!(loaded.pending_upload, 4321);
        assert!(!loaded.wildcard_permissions);
    }

    #[test]
    fn concurrent_saves_never_delete_destination_files() {
        // Regression for a same-process race: two threads saving around the
        // same time (plausible for background/blocking MCP work racing a
        // shutdown save) must not share a temp file name. A shared name let
        // one thread's rename win, then the other found its temp file gone,
        // fell into the retry fallback, and deleted the destination the
        // first thread had just written.
        let dir = tempfile::tempdir().expect("tempdir");
        let tokensave_dir = dir.path().to_path_buf();

        let handles: Vec<_> = (0..8_u64)
            .map(|i| {
                let tokensave_dir = tokensave_dir.clone();
                std::thread::spawn(move || {
                    set_test_home_dir(Some(tokensave_dir));
                    let mut config = sample_config();
                    config.pending_upload = i;
                    assert!(config.save(), "save() should not fail under contention");
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("saving thread panicked");
        }

        // config_path()/state_path() key off a thread-local override that
        // isn't set on this (the joining) thread, so check the files
        // directly under the shared tokensave dir instead.
        assert!(tokensave_dir.join("config.toml").exists());
        assert!(tokensave_dir.join("state.toml").exists());
    }

    #[test]
    fn concurrent_saves_do_not_mix_config_and_state() {
        // Regression for interleaving between the state.toml write and the
        // config.toml write of two different same-process saves (e.g. A
        // writes state, B writes state, B writes config, A writes config).
        // Without SAVE_LOCK serializing the pair, load() could combine one
        // caller's config with another caller's state into a value neither
        // of them ever saved. Each thread ties its state field
        // (pending_upload) to its config field (wildcard_permissions) so any
        // such mixing is externally observable: a consistent on-disk pair
        // must have wildcard_permissions == (pending_upload % 2 == 0).
        let dir = tempfile::tempdir().expect("tempdir");
        let tokensave_dir = dir.path().to_path_buf();

        let handles: Vec<_> = (0..8_u64)
            .map(|i| {
                let tokensave_dir = tokensave_dir.clone();
                std::thread::spawn(move || {
                    set_test_home_dir(Some(tokensave_dir));
                    for _ in 0..25 {
                        let mut config = sample_config();
                        config.pending_upload = i;
                        config.wildcard_permissions = i % 2 == 0;
                        assert!(config.save(), "save() should not fail under contention");
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("saving thread panicked");
        }

        // Load on a fresh thread rather than this one, so we don't leave the
        // thread-local override set on a test-harness thread that might be
        // reused by a later test.
        let tokensave_dir_for_check = tokensave_dir.clone();
        let loaded = std::thread::spawn(move || {
            set_test_home_dir(Some(tokensave_dir_for_check));
            UserConfig::load()
        })
        .join()
        .expect("checker thread panicked");

        assert_eq!(
            loaded.wildcard_permissions,
            loaded.pending_upload % 2 == 0,
            "config.toml and state.toml came from different saves: pending_upload={}, wildcard_permissions={}",
            loaded.pending_upload,
            loaded.wildcard_permissions,
        );
    }

    #[test]
    fn load_never_returns_torn_config_state_pair_during_concurrent_saves() {
        // Regression: load() didn't hold SAVE_LOCK, so it could read
        // config.toml before a racing save() and state.toml after it (or
        // vice versa), returning a value that mixes two different callers'
        // writes. Each save() below ties its config field
        // (wildcard_permissions) to its state field (pending_upload) so any
        // such tear is externally observable, and the invariant also holds
        // for the pre-save default (pending_upload=0, wildcard_permissions
        // =false), so a load() racing the very first save is covered too.
        let dir = tempfile::tempdir().expect("tempdir");
        let tokensave_dir = dir.path().to_path_buf();

        let saver_handles: Vec<_> = (0..4_u64)
            .map(|i| {
                let tokensave_dir = tokensave_dir.clone();
                std::thread::spawn(move || {
                    set_test_home_dir(Some(tokensave_dir));
                    for iter in 0..25_u64 {
                        let mut config = sample_config();
                        if (i + iter) % 2 == 0 {
                            config.pending_upload = 0;
                            config.wildcard_permissions = false;
                        } else {
                            config.pending_upload = i + 1;
                            config.wildcard_permissions = true;
                        }
                        assert!(config.save(), "save() should not fail under contention");
                    }
                })
            })
            .collect();

        let loader_handles: Vec<_> = (0..4_u64)
            .map(|_| {
                let tokensave_dir = tokensave_dir.clone();
                std::thread::spawn(move || {
                    set_test_home_dir(Some(tokensave_dir));
                    for _ in 0..50 {
                        let loaded = UserConfig::load();
                        assert_eq!(
                            loaded.wildcard_permissions,
                            loaded.pending_upload != 0,
                            "load() returned a torn config/state pair: pending_upload={}, wildcard_permissions={}",
                            loaded.pending_upload,
                            loaded.wildcard_permissions,
                        );
                    }
                })
            })
            .collect();

        for handle in saver_handles {
            handle.join().expect("saving thread panicked");
        }
        for handle in loader_handles {
            handle.join().expect("loading thread panicked");
        }
    }

    #[test]
    fn load_preserves_corrupt_state_file_instead_of_overwriting_it() {
        // Regression: load() used to fall back to defaults for an
        // unparseable state.toml without touching the file itself. A
        // subsequent, entirely routine save() would then overwrite it in
        // place with those defaults, permanently losing pending_upload,
        // installed_agents, and every cached timestamp that had been in the
        // corrupt file. The original content must survive on disk.
        let home = test_home();

        let state_p = state_path().expect("state path");
        if let Some(parent) = state_p.parent() {
            std::fs::create_dir_all(parent).expect("create tokensave dir");
        }
        let corrupt_contents = "pending_upload = [not valid toml";
        std::fs::write(&state_p, corrupt_contents).expect("seed corrupt state.toml");

        let loaded = UserConfig::load();
        assert_eq!(
            loaded.pending_upload, 0,
            "unrecoverable state fields should fall back to defaults"
        );
        assert!(
            !state_p.exists(),
            "corrupt state.toml should have been moved aside, not left at the original path"
        );

        let tokensave_dir = state_p.parent().expect("state parent").to_path_buf();
        let backups: Vec<_> = std::fs::read_dir(&tokensave_dir)
            .expect("read tokensave dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("state.toml.corrupt.")
            })
            .collect();
        assert_eq!(
            backups.len(),
            1,
            "expected exactly one preserved backup of the corrupt state.toml"
        );
        let backup_path = backups[0].path();
        assert_eq!(
            std::fs::read_to_string(&backup_path).expect("read preserved backup"),
            corrupt_contents
        );

        // A subsequent, routine save() must not be blocked by the earlier
        // corruption -- it writes a fresh state.toml at the original path...
        assert!(loaded.save());
        assert!(state_p.exists());
        // ...leaving the preserved backup of the original corrupt content
        // untouched.
        assert_eq!(
            std::fs::read_to_string(&backup_path).expect("read preserved backup after save"),
            corrupt_contents
        );

        drop(home);
    }

    #[test]
    fn migrates_legacy_mixed_config_file() {
        let home = test_home();
        let legacy = sample_config();
        let legacy_toml = toml::to_string_pretty(&legacy).expect("serialize legacy config");
        std::fs::write(config_path().expect("config path"), legacy_toml).expect("write legacy");

        assert!(!state_path().expect("state path").exists());

        let loaded = UserConfig::load();
        assert_eq!(loaded.pending_upload, legacy.pending_upload);
        assert_eq!(loaded.previous_version, legacy.previous_version);
        assert_eq!(loaded.cached_latest_version, legacy.cached_latest_version);

        assert!(loaded.save());
        assert!(state_path().expect("state path").exists());

        let config_contents =
            std::fs::read_to_string(config_path().expect("config path")).expect("read config");
        assert!(!config_contents.contains("pending_upload"));
        assert!(!config_contents.contains("previous_version"));

        drop(home);
    }

    #[test]
    fn failed_state_write_does_not_lose_legacy_config_state() {
        let home = test_home();
        let legacy = sample_config();
        let legacy_toml = toml::to_string_pretty(&legacy).expect("serialize legacy config");
        let config_p = config_path().expect("config path");
        std::fs::write(&config_p, &legacy_toml).expect("write legacy");

        // Force the state.toml write to fail: a directory in its place
        // cannot be renamed over, simulating a permission error or full disk.
        let state_p = state_path().expect("state path");
        std::fs::create_dir_all(&state_p).expect("create state dir");

        let loaded = UserConfig::load();
        assert_eq!(loaded.pending_upload, legacy.pending_upload);
        assert_eq!(loaded.previous_version, legacy.previous_version);

        assert!(!loaded.save());

        // config.toml must be untouched: legacy state is still recoverable
        // on the next load rather than lost between the two writes.
        let config_contents = std::fs::read_to_string(&config_p).expect("read config");
        assert_eq!(config_contents, legacy_toml);

        drop(home);
    }

    #[test]
    fn replace_via_backup_replaces_destination_on_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("state.toml");
        std::fs::write(&target, "original").expect("seed target");
        let tmp_path = dir.path().join("state.tmp.0");
        std::fs::write(&tmp_path, "updated").expect("seed tmp");

        assert!(replace_via_backup(&tmp_path, &target, 0));

        assert_eq!(
            std::fs::read_to_string(&target).expect("read target"),
            "updated"
        );
        assert!(!tmp_path.exists(), "temp file should be consumed");

        let leftover_count = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .count();
        assert_eq!(leftover_count, 1, "backup file was not cleaned up");
    }

    #[test]
    fn replace_via_backup_restores_destination_if_replacement_fails() {
        // Regression: this used to remove `target` unconditionally before
        // attempting the real rename, so a *second* failure (disk full,
        // permissions, an antivirus lock, another process) left the
        // destination permanently deleted. Force that second rename to fail
        // deterministically by pointing at a temp file that doesn't exist.
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("state.toml");
        std::fs::write(&target, "original").expect("seed target");
        let missing_tmp_path = dir.path().join("state.tmp.missing");

        assert!(!replace_via_backup(&missing_tmp_path, &target, 0));

        assert_eq!(
            std::fs::read_to_string(&target).expect("read target"),
            "original",
            "destination was lost instead of restored"
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_writes_through_config_symlink() {
        let home = test_home();

        // Stand in for a dotfiles repo living outside ~/.tokensave.
        let repo_dir = tempfile::tempdir().expect("repo tempdir");
        let repo_config = repo_dir.path().join("config.toml");
        std::fs::write(&repo_config, "upload_enabled = true\n").expect("seed repo config");

        let config_p = config_path().expect("config path");
        if let Some(parent) = config_p.parent() {
            std::fs::create_dir_all(parent).expect("create tokensave dir");
        }
        std::os::unix::fs::symlink(&repo_config, &config_p).expect("symlink config");

        assert!(sample_config().save());

        // The symlink itself must survive the save...
        let meta = std::fs::symlink_metadata(&config_p).expect("symlink metadata");
        assert!(
            meta.file_type().is_symlink(),
            "save() replaced the config.toml symlink with a regular file"
        );

        // ...and the write must have landed on the repo copy it points at.
        let repo_contents = std::fs::read_to_string(&repo_config).expect("read repo config");
        assert!(repo_contents.contains("wildcard_permissions = true"));

        drop(home);
    }

    #[cfg(unix)]
    #[test]
    fn save_writes_through_dangling_config_symlink() {
        // Regression: canonicalize() fails for a symlink whose target doesn't
        // exist yet (e.g. a freshly cloned, still-empty dotfiles repo). The
        // fallback must resolve the link's target explicitly rather than
        // falling back to the symlink path itself, which would make the
        // rename replace the link with a regular file.
        let home = test_home();

        let repo_dir = tempfile::tempdir().expect("repo tempdir");
        let repo_config = repo_dir.path().join("config.toml");
        assert!(!repo_config.exists(), "repo config must start absent");

        let config_p = config_path().expect("config path");
        if let Some(parent) = config_p.parent() {
            std::fs::create_dir_all(parent).expect("create tokensave dir");
        }
        std::os::unix::fs::symlink(&repo_config, &config_p).expect("symlink config");

        assert!(sample_config().save());

        // The (previously dangling) symlink itself must survive the save...
        let meta = std::fs::symlink_metadata(&config_p).expect("symlink metadata");
        assert!(
            meta.file_type().is_symlink(),
            "save() replaced the dangling config.toml symlink with a regular file"
        );

        // ...and the write must have created and landed on the repo target.
        let repo_contents = std::fs::read_to_string(&repo_config).expect("read repo config");
        assert!(repo_contents.contains("wildcard_permissions = true"));

        drop(home);
    }

    #[cfg(unix)]
    #[test]
    fn save_writes_through_nested_dangling_config_symlink() {
        // Regression: resolve_write_target() previously followed only one
        // symlink hop, so a two-level chain (config.toml -> managed-config ->
        // missing-target) resolved to `managed-config` -- the *second* link
        // in the chain, not its real (currently missing) end -- and
        // write_atomic() then renamed over that link, breaking it.
        let home = test_home();

        let repo_dir = tempfile::tempdir().expect("repo tempdir");
        let missing_target = repo_dir.path().join("missing-target");
        assert!(!missing_target.exists(), "chain target must start absent");

        let managed_config = repo_dir.path().join("managed-config");
        std::os::unix::fs::symlink(&missing_target, &managed_config)
            .expect("symlink managed-config -> missing-target");

        let config_p = config_path().expect("config path");
        if let Some(parent) = config_p.parent() {
            std::fs::create_dir_all(parent).expect("create tokensave dir");
        }
        std::os::unix::fs::symlink(&managed_config, &config_p)
            .expect("symlink config.toml -> managed-config");

        assert!(sample_config().save());

        // Both links in the chain must survive...
        let config_meta = std::fs::symlink_metadata(&config_p).expect("config symlink metadata");
        assert!(
            config_meta.file_type().is_symlink(),
            "save() replaced the config.toml symlink with a regular file"
        );
        let managed_meta =
            std::fs::symlink_metadata(&managed_config).expect("managed-config symlink metadata");
        assert!(
            managed_meta.file_type().is_symlink(),
            "save() replaced the managed-config symlink with a regular file"
        );

        // ...and the write must have landed on the chain's real end.
        let contents = std::fs::read_to_string(&missing_target).expect("read chain target");
        assert!(contents.contains("wildcard_permissions = true"));

        drop(home);
    }

    #[cfg(unix)]
    #[test]
    fn save_fails_on_cyclic_config_symlink() {
        // Regression: resolve_write_target() previously returned `current`
        // unconditionally once MAX_SYMLINK_HOPS was exhausted, even though a
        // cyclic chain (config.toml -> other -> config.toml) means every hop
        // resolved is still a live symlink. write_atomic() then renamed the
        // temp file over that path, silently breaking one link in the cycle.
        // save() must fail instead of guessing a target to write through.
        let home = test_home();

        let config_p = config_path().expect("config path");
        let other_p = config_p
            .parent()
            .expect("config parent")
            .join("other-cycle-link");

        std::os::unix::fs::symlink(&other_p, &config_p).expect("symlink config.toml -> other");
        std::os::unix::fs::symlink(&config_p, &other_p).expect("symlink other -> config.toml");

        assert!(
            !sample_config().save(),
            "save() must not succeed on a cyclic symlink chain"
        );

        // Neither link in the cycle should have been replaced with a regular
        // file by a rename that gave up and wrote through the wrong path.
        let config_meta = std::fs::symlink_metadata(&config_p).expect("config symlink metadata");
        assert!(
            config_meta.file_type().is_symlink(),
            "save() replaced the config.toml symlink with a regular file"
        );
        let other_meta = std::fs::symlink_metadata(&other_p).expect("other symlink metadata");
        assert!(
            other_meta.file_type().is_symlink(),
            "save() replaced the other-cycle-link symlink with a regular file"
        );

        drop(home);
    }

    #[cfg(unix)]
    fn mode_of(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn save_preserves_existing_file_mode() {
        use std::os::unix::fs::PermissionsExt;
        // Regression: write_atomic() wrote the temp file with std::fs::write,
        // which uses the process umask (commonly 0o644), then renamed it over
        // the destination — silently widening a config.toml a user (or a
        // dotfiles repo) had deliberately locked down to 0o600.
        let home = test_home();

        assert!(sample_config().save());
        let config_p = config_path().expect("config path");
        let state_p = state_path().expect("state path");
        std::fs::set_permissions(&config_p, std::fs::Permissions::from_mode(0o600))
            .expect("chmod config.toml");
        std::fs::set_permissions(&state_p, std::fs::Permissions::from_mode(0o600))
            .expect("chmod state.toml");

        assert!(sample_config().save());

        assert_eq!(
            mode_of(&config_p),
            0o600,
            "save() widened config.toml's mode away from the caller-set 0o600"
        );
        assert_eq!(
            mode_of(&state_p),
            0o600,
            "save() widened state.toml's mode away from the caller-set 0o600"
        );

        drop(home);
    }

    #[cfg(unix)]
    #[test]
    fn save_creates_files_with_restrictive_mode() {
        // A brand-new config.toml/state.toml (no prior file to inherit a mode
        // from) should default to 0o600 rather than whatever the umask would
        // otherwise produce, since state.toml in particular holds local
        // bookkeeping (pending upload counts, installed agents) that has no
        // reason to be world-readable.
        let home = test_home();

        assert!(sample_config().save());

        let config_p = config_path().expect("config path");
        let state_p = state_path().expect("state path");
        assert_eq!(
            mode_of(&config_p),
            0o600,
            "a freshly created config.toml should default to 0o600"
        );
        assert_eq!(
            mode_of(&state_p),
            0o600,
            "a freshly created state.toml should default to 0o600"
        );

        drop(home);
    }

    #[cfg(unix)]
    #[test]
    fn save_through_symlink_preserves_target_mode() {
        use std::os::unix::fs::PermissionsExt;
        // The finding's motivating case: a dotfiles-managed config.toml is
        // symlinked in and deliberately locked to 0o600. write_atomic()
        // resolves through the symlink to the repo file, so the mode it must
        // preserve is the repo file's, not the symlink's own (meaningless)
        // mode.
        let home = test_home();

        let repo_dir = tempfile::tempdir().expect("repo tempdir");
        let repo_config = repo_dir.path().join("config.toml");
        std::fs::write(&repo_config, "upload_enabled = true\n").expect("seed repo config");
        std::fs::set_permissions(&repo_config, std::fs::Permissions::from_mode(0o600))
            .expect("chmod repo config.toml");

        let config_p = config_path().expect("config path");
        if let Some(parent) = config_p.parent() {
            std::fs::create_dir_all(parent).expect("create tokensave dir");
        }
        std::os::unix::fs::symlink(&repo_config, &config_p).expect("symlink config");

        assert!(sample_config().save());

        let meta = std::fs::symlink_metadata(&config_p).expect("symlink metadata");
        assert!(
            meta.file_type().is_symlink(),
            "save() replaced the config.toml symlink with a regular file"
        );
        assert_eq!(
            mode_of(&repo_config),
            0o600,
            "save() widened the dotfiles repo's config.toml mode away from 0o600"
        );

        drop(home);
    }
}
