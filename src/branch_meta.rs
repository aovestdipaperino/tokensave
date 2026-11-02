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

    /// Removes all tracked branches except the default. Returns removed entries.
    pub fn remove_all_branches(&mut self) -> Vec<(String, BranchEntry)> {
        let default = self.default_branch.clone();
        let removed: Vec<(String, BranchEntry)> = self
            .branches
            .keys()
            .filter(|name| *name != &default)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|name| self.branches.remove(&name).map(|e| (name, e)))
            .collect();
        removed
    }

    /// Returns true if the given branch is tracked.
    pub fn is_tracked(&self, name: &str) -> bool {
        self.branches.contains_key(name)
    }

    /// Renames the entry `from` to `to`, keeping its `db_file`. Follows the
    /// default branch and every `parent` that named `from`. Returns `false`
    /// (and changes nothing) when `from` is untracked or `to` already is.
    pub fn rename_branch(&mut self, from: &str, to: &str) -> bool {
        if from == to || self.branches.contains_key(to) {
            return false;
        }
        let Some(entry) = self.branches.remove(from) else {
            return false;
        };
        self.branches.insert(to.to_string(), entry);
        if self.default_branch == from {
            self.default_branch = to.to_string();
        }
        for entry in self.branches.values_mut() {
            if entry.parent.as_deref() == Some(from) {
                entry.parent = Some(to.to_string());
            }
        }
        true
    }

    /// Makes the tracked branch `name` the default. The previous default
    /// keeps its entry and database as an ordinary branch, which
    /// `remove_branch` can then drop. Returns `false` when `name` is untracked.
    pub fn set_default(&mut self, name: &str) -> bool {
        let Some(entry) = self.branches.get_mut(name) else {
            return false;
        };
        // The default branch is the root of the parent chain.
        entry.parent = None;
        self.default_branch = name.to_string();
        true
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
///
/// The write is atomic: the JSON is written to a temporary file in the same
/// directory, flushed to disk, then renamed over the target. A concurrent
/// `track_branch_copy` or an interrupted write can therefore never leave a
/// torn `branch-meta.json` behind (the rename is atomic on POSIX and Windows).
pub fn save_branch_meta(tokensave_dir: &Path, meta: &BranchMeta) -> std::io::Result<()> {
    use std::io::Write;
    let path = tokensave_dir.join(BRANCH_META_FILENAME);
    let json = serde_json::to_string_pretty(meta).map_err(std::io::Error::other)?;
    let dir = path.parent().unwrap_or(tokensave_dir);
    // Unique temp name in the same directory so the final rename is atomic and
    // two concurrent writers cannot clobber each other's temp file.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(
        ".{BRANCH_META_FILENAME}.{}.{nanos}.tmp",
        std::process::id()
    ));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn save_branch_meta_is_atomic_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut meta = BranchMeta::new("main");
        meta.add_branch("feature/foo", "branches/feature_foo.db", "main");
        save_branch_meta(dir.path(), &meta).unwrap();

        // The live file loads back intact.
        let loaded = load_branch_meta(dir.path()).unwrap();
        assert!(loaded.is_tracked("feature/foo"));

        // No leftover temp file next to it (only branch-meta.json remains).
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(entries, vec![BRANCH_META_FILENAME.to_string()]);

        // Overwriting is also clean (rename over an existing file).
        meta.add_branch("feature/bar", "branches/feature_bar.db", "main");
        save_branch_meta(dir.path(), &meta).unwrap();
        let entries2: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(entries2, vec![BRANCH_META_FILENAME.to_string()]);
    }

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
        assert_eq!(meta.branches["feature/foo"].parent.as_deref(), Some("main"));

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
    fn rename_branch_keeps_db_file_and_rewrites_parents() {
        let mut meta = BranchMeta::new("master");
        meta.add_branch("feature", "branches/feature.db", "master");
        assert!(meta.rename_branch("master", "main"));
        assert_eq!(meta.default_branch, "main");
        assert!(!meta.is_tracked("master"));
        assert_eq!(meta.branches["main"].db_file, "tokensave.db");
        assert_eq!(meta.branches["feature"].parent.as_deref(), Some("main"));
    }

    #[test]
    fn rename_branch_refuses_to_overwrite_a_tracked_branch() {
        let mut meta = BranchMeta::new("master");
        meta.add_branch("main", "branches/main.db", "master");
        assert!(!meta.rename_branch("master", "main"));
        assert!(!meta.rename_branch("gone", "other"));
        assert_eq!(meta.default_branch, "master");
        assert_eq!(meta.branches["main"].db_file, "branches/main.db");
    }

    #[test]
    fn set_default_demotes_the_previous_default() {
        let mut meta = BranchMeta::new("master");
        meta.add_branch("main", "branches/main.db", "master");
        assert!(meta.set_default("main"));
        assert_eq!(meta.default_branch, "main");
        assert!(meta.branches["main"].parent.is_none());
        // The old default is now an ordinary, removable entry.
        assert!(meta.remove_branch("master").is_some());
        assert!(!meta.set_default("gone"));
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
