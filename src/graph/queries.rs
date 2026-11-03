// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};

use crate::db::{Database, EdgeEndpoint};
use crate::errors::Result;
use crate::types::{EdgeKind, Node, NodeKind};

/// Godot virtual callbacks that the engine invokes on `Object`, `Node`,
/// `CanvasItem`, `Control`, `Resource` and the physics bodies. A `.gd`
/// method with one of these names has no caller in project code (#598).
const GODOT_ENGINE_VIRTUALS: &[&str] = &[
    "_init",
    "_static_init",
    "_ready",
    "_enter_tree",
    "_exit_tree",
    "_process",
    "_physics_process",
    "_input",
    "_unhandled_input",
    "_unhandled_key_input",
    "_shortcut_input",
    "_gui_input",
    "_notification",
    "_draw",
    "_to_string",
    "_get",
    "_set",
    "_get_property_list",
    "_validate_property",
    "_property_can_revert",
    "_property_get_revert",
    "_get_configuration_warnings",
    "_integrate_forces",
    "_has_point",
    "_get_minimum_size",
    "_make_custom_tooltip",
    "_get_drag_data",
    "_can_drop_data",
    "_drop_data",
    "_structured_text_parser",
    "_setup_local_to_scene",
    "_run",
];

/// Metrics describing the connectivity and structure around a single node.
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    /// Number of incoming edges (all kinds).
    pub incoming_edge_count: usize,
    /// Number of outgoing edges (all kinds).
    pub outgoing_edge_count: usize,
    /// Number of outgoing `Calls` edges (functions this node calls).
    pub call_count: usize,
    /// Number of incoming `Calls` edges (functions that call this node).
    pub caller_count: usize,
    /// Number of outgoing `Contains` edges (direct children).
    pub child_count: usize,
    /// Depth of the node in the containment hierarchy.
    pub depth: usize,
}

/// Provides analytical query operations over the code graph.
pub struct GraphQueryManager<'a> {
    db: &'a Database,
}

impl<'a> GraphQueryManager<'a> {
    /// Creates a new `GraphQueryManager` backed by the given database.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Finds nodes with zero incoming edges, indicating potentially dead code.
    ///
    /// Excludes:
    /// - Nodes named `"main"` (program entry points).
    /// - Nodes named `"_bind_methods"` (godot-cpp `GDExtension` binding
    ///   callbacks). This method is invoked by the Godot engine's `ClassDB`
    ///   registration machinery, never by indexed code, so it carries no
    ///   incoming edge and was misreported as dead (issue #269). It is an
    ///   engine-contract name, exempted like other entry points.
    /// - Nodes whose name starts with `"test"` (likely test functions).
    /// - Nodes annotated with a test-marker attribute — `#[test]`,
    ///   `#[tokio::test]`, `#[async_std::test]`, `#[wasm_bindgen_test]`, etc.
    ///   The libtest harness is the implicit caller of these and never shows
    ///   up as a graph edge, so without this filter most Rust tests with
    ///   non-`test*` names get misreported as dead.
    /// - By default, `pub` items (they may be part of a public API). Pass
    ///   `include_public=true` to drop this exclusion — useful when
    ///   auditing a workspace where most items are `pub` but only a subset
    ///   actually have callers in the indexed scope. Without the flag,
    ///   `pub`-heavy codebases were reporting 0 dead symbols.
    ///
    /// If `kinds` is non-empty, only nodes of the specified kinds are checked.
    pub async fn find_dead_code(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
    ) -> Result<Vec<Node>> {
        let mut dead = self
            .db
            .find_dead_code_candidates(
                kinds,
                include_public,
                include_trait_impls,
                GODOT_ENGINE_VIRTUALS,
            )
            .await?;

        if dead
            .iter()
            .any(|n| crate::resolution::is_gdscript(&n.file_path))
        {
            // Best effort: a failed lookup leaves the overrides reported, and
            // must not fail the whole dead-code query.
            let live = self.gdscript_live_overrides().await.unwrap_or_default();
            if !live.is_empty() {
                dead.retain(|n| !live.contains(&n.id));
            }
        }
        Ok(dead)
    }

    /// `GDScript` methods that override a base-class method which is called or
    /// used (#598).
    ///
    /// A base class calling `_fields()` on `self` resolves to the base
    /// declaration, yet for a subclass instance it is the override that runs.
    /// The override has no edge of its own, and adding a synthetic one would
    /// make `callers`/`callees` claim a call the source does not make, so the
    /// override is exempted here instead. Inheritance comes from resolved
    /// `extends` edges, followed transitively, so a grandchild override of a
    /// called method is live too; an override of a base method nothing
    /// reaches stays dead, like the base method itself.
    async fn gdscript_live_overrides(&self) -> Result<HashSet<String>> {
        self.db.gdscript_live_overrides().await
    }

    /// Computes metrics for a single node describing its graph connectivity.
    pub async fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics> {
        let incoming = self.db.get_incoming_edges(node_id, &[]).await?;
        let outgoing = self.db.get_outgoing_edges(node_id, &[]).await?;

        let caller_count = incoming
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count();
        let call_count = outgoing
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count();
        // Children come from parent_id after v9, not from Contains edges.
        let child_count = self.db.get_children_of(node_id).await?.len();

        // Compute depth by walking up the containment hierarchy.
        let depth = self.compute_depth(node_id).await?;

        Ok(NodeMetrics {
            incoming_edge_count: incoming.len(),
            outgoing_edge_count: outgoing.len(),
            call_count,
            caller_count,
            child_count,
            depth,
        })
    }

