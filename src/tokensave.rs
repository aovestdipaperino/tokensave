// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

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
use crate::resolution::ReferenceResolver;
use crate::sync;
use crate::types::*;

/// Run `extractor.extract()` inside `catch_unwind` so a panic (e.g. from a
/// malformed file or an extractor bug) skips the file instead of aborting sync.
fn safe_extract(
    extractor: &dyn crate::extraction::LanguageExtractor,
    file_path: &str,
    source: &str,
) -> Option<ExtractionResult> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        extractor.extract(file_path, source)
    }))
    .map_err(|_| {
        eprintln!("[tokensave] extraction panicked for {file_path}, skipping");
    })
    .ok()
}

/// Tuple shape produced per file by both extraction paths.
type ExtractTuple = (String, ExtractionResult, String, u64, i64);

/// Extract every file in `files`, isolating each extraction in a subprocess
/// when possible. Subprocess isolation contains C/C++ grammar aborts that
/// `catch_unwind` cannot intercept; it is the primary defense against
/// tree-sitter scanners that call `abort()` (issue #49).
///
/// Falls back to in-process extraction with `safe_extract` if the worker
/// pool cannot start (e.g. when running under `cargo test`, where
/// `current_exe()` points at the test harness rather than the tokensave
/// binary). Either way, returns one tuple per successfully-processed file
/// plus a list of `(path, reason)` pairs for files that timed out or
/// repeatedly crashed during extraction.
fn extract_files_isolated(
    project_root: &Path,
    registry: &crate::extraction::LanguageRegistry,
    files: Vec<String>,
) -> (Vec<ExtractTuple>, Vec<(String, String)>) {
    if should_use_subprocess() {
        let workers = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
        let timeout = std::time::Duration::from_secs(
            crate::user_config::UserConfig::load().extraction_timeout_secs,
        );
        match crate::extraction_worker::WorkerPool::new(workers, project_root.to_path_buf()) {
            Ok(pool) => {
                let outcome = pool.extract_files(files, |_, _, _| {}, timeout);
                return (outcome.results, outcome.skipped);
            }
            Err(e) => eprintln!(
                "[tokensave] could not spawn extraction worker pool ({e}), \
                 falling back to in-process extraction"
            ),
        }
    }
    (
        extract_files_in_process(project_root, registry, &files),
        Vec::new(),
    )
}

fn extract_files_in_process(
    project_root: &Path,
    registry: &crate::extraction::LanguageRegistry,
    files: &[String],
) -> Vec<ExtractTuple> {
    files
        .par_iter()
        .filter_map(|file_path| {
            let abs_path = project_root.join(file_path);
            let source = sync::read_source_file(&abs_path).ok()?;
            let extractor = registry.extractor_for_file(file_path)?;
            let mut result = safe_extract(extractor, file_path, &source)?;
            result.sanitize();
            let hash = sync::content_hash(&source);
            let size = source.len() as u64;
            let mtime = sync::file_stat(&abs_path).map_or_else(current_timestamp, |(m, _)| m);
            Some((file_path.clone(), result, hash, size, mtime))
        })
        .collect()
}

/// Subprocess extraction is the production path. Tests and any environment
/// where `current_exe()` does not point at the real `tokensave` binary
/// transparently fall back to in-process extraction.
fn should_use_subprocess() -> bool {
    if std::env::var_os("TOKENSAVE_DISABLE_SUBPROCESS").is_some() {
        return false;
    }
    let Ok(path) = std::env::current_exe() else {
        return false;
    };
    matches!(path.file_stem().and_then(|s| s.to_str()), Some("tokensave"))
}

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
            // DB is fine — clean up the stale sentinel.
            clear_dirty_sentinel(project_root);
        }

        let ts = Self {
            db,
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

        let Some(branch) = branch else {
            // Detached HEAD — use default branch DB
            return (
                default_db,
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
            default_db,
            Some(serving),
            Some(format!(
                "branch '{branch}' is not tracked — serving from '{}'. \
                 Run `tokensave branch add {branch}` to track it.",
                meta.default_branch
            )),
        )
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
                message: format!("branch '{branch_name}' is not tracked"),
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

// ---------------------------------------------------------------------------
// Dirty sentinel — detects interrupted sync/index operations
// ---------------------------------------------------------------------------

/// Creates a `.tokensave/dirty` sentinel file before a sync or index begins.
///
/// This file is intentionally NOT cleaned up by a Drop guard — it must be
/// removed explicitly by `clear_dirty_sentinel` after the operation succeeds.
/// If the process is killed (SIGKILL, OOM), the sentinel survives and signals
/// a potential crash on the next open.
fn write_dirty_sentinel(project_root: &Path) {
    let path = get_tokensave_dir(project_root).join("dirty");
    let _ = std::fs::write(
        &path,
        format!(
            "pid={}\ntime={}\nversion={}",
            std::process::id(),
            current_timestamp(),
            env!("CARGO_PKG_VERSION"),
        ),
    );
}

/// Removes the dirty sentinel after a successful sync/index.
fn clear_dirty_sentinel(project_root: &Path) {
    let path = get_tokensave_dir(project_root).join("dirty");
    let _ = std::fs::remove_file(path);
}

/// Returns `true` if the dirty sentinel exists (previous operation was
/// interrupted).
fn has_dirty_sentinel(project_root: &Path) -> bool {
    get_tokensave_dir(project_root).join("dirty").exists()
}

/// Deletes the database and its WAL/SHM sidecars.
fn delete_db_files(db_path: &std::path::Path) {
    let _ = std::fs::remove_file(db_path);
    // WAL and SHM files use the same base name with different extensions
    let mut wal = db_path.to_path_buf();
    wal.set_extension("db-wal");
    let _ = std::fs::remove_file(&wal);
    wal.set_extension("db-shm");
    let _ = std::fs::remove_file(&wal);
}

/// Prints a user-facing warning about database corruption with a request to
/// report the issue.
fn print_corruption_warning() {
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("[tokensave] \x1b[33m⚠ database corruption detected — rebuilding index\x1b[0m");
    eprintln!("[tokensave]");
    eprintln!("[tokensave] This was likely caused by a crash or kill during indexing.");
    eprintln!("[tokensave] Please report this at:");
    eprintln!("[tokensave]   https://github.com/aovestdipaperino/tokensave/issues");
    eprintln!(
        "[tokensave]   Include: tokensave version (v{version}), OS, and what happened before the crash."
    );
    eprintln!("[tokensave]");
}

// ---------------------------------------------------------------------------
// Sync lock — prevents concurrent sync/index operations
// ---------------------------------------------------------------------------

/// RAII guard that holds the sync lockfile open. Removing the lockfile on drop
/// is best-effort; if it fails (e.g. permissions), the stale-PID check on the
/// next attempt will reclaim it.
///
/// Internal: exposed for integration tests; not part of the stable public API.
#[doc(hidden)]
pub struct SyncLockGuard {
    path: PathBuf,
}

impl Drop for SyncLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Try to acquire the sync lock for `project_root`.
///
/// Creates `.tokensave/sync.lock` containing the current PID. If the file
/// already exists and the PID inside is still alive, returns a `SyncLock`
/// error. Stale lockfiles (dead PID or unreadable content) are reclaimed
/// automatically.
///
/// Internal: exposed for integration tests; not part of the stable public API.
#[doc(hidden)]
pub fn try_acquire_sync_lock(project_root: &Path) -> Result<SyncLockGuard> {
    use std::io::Write;
    let lock_path = get_tokensave_dir(project_root).join("sync.lock");
    let pid = std::process::id();

    // Fast path: try atomic create.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut f) => {
            let _ = write!(f, "{pid}");
            return Ok(SyncLockGuard { path: lock_path });
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Fall through to stale-check below.
        }
        Err(e) => {
            return Err(TokenSaveError::SyncLock {
                message: format!("could not create lockfile: {e}"),
            });
        }
    }

    // Lockfile exists — check if the owning process is still alive.
    let contents = std::fs::read_to_string(&lock_path).unwrap_or_default();
    if let Ok(existing_pid) = contents.trim().parse::<u32>() {
        if is_pid_alive(existing_pid) {
            return Err(TokenSaveError::SyncLock {
                message: format!(
                    "another sync is already in progress (PID {existing_pid}). \
                     If this is stale, remove {}",
                    lock_path.display()
                ),
            });
        }
    }

    // Stale lock — reclaim it.
    let _ = std::fs::remove_file(&lock_path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|e| TokenSaveError::SyncLock {
            message: format!("could not reclaim lockfile: {e}"),
        })?;
    let _ = write!(f, "{pid}");
    Ok(SyncLockGuard { path: lock_path })
}

