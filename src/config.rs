use std::fs;
use std::path::{Path, PathBuf};

use glob::Pattern;
use serde::{Deserialize, Serialize};

use crate::errors::{CodeGraphError, Result};

/// Name of the configuration file stored inside the `.codegraph` directory.
pub const CONFIG_FILENAME: &str = "config.json";

/// Name of the hidden directory used to store CodeGraph metadata.
pub const CODEGRAPH_DIR: &str = ".codegraph";

/// Configuration for a CodeGraph project.
///
/// Controls which files are indexed, size limits, and feature toggles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeGraphConfig {
    /// Schema version of the configuration.
    pub version: u32,
    /// Root directory of the project being indexed.
    pub root_dir: String,
    /// Glob patterns for files to include during indexing.
    pub include: Vec<String>,
    /// Glob patterns for files to exclude during indexing.
    pub exclude: Vec<String>,
    /// Maximum file size in bytes; files larger than this are skipped.
    pub max_file_size: u64,
    /// Whether to extract doc comments from source files.
    pub extract_docstrings: bool,
    /// Whether to track call-site locations for edges.
    pub track_call_sites: bool,
    /// Whether to generate embeddings for semantic search.
    pub enable_embeddings: bool,
}

impl Default for CodeGraphConfig {
    fn default() -> Self {
        Self {
            version: 1,
            root_dir: String::new(),
            include: vec![
                "**/*.rs".to_string(),
                "**/*.go".to_string(),
                "**/*.java".to_string(),
            ],
            exclude: vec![
                "target/**".to_string(),
                ".git/**".to_string(),
                ".codegraph/**".to_string(),
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
            enable_embeddings: false,
        }
    }
}

/// Returns the path to the `.codegraph` directory within the given project root.
pub fn get_codegraph_dir(project_root: &Path) -> PathBuf {
    project_root.join(CODEGRAPH_DIR)
}

/// Returns the path to the configuration file (`config.json`) within the `.codegraph` directory.
pub fn get_config_path(project_root: &Path) -> PathBuf {
    get_codegraph_dir(project_root).join(CONFIG_FILENAME)
}

/// Loads the configuration from disk.
///
/// If the configuration file does not exist, returns a default configuration
/// with `root_dir` set to the given project root.
pub fn load_config(project_root: &Path) -> Result<CodeGraphConfig> {
    let config_path = get_config_path(project_root);

    if !config_path.exists() {
        return Ok(CodeGraphConfig {
            root_dir: project_root.to_string_lossy().to_string(),
            ..CodeGraphConfig::default()
        });
    }

    let contents = fs::read_to_string(&config_path).map_err(|e| CodeGraphError::Config {
        message: format!(
            "failed to read config file '{}': {}",
            config_path.display(),
            e
        ),
    })?;

    let config: CodeGraphConfig =
        serde_json::from_str(&contents).map_err(|e| CodeGraphError::Config {
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
pub fn save_config(project_root: &Path, config: &CodeGraphConfig) -> Result<()> {
    let codegraph_dir = get_codegraph_dir(project_root);
    fs::create_dir_all(&codegraph_dir).map_err(|e| CodeGraphError::Config {
        message: format!(
            "failed to create codegraph directory '{}': {}",
            codegraph_dir.display(),
            e
        ),
    })?;

    let config_path = get_config_path(project_root);
    let tmp_path = config_path.with_extension("tmp");

    let json = serde_json::to_string_pretty(config).map_err(|e| CodeGraphError::Config {
        message: format!("failed to serialize config: {}", e),
    })?;

    fs::write(&tmp_path, &json).map_err(|e| CodeGraphError::Config {
        message: format!(
            "failed to write temporary config file '{}': {}",
            tmp_path.display(),
            e
        ),
    })?;

    fs::rename(&tmp_path, &config_path).map_err(|e| CodeGraphError::Config {
        message: format!(
            "failed to rename temporary config file '{}' to '{}': {}",
            tmp_path.display(),
            config_path.display(),
            e
        ),
    })?;

    Ok(())
}

/// Determines whether a file should be included based on the configuration's
/// include and exclude glob patterns.
///
/// A file is included only if it matches at least one include pattern and
/// does not match any exclude pattern. Exclude patterns take precedence.
pub fn should_include_file(file_path: &str, config: &CodeGraphConfig) -> bool {
    let match_opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };

    // Check exclude patterns first — if any match, the file is excluded.
    for pattern_str in &config.exclude {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches_with(file_path, match_opts) {
                return false;
            }
        }
    }

    // Check include patterns — the file must match at least one.
    for pattern_str in &config.include {
        if let Ok(pattern) = Pattern::new(pattern_str) {
            if pattern.matches_with(file_path, match_opts) {
                return true;
            }
        }
    }

    false
}
