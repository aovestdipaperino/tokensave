//! Repository-local settings and storage, independent of global consent.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{Result, TokenSaveError};
use crate::user_config::{ProfilingPreferences, UserConfig};

use super::Settings;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub profiling_share_enabled: bool,
    pub profiling_share_asked: bool,
    pub profiling: ProfilingPreferences,
}

#[derive(Debug, Clone, Default)]
pub enum Scope {
    #[default]
    Global,
    Repository(PathBuf),
}

fn error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

impl Scope {
    /// Use the same project discovery as indexing and serving, including
    /// borrowed worktree indexes and nested repository boundaries.
    pub fn from_cwd() -> Result<Self> {
        let cwd = std::env::current_dir().map_err(|e| error(e.to_string()))?;
        match crate::config::discover_project(&cwd) {
            crate::config::ProjectDiscovery::Initialized(root)
            | crate::config::ProjectDiscovery::Incomplete(root)
            | crate::config::ProjectDiscovery::Uninitialized {
                git_root: Some(root),
            } => Ok(Self::Repository(root)),
            crate::config::ProjectDiscovery::Uninitialized { git_root: None } => Err(error(
                "--local requires a TokenSave project or Git repository",
            )),
        }
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        match self {
            Self::Global => None,
            Self::Repository(root) => Some(root.join(".tokensave").join("profile.toml")),
        }
    }

    /// A local file selects independent local settings, even if malformed.
    /// Runtime callers fail closed on unreadable or malformed configuration.
    pub fn for_project(root: &Path) -> Self {
        let scope = Self::Repository(root.to_path_buf());
        if let Some(path) = scope.config_path() {
            match path.symlink_metadata() {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::Global,
                // Permission errors must not fall back to global consent.
                _ => scope,
            }
        } else {
            Self::Global
        }
    }

    pub fn load(&self) -> Result<Preferences> {
        let Some(path) = self.config_path() else {
            let config = UserConfig::load();
            return Ok(Preferences {
                profiling: config.profiling,
                profiling_share_enabled: crate::user_config::profiling_sharing_consent(),
                profiling_share_asked: config.profiling_share_asked,
            });
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| error(format!("{}: {e}", path.display())))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
            Err(e) => Err(error(format!("{}: {e}", path.display()))),
        }
    }

    /// Mutate only the requested preference, preserving the other controls.
    pub fn save_mode(&self, mode: super::Mode, expires: Option<i64>) -> Result<()> {
        match self {
            Self::Global => {
                let mut config = UserConfig::load();
                config.profiling.mode = mode;
                config.profiling.detailed_expires_at = expires;
                if config.save() {
                    Ok(())
                } else {
                    Err(error("failed to save global profiling settings"))
                }
            }
            Self::Repository(_) => self.update(|prefs| {
                prefs.profiling.mode = mode;
                prefs.profiling.detailed_expires_at = expires;
            }),
        }
    }

    pub fn save_sharing(&self, enabled: bool) -> Result<()> {
        match self {
            Self::Global => {
                let mut config = UserConfig::load();
                config.profiling_share_enabled = enabled;
                config.profiling_share_asked = true;
                if config.save() {
                    Ok(())
                } else {
                    Err(error("failed to save global sharing consent"))
                }
            }
            Self::Repository(_) => self.update(|prefs| {
                prefs.profiling_share_enabled = enabled;
                prefs.profiling_share_asked = true;
            }),
        }
    }

    fn update(&self, change: impl FnOnce(&mut Preferences)) -> Result<()> {
        let path = self
            .config_path()
            .ok_or_else(|| error("expected local scope"))?;
        let parent = path
            .parent()
            .ok_or_else(|| error("missing configuration parent"))?;
        std::fs::create_dir_all(parent).map_err(|e| error(e.to_string()))?;
        // Lock a stable companion inode; the settings file is atomically replaced.
        // Blocking is restricted to explicit CLI mutations, never the flush path.
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(parent.join("profile-settings.lock"))
            .map_err(|e| error(e.to_string()))?;
        fs2::FileExt::lock_exclusive(&lock).map_err(|e| error(e.to_string()))?;
        let mut prefs = self.load()?;
        change(&mut prefs);
        let text = toml::to_string_pretty(&prefs).map_err(|e| error(e.to_string()))?;
        if crate::user_config::write_atomic(&path, &text) {
            Ok(())
        } else {
            Err(error(format!("failed to save {}", path.display())))
        }
    }

    pub fn profile_dir(&self) -> Result<PathBuf> {
        match self {
            Self::Global => crate::user_config::profile_dir()
                .ok_or_else(|| error("could not locate the home directory")),
            Self::Repository(root) => Ok(root.join(".tokensave").join("profile")),
        }
    }

    pub fn settings(&self) -> Settings {
        let Ok(prefs) = self.load() else {
            return Settings::resolve(
                &ProfilingPreferences::default(),
                None,
                super::unix_now() as i64,
            );
        };
        let env = std::env::var("TOKENSAVE_PROFILE").ok();
        Settings::resolve(&prefs.profiling, env.as_deref(), super::unix_now() as i64)
    }

    pub fn sharing_consent(&self) -> bool {
        self.load().is_ok_and(|p| p.profiling_share_enabled)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::Mode;

    #[test]
    fn defaults_and_corrupt_settings_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let scope = Scope::Repository(tmp.path().to_path_buf());
        assert!(matches!(Scope::for_project(tmp.path()), Scope::Global));
        let prefs = scope.load().unwrap();
        assert_eq!(prefs.profiling.mode, Mode::Off);
        assert!(!prefs.profiling_share_enabled);
        assert!(!scope.profile_dir().unwrap().exists());
        scope.save_mode(Mode::Light, None).unwrap();
        let path = scope.config_path().unwrap();
        std::fs::write(&path, "invalid [toml").unwrap();
        assert!(matches!(
            Scope::for_project(tmp.path()),
            Scope::Repository(_)
        ));
        assert!(scope.load().is_err());
        assert_eq!(scope.settings().mode, Mode::Off);
        assert!(!scope.sharing_consent());
        assert!(scope.save_sharing(true).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid [toml");
    }

    #[test]
    fn independent_controls_and_concurrent_updates_preserve_revocation() {
        let tmp = tempfile::tempdir().unwrap();
        let scope = Scope::Repository(tmp.path().to_path_buf());
        scope.save_mode(Mode::Detailed, Some(100)).unwrap();
        scope.save_sharing(true).unwrap();
        let mut workers = Vec::new();
        for _ in 0..4 {
            let scope = scope.clone();
            workers.push(std::thread::spawn(move || {
                for _ in 0..10 {
                    scope.save_mode(Mode::Light, None).unwrap();
                }
            }));
        }
        scope.save_sharing(false).unwrap();
        for worker in workers {
            worker.join().unwrap();
        }
        let prefs = scope.load().unwrap();
        assert_eq!(prefs.profiling.mode, Mode::Light);
        assert_eq!(prefs.profiling.detailed_expires_at, None);
        assert!(!prefs.profiling_share_enabled);
        assert!(prefs.profiling_share_asked);
        assert!(!scope.profile_dir().unwrap().exists());
    }
}
