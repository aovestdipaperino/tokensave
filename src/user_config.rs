//! User-level configuration stored at `~/.tokensave/config.toml`.
//!
//! All fields have defaults so a missing file or missing fields are handled
//! gracefully. Unknown fields are silently ignored for forward compatibility.

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

    /// Per-file extraction timeout in seconds. The worker is killed and
    /// the file is recorded in `SyncResult.skipped_paths` if a single
    /// file's extraction takes longer. Bounds the worst case from any
    /// pathological grammar / input combo.
    #[serde(default = "default_extraction_timeout_secs")]
    pub extraction_timeout_secs: u64,
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
            extraction_timeout_secs: default_extraction_timeout_secs(),
        }
    }
}

/// Returns the path to the config file: `~/.tokensave/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave").join("config.toml"))
}

impl UserConfig {
    /// Loads the config from `~/.tokensave/config.toml`.
    /// Returns defaults if the file is missing or unreadable.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&contents).unwrap_or_default()
    }

    /// Saves the config to `~/.tokensave/config.toml`. Best-effort.
    /// Returns true if the file was saved, false on any error.
    pub fn save(&self) -> bool {
        let Some(path) = config_path() else {
            return false;
        };
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return false;
            }
        }
        let Ok(contents) = toml::to_string_pretty(self) else {
            return false;
        };
        std::fs::write(&path, contents).is_ok()
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
}
