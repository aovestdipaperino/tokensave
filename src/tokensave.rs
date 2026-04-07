// Rust guideline compliant 2025-10-17
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::branch;
use crate::branch_meta::{self, BranchMeta};
use crate::config::{get_tokensave_dir, is_excluded, load_config, save_config, TokenSaveConfig};
use crate::context::ContextBuilder;
use crate::db::Database;
use crate::errors::{TokenSaveError, Result};
use crate::extraction::LanguageRegistry;
use crate::graph::{GraphQueryManager, GraphTraverser};
use crate::resolution::ReferenceResolver;
use crate::sync;
use crate::types::*;

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
    /// Set when serving from a fallback (ancestor) DB instead of the exact branch.
    fallback_warning: Option<String>,
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
    /// Initializes a new TokenSave project at the given root.
    ///
    /// Creates the `.tokensave` directory, writes a default configuration,
    /// and initializes a fresh SQLite database.
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
        let default_branch = branch::detect_default_branch(project_root)
            .or_else(|| active_branch.clone());
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
            fallback_warning: None,
        })
    }

    /// Opens an existing TokenSave project at the given root.
    ///
    /// If branch metadata exists, resolves the current git branch and opens
    /// the corresponding DB. Falls back to the nearest tracked ancestor DB
    /// with a warning if the current branch is untracked.
    pub async fn open(project_root: &Path) -> Result<Self> {
        let config = load_config(project_root)?;
        let tokensave_dir = get_tokensave_dir(project_root);
        let active_branch = branch::current_branch(project_root);

        let (db_path, fallback_warning) =
            Self::resolve_db_for_branch(project_root, &tokensave_dir, active_branch.as_deref());

        if !db_path.exists() {
            return Err(TokenSaveError::Config {
                message: format!(
                    "no TokenSave database found at '{}'; run 'tokensave sync' first",
                    db_path.display()
                ),
            });
        }

        let (db, migrated) = Database::open(&db_path).await?;
        let ts = Self {
            db,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
            active_branch,
            fallback_warning,
        };

        if migrated {
            eprintln!("[tokensave] schema changed — performing full re-index…");
            ts.index_all_with_progress(|current, total, file| {
                eprintln!("[tokensave] re-indexing [{current}/{total}] {file}");
            }).await?;
            eprintln!("[tokensave] re-index complete.");
        }

        Ok(ts)
    }

    /// Resolves which DB file to open for a given branch.
    ///
    /// Returns `(db_path, fallback_warning)`. The warning is `Some` when
    /// falling back to an ancestor branch's DB.
    fn resolve_db_for_branch(
        project_root: &Path,
        tokensave_dir: &Path,
        branch: Option<&str>,
    ) -> (PathBuf, Option<String>) {
        let default_db = tokensave_dir.join("tokensave.db");

        let Some(meta) = branch_meta::load_branch_meta(tokensave_dir) else {
            // No branch metadata — single-DB mode (backward compat)
            return (default_db, None);
        };

        let Some(branch) = branch else {
            // Detached HEAD — use default branch DB
            return (default_db, Some("detached HEAD — using default branch index".to_string()));
        };

        // Exact match: branch is tracked
        if let Some(path) = branch::resolve_branch_db_path(tokensave_dir, branch, &meta) {
            if path.exists() {
                return (path, None);
            }
        }

        // Fallback: find nearest tracked ancestor
        if let Some(ancestor) = branch::find_nearest_tracked_ancestor(project_root, branch, &meta)
        {
            if let Some(path) = branch::resolve_branch_db_path(tokensave_dir, &ancestor, &meta) {
                if path.exists() {
                    return (
                        path,
                        Some(format!(
                            "branch '{branch}' is not tracked — serving from '{ancestor}'. \
                             Run `tokensave branch add {branch}` to track it."
                        )),
                    );
                }
            }
        }

        // Last resort: default branch DB
        (
            default_db,
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

        let db_path =
            branch::resolve_branch_db_path(&tokensave_dir, branch_name, &meta).ok_or_else(
                || TokenSaveError::Config {
                    message: format!("branch '{branch_name}' is not tracked"),
                },
            )?;

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
            fallback_warning: None,
        })
    }

    /// Lists tracked branches from metadata. Returns `None` if no branch tracking.
    pub fn list_tracked_branches(project_root: &Path) -> Option<Vec<String>> {
        let tokensave_dir = get_tokensave_dir(project_root);
        let meta = branch_meta::load_branch_meta(&tokensave_dir)?;
        Some(meta.branches.keys().cloned().collect())
    }

    /// Returns `true` if a TokenSave project has been initialized at the given root.
    pub fn is_initialized(project_root: &Path) -> bool {
        get_tokensave_dir(project_root)
            .join("tokensave.db")
            .exists()
    }
}