/// Returns `true` if a process with the given PID is currently running.
fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Appends runtime skip-folder patterns to the exclude list.
    ///
    /// Each folder name is converted to a `folder/**` glob so that all
    /// files underneath it are excluded during scanning.
    pub fn add_skip_folders(&mut self, folders: &[String]) {
        for folder in folders {
            self.config.exclude.push(format!("{folder}/**"));
        }
    }

    /// Performs a full index: clears existing data, scans all Rust files,
    /// extracts nodes and edges, resolves references, and stores everything
    /// in the database.
    pub async fn index_all(&self) -> Result<IndexResult> {
        self.index_all_with_progress(|_, _, _| {}).await
    }

    /// Like `index_all()`, but calls `on_file(current, total, path)` before
    /// processing each file. Use this to drive a progress spinner with ETA in
    /// the CLI.
    pub async fn index_all_with_progress<F>(&self, on_file: F) -> Result<IndexResult>
    where
        F: Fn(usize, usize, &str),
    {
        self.index_all_with_progress_verbose(on_file, |_| {}).await
    }

    /// Like `index_all_with_progress()`, but also calls `on_verbose` after
    /// each phase completes with a diagnostic summary line.
    pub async fn index_all_with_progress_verbose<F, V>(
        &self,
        on_file: F,
        on_verbose: V,
    ) -> Result<IndexResult>
    where
        F: Fn(usize, usize, &str),
        V: Fn(&str),
    {
        debug_assert!(self.project_root.exists(), "project root does not exist");
        debug_assert!(
            self.project_root.is_dir(),
            "project root is not a directory"
        );
        let _lock = try_acquire_sync_lock(&self.project_root)?;
        write_dirty_sentinel(&self.project_root);
        let start = Instant::now();

        // 1. Clear existing data and enter bulk-load mode
        self.db.clear().await?;
        self.db.begin_bulk_load().await?;

        // 2. Scan for source files
        let phase_start = Instant::now();
        let files = self.scan_files();
        let total = files.len();
        on_verbose(&format!(
            "scanned {} files in {:.1}s",
            total,
            phase_start.elapsed().as_secs_f64()
        ));

        // 3. Parallel extraction: read + parse + hash on all cores
        let project_root = self.project_root.clone();
        let registry = &self.registry;

        let phase_start = Instant::now();
        let (extractions, _skipped) =
            extract_files_isolated(&project_root, registry, files.clone());

        // 4. Collect all data
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();
        let mut all_unresolved = Vec::new();
        let mut file_records = Vec::new();
        let mut total_nodes = 0;

        for (idx, (file_path, result, hash, size, mtime)) in extractions.iter().enumerate() {
            on_file(idx + 1, total, file_path);
            total_nodes += result.nodes.len();
            all_nodes.extend_from_slice(&result.nodes);
            all_edges.extend_from_slice(&result.edges);
            all_unresolved.extend_from_slice(&result.unresolved_refs);
            file_records.push(FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            });
        }

        on_verbose(&format!(
            "extracted {} nodes, {} edges from {} files in {:.1}s",
            total_nodes,
            all_edges.len(),
            extractions.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // 5. Resolve references in-memory (parallel) before DB insert
        let phase_start = Instant::now();
        if !all_unresolved.is_empty() {
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            let resolution = resolver.resolve_all(&all_unresolved);
            all_edges.extend(resolver.create_edges(&resolution.resolved));
        }
        on_verbose(&format!(
            "resolved {} references in {:.1}s",
            all_unresolved.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // 6. Sort by PK order + dedup edges
        all_nodes.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        all_edges.sort_unstable_by(|a, b| {
            (&a.source, &a.target, a.kind.as_str(), &a.line).cmp(&(
                &b.source,
                &b.target,
                b.kind.as_str(),
                &b.line,
            ))
        });
        all_edges.dedup_by(|a, b| {
            a.source == b.source && a.target == b.target && a.kind == b.kind && a.line == b.line
        });
        file_records.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        let total_edges = all_edges.len();

        // 7. Bulk-insert via prepared statements (zero SQL re-parsing)
        let phase_start = Instant::now();
        self.db.insert_nodes(&all_nodes).await?;
        self.db.insert_edges(&all_edges).await?;
        self.db.upsert_files(&file_records).await?;

        // 8. Restore indexes and normal durability
        self.db.end_bulk_load().await?;
        on_verbose(&format!(
            "wrote to database in {:.1}s",
            phase_start.elapsed().as_secs_f64()
        ));

        let duration_ms = start.elapsed().as_millis() as u64;
        let now_str = current_timestamp().to_string();
        self.db.set_metadata("last_full_sync_at", &now_str).await?;
        self.db.set_metadata("last_sync_at", &now_str).await?;
        self.db
            .set_metadata("last_sync_duration_ms", &duration_ms.to_string())
            .await?;

        let result = IndexResult {
            file_count: files.len(),
            node_count: total_nodes,
            edge_count: total_edges,
            duration_ms,
        };
        debug_assert!(
            result.node_count >= result.file_count || result.file_count == 0,
            "fewer nodes than files is unexpected"
        );
        debug_assert!(
            result.duration_ms > 0 || result.file_count == 0,
            "non-empty index completed in zero milliseconds"
        );
        clear_dirty_sentinel(&self.project_root);
        Ok(result)
    }

    /// Performs an incremental sync: detects changed, new, and removed files
    /// and re-indexes only those that need updating.
    pub async fn sync(&self) -> Result<SyncResult> {
        self.sync_with_progress(|_, _, _| {}).await
    }

    /// Like `sync()`, but calls `on_progress` for spinner updates.
    /// Equivalent to `sync_with_progress_verbose(on_progress, |_| {})`.
    pub async fn sync_with_progress<F>(&self, on_progress: F) -> Result<SyncResult>
    where
        F: Fn(usize, usize, &str),
    {
        self.sync_with_progress_verbose(on_progress, |_| {}).await
    }

    /// Sync only the specified files if they are stale, then recheck.
    ///
    /// Returns `Ok(false)` if all files are now in sync after the call.
    /// Returns `Ok(true)` if files are still stale after sync (either sync
    /// didn't update these specific files, or sync failed to acquire lock).
    /// Returns `Err` on sync failure.
    pub async fn sync_if_stale(&self, stale_files: &[String]) -> Result<bool> {
        if stale_files.is_empty() {
            return Ok(false);
        }

        // Quick check: are these files still stale before we even try to sync?
        let still_stale_before = self.check_file_staleness(stale_files).await;
        if still_stale_before.is_empty() {
            return Ok(false);
        }

        // Try to acquire sync lock and do an incremental sync.
        // The full sync will pick up any changed files, including our stale ones.
        let Ok(lock) = try_acquire_sync_lock(&self.project_root) else {
            // Another sync is in progress (likely another MCP peer) — let caller warn
            return Ok(true);
        };

        // Do a minimal sync focused on changed files
        let result = self.sync_single_files(stale_files).await;

        // Release lock
        drop(lock);

        match result {
            Ok(()) => {
                // Recheck if our files are still stale
                let still_stale_after = self.check_file_staleness(stale_files).await;
                Ok(!still_stale_after.is_empty())
            }
            Err(_) => Ok(true), // Sync failed — warn caller
        }
    }

    /// Like `sync_if_stale` but treats lock contention as success.
    ///
    /// Use this from the embedded MCP watcher when another MCP (or any peer
    /// process) already holds the project sync lock — the peer will produce
    /// the updated DB, so this caller has nothing to do and no reason to warn.
    pub async fn sync_if_stale_silent(&self, stale_files: &[String]) -> Result<()> {
        if stale_files.is_empty() {
            return Ok(());
        }

        let still_stale_before = self.check_file_staleness(stale_files).await;
        if still_stale_before.is_empty() {
            return Ok(());
        }

        let Ok(lock) = try_acquire_sync_lock(&self.project_root) else {
            // Peer is syncing. That's fine — they'll write the updated DB.
            return Ok(());
        };

        let _ = self.sync_single_files(stale_files).await;
        drop(lock);
        Ok(())
    }

    /// Index/reexamine the given file paths, updating their graph nodes and edges.
    /// This is a focused, single-shot operation used by `sync_if_stale`.
    async fn sync_single_files(&self, file_paths: &[String]) -> Result<()> {
        use crate::sync as sync_mod;

        let start = Instant::now();
        let project_root = &self.project_root;
        let registry = &self.registry;

        // Read and hash the files
        let mut hash_map: HashMap<String, String> = HashMap::new();
        let mut stat_map: HashMap<String, (i64, u64)> = HashMap::new();

        for path in file_paths {
            let abs_path = project_root.join(path);
            if let Some((mtime, size)) = sync_mod::file_stat(&abs_path) {
                stat_map.insert(path.clone(), (mtime, size));
            }
            if let Ok(source) = sync_mod::read_source_file(&abs_path) {
                let hash = sync_mod::content_hash(&source);
                hash_map.insert(path.clone(), hash);
            }
        }

        // Extract graph data from the files in parallel (subprocess-isolated)
        let _ = stat_map; // worker re-stats internally; map kept for potential future use
        let (sync_extractions, _skipped_extractions) =
            extract_files_isolated(project_root, registry, file_paths.to_vec());

        // Phase 1: insert all nodes (and metadata) so cross-file edges
        // can reference them. Edges are queued for phase 2 (#58).
        let mut queued_edges: Vec<&Edge> = Vec::new();
        for (file_path, result, hash, size, mtime) in &sync_extractions {
            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            queued_edges.extend(&result.edges);
            if !result.unresolved_refs.is_empty() {
                self.db
                    .insert_unresolved_refs(&result.unresolved_refs)
                    .await?;
            }

            let file_record = FileRecord {
                path: (*file_path).clone(),
                content_hash: (*hash).clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record).await?;
        }

        // Phase 2: insert all queued edges now that every node is present.
        // The conditional INSERT in `insert_edges` silently skips edges
        // whose endpoints are truly missing (e.g. unindexed files).
        if !queued_edges.is_empty() {
            let owned: Vec<Edge> = queued_edges.into_iter().cloned().collect();
            self.db.insert_edges(&owned).await?;
        }

        // Resolve references for any new/changed unresolved refs
        if !file_paths.is_empty() {
            let all_nodes = self.db.get_all_nodes().await.unwrap_or_default();
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            let unresolved = self.db.get_unresolved_refs().await?;
            if !unresolved.is_empty() {
                let resolution = resolver.resolve_all(&unresolved);
                let edges = resolver.create_edges(&resolution.resolved);
                if !edges.is_empty() {
                    self.db.insert_edges(&edges).await?;
                }
            }
        }

        self.db
            .set_metadata("last_sync_at", &current_timestamp().to_string())
            .await?;
        self.db
            .set_metadata(
                "last_sync_duration_ms",
                &start.elapsed().as_millis().to_string(),
            )
            .await?;

        clear_dirty_sentinel(&self.project_root);
        Ok(())
    }

    /// Like `sync()`, but calls `on_progress` with a description and the
    /// current step for each phase of work, and `on_verbose` after each phase
    /// completes with a diagnostic summary line (count + timing).
    ///
    /// The progress callback receives `(current_file_index, total_files, message)`
    /// where `current_file_index` and `total_files` are zero during non-file phases
    /// (scanning, hashing, detecting, resolving) and populated during the
    /// per-file syncing phase.
    pub async fn sync_with_progress_verbose<F, V>(
        &self,
        on_progress: F,
        on_verbose: V,
    ) -> Result<SyncResult>
    where
        F: Fn(usize, usize, &str),
        V: Fn(&str),
    {
        debug_assert!(
            self.project_root.exists(),
            "sync: project root does not exist"
        );
        debug_assert!(
            self.project_root.is_dir(),
            "sync: project root is not a directory"
        );
        let _lock = try_acquire_sync_lock(&self.project_root)?;
        write_dirty_sentinel(&self.project_root);
        let start = Instant::now();

        on_progress(0, 0, "scanning files");
        let phase_start = Instant::now();
        let current_files = self.scan_files();
        on_verbose(&format!(
            "scanned {} files in {:.1}s",
            current_files.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // Stat all files in parallel to get (mtime, size) — ~11ms for 20k files
        on_progress(0, 0, "checking file timestamps");
        let phase_start = Instant::now();
        let project_root = &self.project_root;
        let file_stats: Vec<(String, i64, u64)> = current_files
            .par_iter()
            .filter_map(|path| {
                let abs_path = project_root.join(path);
                let (mtime, size) = sync::file_stat(&abs_path)?;
                Some((path.clone(), mtime, size))
            })
            .collect();
        on_verbose(&format!(
            "stat-checked {} files in {:.1}s",
            file_stats.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // Load all DB file records into a map for O(1) lookups
        let db_files = self.db.get_all_files().await?;
        let db_map: HashMap<String, FileRecord> =
            db_files.into_iter().map(|f| (f.path.clone(), f)).collect();

        // Partition files by comparing (mtime, size) against stored values
        let mut new_files: Vec<String> = Vec::new();
        let mut stat_changed: Vec<String> = Vec::new();
        let mut current_set: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(file_stats.len());
        let mut stat_map: HashMap<String, (i64, u64)> = HashMap::with_capacity(file_stats.len());

        for (path, mtime, size) in &file_stats {
            current_set.insert(path.as_str());
            stat_map.insert(path.clone(), (*mtime, *size));
            match db_map.get(path) {
                None => new_files.push(path.clone()),
                Some(record) => {
                    if record.modified_at != *mtime || record.size != *size {
                        stat_changed.push(path.clone());
                    }
                }
            }
        }

        // Detect removed files from the same DB map
        let removed: Vec<String> = db_map
            .keys()
            .filter(|path| !current_set.contains(path.as_str()))
            .cloned()
            .collect();

        on_verbose(&format!(
            "changes: {} new, {} stat-changed, {} removed, {} unchanged",
            new_files.len(),
            stat_changed.len(),
            removed.len(),
            file_stats.len() - new_files.len() - stat_changed.len()
        ));

        // Read + hash only files with changed stats or new files
        on_progress(0, 0, "hashing changed files");
        let phase_start = Instant::now();
        let needs_read: Vec<&String> = new_files.iter().chain(stat_changed.iter()).collect();
        let hash_results: Vec<_> = needs_read
            .par_iter()
            .map(|path| {
                let abs_path = project_root.join(path.as_str());
                match sync::read_source_file(&abs_path) {
                    Ok(source) => Ok(((*path).clone(), sync::content_hash(&source))),
                    Err(e) => Err(((*path).clone(), e.to_string())),
                }
            })
            .collect();

        let mut skipped: Vec<(String, String)> = Vec::new();
        let mut hash_map: HashMap<String, String> = HashMap::new();
        for result in hash_results {
            match result {
                Ok((path, hash)) => {
                    hash_map.insert(path, hash);
                }
                Err((path, reason)) => {
                    skipped.push((path, reason));
                }
            }
        }
        on_verbose(&format!(
            "hashed {} files in {:.1}s ({} read errors)",
            hash_map.len(),
            phase_start.elapsed().as_secs_f64(),
            skipped.len()
        ));

        // Among stat_changed files, find those with actually different content
        on_progress(0, 0, "detecting changes");
        let mut stale: Vec<String> = Vec::new();
        let mut mtime_only_changed: Vec<String> = Vec::new();
        for path in &stat_changed {
            if let Some(new_hash) = hash_map.get(path) {
                if let Some(record) = db_map.get(path) {
                    if record.content_hash == *new_hash {
                        // mtime changed but content identical (e.g. touch) —
                        // update stored mtime so we skip it next time
                        mtime_only_changed.push(path.clone());
                    } else {
                        stale.push(path.clone());
                    }
                }
            }
        }
        on_verbose(&format!(
            "content check: {} modified, {} mtime-only",
            stale.len(),
            mtime_only_changed.len()
        ));

        // Update mtime for false-positive files so future syncs skip them
        for path in &mtime_only_changed {
            if let (Some(record), Some(&(mtime, size))) = (db_map.get(path), stat_map.get(path)) {
                let updated = FileRecord {
                    modified_at: mtime,
                    size,
                    ..record.clone()
                };
                self.db.upsert_file(&updated).await?;
            }
        }

        // Remove deleted files
        for path in &removed {
            on_progress(0, 0, &format!("removing {path}"));
            self.db.delete_file(path).await?;
        }

        // Re-index stale and new files — extract in parallel, insert sequentially
        let to_index: Vec<String> = stale.iter().chain(new_files.iter()).cloned().collect();
        let registry = &self.registry;

        let phase_start = Instant::now();
        let _ = stat_map; // worker re-stats internally
        let (sync_extractions, sync_skipped): (Vec<_>, Vec<_>) =
            extract_files_isolated(project_root, registry, to_index.clone());
        // Surface extractor timeouts/crashes in `SyncResult.skipped_paths`
        // so the user can see them in `tokensave sync --doctor`.
        skipped.extend(sync_skipped);

        // Phase 1: insert all nodes (and metadata) so cross-file edges
        // can reference them. Edges are queued for phase 2 (#58).
        let total = sync_extractions.len();
        let mut total_nodes = 0usize;
        let mut total_edges = 0usize;
        let mut queued_edges: Vec<&Edge> = Vec::new();
        for (idx, (file_path, result, hash, size, mtime)) in sync_extractions.iter().enumerate() {
            on_progress(idx + 1, total, file_path);

            total_nodes += result.nodes.len();
            total_edges += result.edges.len();

            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            queued_edges.extend(&result.edges);
            if !result.unresolved_refs.is_empty() {
                self.db
                    .insert_unresolved_refs(&result.unresolved_refs)
                    .await?;
            }

            let file_record = FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record).await?;
        }

        // Phase 2: insert all queued edges now that every node is present.
        if !queued_edges.is_empty() {
            let owned: Vec<Edge> = queued_edges.into_iter().cloned().collect();
            self.db.insert_edges(&owned).await?;
        }

        if !to_index.is_empty() {
            on_verbose(&format!(
                "indexed {} files ({} nodes, {} edges) in {:.1}s",
                to_index.len(),
                total_nodes,
                total_edges,
                phase_start.elapsed().as_secs_f64()
            ));
        }

        // Resolve references (call edges, uses, etc.) across all files.
        // This must run after all files are indexed so cross-file references
        // can find their targets.
        if !to_index.is_empty() {
            on_progress(0, 0, "resolving references");
            let phase_start = Instant::now();
            let unresolved = self.db.get_unresolved_refs().await?;
            if !unresolved.is_empty() {
                let all_nodes = self.db.get_all_nodes().await.unwrap_or_default();
                let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
                let resolution = resolver.resolve_all(&unresolved);
                let edges = resolver.create_edges(&resolution.resolved);
                if !edges.is_empty() {
                    self.db.insert_edges(&edges).await?;
                }
            }
            on_verbose(&format!(
                "resolved {} references in {:.1}s",
                unresolved.len(),
                phase_start.elapsed().as_secs_f64()
            ));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        self.db
            .set_metadata("last_sync_at", &current_timestamp().to_string())
            .await?;
        self.db
            .set_metadata("last_sync_duration_ms", &duration_ms.to_string())
            .await?;

        clear_dirty_sentinel(&self.project_root);
        Ok(SyncResult {
            files_added: new_files.len(),
            files_modified: stale.len(),
            files_removed: removed.len(),
            duration_ms,
            added_paths: new_files,
            modified_paths: stale,
            skipped_paths: skipped,
            removed_paths: removed,
        })
    }

    /// Scans the project root for source files in all supported languages,
    /// respecting the configured exclude patterns and max file size.
    ///
    /// When `git_ignore` is enabled in the config, `.gitignore` rules are
    /// applied via the `ignore` crate. Otherwise, hidden directories and
    /// `target/` are skipped with a simple name-based filter.
    ///
    /// Supported extensions are derived from the `LanguageRegistry` so that
    /// adding a new extractor automatically picks up its files.
    fn scan_files(&self) -> Vec<String> {
        debug_assert!(
            self.project_root.is_dir(),
            "scan_files: project_root is not a directory"
        );
        let supported_exts = self.registry.supported_extensions();
        debug_assert!(
            !supported_exts.is_empty(),
            "scan_files: no supported extensions registered"
        );

        if self.config.git_ignore {
            let files = self.scan_files_with_gitignore(&supported_exts);
            if files.is_empty() {
                // The project directory may be gitignored by a parent repo,
                // causing the ignore-aware walker to skip everything. Fall
                // back to plain walkdir if source files clearly exist.
                let has_source = WalkDir::new(&self.project_root)
                    .follow_links(true)
                    .max_depth(2)
                    .into_iter()
                    .filter_map(std::result::Result::ok)
                    .any(|e| {
                        e.file_type().is_file()
                            && e.path()
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| supported_exts.contains(&ext))
                    });
                if has_source {
                    eprintln!("warning: gitignore-aware scan found no files; falling back to plain walk (project may be gitignored by parent repo)");
                    return self.scan_files_walkdir(&supported_exts);
                }
            }
            files
        } else {
            self.scan_files_walkdir(&supported_exts)
        }
    }

    /// Walk using `walkdir`, skipping hidden directories and `target/`.
    ///
    /// Hidden (dot-prefixed) entries that match a configured `include` glob
    /// are allowed through despite the default filter.
    fn scan_files_walkdir(&self, supported_exts: &[&str]) -> Vec<String> {
        let mut files = Vec::new();
        let root = &self.project_root;
        let config = &self.config;
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || name == "target" {
                    // Allow if the relative path matches an include glob.
                    if let Ok(rel) = e.path().strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        return is_included(&rel_str, config);
                    }
                    return false;
                }
                // Prune directories covered by an exclude glob before descending.
                // This prevents entering large trees (e.g. node_modules) and
                // avoids following symlinks that cycle back into source directories.
                if e.file_type().is_dir() {
                    if let Ok(rel) = e.path().strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if is_excluded_dir(&rel_str, config) {
                            return false;
                        }
                    }
                }
                true
            })
        {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts) {
                files.push(rel_str);
            }
        }
        files
    }

    /// Walk using the `ignore` crate, which respects `.gitignore` rules,
    /// `.git/info/exclude`, and the user's global gitignore.
    ///
    /// `git_ignore(true)` alone only reads nested `.gitignore` files when a
    /// `.git` directory is reachable from the walk root (it relies on git repo
    /// discovery). `add_custom_ignore_filename(".gitignore")` makes the crate
    /// additionally treat every `.gitignore` it encounters as a standalone
    /// ignore file, ensuring nested rules are applied even outside a git repo.
    ///
    /// When `include` globs are configured, the crate's built-in hidden filter
    /// is disabled and hidden entries are filtered manually so that included
    /// dot-paths can pass through.
    fn scan_files_with_gitignore(&self, supported_exts: &[&str]) -> Vec<String> {
        let has_includes = !self.config.include.is_empty();
        let mut files = Vec::new();
        let walker = ignore::WalkBuilder::new(&self.project_root)
            .follow_links(true)
            .hidden(!has_includes) // disable when we need to check includes
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".gitignore")
            .build();

        for entry in walker {
            let Ok(entry) = entry else { continue };
            let Some(ft) = entry.file_type() else {
                continue;
            };

            // When we disabled the crate's hidden filter, manually skip hidden
            // entries that don't match an include glob.
            if has_includes && entry.depth() > 0 {
                let name = entry.file_name().to_string_lossy();
                if name.starts_with('.') {
                    if let Ok(rel) = entry.path().strip_prefix(&self.project_root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if !is_included(&rel_str, &self.config) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
            }

            if !ft.is_file() {
                continue;
            }
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts) {
                files.push(rel_str);
            }
        }
        files
    }

    /// Checks whether a file should be included: correct extension, not
    /// excluded by config globs, and within the max file size.
    fn accept_file(&self, path: &Path, supported_exts: &[&str]) -> Option<String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !supported_exts.contains(&ext) {
            return None;
        }
        let relative = path.strip_prefix(&self.project_root).ok()?;
        // Normalize to forward slashes so paths are consistent across
        // platforms and between different directory walkers on Windows.
        let rel_str = relative.to_string_lossy().replace('\\', "/");
        if is_excluded(&rel_str, &self.config) {
            return None;
        }
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > self.config.max_file_size {
            return None;
        }
        Some(rel_str)
    }

    /// Resolves a path to a relative path string.
    /// If the path is already relative, returns it as-is.
    /// If absolute, strips the `project_root` prefix.
    fn resolve_path(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        if path.is_absolute() {
            let relative = path.strip_prefix(&self.project_root).ok()?;
            Some(relative.to_string_lossy().replace('\\', "/"))
        } else {
            Some(path.to_string_lossy().replace('\\', "/"))
        }
    }

    /// Gets the absolute path for a relative path.
    fn absolute_path(&self, relative_path: &str) -> PathBuf {
        self.project_root.join(relative_path)
    }

    /// Re-indexes a single file after an edit.
    async fn reindex_file(&self, file_path: &str) -> Result<()> {
        let abs_path = self.absolute_path(file_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read file {file_path}: {e}"),
        })?;

        let Some(extractor) = self.registry.extractor_for_file(file_path) else {
            return Ok(());
        };

        let mut result =
            safe_extract(extractor, file_path, &source).ok_or_else(|| TokenSaveError::Config {
                message: format!("extraction panicked for {file_path}"),
            })?;
        result.sanitize();

        let hash = sync::content_hash(&source);
        let size = source.len() as u64;
        let mtime = sync::file_stat(&abs_path).map_or_else(current_timestamp, |(m, _)| m);

        self.db.delete_nodes_by_file(file_path).await?;
        self.db.insert_nodes(&result.nodes).await?;
        self.db.insert_edges(&result.edges).await?;
        if !result.unresolved_refs.is_empty() {
            self.db
                .insert_unresolved_refs(&result.unresolved_refs)
                .await?;
        }

        let file_record = FileRecord {
            path: file_path.to_string(),
            content_hash: hash,
            size,
            modified_at: mtime,
            indexed_at: current_timestamp(),
            node_count: result.nodes.len() as u32,
        };
        self.db.upsert_file(&file_record).await?;

        Ok(())
    }

    /// Performs a single string replacement.
    /// Fails if `old_str` is not found or matches more than once.
    pub async fn str_replace(
        &self,
        path: &str,
        old_str: &str,
        new_str: &str,
    ) -> Result<EditResult> {
        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {path}: {e}"),
        })?;

        let matches: Vec<_> = source.match_indices(old_str).collect();
        match matches.len() {
            0 => {
                return Ok(EditResult {
                    success: false,
                    file_path: rel_path.clone(),
                    matched_str: old_str.to_string(),
                    new_str: new_str.to_string(),
                    message: format!("old_str not found in {path}"),
                })
            }
            1 => {}
            n => {
                return Ok(EditResult {
                    success: false,
                    file_path: rel_path.clone(),
                    matched_str: old_str.to_string(),
                    new_str: new_str.to_string(),
                    message: format!("old_str matches {n} times, must match exactly once"),
                })
            }
        }

        let modified = source.replacen(old_str, new_str, 1);

        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {path}: {e}"),
            })?;

        self.reindex_file(&rel_path).await?;

        Ok(EditResult {
            success: true,
            file_path: rel_path,
            matched_str: old_str.to_string(),
            new_str: new_str.to_string(),
            message: "replacement successful".to_string(),
        })
    }

    /// Applies multiple string replacements atomically.
    /// Fails if any `old_str` doesn't match exactly once.
    pub async fn multi_str_replace(
        &self,
        path: &str,
        replacements: &[(&str, &str)],
    ) -> Result<MultiEditResult> {
        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {path}: {e}"),
        })?;

        for (old, _) in replacements {
            let count = source.matches(old).count();
            if count != 1 {
                return Ok(MultiEditResult {
                    success: false,
                    file_path: rel_path.clone(),
                    applied_count: 0,
                    message: format!(
                        "replacement '{}' matches {} times, must match exactly once",
                        crate::text::utf8_prefix_at_or_before(old, 20),
                        count
                    ),
                });
            }
        }

        let mut modified = source;
        for (old, new) in replacements {
            modified = modified.replacen(old, new, 1);
        }

        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {path}: {e}"),
            })?;

        self.reindex_file(&rel_path).await?;

        Ok(MultiEditResult {
            success: true,
            file_path: rel_path,
            applied_count: replacements.len(),
            message: format!("applied {} replacements", replacements.len()),
        })
    }

    /// Inserts content before or after a unique anchor.
    /// Anchor can be a string or 1-indexed line number.
    pub async fn insert_at(
        &self,
        path: &str,
        anchor: &str,
        content: &str,
        before: bool,
    ) -> Result<InsertResult> {
        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {path}: {e}"),
        })?;

        let lines: Vec<&str> = source.lines().collect();

        let anchor_line = if anchor.chars().all(|c| c.is_ascii_digit()) {
            let line_num: usize = anchor.parse().map_err(|_| TokenSaveError::Config {
                message: format!("invalid line number: {anchor}"),
            })?;
            if line_num == 0 || line_num > lines.len() {
                return Ok(InsertResult {
                    success: false,
                    file_path: rel_path.clone(),
                    anchor_line: line_num as u32,
                    content: content.to_string(),
                    before,
                    message: format!(
                        "line number {line_num} out of range (file has {} lines)",
                        lines.len()
                    ),
                });
            }
            line_num - 1
        } else {
            let anchor_prefix = crate::text::utf8_prefix_at_or_before(anchor, 100);
            let matching_lines: Vec<usize> = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains(anchor_prefix))
                .map(|(i, _)| i)
                .collect();

            if matching_lines.is_empty() {
                return Ok(InsertResult {
                    success: false,
                    file_path: rel_path.clone(),
                    anchor_line: 0,
                    content: content.to_string(),
                    before,
                    message: format!("anchor '{anchor}' not found"),
                });
            }
            if matching_lines.len() > 1 {
                return Ok(InsertResult {
                    success: false,
                    file_path: rel_path.clone(),
                    anchor_line: matching_lines.len() as u32,
                    content: content.to_string(),
                    before,
                    message: format!(
                        "anchor '{anchor}' matches {} lines, must match exactly one",
                        matching_lines.len()
                    ),
                });
            }
            matching_lines[0]
        };

        let insert_idx = if before { anchor_line } else { anchor_line + 1 };
        let mut new_lines: Vec<&str> = lines[..insert_idx].to_vec();
        new_lines.push(content);
        new_lines.extend_from_slice(&lines[insert_idx..]);
        let mut modified = new_lines.join("\n");
        if source.ends_with('\n') {
            modified.push('\n');
        }

        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {path}: {e}"),
            })?;

        self.reindex_file(&rel_path).await?;

        Ok(InsertResult {
            success: true,
            file_path: rel_path,
            anchor_line: (anchor_line + 1) as u32,
            content: content.to_string(),
            before,
            message: format!("inserted at line {}", anchor_line + 1),
        })
    }

    /// Performs structural rewrite using ast-grep CLI.
    pub async fn ast_grep_rewrite(
        &self,
        path: &str,
        pattern: &str,
        rewrite: &str,
    ) -> Result<AstGrepResult> {
        use std::process::Command;

        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);

        let check_output = Command::new("ast-grep").args(["--version"]).output();

        if check_output.is_err() {
            if can_use_literal_rewrite_fallback(pattern) {
                let mut source = std::fs::read_to_string(&abs_path).map_err(TokenSaveError::Io)?;
                if !source.contains(pattern) {
                    return Ok(AstGrepResult {
                        success: false,
                        file_path: rel_path.clone(),
                        pattern: pattern.to_string(),
                        rewrite: rewrite.to_string(),
                        message: "pattern not found (built-in literal fallback)".to_string(),
                    });
                }
                source = source.replace(pattern, rewrite);
                std::fs::write(&abs_path, source).map_err(TokenSaveError::Io)?;
                self.reindex_file(&rel_path).await?;
                return Ok(AstGrepResult {
                    success: true,
                    file_path: rel_path,
                    pattern: pattern.to_string(),
                    rewrite: rewrite.to_string(),
                    message: "literal rewrite completed using built-in fallback".to_string(),
                });
            }
            return Ok(AstGrepResult {
                success: false,
                file_path: rel_path.clone(),
                pattern: pattern.to_string(),
                rewrite: rewrite.to_string(),
                message: "ast-grep is not installed and this pattern needs SGPattern matching. Simple literal rewrites are handled by the built-in fallback.".to_string(),
            });
        }

        let output = Command::new("ast-grep")
            .args([
                "run",
                "-p",
                pattern,
                "-r",
                rewrite,
                "-U",
                abs_path.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to run ast-grep: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr_trim = stderr.trim();
            let stdout_trim = stdout.trim();
            let exit = output
                .status
                .code()
                .map_or_else(|| "killed by signal".to_string(), |c| c.to_string());
            let message = if !stderr_trim.is_empty() {
                format!("ast-grep failed (exit {exit}): {stderr_trim}")
            } else if !stdout_trim.is_empty() {
                format!("ast-grep failed (exit {exit}). stdout: {stdout_trim}")
            } else {
                format!(
                    "ast-grep failed (exit {exit}) with no output. Likely causes: \
                     pattern matched 0 nodes, language not inferred from file extension \
                     (e.g. .txt has no parser), or invalid pattern syntax. \
                     File: {rel_path}, pattern: {pattern:?}"
                )
            };
            return Ok(AstGrepResult {
                success: false,
                file_path: rel_path.clone(),
                pattern: pattern.to_string(),
                rewrite: rewrite.to_string(),
                message,
            });
        }

        self.reindex_file(&rel_path).await?;

        Ok(AstGrepResult {
            success: true,
            file_path: rel_path,
            pattern: pattern.to_string(),
            rewrite: rewrite.to_string(),
            message: "ast-grep rewrite completed".to_string(),
        })
    }
}

