//! Branch metadata persistence for multi-branch indexing.
//!
//! Stores tracking information in `.tokensave/branch-meta.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const BRANCH_META_FILENAME: &str = "branch-meta.json";

/// Metadata for a single tracked branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchEntry {
    /// Relative path to the DB file (e.g. `tokensave.db` or `branches/feature_foo.db`).
    pub db_file: String,
    /// Branch this was copied from (None for the default branch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// UNIX timestamp (seconds) when this branch DB was created.
    pub created_at: String,
    /// UNIX timestamp (seconds) of last successful sync.
    pub last_synced_at: String,
}

/// Top-level branch metadata for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMeta {
    /// The auto-detected or configured default branch name.
    pub default_branch: String,
    /// Map of branch name → entry.
    pub branches: HashMap<String, BranchEntry>,
}

impl BranchMeta {
    /// Creates a new metadata with a single default branch entry.
    pub fn new(default_branch: &str) -> Self {
        let now = now_unix_str();
        let mut branches = HashMap::new();
        branches.insert(
            default_branch.to_string(),
            BranchEntry {
                db_file: "tokensave.db".to_string(),
                parent: None,
                created_at: now.clone(),
                last_synced_at: now,
            },
        );
        Self {
            default_branch: default_branch.to_string(),
            branches,
        }
    }

    /// Adds a new tracked branch entry.
    pub fn add_branch(&mut self, name: &str, db_file: &str, parent: &str) {
        let now = now_unix_str();
        self.branches.insert(
            name.to_string(),
            BranchEntry {
                db_file: db_file.to_string(),
                parent: Some(parent.to_string()),
                created_at: now.clone(),
                last_synced_at: now,
            },
        );
    }

    /// Removes a tracked branch entry. Returns the entry if it existed.
    pub fn remove_branch(&mut self, name: &str) -> Option<BranchEntry> {
        if name == self.default_branch {
            return None; // never remove the default branch
        }
        self.branches.remove(name)
    }

    /// Updates the `last_synced_at` timestamp for a branch.
    pub fn touch_synced(&mut self, name: &str) {
        if let Some(entry) = self.branches.get_mut(name) {
            entry.last_synced_at = now_unix_str();
        }
    }

    /// Returns true if the given branch is tracked.
    pub fn is_tracked(&self, name: &str) -> bool {
        self.branches.contains_key(name)
    }
}

/// Loads branch metadata from `.tokensave/branch-meta.json`.
///
/// Returns `None` if the file doesn't exist (single-DB mode / pre-branch projects).
/// Prints a warning to stderr if the file exists but is malformed.
pub fn load_branch_meta(tokensave_dir: &Path) -> Option<BranchMeta> {
    let path = tokensave_dir.join(BRANCH_META_FILENAME);
    let content = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&content) {
        Ok(meta) => Some(meta),
        Err(e) => {
            eprintln!(
                "warning: corrupt branch metadata at '{}': {e} — falling back to single-DB mode",
                path.display()
            );
            None
        }
    }
}

/// Saves branch metadata to `.tokensave/branch-meta.json`.
pub fn save_branch_meta(tokensave_dir: &Path, meta: &BranchMeta) -> std::io::Result<()> {
    let path = tokensave_dir.join(BRANCH_META_FILENAME);
    let json = serde_json::to_string_pretty(meta)
        .map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

/// Returns the path to the `branches/` subdirectory, creating it if needed.
pub fn ensure_branches_dir(tokensave_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = tokensave_dir.join("branches");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn now_unix_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

/// Formats a UNIX timestamp string as a human-readable relative time.
pub fn format_timestamp(ts: &str) -> String {
    let Ok(secs) = ts.parse::<u64>() else {
        return ts.to_string();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age = now.saturating_sub(secs);
    if age < 60 {
        "just now".to_string()
    } else if age < 3600 {
        format!("{}m ago", age / 60)
    } else if age < 86400 {
        format!("{}h {}m ago", age / 3600, (age % 3600) / 60)
    } else {
        format!("{}d ago", age / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_meta_has_default_branch() {
        let meta = BranchMeta::new("main");
        assert_eq!(meta.default_branch, "main");
        assert!(meta.is_tracked("main"));
        assert_eq!(meta.branches["main"].db_file, "tokensave.db");
        assert!(meta.branches["main"].parent.is_none());
    }

    #[test]
    fn add_and_remove_branch() {
        let mut meta = BranchMeta::new("main");
        meta.add_branch("feature/foo", "branches/feature_foo.db", "main");
        assert!(meta.is_tracked("feature/foo"));
        assert_eq!(
            meta.branches["feature/foo"].parent.as_deref(),
            Some("main")
        );

        let removed = meta.remove_branch("feature/foo");
        assert!(removed.is_some());
        assert!(!meta.is_tracked("feature/foo"));
    }

    #[test]
    fn cannot_remove_default_branch() {
        let mut meta = BranchMeta::new("main");
        assert!(meta.remove_branch("main").is_none());
    }

    #[test]
    fn roundtrip_json() {
        let mut meta = BranchMeta::new("main");
        meta.add_branch("feature/bar", "branches/feature_bar.db", "main");
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: BranchMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_branch, "main");
        assert!(parsed.is_tracked("feature/bar"));
    }
}
