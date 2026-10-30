//! Automatic initialization of git repositories that have no index (#876).
//!
//! A cloned repository is indexed by the `post-checkout` hook, but a
//! repository started with `git init` never sees a fresh checkout, so before
//! this it stayed unindexed until someone ran `tokensave init`. The
//! user-level `auto_init` setting (default on, `TOKENSAVE_AUTO_INIT` overrides
//! it per run) lets two entry points create the index instead:
//!
//! * `tokensave sync` run without a path, which is what the installed
//!   `post-commit` and `post-merge` hooks call, so hooks written by earlier
//!   versions get the behavior without being rewritten;
//! * `tokensave serve`, which also covers machines without global git hooks.
//!
//! [`decide`] is the single gate both share. It only ever initializes the top
//! level of a git working tree that no project owns yet, and reuses the
//! guards the `post-checkout` auto-init already applies.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::{discover_project, ProjectDiscovery, TokenSaveConfig};

/// Environment variable that overrides the `auto_init` setting for one run,
/// parsed like `TOKENSAVE_AUTO_TRACK`.
pub const AUTO_INIT_ENV: &str = "TOKENSAVE_AUTO_INIT";

/// The effective `auto_init` value: `TOKENSAVE_AUTO_INIT` when set, otherwise
/// `auto_init` in `~/.tokensave/config.toml`, otherwise `true`.
pub fn enabled() -> bool {
    crate::config::env_bool_override(AUTO_INIT_ENV, crate::user_config::auto_init_setting())
}

/// Why [`decide`] declined to initialize a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    /// `auto_init` is off, in the config or through `TOKENSAVE_AUTO_INIT`.
    Disabled,
    /// A project (complete or interrupted) already owns the directory.
    AlreadyInitialized,
    /// The directory is not the top level of a git working tree. This covers
    /// a bare repository, a subdirectory and a folder outside any repository.
    NotRepositoryRoot,
    /// The repository sits under the system temp dir, where other tools stage
    /// throwaway clones they expect to own (#569).
    UnderTempDir,
    /// The repository is the home directory itself, e.g. a dotfiles repo.
    HomeDir,
    /// The repository holds more candidate files than `max_auto_sync_files`.
    /// `count` stops one past `limit`, so it is a lower bound.
    TooManyFiles { count: usize, limit: usize },
}

impl Skip {
    /// A hint for the user when the skip is worth reporting, or `None` when
    /// the caller should keep its usual "not initialized" behavior.
    pub fn hint(&self, root: &Path) -> Option<String> {
        match self {
            Self::TooManyFiles { count, limit } => Some(format!(
                "auto_init deferred: '{}' has more than {limit} candidate files \
                 (counted {count}+), above max_auto_sync_files. \
                 Run `tokensave init` to index it deliberately.",
                root.display()
            )),
            _ => None,
        }
    }
}

/// What to do with a directory that may need an index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Initialize a project at this root.
    Init(PathBuf),
    /// Leave the directory alone.
    Skip(Skip),
}

/// Decides whether `cwd` should be initialized automatically.
///
/// Checks run cheapest first, and the file count, which spawns `git`, runs
/// last. The checks, in order: the setting is on; no project owns `cwd`; `cwd`
/// is the top level of a git working tree (so no state is ever created outside
/// it, #749); it is neither under the system temp dir nor `$HOME`; and the
/// repository holds no more than `max_auto_sync_files` candidate files.
pub fn decide(cwd: &Path) -> Decision {
    if !enabled() {
        return Decision::Skip(Skip::Disabled);
    }
    let root = match discover_project(cwd) {
        ProjectDiscovery::Initialized(_) | ProjectDiscovery::Incomplete(_) => {
            return Decision::Skip(Skip::AlreadyInitialized);
        }
        ProjectDiscovery::Uninitialized {
            git_root: Some(root),
        } if root == cwd => root,
        ProjectDiscovery::Uninitialized { .. } => {
            return Decision::Skip(Skip::NotRepositoryRoot);
        }
    };
    let system_temp = canonical(std::env::temp_dir());
    let home = crate::agents::home_dir().map(canonical);
    if let Some(skip) = location_guard(&canonical(root.clone()), &system_temp, home.as_deref()) {
        return Decision::Skip(skip);
    }
    let limit = TokenSaveConfig::default().max_auto_sync_files;
    if limit > 0 {
        let count = count_candidate_files(&root, limit);
        if count > limit {
            return Decision::Skip(Skip::TooManyFiles { count, limit });
        }
    }
    Decision::Init(root)
}