// ---------------------------------------------------------------------------
// Sync lock — prevents concurrent sync/index operations
// ---------------------------------------------------------------------------

/// RAII guard that holds the sync lockfile open. Removing the lockfile on drop
/// is best-effort; if it fails (e.g. permissions), the stale-PID check on the
/// next attempt will reclaim it.
struct SyncLockGuard {
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
fn try_acquire_sync_lock(project_root: &Path) -> Result<SyncLockGuard> {
    let lock_path = get_tokensave_dir(project_root).join("sync.lock");
    let pid = std::process::id();

    // Fast path: try atomic create.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut f) => {
            use std::io::Write;
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
    use std::io::Write;
    let _ = write!(f, "{pid}");
    Ok(SyncLockGuard { path: lock_path })
}

/// Returns `true` if a process with the given PID is currently running.
fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
        debug_assert!(self.project_root.exists(), "project root does not exist");
        debug_assert!(self.project_root.is_dir(), "project root is not a directory");
        let _lock = try_acquire_sync_lock(&self.project_root)?;
        let start = Instant::now();

        // 1. Clear existing data and enter bulk-load mode
        self.db.clear().await?;
        self.db.begin_bulk_load().await?;

        // 2. Scan for source files
        let files = self.scan_files()?;
        let total = files.len();

        // 3. Parallel extraction: read + parse + hash on all cores
        let project_root = &self.project_root;
        let registry = &self.registry;

        let extractions: Vec<_> = files
            .par_iter()
            .filter_map(|file_path| {
                let abs_path = project_root.join(file_path);
                let source = std::fs::read_to_string(&abs_path).ok()?;
                let extractor = registry.extractor_for_file(file_path)?;
                let mut result = extractor.extract(file_path, &source);
                result.sanitize();
                let hash = sync::content_hash(&source);
                let size = source.len() as u64;
                Some((file_path.clone(), result, hash, size))
            })
            .collect();

        // 4. Collect all data
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();
        let mut all_unresolved = Vec::new();
        let mut file_records = Vec::new();
        let mut total_nodes = 0;
        let total_edges;

