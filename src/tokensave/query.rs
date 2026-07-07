//! Query delegation to the graph and database layers.
use super::*;

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
                // Path-based ranking: surface first-party application code
                // (src/, app/, lib/) ahead of equally-relevant matches buried
                // in vendor / generated trees (node_modules, dist, target, …).
                // Proportional, not a filter — a strong match in node_modules
                // still appears, just lower. Shared with context ranking so the
                // heuristics live in one place.
                r.score *= crate::context::ranking::path_rank_multiplier(&r.node.file_path);
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

    /// Finds the shortest directed call chain from `from_id` to `to_id`,
    /// following only outgoing `Calls` edges. Returns `None` if no chain
    /// exists within `max_depth` hops.
    pub async fn get_call_chain(
        &self,
        from_id: &str,
        to_id: &str,
        max_depth: usize,
    ) -> Result<Option<crate::graph::traversal::GraphPath>> {
        let traverser = GraphTraverser::new(&self.db);
        traverser
            .find_path_directed(from_id, to_id, &[crate::types::EdgeKind::Calls], max_depth)
            .await
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
    ///
    /// When `include_trait_impls` is `false` (the default), Rust trait-impl
    /// methods are excluded — they are dispatched implicitly by the compiler
    /// (e.g. `Display::fmt`, `Deref::deref`, `Drop::drop`) so they carry no
    /// explicit caller edge yet are never truly dead. Pass `true` to include
    /// them anyway. See issue #137.
    pub async fn find_dead_code(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
    ) -> Result<Vec<Node>> {
        let qm = GraphQueryManager::new(&self.db);
        qm.find_dead_code(kinds, include_public, include_trait_impls)
            .await
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

    /// Histogram of annotation usages across the project (or under
    /// `path_prefix`), sorted descending by count.
    pub async fn get_annotation_histogram(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, u64)>> {
        self.db.get_annotation_histogram(path_prefix).await
    }

    /// Annotation→target sites with optional filters. See
    /// [`crate::db::TokenSaveDb::get_annotation_sites`] for filter semantics.
    pub async fn get_annotation_sites(
        &self,
        name: Option<&str>,
        path_prefix: Option<&str>,
        target_kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        self.db
            .get_annotation_sites(name, path_prefix, target_kind, limit)
            .await
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

    /// Exact bare-name lookup using `idx_nodes_name`. No relevance scoring,
    /// no fuzzy matching — for that, use [`search`](Self::search).
    pub async fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>> {
        self.db.get_nodes_by_name(name).await
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

    /// Recompute the on-disk path to the `SQLite` DB this instance is
    /// serving. Useful for diagnostics (e.g. WAL/SHM size sampling) —
    /// returns the same path that `Database::open` was called with.
    pub fn db_path(&self) -> PathBuf {
        let tokensave_dir = get_tokensave_dir(&self.project_root);
        let (path, _, _) = Self::resolve_db_for_branch(
            &self.project_root,
            &tokensave_dir,
            self.serving_branch.as_deref(),
        );
        path
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

/// Resolves a symbol name to a single node suitable for symbol-aware editing.
///
/// Exact-qualified-name match wins; on ambiguity the resolver narrows to
/// callable kinds (function/method/etc.). If still more than one candidate
/// remains the edit is refused — silently picking the wrong site is far
/// worse than asking the caller to disambiguate.
pub(crate) async fn resolve_symbol_for_edit(cg: &TokenSave, symbol: &str) -> Result<Node> {
    let nodes = cg.get_nodes_by_qualified_name(symbol).await?;
    let mut iter = nodes.into_iter();
    let Some(first) = iter.next() else {
        return Err(TokenSaveError::Config {
            message: format!("symbol '{symbol}' not found"),
        });
    };
    let rest: Vec<Node> = iter.collect();
    if rest.is_empty() {
        return Ok(first);
    }
    let total = rest.len() + 1;
    let mut callables: Vec<Node> = std::iter::once(first)
        .chain(rest)
        .filter(|n| {
            matches!(
                n.kind,
                NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::StructMethod
                    | NodeKind::Constructor
                    | NodeKind::AbstractMethod
                    | NodeKind::ArrowFunction
                    | NodeKind::Procedure
            )
        })
        .collect();
    if callables.len() == 1 {
        return Ok(callables.remove(0));
    }
    Err(TokenSaveError::Config {
        message: format!(
            "symbol '{symbol}' is ambiguous ({total} matches); pass a fully qualified name"
        ),
    })
}
