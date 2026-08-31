// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::branch;
use crate::branch_meta::{self, BranchMeta};
use crate::config::{
    get_tokensave_dir, is_excluded, is_excluded_dir, is_included, load_config, save_config,
    TokenSaveConfig,
};
use crate::context::ContextBuilder;
use crate::db::Database;
use crate::errors::{Result, TokenSaveError};
use crate::extraction::LanguageRegistry;
use crate::graph::{GraphQueryManager, GraphTraverser};
use crate::resolution::{AmbiguityRefKey, ReferenceResolver, TouchedNode, TouchedSet};
use crate::sync;
use crate::types::*;

mod extract;
mod guard;
mod indexing;
mod memory;
mod query;
mod staleness;
mod util;

pub(crate) use extract::*;
pub(crate) use guard::*;
pub use guard::{try_acquire_sync_lock, SyncLockGuard};
pub use indexing::detect_skipped_hidden_dirs;
pub use staleness::{AutoSyncScope, BranchDrift, DEFAULT_MAX_AUTO_SYNC_FILES};
pub use util::is_test_file;
pub(crate) use util::*;

/// Central orchestrator that coordinates all subsystems of the code graph.
///
/// Provides a high-level API for initializing, indexing, querying, and
/// syncing a Rust codebase's semantic knowledge graph.
pub struct TokenSave {
    db: Database,
    config: TokenSaveConfig,
    project_root: PathBuf,
    registry: LanguageRegistry,
    /// The active git branch (None if detached HEAD or not a git repo).
    active_branch: Option<String>,
    /// The branch whose DB is actually being served (may differ from `active_branch` on fallback).
    serving_branch: Option<String>,
    /// Set when serving from a fallback (ancestor) DB instead of the exact branch.
    fallback_warning: Option<String>,
    /// Ceiling on how many files a single *automatic* sync will take on
    /// before refusing (`0` disables the check). Read by
    /// [`Self::find_stale_files_bounded`]; explicit `sync` ignores it.
    /// Seeded from `config.max_auto_sync_files`.
    max_auto_sync_files: usize,
}

/// A decision recorded by an agent during a session.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DecisionRecord {
    /// Row id.
    pub id: i64,
    /// The decision text.
    pub text: String,
    /// Optional rationale for the decision.
    pub reason: Option<String>,
    /// UNIX timestamp (seconds) when the decision was recorded.
    pub created_at: i64,
    /// File paths relevant to this decision.
    pub files: Vec<String>,
    /// Arbitrary tags for categorisation.
    pub tags: Vec<String>,
}

/// A code area (file path) that an agent has touched during a session.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CodeAreaRecord {
    /// Row id.
    pub id: i64,
    /// Relative file path.
    pub path: String,
    /// Optional human-readable description of the area.
    pub description: Option<String>,
    /// UNIX timestamp (seconds) of the most recent touch.
    pub last_touched_at: i64,
    /// How many times this path has been touched.
    pub touch_count: u32,
}

/// Result of a full indexing operation.
pub struct IndexResult {
    /// Number of files scanned and indexed.
    pub file_count: usize,
    /// Total number of nodes extracted.
    pub node_count: usize,
    /// Total number of edges (extracted + resolved).
    pub edge_count: usize,
    /// Time taken in milliseconds.
    pub duration_ms: u64,
    /// Source-like extensions skipped because no registered extractor
    /// handles them, as `(extension, file_count)` sorted by count
    /// descending (#345). Known binary/asset extensions are omitted.
    pub skipped_extensions: Vec<(String, usize)>,
}

/// Result of an incremental sync operation.
#[derive(Debug)]
pub struct SyncResult {
    /// Number of newly added files.
    pub files_added: usize,
    /// Number of modified (re-indexed) files.
    pub files_modified: usize,
    /// Number of removed files.
    pub files_removed: usize,
    /// Time taken in milliseconds.
    pub duration_ms: u64,
    /// Paths of added files (populated only when doctor mode is requested).
    pub added_paths: Vec<String>,
    /// Paths of modified files (populated only when doctor mode is requested).
    pub modified_paths: Vec<String>,
    /// Paths of removed files (populated only when doctor mode is requested).
    pub removed_paths: Vec<String>,
    /// Files that were found on disk but could not be read (path, error message).
    pub skipped_paths: Vec<(String, String)>,
    /// Source-like extensions skipped because no registered extractor
    /// handles them, as `(extension, file_count)` sorted by count
    /// descending (#262, #270). Known binary/asset extensions are omitted.
    pub skipped_extensions: Vec<(String, usize)>,
}