        for (idx, (file_path, result, hash, size)) in extractions.iter().enumerate() {
            on_file(idx + 1, total, file_path);
            total_nodes += result.nodes.len();
            all_nodes.extend_from_slice(&result.nodes);
            all_edges.extend_from_slice(&result.edges);
            all_unresolved.extend_from_slice(&result.unresolved_refs);
            file_records.push(FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: current_timestamp(),
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            });
        }

        // 5. Resolve references in-memory (parallel) before DB insert
        if !all_unresolved.is_empty() {
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            let resolution = resolver.resolve_all(&all_unresolved);
            all_edges.extend(resolver.create_edges(&resolution.resolved));
        }

        // 6. Sort by PK order + dedup edges
        all_nodes.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        all_edges.sort_unstable_by(|a, b| {
            (&a.source, &a.target, a.kind.as_str(), &a.line)
                .cmp(&(&b.source, &b.target, b.kind.as_str(), &b.line))
        });
        all_edges.dedup_by(|a, b| {
            a.source == b.source && a.target == b.target && a.kind == b.kind && a.line == b.line
        });
        file_records.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        total_edges = all_edges.len();

        // 7. Bulk-insert via prepared statements (zero SQL re-parsing)
        self.db.insert_nodes(&all_nodes).await?;
        self.db.insert_edges(&all_edges).await?;
        self.db.upsert_files(&file_records).await?;

        // 8. Restore indexes and normal durability
        self.db.end_bulk_load().await?;

        let now_str = current_timestamp().to_string();
        self.db.set_metadata("last_full_sync_at", &now_str).await?;
        self.db.set_metadata("last_sync_at", &now_str).await?;

        let result = IndexResult {
            file_count: files.len(),
            node_count: total_nodes,
            edge_count: total_edges,
            duration_ms: start.elapsed().as_millis() as u64,
        };
        debug_assert!(result.node_count >= result.file_count || result.file_count == 0,
            "fewer nodes than files is unexpected");
        debug_assert!(result.duration_ms > 0 || result.file_count == 0,
            "non-empty index completed in zero milliseconds");
        Ok(result)
    }

    /// Performs an incremental sync: detects changed, new, and removed files
    /// and re-indexes only those that need updating.
    pub async fn sync(&self) -> Result<SyncResult> {
        self.sync_with_progress(|_, _, _| {}).await
    }

    /// Like `sync()`, but calls `on_progress` with a description and the
    /// current step for each phase of work. Use this to drive a progress
    /// spinner in the CLI.
    ///
    /// The callback receives `(current_file_index, total_files, message)` where
    /// `current_file_index` and `total_files` are zero during non-file phases
    /// (scanning, hashing, detecting, resolving) and populated during the
    /// per-file syncing phase.
    pub async fn sync_with_progress<F>(&self, on_progress: F) -> Result<SyncResult>
    where
        F: Fn(usize, usize, &str),
    {
        debug_assert!(self.project_root.exists(), "sync: project root does not exist");
        debug_assert!(self.project_root.is_dir(), "sync: project root is not a directory");
        let _lock = try_acquire_sync_lock(&self.project_root)?;
        let start = Instant::now();

        on_progress(0, 0, "scanning files");
        let current_files = self.scan_files()?;

        // Compute current hashes in parallel
        on_progress(0, 0, "hashing files");
        let project_root = &self.project_root;
        let current_hashes: Vec<_> = current_files
            .par_iter()
            .filter_map(|path| {
                let abs_path = project_root.join(path);
                let source = std::fs::read_to_string(&abs_path).ok()?;
                Some((path.clone(), sync::content_hash(&source)))
            })
            .collect();

        on_progress(0, 0, "detecting changes");
        let stale = sync::find_stale_files(&self.db, &current_hashes).await?;
        let new = sync::find_new_files(&self.db, &current_files).await?;
        let removed = sync::find_removed_files(&self.db, &current_files).await?;

        // Remove deleted files
        for path in &removed {
            on_progress(0, 0, &format!("removing {path}"));
            self.db.delete_file(path).await?;
        }

        // Re-index stale and new files — extract in parallel, insert sequentially
        let to_index: Vec<String> = stale.iter().chain(new.iter()).cloned().collect();
        let registry = &self.registry;

        let sync_extractions: Vec<_> = to_index
            .par_iter()
            .filter_map(|file_path| {
                let abs_path = project_root.join(file_path);
                let source = std::fs::read_to_string(&abs_path).ok()?;
                let extractor = registry.extractor_for_file(file_path)?;
                let mut result = extractor.extract(file_path, &source);
                result.sanitize();
                let hash = sync::content_hash(&source);
                let size = source.len() as u64;
                Some((file_path.clone(), result, hash, size))
            })
            .collect();

        let total = sync_extractions.len();
        for (idx, (file_path, result, hash, size)) in sync_extractions.iter().enumerate() {
            on_progress(idx + 1, total, file_path);

            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            self.db.insert_edges(&result.edges).await?;
            if !result.unresolved_refs.is_empty() {
                self.db.insert_unresolved_refs(&result.unresolved_refs).await?;
            }

            let file_record = FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: current_timestamp(),
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record).await?;
        }

        // Resolve references (call edges, uses, etc.) across all files.
        // This must run after all files are indexed so cross-file references
        // can find their targets.
        if !to_index.is_empty() {
            on_progress(0, 0, "resolving references");
            let unresolved = self.db.get_unresolved_refs().await?;
            if !unresolved.is_empty() {
                let resolver = ReferenceResolver::new(&self.db).await;
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

        Ok(SyncResult {
            files_added: new.len(),
            files_modified: stale.len(),
            files_removed: removed.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            added_paths: new,
            modified_paths: stale,
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
    fn scan_files(&self) -> Result<Vec<String>> {
        debug_assert!(self.project_root.is_dir(), "scan_files: project_root is not a directory");
        let supported_exts = self.registry.supported_extensions();
        debug_assert!(!supported_exts.is_empty(), "scan_files: no supported extensions registered");

        if self.config.git_ignore {
            let files = self.scan_files_with_gitignore(&supported_exts)?;
            if files.is_empty() {
                // The project directory may be gitignored by a parent repo,
                // causing the ignore-aware walker to skip everything. Fall
                // back to plain walkdir if source files clearly exist.
                let has_source = WalkDir::new(&self.project_root)
                    .max_depth(2)
                    .into_iter()
                    .filter_map(|e| e.ok())
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
            Ok(files)
        } else {
            self.scan_files_walkdir(&supported_exts)
        }
    }

    /// Walk using `walkdir`, skipping hidden directories and `target/`.
    fn scan_files_walkdir(
        &self,
        supported_exts: &[&str],
    ) -> Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.project_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target"
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts) {
                files.push(rel_str);
            }
        }
        Ok(files)
    }

    /// Walk using the `ignore` crate, which respects `.gitignore` rules,
    /// `.git/info/exclude`, and the user's global gitignore.
    fn scan_files_with_gitignore(
        &self,
        supported_exts: &[&str],
    ) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let walker = ignore::WalkBuilder::new(&self.project_root)
            .follow_links(false)
            .hidden(true) // skip hidden files/dirs
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let Some(ft) = entry.file_type() else {
                continue;
            };
            if !ft.is_file() {
                continue;
            }
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts) {
                files.push(rel_str);
            }
        }
        Ok(files)
    }

    /// Checks whether a file should be included: correct extension, not
    /// excluded by config globs, and within the max file size.
    fn accept_file(
        &self,
        path: &Path,
        supported_exts: &[&str],
    ) -> Option<String> {
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
}

