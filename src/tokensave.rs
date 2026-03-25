// Rust guideline compliant 2025-10-17
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use walkdir::WalkDir;

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
pub struct SyncResult {
    /// Number of newly added files.
    pub files_added: usize,
    /// Number of modified (re-indexed) files.
    pub files_modified: usize,
    /// Number of removed files.
    pub files_removed: usize,
    /// Time taken in milliseconds.
    pub duration_ms: u64,
}

/// Returns the current UNIX timestamp in seconds.
fn current_timestamp() -> i64 {
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
        let db = Database::initialize(&db_path).await?;

        Ok(Self {
            db,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
        })
    }

    /// Opens an existing TokenSave project at the given root.
    ///
    /// Loads the configuration from disk and opens the existing database.
    pub async fn open(project_root: &Path) -> Result<Self> {
        let config = load_config(project_root)?;
        let db_path = get_tokensave_dir(project_root).join("tokensave.db");

        if !db_path.exists() {
            return Err(TokenSaveError::Config {
                message: format!(
                    "no TokenSave database found at '{}'; run 'tokensave sync' first",
                    db_path.display()
                ),
            });
        }

        let db = Database::open(&db_path).await?;
        Ok(Self {
            db,
            config,
            project_root: project_root.to_path_buf(),
            registry: LanguageRegistry::new(),
        })
    }

    /// Returns `true` if a TokenSave project has been initialized at the given root.
    pub fn is_initialized(project_root: &Path) -> bool {
        get_tokensave_dir(project_root)
            .join("tokensave.db")
            .exists()
    }
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Performs a full index: clears existing data, scans all Rust files,
    /// extracts nodes and edges, resolves references, and stores everything
    /// in the database.
    pub async fn index_all(&self) -> Result<IndexResult> {
        self.index_all_with_progress(|_| {}).await
    }

    /// Like `index_all()`, but calls `on_file` with each file path before
    /// processing it. Use this to drive a progress spinner in the CLI.
    pub async fn index_all_with_progress<F>(&self, on_file: F) -> Result<IndexResult>
    where
        F: Fn(&str),
    {
        debug_assert!(self.project_root.exists(), "project root does not exist");
        debug_assert!(self.project_root.is_dir(), "project root is not a directory");
        let start = Instant::now();

        // 1. Clear existing data
        self.db.clear().await?;

        // 2. Scan for Rust files using walkdir
        let files = self.scan_files()?;

        // 3. For each file: read, extract with RustExtractor, store nodes/edges/unresolved_refs
        let mut total_nodes = 0;
        let mut total_edges = 0;

        for file_path in &files {
            on_file(file_path);

            let abs_path = self.project_root.join(file_path);
            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let extractor = match self.registry.extractor_for_file(file_path) {
                Some(e) => e,
                None => continue,
            };
            let result = extractor.extract(file_path, &source);

            // Store nodes and edges
            self.db.insert_nodes(&result.nodes).await?;
            self.db.insert_edges(&result.edges).await?;

            if !result.unresolved_refs.is_empty() {
                self.db.insert_unresolved_refs(&result.unresolved_refs).await?;
            }

            // Store file record
            let file_record = FileRecord {
                path: file_path.clone(),
                content_hash: sync::content_hash(&source),
                size: source.len() as u64,
                modified_at: current_timestamp(),
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record).await?;

            total_nodes += result.nodes.len();
            total_edges += result.edges.len();
        }

        // 4. Resolve references
        let unresolved = self.db.get_unresolved_refs().await?;
        if !unresolved.is_empty() {
            let resolver = ReferenceResolver::new(&self.db).await;
            let resolution = resolver.resolve_all(&unresolved);
            let edges = resolver.create_edges(&resolution.resolved);
            if !edges.is_empty() {
                self.db.insert_edges(&edges).await?;
                total_edges += edges.len();
            }
        }

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
        self.sync_with_progress(|_, _| {}).await
    }

    /// Like `sync()`, but calls `on_progress` with a description and the
    /// current step for each phase of work. Use this to drive a progress
    /// spinner in the CLI.
    pub async fn sync_with_progress<F>(&self, on_progress: F) -> Result<SyncResult>
    where
        F: Fn(&str, &str),
    {
        debug_assert!(self.project_root.exists(), "sync: project root does not exist");
        debug_assert!(self.project_root.is_dir(), "sync: project root is not a directory");
        let start = Instant::now();

        on_progress("scanning files", "");
        let current_files = self.scan_files()?;

        // Compute current hashes
        on_progress("hashing files", "");
        let mut current_hashes = Vec::new();
        for path in &current_files {
            let abs_path = self.project_root.join(path);
            if let Ok(source) = std::fs::read_to_string(&abs_path) {
                current_hashes.push((path.clone(), sync::content_hash(&source)));
            }
        }

        on_progress("detecting changes", "");
        let stale = sync::find_stale_files(&self.db, &current_hashes).await?;
        let new = sync::find_new_files(&self.db, &current_files).await?;
        let removed = sync::find_removed_files(&self.db, &current_files).await?;

        // Remove deleted files
        for path in &removed {
            on_progress("removing", path);
            self.db.delete_file(path).await?;
        }

        // Re-index stale and new files
        let to_index: Vec<String> = stale.iter().chain(new.iter()).cloned().collect();
        for file_path in &to_index {
            on_progress("syncing", file_path);

            // Delete old data for this file
            self.db.delete_nodes_by_file(file_path).await?;

            let abs_path = self.project_root.join(file_path);
            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let extractor = match self.registry.extractor_for_file(file_path) {
                Some(e) => e,
                None => continue,
            };
            let result = extractor.extract(file_path, &source);
            self.db.insert_nodes(&result.nodes).await?;
            self.db.insert_edges(&result.edges).await?;
            if !result.unresolved_refs.is_empty() {
                self.db.insert_unresolved_refs(&result.unresolved_refs).await?;
            }

            let file_record = FileRecord {
                path: file_path.clone(),
                content_hash: sync::content_hash(&source),
                size: source.len() as u64,
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
            on_progress("resolving references", "");
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

        Ok(SyncResult {
            files_added: new.len(),
            files_modified: stale.len(),
            files_removed: removed.len(),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Scans the project root for source files in all supported languages,
    /// respecting the configured exclude patterns and max file size.
    ///
    /// Supported extensions are derived from the `LanguageRegistry` so that
    /// adding a new extractor automatically picks up its files.
    fn scan_files(&self) -> Result<Vec<String>> {
        debug_assert!(self.project_root.is_dir(), "scan_files: project_root is not a directory");
        let supported_exts = self.registry.supported_extensions();
        debug_assert!(!supported_exts.is_empty(), "scan_files: no supported extensions registered");
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.project_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Always allow the root directory itself (depth 0), even if
                // its name starts with '.' (e.g. temp dirs on macOS).
                if e.depth() == 0 {
                    return true;
                }
                // Skip hidden directories and target/
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
            let path = entry.path();
            // Check extension against registry-supported languages
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !supported_exts.contains(&ext) {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(&self.project_root) {
                let rel_str = relative.to_string_lossy().to_string();
                if !is_excluded(&rel_str, &self.config) {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        if metadata.len() <= self.config.max_file_size {
                            files.push(rel_str);
                        }
                    }
                }
            }
        }
        Ok(files)
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

    /// Checkpoints the WAL and closes the database connection.
    pub async fn checkpoint(&self) -> Result<()> {
        self.db.checkpoint().await
    }

    /// Returns a reference to the current configuration.
    pub fn get_config(&self) -> &TokenSaveConfig {
        &self.config
    }

    /// Returns the project root path.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}
