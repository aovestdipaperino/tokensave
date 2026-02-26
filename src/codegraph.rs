use std::path::{Path, PathBuf};
use std::time::Instant;

use walkdir::WalkDir;

use crate::config::{
    get_codegraph_dir, load_config, save_config, should_include_file, CodeGraphConfig,
};
use crate::context::ContextBuilder;
use crate::db::Database;
use crate::errors::{CodeGraphError, Result};
use crate::extraction::RustExtractor;
use crate::graph::{GraphQueryManager, GraphTraverser};
use crate::resolution::ReferenceResolver;
use crate::sync;
use crate::types::*;

/// Central orchestrator that coordinates all subsystems of the code graph.
pub struct CodeGraph {
    db: Database,
    config: CodeGraphConfig,
    project_root: PathBuf,
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

impl CodeGraph {
    /// Initializes a new CodeGraph project at the given root.
    ///
    /// Creates the `.codegraph` directory, writes a default configuration,
    /// and initializes a fresh SQLite database.
    pub fn init(project_root: &Path) -> Result<Self> {
        let config = CodeGraphConfig {
            root_dir: project_root.to_string_lossy().to_string(),
            ..CodeGraphConfig::default()
        };
        save_config(project_root, &config)?;

        let db_path = get_codegraph_dir(project_root).join("codegraph.db");
        let db = Database::initialize(&db_path)?;

        Ok(Self {
            db,
            config,
            project_root: project_root.to_path_buf(),
        })
    }

    /// Opens an existing CodeGraph project at the given root.
    ///
    /// Loads the configuration from disk and opens the existing database.
    pub fn open(project_root: &Path) -> Result<Self> {
        let config = load_config(project_root)?;
        let db_path = get_codegraph_dir(project_root).join("codegraph.db");

        if !db_path.exists() {
            return Err(CodeGraphError::Config {
                message: format!(
                    "no CodeGraph database found at '{}'; run 'codegraph init' first",
                    db_path.display()
                ),
            });
        }

        let db = Database::open(&db_path)?;
        Ok(Self {
            db,
            config,
            project_root: project_root.to_path_buf(),
        })
    }