/// Canonicalizes `path`, keeping it as given when that fails. On macOS `/tmp`
/// is a symlink to `/private/tmp`, so comparing unresolved forms never matches.
fn canonical(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

/// The location guards, as a pure check on canonical paths.
pub fn location_guard(root: &Path, system_temp: &Path, home: Option<&Path>) -> Option<Skip> {
    if crate::agents::hooks::is_under_temp_dir(root, system_temp) {
        return Some(Skip::UnderTempDir);
    }
    if home == Some(root) {
        return Some(Skip::HomeDir);
    }
    None
}

/// Counts the files in the repository at `root` that an initial index would
/// consider, stopping once the count passes `limit`.
///
/// A cheap estimate taken before any state exists: the paths `git ls-files`
/// reports as tracked or untracked-but-not-ignored, keeping those with a
/// supported or artifact extension that the default `exclude` globs and the
/// hidden-path filter leave in. Reading stops at `limit + 1`, so `git init`
/// in a huge existing directory costs no more than that before it is
/// deferred. Returns 0 when `git` cannot run, leaving the index itself to
/// apply the real rules.
pub fn count_candidate_files(root: &Path, limit: usize) -> usize {
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return 0;
    };
    let config = TokenSaveConfig::default();
    let registry = crate::extraction::LanguageRegistry::new();
    let mut extensions = registry.supported_extensions();
    extensions.extend(config.artifact_extensions.iter().map(String::as_str));
    let mut count = 0;
    if let Some(stdout) = child.stdout.take() {
        for entry in BufReader::new(stdout).split(b'\0') {
            let Ok(entry) = entry else { break };
            let path = String::from_utf8_lossy(&entry);
            if is_candidate(&path, &extensions, &config) {
                count += 1;
                if count > limit {
                    break;
                }
            }
        }
    }
    // Stop a walk cut short; `wait` reaps the child either way.
    let _ = child.kill();
    let _ = child.wait();
    count
}

/// Whether a repository-relative `path` from `git ls-files` would be indexed
/// under the default configuration.
fn is_candidate(path: &str, extensions: &[&str], config: &TokenSaveConfig) -> bool {
    if path.split('/').any(|component| component.starts_with('.')) {
        return false;
    }
    let Some(extension) = Path::new(path).extension().and_then(|e| e.to_str()) else {
        return false;
    };
    extensions.contains(&extension) && !crate::config::is_excluded(path, config)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn location_guard_skips_temp_and_home() {
        let temp = Path::new("/tmp");
        let home = Path::new("/home/user");
        assert_eq!(
            location_guard(Path::new("/tmp/scratch"), temp, Some(home)),
            Some(Skip::UnderTempDir)
        );
        assert_eq!(location_guard(home, temp, Some(home)), Some(Skip::HomeDir));
        assert_eq!(
            location_guard(Path::new("/home/user/project"), temp, Some(home)),
            None
        );
        assert_eq!(location_guard(Path::new("/srv/project"), temp, None), None);
    }

    #[test]
    fn candidates_follow_extension_hidden_and_exclude_rules() {
        let config = TokenSaveConfig::default();
        let extensions = ["rs", "md"];
        assert!(is_candidate("src/lib.rs", &extensions, &config));
        assert!(is_candidate("README.md", &extensions, &config));
        assert!(!is_candidate("logo.png", &extensions, &config));
        assert!(!is_candidate("Makefile", &extensions, &config));
        assert!(!is_candidate(".github/notes.md", &extensions, &config));
        assert!(!is_candidate(
            "node_modules/pkg/index.rs",
            &extensions,
            &config
        ));
    }

    #[test]
    fn only_the_hint_for_too_many_files_is_reported() {
        let root = Path::new("/srv/project");
        let hint = Skip::TooManyFiles {
            count: 2001,
            limit: 2000,
        }
        .hint(root)
        .unwrap();
        assert!(hint.contains("max_auto_sync_files"), "{hint}");
        assert!(hint.contains("tokensave init"), "{hint}");
        assert_eq!(Skip::UnderTempDir.hint(root), None);
        assert_eq!(Skip::Disabled.hint(root), None);
    }
}
