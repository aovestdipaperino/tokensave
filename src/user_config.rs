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

    /// Agent integrations that have been installed (e.g. ["claude", "gemini"]).
    #[serde(default)]
    pub installed_agents: Vec<String>,

    /// Debounce duration for the daemon file watcher (e.g. "15s", "1m").
    #[serde(default = "default_daemon_debounce")]
    pub daemon_debounce: String,

    /// Cached country flags from the worldwide counter.
    #[serde(default)]
    pub cached_country_flags: Vec<String>,

    /// UNIX timestamp of last country flags fetch.
    #[serde(default)]
    pub last_flags_fetch_at: i64,

    /// Version that last ran `install` or `reinstall`. Used to trigger a
    /// silent reinstall when the binary is upgraded.
    #[serde(default)]
    pub last_installed_version: String,

    /// UNIX timestamp of last LiteLLM pricing fetch.
    #[serde(default)]
    pub last_pricing_fetch_at: i64,
}

fn default_true() -> bool {
    true
}

fn default_daemon_debounce() -> String {
    "15s".to_string()
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
            daemon_debounce: default_daemon_debounce(),
            cached_country_flags: Vec::new(),
            last_flags_fetch_at: 0,
            last_installed_version: String::new(),
            last_pricing_fetch_at: 0,
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