    /// Returns `true` if a CodeGraph project has been initialized at the given root.
    pub fn is_initialized(project_root: &Path) -> bool {
        get_codegraph_dir(project_root)
            .join("codegraph.db")
            .exists()
    }
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

impl CodeGraph {
    /// Performs a full index: clears existing data, scans all Rust files,
    /// extracts nodes and edges, resolves references, and stores everything
    /// in the database.
    pub fn index_all(&self) -> Result<IndexResult> {
        let start = Instant::now();

        // 1. Clear existing data
        self.db.clear()?;

        // 2. Scan for Rust files using walkdir
        let files = self.scan_files()?;

        // 3. For each file: read, extract with RustExtractor, store nodes/edges/unresolved_refs
        let mut total_nodes = 0;
        let mut total_edges = 0;

        for file_path in &files {
            let abs_path = self.project_root.join(file_path);
            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let result = RustExtractor::extract(file_path, &source);

            // Store nodes and edges
            self.db.insert_nodes(&result.nodes)?;
            self.db.insert_edges(&result.edges)?;

            if !result.unresolved_refs.is_empty() {
                self.db.insert_unresolved_refs(&result.unresolved_refs)?;
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
            self.db.upsert_file(&file_record)?;

            total_nodes += result.nodes.len();
            total_edges += result.edges.len();
        }

        // 4. Resolve references
        let unresolved = self.db.get_unresolved_refs()?;
        if !unresolved.is_empty() {
            let resolver = ReferenceResolver::new(&self.db);
            let resolution = resolver.resolve_all(&unresolved);
            let edges = resolver.create_edges(&resolution.resolved);
            if !edges.is_empty() {
                self.db.insert_edges(&edges)?;
                total_edges += edges.len();
            }
        }

        Ok(IndexResult {
            file_count: files.len(),
            node_count: total_nodes,
            edge_count: total_edges,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Performs an incremental sync: detects changed, new, and removed files
    /// and re-indexes only those that need updating.
    pub fn sync(&self) -> Result<SyncResult> {
        let start = Instant::now();
        let current_files = self.scan_files()?;

        // Compute current hashes
        let mut current_hashes = Vec::new();
        for path in &current_files {
            let abs_path = self.project_root.join(path);
            if let Ok(source) = std::fs::read_to_string(&abs_path) {
                current_hashes.push((path.clone(), sync::content_hash(&source)));
            }
        }

        let stale = sync::find_stale_files(&self.db, &current_hashes)?;
        let new = sync::find_new_files(&self.db, &current_files)?;
        let removed = sync::find_removed_files(&self.db, &current_files)?;

        // Remove deleted files
        for path in &removed {
            self.db.delete_file(path)?;
        }

        // Re-index stale and new files
        let to_index: Vec<String> = stale.iter().chain(new.iter()).cloned().collect();
        for file_path in &to_index {
            // Delete old data for this file
            self.db.delete_nodes_by_file(file_path)?;

            let abs_path = self.project_root.join(file_path);
            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let result = RustExtractor::extract(file_path, &source);
            self.db.insert_nodes(&result.nodes)?;
            self.db.insert_edges(&result.edges)?;
            if !result.unresolved_refs.is_empty() {
                self.db.insert_unresolved_refs(&result.unresolved_refs)?;
            }

            let file_record = FileRecord {
                path: file_path.clone(),
                content_hash: sync::content_hash(&source),
                size: source.len() as u64,
                modified_at: current_timestamp(),
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record)?;
        }

        Ok(SyncResult {
            files_added: new.len(),
            files_modified: stale.len(),
            files_removed: removed.len(),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Scans the project root for Rust files, respecting the configured
    /// include/exclude patterns and max file size.
    fn scan_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.project_root)
            .into_iter()
            .filter_entry(|e| {
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
            if let Ok(relative) = path.strip_prefix(&self.project_root) {
                let rel_str = relative.to_string_lossy().to_string();
                if should_include_file(&rel_str, &self.config) {
                    // Check file size
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

impl CodeGraph {
    /// Searches for nodes matching the given query string.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.db.search_nodes(query, limit)
    }

    /// Returns aggregate statistics about the code graph.
    pub fn get_stats(&self) -> Result<GraphStats> {
        self.db.get_stats()
    }

    /// Retrieves a single node by its unique ID.
    pub fn get_node(&self, id: &str) -> Result<Option<Node>> {
        self.db.get_node_by_id(id)
    }

    /// Returns all nodes that transitively call the given node, up to `max_depth`.
    pub fn get_callers(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_callers(node_id, max_depth)
    }

    /// Returns all nodes that the given node transitively calls, up to `max_depth`.
    pub fn get_callees(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_callees(node_id, max_depth)
    }

    /// Computes the impact radius: all nodes that directly or indirectly
    /// depend on the given node, up to `max_depth`.
    pub fn get_impact_radius(&self, node_id: &str, max_depth: usize) -> Result<Subgraph> {
        let traverser = GraphTraverser::new(&self.db);
        traverser.get_impact_radius(node_id, max_depth)
    }

    /// Finds potentially dead code (nodes with no incoming edges).
    pub fn find_dead_code(&self, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.find_dead_code(kinds)
    }

    /// Builds an AI-ready context for a given task description.
    pub fn build_context(&self, task: &str, options: &BuildContextOptions) -> Result<TaskContext> {
        let builder = ContextBuilder::new(&self.db, &self.project_root);
        builder.build_context(task, options)
    }

    /// Returns a reference to the current configuration.
    pub fn get_config(&self) -> &CodeGraphConfig {
        &self.config
    }

    /// Returns the project root path.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}