// ---------------------------------------------------------------------------
// Query delegation
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Searches for nodes matching the given query string.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.db.search_nodes(query, limit).await
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

    /// Builds a bidirectional call graph around a node.
    pub async fn get_call_graph(&self, node_id: &str, depth: usize) -> Result<Subgraph> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_call_graph(node_id, depth).await
    }

    /// Finds potentially dead code (nodes with no incoming edges).
    pub async fn find_dead_code(&self, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.find_dead_code(kinds).await
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
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        self.db
            .get_ranked_nodes_by_edge_kind(edge_kind, node_kind, incoming, limit)
            .await
    }

    /// Returns nodes ranked by line span, optionally filtered by node kind.
    pub async fn get_largest_nodes(
        &self,
        node_kind: Option<&NodeKind>,
        limit: usize,
    ) -> Result<Vec<(Node, u32)>> {
        self.db.get_largest_nodes(node_kind, limit).await
    }

    /// Returns files ranked by coupling (fan-in or fan-out).
    pub async fn get_file_coupling(
        &self,
        fan_in: bool,
        limit: usize,
    ) -> Result<Vec<(String, u64)>> {
        self.db.get_file_coupling(fan_in, limit).await
    }

    /// Returns classes/interfaces ranked by inheritance depth via extends chains.
    pub async fn get_inheritance_depth(&self, limit: usize) -> Result<Vec<(Node, u64)>> {
        self.db.get_inheritance_depth(limit).await
    }

    /// Returns node kind distribution, optionally filtered by path prefix.
    pub async fn get_node_distribution(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, u64)>> {
        self.db.get_node_distribution(path_prefix).await
    }

    /// Returns all calls edges as (source_id, target_id) pairs for cycle detection.
    pub async fn get_call_edges(&self) -> Result<Vec<(String, String)>> {
        self.db.get_call_edges().await
    }

    /// Returns functions/methods ranked by composite complexity score.
    pub async fn get_complexity_ranked(
        &self,
        node_kind: Option<&NodeKind>,
        limit: usize,
    ) -> Result<Vec<(Node, u32, u64, u64, u64)>> {
        self.db.get_complexity_ranked(node_kind, limit).await
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
    pub async fn get_god_classes(&self, limit: usize) -> Result<Vec<(Node, u64, u64, u64)>> {
        self.db.get_god_classes(limit).await
    }

    /// Detects circular dependencies at the file level.
    pub async fn find_circular_dependencies(&self) -> Result<Vec<Vec<String>>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.find_circular_dependencies().await
    }

    /// Builds an AI-ready context for a given task description.
    pub async fn build_context(&self, task: &str, options: &BuildContextOptions) -> Result<TaskContext> {
        let builder = ContextBuilder::new(&self.db, &self.project_root);
        builder.build_context(task, options).await
    }

    /// Returns all indexed file records.
    pub async fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        self.db.get_all_files().await
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
    /// Check if specific files have been modified on disk since they were indexed.
    /// Returns a list of relative paths for files whose mtime is newer than `indexed_at`.
    pub async fn check_file_staleness(&self, file_paths: &[String]) -> Vec<String> {
        let mut stale = Vec::new();
        for path in file_paths {
            if let Ok(Some(record)) = self.db.get_file(path).await {
                let abs_path = self.project_root.join(path);
                if let Ok(metadata) = std::fs::metadata(&abs_path) {
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
        let repo = match gix::open(&self.project_root) {
            Ok(r) => r,
            Err(_) => return 0,
        };
        let head = match repo.head_commit() {
            Ok(h) => h,
            Err(_) => return 0,
        };
        let sorting = gix::revision::walk::Sorting::ByCommitTimeCutoff {
            order: gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            seconds: since_timestamp,
        };
        let walk = match head.ancestors().sorting(sorting).all() {
            Ok(w) => w,
            Err(_) => return 0,
        };
        walk.filter_map(|r| r.ok()).count()
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Returns `true` if the file path looks like a test file.
pub fn is_test_file(path: &str) -> bool {
    let test_segments = [
        "test/", "tests/", "__tests__/", "spec/", "e2e/",
        ".test.", ".spec.", "_test.", "_spec.",
    ];
    let lower = path.to_ascii_lowercase();
    test_segments.iter().any(|s| lower.contains(s))
}