fn can_use_literal_rewrite_fallback(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    !trimmed.is_empty()
        && trimmed == pattern
        && !pattern.contains('$')
        && !pattern.contains('\n')
        && !pattern.contains('\r')
}

// ---------------------------------------------------------------------------
// Query delegation
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Searches for nodes matching the given query string.
    ///
    /// Over-fetches from the FTS layer and re-ranks results so that symbol
    /// definitions (functions, structs, traits, etc.) sort above mere
    /// references (`use`, `module`, annotation usages) that happen to share
    /// the same name. BM25 alone does not distinguish kinds, so a `use foo`
    /// statement could outrank the actual `pub fn foo()` definition.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let overfetch = limit.saturating_mul(3).max(30);
        let trimmed_query = query.trim();
        let mut raw = self.db.search_nodes(query, overfetch).await?;

        // FTS/BM25 can bury exact symbol definitions below many short import
        // rows. On Sonium, `LinearOperator` had dozens of `use ...LinearOperator`
        // rows in the top FTS window while the actual trait definition was
        // outside `overfetch`, so the kind tier below never saw it. Seed the
        // candidate set with exact `name = query` hits first, then dedup.
        if !trimmed_query.is_empty() {
            let mut exact_names = vec![trimmed_query.to_string()];
            if let Some(short) = trimmed_query.rsplit("::").next() {
                if short != trimmed_query && !short.is_empty() {
                    exact_names.push(short.to_string());
                }
            }
            let exact = self
                .db
                .search_nodes_by_exact_name(&exact_names, overfetch)
                .await?;
            raw.extend(
                exact
                    .into_iter()
                    .map(|node| SearchResult { node, score: 0.0 }),
            );
        }

        let mut seen = HashSet::new();
        let mut ranked: Vec<SearchResult> = raw
            .into_iter()
            .filter(|r| seen.insert(r.node.id.clone()))
            .map(|mut r| {
                r.score += kind_rank_bonus(&r.node.kind);
                // Exact-name match boost: when the node's `name` equals the
                // query verbatim, surface it ahead of partial / qualified-name
                // matches. Without this, searching for a trait like
                // `LinearOperator` could be outranked by a `Method` whose
                // qualified name happens to contain `LinearOperator` (e.g.
                // a method declared inside the trait body), or by a `Field`
                // that shares the same simple name.
                if !trimmed_query.is_empty() && r.node.name == trimmed_query {
                    r.score += 10.0;
                }
                r
            })
            .collect();
        // Sort by kind tier first (definitions > references), then score
        // descending. Tier-first avoids any chance that a `use` re-export
        // (kind tier = `Use`) outscores a real definition because BM25
        // happened to weight the short re-export row highly. Score is the
        // secondary key so within a tier we still respect BM25.
        ranked.sort_by(|a, b| {
            kind_tier(&a.node.kind)
                .cmp(&kind_tier(&b.node.kind))
                .then_with(|| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        ranked.truncate(limit);
        Ok(ranked)
    }

    /// Returns aggregate statistics about the code graph.
    pub async fn get_stats(&self) -> Result<GraphStats> {
        self.db.get_stats().await
    }

    /// Retrieves a single node by its unique ID.
    pub async fn get_node(&self, id: &str) -> Result<Option<Node>> {
        self.db.get_node_by_id(id).await
    }

    /// Returns all nodes that transitively call the given node, up to `max_depth`.
    pub async fn get_callers(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_callers(node_id, max_depth).await
    }

    /// Returns all nodes that the given node transitively calls, up to `max_depth`.
    pub async fn get_callees(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_callees(node_id, max_depth).await
    }

    /// Computes the impact radius: all nodes that directly or indirectly
    /// depend on the given node, up to `max_depth`.
    pub async fn get_impact_radius(&self, node_id: &str, max_depth: usize) -> Result<Subgraph> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_impact_radius(node_id, max_depth).await
    }

    /// Same as `get_impact_radius` but multi-source: takes many seed node
    /// IDs and walks the union of their impact radii with a single shared
    /// `visited` set, so each downstream node is traversed at most once.
    pub async fn get_impact_radius_multi(
        &self,
        seed_ids: &[String],
        max_depth: usize,
    ) -> Result<Vec<Node>> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_impact_radius_multi(seed_ids, max_depth).await
    }

    /// Builds a bidirectional call graph around a node.
    pub async fn get_call_graph(&self, node_id: &str, depth: usize) -> Result<Subgraph> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_call_graph(node_id, depth).await
    }

    /// Finds potentially dead code (nodes with no incoming edges).
    ///
    /// When `include_public` is `false` (the default), `pub` items are
    /// excluded — they may be referenced by code outside the indexed
    /// scope. Pass `true` to also surface pub items with zero indexed
    /// callers (useful for workspace-internal audits).
    pub async fn find_dead_code(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
    ) -> Result<Vec<Node>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.find_dead_code(kinds, include_public).await
    }

    /// Returns all nodes for a given file, ordered by start line.
    pub async fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        self.db.get_nodes_by_file(file_path).await
    }

    /// Returns every node in the database.
    pub async fn get_all_nodes(&self) -> Result<Vec<Node>> {
        self.db.get_all_nodes().await
    }

    /// Returns incoming edges to a target node.
    pub async fn get_incoming_edges(&self, node_id: &str) -> Result<Vec<Edge>> {
        self.db.get_incoming_edges(node_id, &[]).await
    }

    /// Returns the subset of `candidate_ids` that have a `#[test]` annotation.
    pub async fn get_test_annotated_node_ids(
        &self,
        candidate_ids: &[String],
    ) -> Result<HashSet<String>> {
        self.db.get_test_annotated_node_ids(candidate_ids).await
    }

    /// Returns all file paths containing at least one `#[test]`-annotated function.
    pub async fn get_files_with_test_annotations(&self) -> Result<HashSet<String>> {
        self.db.get_files_with_test_annotations().await
    }

    /// Returns all node IDs marked with `/// skip-test-coverage`.
    pub async fn get_skip_test_coverage_node_ids(&self) -> Result<HashSet<String>> {
        self.db.get_skip_test_coverage_node_ids().await
    }

    /// Returns incoming edges for many target nodes in one round-trip.
    /// Empty `kinds` matches every edge kind.
    pub async fn get_incoming_edges_bulk(
        &self,
        target_ids: &[String],
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        self.db.get_incoming_edges_bulk(target_ids, kinds).await
    }

    /// Returns all nodes whose `qualified_name` matches `qname`.
    /// Cross-run lookup independent of the content-hash node IDs.
    pub async fn get_nodes_by_qualified_name(&self, qname: &str) -> Result<Vec<Node>> {
        self.db.get_nodes_by_qualified_name(qname).await
    }

    /// Returns outgoing edges from a source node.
    pub async fn get_outgoing_edges(&self, node_id: &str) -> Result<Vec<Edge>> {
        self.db.get_outgoing_edges(node_id, &[]).await
    }

    /// Returns every edge in the database.
    pub async fn get_all_edges(&self) -> Result<Vec<Edge>> {
        self.db.get_all_edges().await
    }

    /// Returns nodes ranked by edge count for a given edge kind and direction,
    /// optionally filtered by node kind.
    pub async fn get_ranked_nodes_by_edge_kind(
        &self,
        edge_kind: &EdgeKind,
        node_kind: Option<&NodeKind>,
        incoming: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        self.db
            .get_ranked_nodes_by_edge_kind(edge_kind, node_kind, incoming, path_prefix, limit)
            .await
    }

    /// Returns nodes ranked by line span, optionally filtered by node kind and path.
    pub async fn get_largest_nodes(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32)>> {
        self.db
            .get_largest_nodes(node_kind, path_prefix, limit)
            .await
    }

    /// Returns files ranked by coupling (fan-in or fan-out).
    pub async fn get_file_coupling(
        &self,
        fan_in: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, u64)>> {
        self.db.get_file_coupling(fan_in, path_prefix, limit).await
    }

    /// Returns classes/interfaces ranked by inheritance depth via extends chains.
    pub async fn get_inheritance_depth(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        self.db.get_inheritance_depth(path_prefix, limit).await
    }

    /// Returns node kind distribution, optionally filtered by path prefix.
    pub async fn get_node_distribution(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, u64)>> {
        self.db.get_node_distribution(path_prefix).await
    }

    /// Returns calls edges as (`source_id`, `target_id`) pairs for cycle detection.
    pub async fn get_call_edges(&self, path_prefix: Option<&str>) -> Result<Vec<(String, String)>> {
        self.db.get_call_edges(path_prefix).await
    }

    /// Returns calls edges as (`source_id`, `target_id`, `line`) tuples.
    pub async fn get_call_edges_with_lines(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, Option<u32>)>> {
        self.db.get_call_edges_with_lines(path_prefix).await
    }

    /// Returns functions/methods ranked by composite complexity score.
    pub async fn get_complexity_ranked(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32, u64, u64, u64)>> {
        self.db
            .get_complexity_ranked(node_kind, path_prefix, limit)
            .await
    }

    /// Returns public symbols missing docstrings.
    pub async fn get_undocumented_public_symbols(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Node>> {
        self.db
            .get_undocumented_public_symbols(path_prefix, limit)
            .await
    }

    /// Returns classes ranked by member count (methods + fields).
    pub async fn get_god_classes(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64, u64, u64)>> {
        self.db.get_god_classes(path_prefix, limit).await
    }

    /// Detects circular dependencies at the file level.
    pub async fn find_circular_dependencies(&self) -> Result<Vec<Vec<String>>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.find_circular_dependencies().await
    }

    /// Builds an AI-ready context for a given task description.
    pub async fn build_context(
        &self,
        task: &str,
        options: &BuildContextOptions,
    ) -> Result<TaskContext> {
        let builder = ContextBuilder::new(&self.db, &self.project_root);
        builder.build_context(task, options).await
    }

    /// Returns all indexed file records.
    pub async fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        self.db.get_all_files().await
    }

    /// Returns the `#[derive(...)]` names attached to the given node.
    ///
    /// The graph's `DerivesMacro` edges are unreliable here: the resolver
    /// fuzzy-binds std-trait names like `Debug` to nonsense nodes (a `Debug`
    /// enum variant in an unrelated test fixture) and the resulting unique
    /// constraint on `(source, target, kind, line)` collapses multiple
    /// distinct derives on the same type onto a single edge — so a struct
    /// that derives `Debug, Clone, PartialEq, Eq, Hash` may surface only one
    /// of them. Instead we re-read the lines between `attrs_start_line` and
    /// `start_line` of the node, which the extractor already promises to
    /// cover the leading attribute block, and parse `#[derive(...)]`
    /// attributes directly. Bounded file I/O — one read per call.
    pub async fn get_derives_for_node(&self, node_id: &str) -> Result<Vec<String>> {
        let Some(node) = self.db.get_node_by_id(node_id).await? else {
            return Ok(Vec::new());
        };
        let file_path = self.project_root().join(&node.file_path);
        let Ok(content) = std::fs::read_to_string(&file_path) else {
            return Ok(Vec::new());
        };
        Ok(parse_derives_in_attr_block(
            &content,
            node.attrs_start_line,
            node.start_line,
        ))
    }

    /// Finds the most specific (smallest-span) node whose source range
    /// contains the given `(file, line)` location.
    ///
    /// Returns `None` when no indexed node covers the location — typically
    /// because the file isn't indexed, or the line is in a region the
    /// extractor didn't capture (e.g. inside a `use` block or top-of-file
    /// comment). Lines are 1-based to match `rustc` / `clippy` output;
    /// `Node.start_line` / `end_line` are 0-based internally so we subtract
    /// before comparing.
    ///
    /// Implementation loads every node in the file (cached at the index
    /// layer) and picks the smallest containing span. At the typical ~50
    /// nodes per file this is faster than a custom range-query and stays
    /// honest about overlap (impl blocks contain methods, etc.).
    pub async fn node_at_location(&self, file: &str, line_1based: u32) -> Result<Option<Node>> {
        if line_1based == 0 {
            return Ok(None);
        }
        let zero_based = line_1based - 1;
        let normalized = normalize_lookup_path(self.project_root(), file);
        let mut nodes = self.db.get_nodes_by_file(&normalized).await?;
        nodes.retain(|n| n.start_line <= zero_based && n.end_line >= zero_based);
        // Prefer the smallest containing span — that's the most specific
        // owner of the source location.
        nodes.sort_by_key(|n| (n.end_line - n.start_line, n.start_line));
        Ok(nodes.into_iter().next())
    }

    /// Returns the indexed size in bytes for a file path, or `0` if unknown.
    /// Used to estimate the token cost of expanding a file in responses.
    pub async fn get_file_size_bytes(&self, path: &str) -> u64 {
        match self.db.get_file(path).await {
            Ok(Some(rec)) => rec.size,
            _ => 0,
        }
    }

    /// Returns `impl` blocks matching the given trait and/or implementing type.
    ///
    /// Both filters are optional:
    /// - With only `trait_name`: every impl of that trait, regardless of the
    ///   implementing type.
    /// - With only `type_name`: every impl block for that type (trait impls
    ///   and inherent impls).
    /// - With both: the intersection.
    /// - With neither: every `impl` node in the graph (use sparingly).
    ///
    /// Each result carries the impl node plus, when available, the resolved
    /// trait node it implements. Matching uses substring containment on the
    /// trait/type names so callers can pass either short or qualified names.
    pub async fn get_impls(
        &self,
        trait_name: Option<&str>,
        type_name: Option<&str>,
    ) -> Result<Vec<(Node, Option<Node>)>> {
        use crate::types::EdgeKind;

        // Candidate impl blocks.
        let mut impls = self.db.get_nodes_by_kind(NodeKind::Impl).await?;

        // Filter by implementing type if requested. The impl node's `name`
        // field holds the type identifier (e.g. "MyType" for `impl Foo for MyType`).
        if let Some(type_q) = type_name {
            impls.retain(|n| node_name_matches(n, type_q));
        }

        // Gather Implements edges per impl, then batch-fetch every trait node
        // in one `get_nodes_by_ids` call to avoid an N+1 across impl blocks.
        let mut per_impl_trait_id: Vec<Option<String>> = Vec::with_capacity(impls.len());
        let mut trait_target_ids: Vec<String> = Vec::new();
        for impl_node in &impls {
            let edges = self
                .db
                .get_outgoing_edges(&impl_node.id, &[EdgeKind::Implements])
                .await
                .unwrap_or_default();
            let target = edges.into_iter().next().map(|e| e.target);
            if let Some(ref t) = target {
                trait_target_ids.push(t.clone());
            }
            per_impl_trait_id.push(target);
        }
        let trait_nodes = if trait_target_ids.is_empty() {
            Vec::new()
        } else {
            self.db.get_nodes_by_ids(&trait_target_ids).await?
        };
        let trait_map: std::collections::HashMap<String, Node> =
            trait_nodes.into_iter().map(|n| (n.id.clone(), n)).collect();

        let mut out: Vec<(Node, Option<Node>)> = Vec::with_capacity(impls.len());
        for (impl_node, trait_id) in impls.into_iter().zip(per_impl_trait_id) {
            let trait_node = trait_id.and_then(|id| trait_map.get(&id).cloned());

            // Trait filter: drop inherent impls when a trait was requested.
            if let Some(trait_q) = trait_name {
                let matched = trait_node
                    .as_ref()
                    .is_some_and(|t| node_name_matches(t, trait_q));
                if !matched {
                    continue;
                }
            }

            out.push((impl_node, trait_node));
        }
        Ok(out)
    }

    /// Resolves a trait method node to the concrete method nodes that satisfy
    /// it across every `impl` block of the enclosing trait.
    ///
    /// Returns an empty vec when the input is not a method whose parent (via
    /// `Contains`) is a trait. Used by `tokensave_callees` to surface concrete
    /// dispatch targets in addition to the trait method itself.
    pub async fn get_trait_dispatch_targets(&self, method: &Node) -> Result<Vec<Node>> {
        use crate::types::EdgeKind;

        // Only method-kind nodes can be trait methods.
        if !matches!(method.kind, NodeKind::Method | NodeKind::Function) {
            return Ok(Vec::new());
        }

        // Find the trait that contains this method. parent_id points at
        // the enclosing scope after v9; verify it's actually a Trait.
        let Some(parent_id) = method.parent_id.as_deref() else {
            return Ok(Vec::new());
        };
        let Some(trait_node) = self.db.get_node_by_id(parent_id).await? else {
            return Ok(Vec::new());
        };
        if trait_node.kind != NodeKind::Trait {
            return Ok(Vec::new());
        }

        // Find every impl block of that trait.
        let impl_edges = self
            .db
            .get_incoming_edges(&trait_node.id, &[EdgeKind::Implements])
            .await?;
        let impl_ids: Vec<String> = impl_edges.into_iter().map(|e| e.source).collect();
        if impl_ids.is_empty() {
            return Ok(Vec::new());
        }

        // For each impl block, surface the method whose name matches the
        // trait method. Multiple impls may share names with unrelated nodes,
        // so we filter by both kind and name.
        let mut targets = Vec::new();
        for impl_id in impl_ids {
            let candidates = self.db.get_children_of(&impl_id).await?;
            for n in candidates {
                if matches!(n.kind, NodeKind::Method | NodeKind::Function) && n.name == method.name
                {
                    targets.push(n);
                }
            }
        }
        Ok(targets)
    }

    /// Returns file paths that depend on the given file.
    pub async fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.get_file_dependents(file_path).await
    }

    /// Returns a map of file path to approximate token count (size / 4).
    pub async fn get_file_token_map(&self) -> Result<HashMap<String, u64>> {
        let files = self.db.get_all_files().await?;
        Ok(files.into_iter().map(|f| (f.path, f.size / 4)).collect())
    }

    /// Returns the persisted tokens-saved counter.
    pub async fn get_tokens_saved(&self) -> Result<u64> {
        match self.db.get_metadata("tokens_saved").await? {
            Some(v) => Ok(v.parse::<u64>().unwrap_or(0)),
            None => Ok(0),
        }
    }

    /// Persists the tokens-saved counter to the database.
    pub async fn set_tokens_saved(&self, value: u64) -> Result<()> {
        self.db
            .set_metadata("tokens_saved", &value.to_string())
            .await
    }

    /// Returns the resettable project-local token counter.
    ///
    /// This is separate from the main `tokens_saved` counter and can be
    /// independently reset via [`Self::reset_local_counter`].
    pub async fn get_local_counter(&self) -> Result<u64> {
        match self.db.get_metadata("local_counter").await? {
            Some(v) => Ok(v.parse::<u64>().unwrap_or(0)),
            None => Ok(0),
        }
    }

    /// Resets the project-local token counter to zero.
    pub async fn reset_local_counter(&self) -> Result<()> {
        self.db.set_metadata("local_counter", "0").await
    }

    /// Increments the project-local token counter by the given amount.
    pub async fn add_local_counter(&self, delta: u64) -> Result<()> {
        let current = self.get_local_counter().await?;
        self.db
            .set_metadata("local_counter", &(current + delta).to_string())
            .await
    }

    /// Returns all nodes under a directory prefix filtered by kinds.
    pub async fn get_nodes_by_dir(&self, dir: &str, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        self.db.get_nodes_by_dir(dir, kinds).await
    }

    /// Returns edges where both source and target are in the given node ID set.
    pub async fn get_internal_edges(&self, node_ids: &[String]) -> Result<Vec<Edge>> {
        self.db.get_internal_edges(node_ids).await
    }

    /// Checkpoints the WAL and closes the database connection.
    pub async fn checkpoint(&self) -> Result<()> {
        self.db.checkpoint().await
    }

    /// Runs VACUUM and ANALYZE to reclaim disk space and update planner stats.
    pub async fn optimize(&self) -> Result<()> {
        self.db.optimize().await
    }

    /// Returns a reference to the current configuration.
    pub fn get_config(&self) -> &TokenSaveConfig {
        &self.config
    }

    /// Returns the project root path.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Returns the active git branch, if any.
    pub fn active_branch(&self) -> Option<&str> {
        self.active_branch.as_deref()
    }

    /// Returns the branch whose DB is actually being served.
    pub fn serving_branch(&self) -> Option<&str> {
        self.serving_branch.as_deref()
    }

    /// Returns a fallback warning if serving from an ancestor branch DB.
    pub fn fallback_warning(&self) -> Option<&str> {
        self.fallback_warning.as_deref()
    }

    /// Returns true if serving from a fallback (ancestor) DB.
    pub fn is_fallback(&self) -> bool {
        self.fallback_warning.is_some()
    }
}

