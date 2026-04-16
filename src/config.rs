use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glob::Pattern;
use serde::{Deserialize, Serialize};

use crate::errors::{Result, TokenSaveError};

/// Name of the configuration file stored inside the `.tokensave` directory.
pub const CONFIG_FILENAME: &str = "config.json";

/// Name of the hidden directory used to store TokenSave metadata.
pub const TOKENSAVE_DIR: &str = ".tokensave";

/// Configuration for a TokenSave project.
///
/// Controls which files are indexed, size limits, and feature toggles.
/// Language inclusion is derived automatically from the installed
/// `LanguageExtractor` set — only exclude patterns live in the config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSaveConfig {
    /// Schema version of the configuration.
    pub version: u32,
    /// Root directory of the project being indexed.
    pub root_dir: String,
    /// Glob patterns for files to exclude during indexing.
    pub exclude: Vec<String>,
    /// Maximum file size in bytes; files larger than this are skipped.
    pub max_file_size: u64,
    /// Whether to extract doc comments from source files.
    pub extract_docstrings: bool,
    /// Whether to track call-site locations for edges.
    pub track_call_sites: bool,
    /// Whether to respect `.gitignore` rules when scanning files.
    #[serde(default)]
    pub git_ignore: bool,
}

impl Default for TokenSaveConfig {
    fn default() -> Self {
        Self {
            version: 1,
            root_dir: String::new(),
            exclude: vec![
                "target/**".to_string(),
                ".git/**".to_string(),
                ".tokensave/**".to_string(),
                "node_modules/**".to_string(),
                "vendor/**".to_string(),
                "**/*.min.*".to_string(),
                "bin/**".to_string(),
                "build/**".to_string(),
                "out/**".to_string(),
                ".gradle/**".to_string(),
            ],
            max_file_size: 1_048_576,
            extract_docstrings: true,
            track_call_sites: true,
            git_ignore: false,
        }
    }
}

/// Returns the path to the `.tokensave` directory within the given project root.
pub fn get_tokensave_dir(project_root: &Path) -> PathBuf {
    project_root.join(TOKENSAVE_DIR)
}

/// Returns the path to the configuration file (`config.json`) within the `.tokensave` directory.
pub fn get_config_path(project_root: &Path) -> PathBuf {
    get_tokensave_dir(project_root).join(CONFIG_FILENAME)
}

/// Loads the configuration from disk.
///
/// If the configuration file does not exist, returns a default configuration
/// with `root_dir` set to the given project root.
pub fn load_config(project_root: &Path) -> Result<TokenSaveConfig> {
    let config_path = get_config_path(project_root);

    if !config_path.exists() {
        return Ok(TokenSaveConfig {
            root_dir: project_root.to_string_lossy().to_string(),
            ..TokenSaveConfig::default()
        });
    }

    let contents = fs::read_to_string(&config_path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to read config file '{}': {}",
            config_path.display(),
            e
        ),
    })?;

    let config: TokenSaveConfig =
        serde_json::from_str(&contents).map_err(|e| TokenSaveError::Config {
            message: format!(
                "failed to parse config file '{}': {}",
                config_path.display(),
                e
            ),
        })?;

    Ok(config)
}

/// Saves the configuration to disk using an atomic write.
///
/// Writes to a temporary file first and then renames it to the final location,
/// ensuring that a partial write never corrupts the configuration.
pub fn save_config(project_root: &Path, config: &TokenSaveConfig) -> Result<()> {
    let tokensave_dir = get_tokensave_dir(project_root);
    fs::create_dir_all(&tokensave_dir).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to create tokensave directory '{}': {}",
            tokensave_dir.display(),
            e
        ),
    })?;

    let config_path = get_config_path(project_root);
    let tmp_path = config_path.with_extension("tmp");

    let json = serde_json::to_string_pretty(config).map_err(|e| TokenSaveError::Config {
        message: format!("failed to serialize config: {}", e),
    })?;

    fs::write(&tmp_path, &json).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to write temporary config file '{}': {}",
            tmp_path.display(),
            e
        ),
    })?;

    fs::rename(&tmp_path, &config_path).map_err(|e| TokenSaveError::Config {
        message: format!(
            "failed to rename temporary config file '{}' to '{}': {}",
            tmp_path.display(),
            config_path.display(),
            e
        ),
    })?;

    Ok(())
}

/// Returns `true` if `.tokensave` is ignored by Git for this project.
///
/// This respects the repository `.gitignore`, `.git/info/exclude`, and the
/// user's global excludes file via `git check-ignore`. If Git cannot answer
/// (for example outside a Git repository), falls back to checking the local
/// `.gitignore` file only.
pub fn is_in_gitignore(project_path: &Path) -> bool {
    if let Some(is_ignored) = is_ignored_by_git(project_path, None) {
        return is_ignored;
    }

    is_in_local_gitignore(project_path)
}

fn is_ignored_by_git(project_path: &Path, git_config_global: Option<&Path>) -> Option<bool> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(project_path)
        .arg("check-ignore")
        .arg("-q")
        .arg(".tokensave/")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(path) = git_config_global {
        command.env("GIT_CONFIG_GLOBAL", path);
    }

    let status = command.status().ok()?;

    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

fn is_in_local_gitignore(project_path: &Path) -> bool {
    let gitignore = project_path.join(".gitignore");
    match fs::read_to_string(&gitignore) {
        Ok(content) => content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == ".tokensave" || trimmed == ".tokensave/" || trimmed == "/.tokensave"
        }),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_ignored_by_git;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn test_is_in_gitignore_respects_global_excludes_file() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        fs::create_dir(&repo).unwrap();

        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .arg("-q")
            .status()
            .unwrap();

        let excludes = sandbox.path().join("global_ignore");
        fs::write(&excludes, ".tokensave\n").unwrap();

        let git_config = sandbox.path().join("gitconfig");
        let status = Command::new("git")
            .env("GIT_CONFIG_GLOBAL", &git_config)
            .arg("config")
            .arg("--global")
            .arg("core.excludesFile")
            .arg(&excludes)
            .status()
            .unwrap();
        assert!(status.success());

        let ignored = is_ignored_by_git(&repo, Some(&git_config));

        assert_eq!(ignored, Some(true));
    }
}

/// Appends `.tokensave` to the project's `.gitignore`, creating the file if
/// needed. Ensures the entry starts on its own line (adds a trailing newline
/// to existing content if missing).
pub fn add_to_gitignore(project_path: &Path) {
    let gitignore = project_path.join(".gitignore");
    let mut content = fs::read_to_string(&gitignore).unwrap_or_default();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(".tokensave\n");
    if let Err(e) = fs::write(&gitignore, content) {
        eprintln!("warning: failed to update .gitignore: {e}");
    }
}

/// Resolves a CLI path argument to an absolute `PathBuf`.
///
/// If `path` is `Some`, uses that value; otherwise falls back to the current
/// working directory.
pub fn resolve_path(path: Option<String>) -> PathBuf {
    match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Returns `true` if the file matches any of the configured exclude patterns.
pub fn is_excluded(file_path: &str, config: &TokenSaveConfig) -> bool {
    let match_opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    for pattern_str in &config.exclude {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches_with(file_path, match_opts) {
                return true;
            }
        }
    }

    false
}