    /// Gets the file paths that the given file depends on.
    ///
    /// Examines outgoing `Uses` and `Calls` edges from all nodes in the
    /// specified file. Returns the deduplicated set of target file paths,
    /// excluding the source file itself.
    pub async fn get_file_dependencies(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path).await?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let target_ids = self
            .db
            .get_edge_endpoint_ids(&node_ids, EdgeEndpoint::Targets)
            .await?;

        if target_ids.is_empty() {
            return Ok(Vec::new());
        }

        let target_nodes = self.db.get_nodes_by_ids(&target_ids).await?;
        let dep_files: HashSet<String> = target_nodes
            .into_iter()
            .filter(|n| n.file_path != file_path)
            .map(|n| n.file_path)
            .collect();

        let mut result: Vec<String> = dep_files.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Gets the file paths that depend on the given file.
    ///
    /// Examines incoming `Uses` and `Calls` edges to all nodes in the
    /// specified file. Returns the deduplicated set of source file paths,
    /// excluding the target file itself.
    pub async fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>> {
        let nodes = self.db.get_nodes_by_file(file_path).await?;
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let source_ids = self
            .db
            .get_edge_endpoint_ids(&node_ids, EdgeEndpoint::Sources)
            .await?;

        if source_ids.is_empty() {
            return Ok(Vec::new());
        }

        let source_nodes = self.db.get_nodes_by_ids(&source_ids).await?;
        let dependent_files: HashSet<String> = source_nodes
            .into_iter()
            .filter(|n| n.file_path != file_path)
            .map(|n| n.file_path)
            .collect();

        let mut result: Vec<String> = dependent_files.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Detects circular dependencies at the file level.
    ///
    /// Builds a file-level dependency graph and groups files into
    /// strongly-connected components via Tarjan's algorithm. Returns one
    /// component per mutually-recursive group of files; trivial
    /// single-file components without self-loops are filtered out (they
    /// aren't cycles). Replaces the prior walk-enumeration approach that
    /// reported 73 overlapping "cycles" on real codebases — each just a
    /// different DFS path through the same component.
    pub async fn find_circular_dependencies(&self) -> Result<Vec<Vec<String>>> {
        let all_files = self.db.get_all_files().await?;
        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
        for file in &all_files {
            let deps = self.get_file_dependencies(&file.path).await?;
            adj.insert(file.path.clone(), deps.into_iter().collect());
        }

        let sccs = super::scc::tarjan_scc(&adj);
        let mut cycles: Vec<Vec<String>> = sccs
            .into_iter()
            .filter(|s| super::scc::is_cyclic_scc(s, &adj))
            .collect();
        // Within each cycle, sort file paths so the output is stable
        // across runs (Tarjan's stack order is implementation-dependent).
        for cycle in &mut cycles {
            cycle.sort_unstable();
        }
        Ok(cycles)
    }

    /// Builds a file-level directed adjacency map from the code graph.
    ///
    /// For each file, collects the files it depends on via `calls` and
    /// `uses` (imports) edges. Self-edges are excluded. `implements` and
    /// `extends` are intentionally **not** followed: the Rust resolver
    /// fuzzy-binds `impl Debug for T` and similar to whatever node happens
    /// to share the trait's short name, which on real codebases produces
    /// long chains of spurious file-to-file dependencies (one bug report
    /// hit 19 levels through a chain of unrelated files terminating in a
    /// foreign crate's `lib.rs`).
    ///
    /// When `path_prefix` is `Some`, only files under that prefix are included
    /// (both as sources and targets).
    pub async fn build_file_adjacency(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<HashMap<String, HashSet<String>>> {
        let pairs = self.db.get_cross_file_edge_pairs().await?;

        // Normalise the prefix once: ensure it ends with '/'.
        let prefix: Option<String> = path_prefix.map(|p| {
            if p.ends_with('/') {
                p.to_string()
            } else {
                format!("{p}/")
            }
        });

        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();

        for (src, tgt) in pairs {
            if let Some(ref pfx) = prefix {
                if !src.starts_with(pfx.as_str()) || !tgt.starts_with(pfx.as_str()) {
                    continue;
                }
            }

            adj.entry(src).or_default().insert(tgt);
        }

        // Ensure every known file appears as a key (even leaf nodes with no deps).
        let all_files = self.db.get_all_files().await?;
        for file in all_files {
            if let Some(ref pfx) = prefix {
                if !file.path.starts_with(pfx.as_str()) {
                    continue;
                }
            }
            adj.entry(file.path).or_default();
        }

        Ok(adj)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Computes the depth of a node in the containment hierarchy by walking
    /// up incoming `Contains` edges.
    async fn compute_depth(&self, node_id: &str) -> Result<usize> {
        const MAX_DEPTH: usize = 100;
        let mut depth: usize = 0;
        let mut current_id = node_id.to_string();
        let mut visited: HashSet<String> = HashSet::new();

        while depth < MAX_DEPTH {
            if visited.contains(&current_id) {
                break;
            }
            visited.insert(current_id.clone());

            // parent_id is the v9 truth for containment.
            let Some(node) = self.db.get_node_by_id(&current_id).await? else {
                break;
            };
            match node.parent_id {
                Some(parent) => {
                    current_id = parent;
                    depth += 1;
                }
                None => break,
            }
        }

        Ok(depth)
    }
}