// ---------------------------------------------------------------------------
// Staleness detection
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Check whether the given files need (re-/un-)indexing to bring the DB
    /// into agreement with the filesystem.
    ///
    /// A file is reported stale when any of:
    /// - it is in the DB and has been modified on disk since `indexed_at`,
    /// - it is in the DB but no longer exists on disk (deletion — DB needs cleanup),
    /// - it exists on disk but has no DB record (new file — needs indexing).
    ///
    /// A file that exists in neither the DB nor on disk is out of scope and
    /// is silently dropped.
    pub async fn check_file_staleness(&self, file_paths: &[String]) -> Vec<String> {
        let mut stale = Vec::new();
        for path in file_paths {
            let abs_path = self.project_root.join(path);
            let file_exists = abs_path.exists();
            match self.db.get_file(path).await {
                Ok(Some(record)) => {
                    if !file_exists {
                        // Indexed but deleted — DB needs cleanup.
                        stale.push(path.clone());
                    } else if let Ok(metadata) = std::fs::metadata(&abs_path) {
                        if let Ok(mtime) = metadata.modified() {
                            let mtime_secs = mtime
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            if mtime_secs > record.indexed_at {
                                stale.push(path.clone());
                            }
                        }
                    }
                }
                _ => {
                    // Not in the DB. If it exists on disk, it's new and needs indexing.
                    if file_exists {
                        stale.push(path.clone());
                    }
                }
            }
        }
        stale
    }

    /// Returns the most recent `indexed_at` timestamp across all indexed files.
    pub async fn last_index_time(&self) -> Result<i64> {
        self.db.last_index_time().await
    }

    /// Count git commits newer than the given UNIX timestamp.
    /// Returns 0 if git is unavailable or the directory is not a git repository.
    pub fn git_commits_since(&self, since_timestamp: i64) -> usize {
        let Ok(repo) = gix::open(&self.project_root) else {
            return 0;
        };
        let Ok(head) = repo.head_commit() else {
            return 0;
        };
        let sorting = gix::revision::walk::Sorting::ByCommitTimeCutoff {
            order: gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            seconds: since_timestamp,
        };
        let Ok(walk) = head.ancestors().sorting(sorting).all() else {
            return 0;
        };
        walk.filter_map(std::result::Result::ok).count()
    }
}