/// Returns the current UNIX timestamp in seconds.
pub fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Initializes a new `TokenSave` project at the given root.
    ///
    /// Creates the `.tokensave` directory, writes a default configuration,
    /// and initializes a fresh `SQLite` database.
    pub async fn init(project_root: &Path) -> Result<Self> {
        let config = TokenSaveConfig {
            root_dir: project_root.to_string_lossy().to_string(),
            ..TokenSaveConfig::default()
        };
        save_config(project_root, &config)?;

        let db_path = get_tokensave_dir(project_root).join("tokensave.db");
        let (db, _migrated) = Database::initialize(&db_path).await?;

        // Bootstrap branch metadata if we can detect a default branch
        let active_branch = branch::current_branch(project_root);
        let default_branch =
            branch::detect_default_branch(project_root).or_else(|| active_branch.clone());
        if let Some(ref default) = default_branch {
            let meta = BranchMeta::new(default);
            let _ = branch_meta::save_branch_meta(&get_tokensave_dir(project_root), &meta);
        }

        Ok(Self {
            db,
            max_auto_sync_files: config.max_auto_sync_files,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
            active_branch,
            serving_branch: None,
            fallback_warning: None,
        })
    }

    /// Returns a reference to the underlying database.
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Overrides the automatic-sync file ceiling for this handle, without
    /// touching the on-disk config. Lets a caller (and the bound's tests)
    /// exercise the limit without materialising thousands of files.
    pub fn set_max_auto_sync_files(&mut self, limit: usize) {
        self.max_auto_sync_files = limit;
    }

    /// Returns `true` if the project DB schema is older than this build's latest.
    ///
    /// A stale schema (for example a project last indexed before a major upgrade
    /// added columns) signals that a forced reindex is needed to backfill the
    /// new schema. Falls back to `false` when the version cannot be read.
    pub async fn needs_schema_upgrade(&self) -> bool {
        crate::db::migrations::read_schema_version(self.db.conn())
            .await
            .is_ok_and(|v| v < crate::db::migrations::latest_version())
    }

    /// Opens an existing `TokenSave` project at the given root.
    ///
    /// If branch metadata exists, resolves the current git branch and opens
    /// the corresponding DB. Falls back to the nearest tracked ancestor DB
    /// with a warning if the current branch is untracked.
    /// If the previous operation was interrupted (dirty sentinel exists),
    /// the database is integrity-checked and rebuilt if corrupted.
    pub async fn open(project_root: &Path) -> Result<Self> {
        let config = load_config(project_root)?;
        let tokensave_dir = get_tokensave_dir(project_root);
        let active_branch = branch::current_branch(project_root);

        // Transparent auto-track: when branch metadata exists and the active
        // branch is untracked, copy the ancestor DB so queries + get_stats serve
        // a real per-branch DB instead of silently falling back. Best-effort —
        // never fail open() on this. Gated by config.auto_track, overridable
        // per-run via TOKENSAVE_AUTO_TRACK (git-hook path is separate).
        let auto_track =
            crate::config::env_bool_override("TOKENSAVE_AUTO_TRACK", config.auto_track);
        if auto_track {
            if let Some(b) = active_branch.as_deref() {
                match branch::track_branch_copy(project_root, &tokensave_dir, b).await {
                    Ok(true) => eprintln!(
                        "[tokensave] auto-tracked branch '{b}' (index copied from \
                         ancestor; run `tokensave sync` to refresh)"
                    ),
                    Ok(false) => {}
                    Err(e) => eprintln!("[tokensave] auto-track skipped for '{b}': {e}"),
                }
            }
        }

        let (db_path, serving_branch, fallback_warning) =
            Self::resolve_db_for_branch(project_root, &tokensave_dir, active_branch.as_deref());

        if !db_path.exists() {
            return Err(TokenSaveError::Config {
                message: format!(
                    "no TokenSave database found at '{}'; run 'tokensave init' first",
                    db_path.display()
                ),
            });
        }

        // If the dirty sentinel exists, a previous sync/index was interrupted.
        // Check integrity and rebuild if necessary.
        let crashed = has_dirty_sentinel(project_root);
        if crashed {
            eprintln!(
                "[tokensave] previous operation was interrupted — checking database integrity…"
            );
        }

        // Try to open; if the database is completely unreadable, delete and
        // re-initialize rather than failing permanently.
        let open_result = Database::open(&db_path).await;
        let (db, migrated) = match open_result {
            Ok(pair) => pair,
            Err(ref e) if Database::is_corruption_error(e) || crashed => {
                print_corruption_warning();
                delete_db_files(&db_path);
                clear_dirty_sentinel(project_root);
                let (db, _) = Database::initialize(&db_path).await?;
                let ts = Self {
                    db,
                    max_auto_sync_files: config.max_auto_sync_files,
                    config,
                    project_root: project_root.to_path_buf(),
                    registry: LanguageRegistry::new(),
                    active_branch: active_branch.clone(),
                    serving_branch: serving_branch.clone(),
                    fallback_warning: fallback_warning.clone(),
                };
                ts.index_all_with_progress(|c, t, f| {
                    eprintln!("[tokensave] re-indexing [{c}/{t}] {f}");
                })
                .await?;
                eprintln!("[tokensave] re-index complete.");
                return Ok(ts);
            }
            Err(e) => return Err(e),
        };

        // If the sentinel was set but the database opened successfully, run a
        // quick integrity check.
        if crashed {
            let intact = db.quick_check().await.unwrap_or(false);
            if !intact {
                print_corruption_warning();
                drop(db);
                delete_db_files(&db_path);
                clear_dirty_sentinel(project_root);
                let (new_db, _) = Database::initialize(&db_path).await?;
                let ts = Self {
                    db: new_db,
                    max_auto_sync_files: config.max_auto_sync_files,
                    config,
                    project_root: project_root.to_path_buf(),
                    registry: LanguageRegistry::new(),
                    active_branch: active_branch.clone(),
                    serving_branch: serving_branch.clone(),
                    fallback_warning: fallback_warning.clone(),
                };
                ts.index_all_with_progress(|c, t, f| {
                    eprintln!("[tokensave] re-indexing [{c}/{t}] {f}");
                })
                .await?;
                eprintln!("[tokensave] re-index complete.");
                return Ok(ts);
            }
            // `quick_check` validates the B-tree but cannot see a bulk load
            // that dropped its indexes and never recreated them: the rows are
            // structurally valid, just un-indexed and possibly duplicated
            // (#318). Detect that state and re-run finalization, which dedupes
            // the edges and rebuilds every dropped index — far cheaper than a
            // full re-index and enough to restore a correct, fast graph.
            if db.needs_bulk_load_finalization().await.unwrap_or(false) {
                eprintln!(
                    "[tokensave] previous bulk load did not finalize — deduping edges and rebuilding indexes…"
                );
                db.end_bulk_load().await?;
            }

            // DB is fine — clean up the stale sentinel.
            clear_dirty_sentinel(project_root);
        }

        let ts = Self {
            db,
            max_auto_sync_files: config.max_auto_sync_files,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
            active_branch,
            serving_branch,
            fallback_warning,
        };

        if migrated {
            eprintln!("[tokensave] schema changed — performing full re-index…");
            ts.index_all_with_progress(|current, total, file| {
                eprintln!("[tokensave] re-indexing [{current}/{total}] {file}");
            })
            .await?;
            eprintln!("[tokensave] re-index complete.");
        }

        Ok(ts)
    }

    /// Resolves which DB file to open for a given branch.
    ///
    /// Returns `(db_path, serving_branch, fallback_warning)`.
    /// `serving_branch` is the branch whose DB is actually opened.
    /// The warning is `Some` when falling back to an ancestor branch's DB.
    fn resolve_db_for_branch(
        project_root: &Path,
        tokensave_dir: &Path,
        branch: Option<&str>,
    ) -> (PathBuf, Option<String>, Option<String>) {
        let default_db = tokensave_dir.join("tokensave.db");

        let Some(meta) = branch_meta::load_branch_meta(tokensave_dir) else {
            // No branch metadata — single-DB mode (backward compat)
            return (default_db, None, None);
        };

        // The default branch is not necessarily mapped to `tokensave.db`: a
        // project whose default branch was added via `branch add` has it under
        // `branches/<name>.db`. Resolve it from branch-meta rather than
        // assuming the conventional filename (#300), falling back to
        // `tokensave.db` only when the metadata has no entry for it.
        let default_branch_db =
            branch::resolve_branch_db_path(tokensave_dir, &meta.default_branch, &meta)
                .unwrap_or_else(|| default_db.clone());

        let Some(branch) = branch else {
            // Detached HEAD — use default branch DB
            return (
                default_branch_db,
                Some(meta.default_branch.clone()),
                Some("detached HEAD — using default branch index".to_string()),
            );
        };

        // Exact match: branch is tracked
        if let Some(path) = branch::resolve_branch_db_path(tokensave_dir, branch, &meta) {
            if path.exists() {
                return (path, Some(branch.to_string()), None);
            }
        }

        // Fallback: find nearest tracked ancestor
        if let Some(ancestor) = branch::find_nearest_tracked_ancestor(project_root, branch, &meta) {
            if let Some(path) = branch::resolve_branch_db_path(tokensave_dir, &ancestor, &meta) {
                if path.exists() {
                    return (
                        path,
                        Some(ancestor.clone()),
                        Some(format!(
                            "branch '{branch}' is not tracked — serving from '{ancestor}'. \
                             Run `tokensave branch add {branch}` to track it."
                        )),
                    );
                }
            }
        }

        // Last resort: default branch DB
        let serving = meta.default_branch.clone();
        (
            default_branch_db,
            Some(serving),
            Some(format!(
                "branch '{branch}' is not tracked — serving from '{}'. \
                 Run `tokensave branch add {branch}` to track it.",
                meta.default_branch
            )),
        )
    }

    /// Opens an initialized project database without mutating it.
    pub async fn open_read_only(project_root: &Path, branch_name: Option<&str>) -> Result<Self> {
        let tokensave_dir = get_tokensave_dir(project_root);
        if !tokensave_dir.join("config.json").is_file() {
            return Err(TokenSaveError::Config {
                message: format!(
                    "'{}' is not an initialized TokenSave project root; \
                     run `tokensave init` there",
                    project_root.display()
                ),
            });
        }

        let config = load_config(project_root)?;
        let active_branch = branch_name
            .map(str::to_owned)
            .or_else(|| branch::current_branch(project_root));

        let (db_path, serving_branch, fallback_warning) = if let Some(explicit) = branch_name {
            let meta = branch_meta::load_branch_meta(&tokensave_dir).ok_or_else(|| {
                TokenSaveError::Config {
                    message: "no branch tracking configured; run `tokensave branch add` first"
                        .to_string(),
                }
            })?;
            let path = branch::resolve_branch_db_path(&tokensave_dir, explicit, &meta).ok_or_else(
                || TokenSaveError::Config {
                    message: format!(
                        "branch '{explicit}' is not tracked; run `tokensave branch add {}` \
                         in the selected project to track it",
                        shell_quote(explicit)
                    ),
                },
            )?;
            if !path.is_file() {
                return Err(TokenSaveError::Config {
                    message: format!(
                        "branch '{explicit}' is tracked but its DB is missing at '{}'; \
                         run `tokensave sync` in the selected project",
                        path.display()
                    ),
                });
            }
            (path, Some(explicit.to_string()), None)
        } else {
            Self::resolve_db_for_branch(project_root, &tokensave_dir, active_branch.as_deref())
        };

        if !db_path.is_file() {
            return Err(TokenSaveError::Config {
                message: format!(
                    "no TokenSave database found at '{}'; run `tokensave init` first",
                    db_path.display()
                ),
            });
        }

        let db = Database::open_read_only(&db_path).await?;
        Ok(Self {
            db,
            max_auto_sync_files: config.max_auto_sync_files,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
            active_branch,
            serving_branch,
            fallback_warning,
        })
    }

    /// Opens a specific branch's DB for read-only queries.
    ///
    /// Returns an error if the branch is not tracked or the DB doesn't exist.
    pub async fn open_branch(project_root: &Path, branch_name: &str) -> Result<Self> {
        let config = load_config(project_root)?;
        let tokensave_dir = get_tokensave_dir(project_root);

        let meta = branch_meta::load_branch_meta(&tokensave_dir).ok_or_else(|| {
            TokenSaveError::Config {
                message: "no branch tracking configured — run `tokensave branch add` first"
                    .to_string(),
            }
        })?;

        let db_path = branch::resolve_branch_db_path(&tokensave_dir, branch_name, &meta)
            .ok_or_else(|| TokenSaveError::Config {
                message: format!(
                    "branch '{branch_name}' is not tracked; run `tokensave branch add {}` \
                     to track it",
                    shell_quote(branch_name)
                ),
            })?;

        if !db_path.exists() {
            return Err(TokenSaveError::Config {
                message: format!(
                    "DB for branch '{branch_name}' not found at '{}'",
                    db_path.display()
                ),
            });
        }

        let (db, _) = Database::open(&db_path).await?;
        Ok(Self {
            db,
            max_auto_sync_files: config.max_auto_sync_files,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
            active_branch: Some(branch_name.to_string()),
            serving_branch: Some(branch_name.to_string()),
            fallback_warning: None,
        })
    }

    /// Lists tracked branches from metadata. Returns `None` if no branch tracking.
    pub fn list_tracked_branches(project_root: &Path) -> Option<Vec<String>> {
        let tokensave_dir = get_tokensave_dir(project_root);
        let meta = branch_meta::load_branch_meta(&tokensave_dir)?;
        Some(meta.branches.keys().cloned().collect())
    }

    /// Returns `true` if a `TokenSave` project has been initialized at the given root.
    pub fn is_initialized(project_root: &Path) -> bool {
        get_tokensave_dir(project_root)
            .join("tokensave.db")
            .exists()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod resolve_db_tests {
    use super::*;
    use crate::branch_meta::BranchEntry;

    /// Builds a `.tokensave` dir whose default branch lives in a non-default
    /// DB file, the shape that exposes hardcoded `tokensave.db` fallbacks.
    fn setup(default_branch: &str, db_file: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let tokensave_dir = dir.path().join(".tokensave");
        std::fs::create_dir_all(tokensave_dir.join("branches")).unwrap();
        let mut branches = HashMap::new();
        branches.insert(
            default_branch.to_string(),
            BranchEntry {
                db_file: db_file.to_string(),
                parent: None,
                created_at: "0".to_string(),
                last_synced_at: "0".to_string(),
            },
        );
        let meta = BranchMeta {
            default_branch: default_branch.to_string(),
            branches,
        };
        branch_meta::save_branch_meta(&tokensave_dir, &meta).unwrap();
        std::fs::write(tokensave_dir.join(db_file), b"").unwrap();
        (dir, tokensave_dir)
    }

    #[test]
    fn last_resort_resolves_default_branch_db_file() {
        // #300: an untracked branch with no tracked ancestor must open the
        // default branch's real db_file, not a hardcoded `tokensave.db`.
        let (dir, tokensave_dir) = setup("master", "branches/master.db");
        let (path, serving, warning) =
            TokenSave::resolve_db_for_branch(dir.path(), &tokensave_dir, Some("untracked-branch"));
        assert_eq!(
            path,
            tokensave_dir.join("branches/master.db"),
            "must resolve the default branch's db_file"
        );
        assert_eq!(serving.as_deref(), Some("master"));
        assert!(warning.is_some(), "fallback must still warn");
    }

    #[test]
    fn detached_head_resolves_default_branch_db_file() {
        // Same bug on the detached-HEAD arm: it reports serving the default
        // branch, so it must open that branch's DB.
        let (dir, tokensave_dir) = setup("master", "branches/master.db");
        let (path, serving, warning) =
            TokenSave::resolve_db_for_branch(dir.path(), &tokensave_dir, None);
        assert_eq!(path, tokensave_dir.join("branches/master.db"));
        assert_eq!(serving.as_deref(), Some("master"));
        assert!(warning.is_some());
    }

    #[test]
    fn conventional_default_branch_layout_is_unchanged() {
        // The common case (default branch mapped to tokensave.db) must not move.
        let (dir, tokensave_dir) = setup("main", "tokensave.db");
        let (path, serving, _) =
            TokenSave::resolve_db_for_branch(dir.path(), &tokensave_dir, Some("untracked"));
        assert_eq!(path, tokensave_dir.join("tokensave.db"));
        assert_eq!(serving.as_deref(), Some("main"));

        let (path, _, _) = TokenSave::resolve_db_for_branch(dir.path(), &tokensave_dir, None);
        assert_eq!(path, tokensave_dir.join("tokensave.db"));
    }

    #[test]
    fn tracked_branch_still_wins_over_fallback() {
        let (dir, tokensave_dir) = setup("master", "branches/master.db");
        let (path, serving, warning) =
            TokenSave::resolve_db_for_branch(dir.path(), &tokensave_dir, Some("master"));
        assert_eq!(path, tokensave_dir.join("branches/master.db"));
        assert_eq!(serving.as_deref(), Some("master"));
        assert!(warning.is_none(), "exact match must not warn");
    }

    #[test]
    fn missing_branch_meta_keeps_single_db_mode() {
        let dir = tempfile::TempDir::new().unwrap();
        let tokensave_dir = dir.path().join(".tokensave");
        std::fs::create_dir_all(&tokensave_dir).unwrap();
        let (path, serving, warning) =
            TokenSave::resolve_db_for_branch(dir.path(), &tokensave_dir, Some("anything"));
        assert_eq!(path, tokensave_dir.join("tokensave.db"));
        assert_eq!(serving, None);
        assert_eq!(warning, None);
    }
}