// ---------------------------------------------------------------------------
// Session memory
// ---------------------------------------------------------------------------

const MAX_RECALL_LIMIT: usize = 200;
const MAX_CODE_AREAS_LIMIT: usize = 200;

impl TokenSave {
    /// Record an agent decision. Returns the new row id.
    pub async fn record_decision(
        &self,
        text: &str,
        reason: Option<&str>,
        files: &[String],
        tags: &[String],
    ) -> crate::errors::Result<i64> {
        debug_assert!(!text.is_empty(), "decision text must not be empty");
        let files_json =
            serde_json::to_string(files).map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("record_decision files serialization failed: {e}"),
                operation: "record_decision".to_string(),
            })?;
        let tags_json =
            serde_json::to_string(tags).map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("record_decision tags serialization failed: {e}"),
                operation: "record_decision".to_string(),
            })?;
        let now = current_timestamp();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO memory_decisions (text, reason, created_at, files, tags) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            libsql::params![text, reason, now, files_json, tags_json],
        )
        .await
        .map_err(|e| crate::errors::TokenSaveError::Database {
            message: format!("record_decision insert failed: {e}"),
            operation: "record_decision".to_string(),
        })?;
        Ok(conn.last_insert_rowid())
    }

    /// Recall decisions. With `query`, runs FTS5 MATCH against text+reason.
    /// Without `query`, returns newest-first.
    pub async fn session_recall(
        &self,
        query: Option<&str>,
        since: Option<i64>,
        limit: usize,
    ) -> crate::errors::Result<Vec<DecisionRecord>> {
        let limit = limit.clamp(1, MAX_RECALL_LIMIT) as i64;
        let conn = self.db.conn();

        let db_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall query failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut rows = match (query, since) {
            (Some(q), Some(ts)) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 AND d.created_at >= ?2 \
                     ORDER BY d.created_at DESC LIMIT ?3",
                    libsql::params![q, ts, limit],
                )
                .await
                .map_err(db_err)?,
            (Some(q), None) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 \
                     ORDER BY d.created_at DESC LIMIT ?2",
                    libsql::params![q, limit],
                )
                .await
                .map_err(db_err)?,
            (None, Some(ts)) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions WHERE created_at >= ?1 \
                     ORDER BY created_at DESC LIMIT ?2",
                    libsql::params![ts, limit],
                )
                .await
                .map_err(db_err)?,
            (None, None) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions ORDER BY created_at DESC LIMIT ?1",
                    libsql::params![limit],
                )
                .await
                .map_err(db_err)?,
        };

        let row_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall row read failed: {e}"),
            operation: "session_recall".to_string(),
        };
        let json_err = |e: serde_json::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall JSON parse failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(row_err)? {
            let files_json: String = row.get(4).map_err(row_err)?;
            let tags_json: String = row.get(5).map_err(row_err)?;
            out.push(DecisionRecord {
                id: row.get(0).map_err(row_err)?,
                text: row.get(1).map_err(row_err)?,
                reason: row.get::<Option<String>>(2).map_err(row_err)?,
                created_at: row.get(3).map_err(row_err)?,
                files: serde_json::from_str(&files_json).map_err(json_err)?,
                tags: serde_json::from_str(&tags_json).map_err(json_err)?,
            });
        }
        Ok(out)
    }

    /// Record (or update) a code area the agent worked in. Increments `touch_count`
    /// on re-touch. Description is set on first insert; subsequent `None` values
    /// preserve the existing description.
    pub async fn record_code_area(
        &self,
        path: &str,
        description: Option<&str>,
    ) -> crate::errors::Result<()> {
        debug_assert!(!path.is_empty(), "code area path must not be empty");
        let now = current_timestamp();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO memory_code_areas (path, description, last_touched_at, touch_count) \
             VALUES (?1, ?2, ?3, 1) \
             ON CONFLICT(path) DO UPDATE SET \
                description = COALESCE(excluded.description, memory_code_areas.description), \
                last_touched_at = excluded.last_touched_at, \
                touch_count = memory_code_areas.touch_count + 1",
            libsql::params![path, description, now],
        )
        .await
        .map_err(|e| crate::errors::TokenSaveError::Database {
            message: format!("record_code_area upsert failed: {e}"),
            operation: "record_code_area".to_string(),
        })?;
        Ok(())
    }

    /// List code areas, most-recently-touched first.
    pub async fn list_code_areas(
        &self,
        limit: usize,
    ) -> crate::errors::Result<Vec<CodeAreaRecord>> {
        let limit = limit.clamp(1, MAX_CODE_AREAS_LIMIT) as i64;
        let conn = self.db.conn();
        let mut rows = conn
            .query(
                "SELECT id, path, description, last_touched_at, touch_count \
                 FROM memory_code_areas ORDER BY last_touched_at DESC LIMIT ?1",
                libsql::params![limit],
            )
            .await
            .map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("list_code_areas query failed: {e}"),
                operation: "list_code_areas".to_string(),
            })?;
        let row_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("list_code_areas row read failed: {e}"),
            operation: "list_code_areas".to_string(),
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(row_err)? {
            out.push(CodeAreaRecord {
                id: row.get(0).map_err(row_err)?,
                path: row.get(1).map_err(row_err)?,
                description: row.get::<Option<String>>(2).map_err(row_err)?,
                last_touched_at: row.get(3).map_err(row_err)?,
                touch_count: row.get::<i64>(4).map_err(row_err)? as u32,
            });
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Search-result rank bonus applied per node kind, so symbol *definitions*
/// outrank mere *references* (use statements, annotation usages, modules)
/// that BM25 may otherwise score equally. Tuned so a definition with a
/// slightly worse BM25 score still surfaces above its imports.
///
/// Exhaustive match by design: when a new `NodeKind` variant is added the
/// compiler will force a re-tune here rather than silently defaulting it to
/// `0.0`, matching the project rule "crash hard if there is an unknown
/// value".
/// Coarse ranking tier used as the primary sort key in `search`. Lower
/// numbers sort first. The tiers separate "real definitions" (functions,
/// types, traits, …) from "references" (`use`, `module`, annotation usage)
/// so a re-export can never beat the thing it re-exports, no matter what
/// BM25 produces for the row.
fn kind_tier(kind: &NodeKind) -> u8 {
    match kind {
        // Tier 0: callable definitions and type definitions — the
        // "what is this?" answers a user usually wants when searching by
        // symbol name.
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::ArrowFunction
        | NodeKind::Procedure
        | NodeKind::Struct
        | NodeKind::Enum
        | NodeKind::Trait
        | NodeKind::Class
        | NodeKind::InnerClass
        | NodeKind::Interface
        | NodeKind::InterfaceType
        | NodeKind::Record
        | NodeKind::CaseClass
        | NodeKind::DataClass
        | NodeKind::SealedClass
        | NodeKind::TypeAlias
        | NodeKind::Union
        | NodeKind::Typedef
        | NodeKind::Mixin
        | NodeKind::Extension
        | NodeKind::Delegate
        | NodeKind::Template
        | NodeKind::PascalRecord
        | NodeKind::ScalaObject
        | NodeKind::KotlinObject
        | NodeKind::CompanionObject
        | NodeKind::Annotation
        | NodeKind::Event => 0,
        // Proto definitions (feature-gated)
        #[cfg(feature = "lang-protobuf")]
        NodeKind::ProtoMessage | NodeKind::ProtoService | NodeKind::ProtoRpc => 0,
        // Tier 1: impl blocks — between definitions and references.
        NodeKind::Impl => 1,
        // Tier 2: values, macros, members of types.
        NodeKind::Const
        | NodeKind::Static
        | NodeKind::Macro
        | NodeKind::PreprocessorDef
        | NodeKind::EnumVariant
        | NodeKind::Field
        | NodeKind::ValField
        | NodeKind::VarField
        | NodeKind::Property
        | NodeKind::CSharpProperty
        | NodeKind::StructTag
        | NodeKind::InitBlock
        | NodeKind::Export => 2,
        // Tier 3: containers (module, namespace, …) — usually not the
        // answer to "find symbol".
        NodeKind::Module
        | NodeKind::Package
        | NodeKind::Namespace
        | NodeKind::ScalaPackage
        | NodeKind::GoPackage
        | NodeKind::KotlinPackage
        | NodeKind::PascalUnit
        | NodeKind::Library
        | NodeKind::File
        | NodeKind::GenericParam
        | NodeKind::PascalProgram => 3,
        // Tier 4: pure references / annotations — always rank last.
        NodeKind::Use | NodeKind::Include | NodeKind::AnnotationUsage | NodeKind::Decorator => 4,
    }
}

fn kind_rank_bonus(kind: &NodeKind) -> f64 {
    match kind {
        // Callable definitions
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::ArrowFunction
        | NodeKind::Procedure => 3.0,
        // Type definitions
        NodeKind::Struct
        | NodeKind::Enum
        | NodeKind::Trait
        | NodeKind::Class
        | NodeKind::InnerClass
        | NodeKind::Interface
        | NodeKind::InterfaceType
        | NodeKind::Record
        | NodeKind::CaseClass
        | NodeKind::DataClass
        | NodeKind::SealedClass
        | NodeKind::TypeAlias
        | NodeKind::Union
        | NodeKind::Typedef
        | NodeKind::Mixin
        | NodeKind::Extension
        | NodeKind::Delegate
        | NodeKind::Template
        | NodeKind::PascalRecord
        | NodeKind::ScalaObject
        | NodeKind::KotlinObject
        | NodeKind::CompanionObject
        | NodeKind::Annotation
        | NodeKind::Event => 2.5,
        // Proto definitions
        #[cfg(feature = "lang-protobuf")]
        NodeKind::ProtoMessage | NodeKind::ProtoService | NodeKind::ProtoRpc => 2.5,
        // Impl blocks (between defs and refs)
        NodeKind::Impl => 2.0,
        // Values, macros, preprocessor defs
        NodeKind::Const
        | NodeKind::Static
        | NodeKind::Macro
        | NodeKind::PreprocessorDef
        | NodeKind::EnumVariant => 1.0,
        // Members of types
        NodeKind::Field
        | NodeKind::ValField
        | NodeKind::VarField
        | NodeKind::Property
        | NodeKind::CSharpProperty
        | NodeKind::StructTag
        | NodeKind::InitBlock
        | NodeKind::Export => 0.5,
        // File / generic-parameter — neutral
        NodeKind::File | NodeKind::GenericParam | NodeKind::PascalProgram => 0.0,
        // References & containers — push below definitions
        NodeKind::Use | NodeKind::Include => -3.0,
        NodeKind::AnnotationUsage | NodeKind::Decorator => -2.0,
        NodeKind::Module
        | NodeKind::Package
        | NodeKind::Namespace
        | NodeKind::ScalaPackage
        | NodeKind::GoPackage
        | NodeKind::KotlinPackage
        | NodeKind::PascalUnit
        | NodeKind::Library => -1.5,
    }
}

/// Parses every `#[derive(A, B, C)]` attribute appearing in `content`
/// between (0-based, inclusive) `start_line` and `end_line`. Multiple
/// derive attributes stack — `#[derive(Debug)]` and `#[derive(Clone)]` on
/// the same item both contribute. The returned list is de-duplicated and
/// preserves source order (Debug before Clone if that's how they're
/// written).
fn parse_derives_in_attr_block(content: &str, start_line: u32, end_line: u32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line as usize;
    let end = (end_line as usize).min(lines.len().saturating_sub(1));
    if start >= lines.len() {
        return out;
    }
    // Join the attribute block into a single string so multi-line
    // `#[derive(\n  Debug,\n  Clone,\n)]` (rustfmt's split form for long
    // derive lists) is handled uniformly with the single-line variant.
    let block = lines[start..=end].join("\n");
    let mut search_from = 0usize;
    while let Some(start_idx) = block[search_from..].find("#[derive(") {
        let abs_start = search_from + start_idx + "#[derive(".len();
        let Some(close_offset) = block[abs_start..].find(')') else {
            break;
        };
        let inner = &block[abs_start..abs_start + close_offset];
        for name in inner.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            // Strip the path prefix on fully-qualified derives so callers
            // see `Serialize` not `serde::Serialize`. Matches the convention
            // the static derive table uses.
            let short = name.rsplit("::").next().unwrap_or(name).to_string();
            if seen.insert(short.clone()) {
                out.push(short);
            }
        }
        search_from = abs_start + close_offset + 1;
    }
    out
}

/// Normalises an external file path (typically from a `cargo check` /
/// `cargo clippy` diagnostic span) into the project-relative,
/// forward-slash form the index stores. Handles three real-world shapes:
///
/// - Absolute paths (cargo emits them when `--manifest-path` points at a
///   project root that differs from `cwd`): strip the `project_root`
///   prefix so `/abs/path/to/project/src/lib.rs` becomes `src/lib.rs`.
/// - Backslash paths (Windows cargo): convert `\` → `/`.
/// - Already-relative forward-slash paths: pass through unchanged.
///
/// Falls back to returning the input verbatim if no transformation
/// applies — `get_nodes_by_file` will then handle "no such file" the
/// same way it always does.
fn normalize_lookup_path(project_root: &std::path::Path, raw: &str) -> String {
    let forward = raw.replace('\\', "/");
    let path = std::path::Path::new(&forward);
    if path.is_absolute() {
        // Try canonicalising both sides; canonicalisation handles
        // symlinks, `..` segments, and trailing slashes uniformly. If
        // either fails (file doesn't exist on disk, project root
        // moved), fall back to a raw prefix strip.
        if let (Ok(abs), Ok(root)) = (path.canonicalize(), project_root.canonicalize()) {
            if let Ok(rel) = abs.strip_prefix(&root) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
        let root_str = project_root.to_string_lossy();
        if let Some(rel) = forward.strip_prefix(root_str.as_ref()) {
            return rel.trim_start_matches('/').to_string();
        }
    }
    forward
}

/// True when the user-supplied query matches either the node's short `name`
/// or its `qualified_name`. Matching is exact on the short name and substring
/// on the qualified name, so callers can pass either form for the impl/trait
/// filter on `tokensave_impls`.
fn node_name_matches(node: &Node, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    node.name == query || node.qualified_name == query || node.qualified_name.contains(query)
}

/// Returns `true` if the file path looks like a test file.
pub fn is_test_file(path: &str) -> bool {
    let test_segments = [
        "test/",
        "tests/",
        "__tests__/",
        "spec/",
        "e2e/",
        ".test.",
        ".spec.",
        "_test.",
        "_spec.",
    ];
    let lower = path.to_ascii_lowercase();
    test_segments.iter().any(|s| lower.contains(s))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod derive_parse_tests {
    use super::parse_derives_in_attr_block;

    #[test]
    fn parses_single_derive_block() {
        let src = "\
#[derive(Debug, Clone, PartialEq)]
pub struct Foo;
";
        let derives = parse_derives_in_attr_block(src, 0, 1);
        assert_eq!(derives, vec!["Debug", "Clone", "PartialEq"]);
    }

    #[test]
    fn stacks_multiple_derive_attributes() {
        let src = "\
#[derive(Debug)]
#[derive(Clone, Hash)]
pub enum K {}
";
        let derives = parse_derives_in_attr_block(src, 0, 2);
        assert_eq!(derives, vec!["Debug", "Clone", "Hash"]);
    }

    #[test]
    fn strips_path_prefix_on_qualified_derive() {
        let src = "#[derive(serde::Serialize, Debug)]\npub struct S;\n";
        let derives = parse_derives_in_attr_block(src, 0, 1);
        assert_eq!(derives, vec!["Serialize", "Debug"]);
    }

    #[test]
    fn ignores_non_derive_attributes() {
        let src = "\
#[cfg(feature = \"foo\")]
#[serde(rename = \"x\")]
#[derive(Debug)]
pub struct S;
";
        let derives = parse_derives_in_attr_block(src, 0, 3);
        assert_eq!(derives, vec!["Debug"]);
    }

    #[test]
    fn deduplicates_repeated_derives() {
        let src = "#[derive(Debug, Debug, Clone)]\npub struct S;\n";
        let derives = parse_derives_in_attr_block(src, 0, 1);
        assert_eq!(derives, vec!["Debug", "Clone"]);
    }

    /// Regression: rustfmt splits long derive lists across lines:
    ///   `#[derive(\n    Debug,\n    Clone,\n    PartialEq,\n)]`
    /// The previous line-bounded parser dropped all of these because it
    /// only matched `#[derive(...)]` when the closing `)` was on the
    /// same line. Production codebases with realistic-sized derive
    /// lists were getting empty `derives` output.
    #[test]
    fn parses_multiline_derive_attribute() {
        let src = "\
#[derive(
    Debug,
    Clone,
    PartialEq,
)]
pub struct Wide;
";
        let derives = parse_derives_in_attr_block(src, 0, 5);
        assert_eq!(derives, vec!["Debug", "Clone", "PartialEq"]);
    }

    #[test]
    fn parses_multiline_derive_mixed_with_single_line() {
        let src = "\
#[derive(Debug)]
#[derive(
    Clone,
    Hash,
)]
pub struct M;
";
        let derives = parse_derives_in_attr_block(src, 0, 5);
        assert_eq!(derives, vec!["Debug", "Clone", "Hash"]);
    }
}
