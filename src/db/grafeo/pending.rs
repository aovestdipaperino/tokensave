//! `Database` methods the Grafeo backend has not ported yet (#904).
//!
//! Each keeps the libSQL signature, so the crate builds with either
//! backend, and fails with a [`pending`] error until it is ported. A
//! ported method moves out of this file into the module for its area.
#![allow(
    unused_variables,
    dead_code,
    clippy::unused_async,
    reason = "placeholders keep the libSQL signatures until each method is ported"
)]

use std::collections::{HashMap, HashSet};

use super::Database;
use crate::db::model::{
    AuditEdgeRow, CachedTraitDispatchCaller, EdgeEndpoint, ImportEdgeRow, NodeFilter,
    StoredFingerprint,
};
use crate::errors::{Result, TokenSaveError};
use crate::resolution::{AmbiguityRefKey, TouchedNode};
use crate::types::{
    AmbiguousCall, CodeAreaRecord, DecisionRecord, Edge, EdgeKind, ExecutableBodyDocument,
    GraphStats, Node, NodeKind, SearchResult, UnresolvedRef,
};

/// The error a not-yet-ported method returns.
fn pending(operation: &str) -> TokenSaveError {
    TokenSaveError::Database {
        message: format!("`{operation}` is not supported by the grafeo storage backend yet"),
        operation: operation.to_string(),
    }
}

impl Database {
    pub async fn for_each_use_node(&self, f: impl FnMut(String, String)) -> Result<()> {
        Err(pending("for_each_use_node"))
    }

    pub async fn for_each_symbol_name_count(&self, f: impl FnMut(String, i64)) -> Result<()> {
        Err(pending("for_each_symbol_name_count"))
    }

    pub async fn for_each_node_name(&self, f: impl FnMut(String, String)) -> Result<()> {
        Err(pending("for_each_node_name"))
    }

    pub async fn for_each_audit_edge(&self, f: impl FnMut(AuditEdgeRow)) -> Result<()> {
        Err(pending("for_each_audit_edge"))
    }

    pub async fn rebuild_ruby_reopenings(&self) -> Result<()> {
        Err(pending("rebuild_ruby_reopenings"))
    }

    pub async fn rebuild_trait_dispatch_callers(&self) -> Result<()> {
        Err(pending("rebuild_trait_dispatch_callers"))
    }

    pub async fn refresh_trait_dispatch_callers(&self) -> Result<()> {
        Err(pending("refresh_trait_dispatch_callers"))
    }

    pub fn has_trait_dispatch_callers(&self, node_id: &str) -> bool {
        false
    }

    pub fn cached_trait_dispatch_callers(&self, node_id: &str) -> Vec<CachedTraitDispatchCaller> {
        Vec::new()
    }

    pub async fn get_incoming_edges_bulk(
        &self,
        target_ids: &[String],
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        Err(pending("get_incoming_edges_bulk"))
    }

    pub async fn get_test_annotated_node_ids(
        &self,
        candidate_ids: &[String],
    ) -> Result<HashSet<String>> {
        Err(pending("get_test_annotated_node_ids"))
    }

    pub async fn get_annotation_histogram(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, u64)>> {
        Err(pending("get_annotation_histogram"))
    }

    pub async fn get_annotation_sites(
        &self,
        name: Option<&str>,
        path_prefix: Option<&str>,
        target_kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        Err(pending("get_annotation_sites"))
    }

    pub async fn get_files_with_test_annotations(&self) -> Result<HashSet<String>> {
        Err(pending("get_files_with_test_annotations"))
    }

    pub async fn get_skip_test_coverage_node_ids(&self) -> Result<HashSet<String>> {
        Err(pending("get_skip_test_coverage_node_ids"))
    }

    pub async fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>> {
        Err(pending("get_nodes_by_name"))
    }

    pub async fn get_nodes_by_qualified_name(&self, qname: &str) -> Result<Vec<Node>> {
        Err(pending("get_nodes_by_qualified_name"))
    }

    pub async fn get_ranked_nodes_by_edge_kind(
        &self,
        edge_kind: &EdgeKind,
        node_kind: Option<&NodeKind>,
        incoming: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        Err(pending("get_ranked_nodes_by_edge_kind"))
    }

    pub async fn get_largest_nodes(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32)>> {
        Err(pending("get_largest_nodes"))
    }

    pub async fn get_file_coupling(
        &self,
        fan_in: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, u64)>> {
        Err(pending("get_file_coupling"))
    }

    pub async fn get_inheritance_depth(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        Err(pending("get_inheritance_depth"))
    }

    pub async fn get_node_distribution(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, u64)>> {
        Err(pending("get_node_distribution"))
    }

    pub async fn get_call_edges(&self, path_prefix: Option<&str>) -> Result<Vec<(String, String)>> {
        Err(pending("get_call_edges"))
    }

    pub async fn get_call_edges_with_lines(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, Option<u32>)>> {
        Err(pending("get_call_edges_with_lines"))
    }

    pub async fn get_complexity_ranked(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32, u64, u64, u64)>> {
        Err(pending("get_complexity_ranked"))
    }

    pub async fn get_undocumented_public_symbols(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Node>> {
        Err(pending("get_undocumented_public_symbols"))
    }

    pub async fn get_god_classes(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64, u64, u64)>> {
        Err(pending("get_god_classes"))
    }

    pub async fn get_all_edges(&self) -> Result<Vec<Edge>> {
        Err(pending("get_all_edges"))
    }

    pub async fn get_edges_by_kind(&self, kind: EdgeKind) -> Result<Vec<Edge>> {
        Err(pending("get_edges_by_kind"))
    }

    pub async fn get_edges_by_kinds(&self, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        Err(pending("get_edges_by_kinds"))
    }

    pub async fn variant_group_candidates(&self) -> Result<Vec<(String, String)>> {
        Err(pending("variant_group_candidates"))
    }

    pub async fn go_variant_candidates(&self) -> Result<Vec<(String, String, String)>> {
        Err(pending("go_variant_candidates"))
    }

    pub async fn calls_edges_into(&self, targets: &[String]) -> Result<Vec<Edge>> {
        Err(pending("calls_edges_into"))
    }

    pub async fn get_top_degree_nodes(&self, limit: usize) -> Result<Vec<(String, u32, u32)>> {
        Err(pending("get_top_degree_nodes"))
    }

    pub async fn delete_edges_by_source(&self, source_id: &str) -> Result<()> {
        Err(pending("delete_edges_by_source"))
    }

    pub async fn delete_call_edges_at_sites(&self, sites: &[(String, u32, String)]) -> Result<()> {
        Err(pending("delete_call_edges_at_sites"))
    }

    pub async fn upsert_fingerprint(
        &self,
        node_id: &str,
        fp: &crate::redundancy::Fingerprint,
    ) -> Result<()> {
        Err(pending("upsert_fingerprint"))
    }

    pub async fn get_fingerprint(&self, node_id: &str) -> Result<Option<StoredFingerprint>> {
        Err(pending("get_fingerprint"))
    }

    pub async fn get_all_fingerprints(&self) -> Result<Vec<StoredFingerprint>> {
        Err(pending("get_all_fingerprints"))
    }

    pub async fn find_dead_code_candidates(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
        godot_virtuals: &[&str],
    ) -> Result<Vec<Node>> {
        Err(pending("find_dead_code_candidates"))
    }

    pub async fn gdscript_live_overrides(&self) -> Result<HashSet<String>> {
        Err(pending("gdscript_live_overrides"))
    }

    pub async fn get_edge_endpoint_ids(
        &self,
        node_ids: &[String],
        endpoint: EdgeEndpoint,
    ) -> Result<Vec<String>> {
        Err(pending("get_edge_endpoint_ids"))
    }

    pub async fn get_cross_file_edge_pairs(&self) -> Result<Vec<(String, String)>> {
        Err(pending("get_cross_file_edge_pairs"))
    }

    pub async fn get_cross_file_import_rows(&self) -> Result<Vec<ImportEdgeRow>> {
        Err(pending("get_cross_file_import_rows"))
    }

    pub async fn insert_decision(
        &self,
        text: &str,
        reason: Option<&str>,
        files: &[String],
        tags: &[String],
        created_at: i64,
    ) -> Result<i64> {
        Err(pending("insert_decision"))
    }

    pub async fn get_decisions(
        &self,
        query: Option<&str>,
        since: Option<i64>,
        limit: i64,
    ) -> Result<Vec<DecisionRecord>> {
        Err(pending("get_decisions"))
    }

    pub async fn upsert_code_area(
        &self,
        path: &str,
        description: Option<&str>,
        touched_at: i64,
    ) -> Result<()> {
        Err(pending("upsert_code_area"))
    }

    pub async fn get_code_areas(&self, limit: i64) -> Result<Vec<CodeAreaRecord>> {
        Err(pending("get_code_areas"))
    }

    pub async fn get_nodes_by_dir(&self, dir: &str, kinds: &[NodeKind]) -> Result<Vec<Node>> {
        Err(pending("get_nodes_by_dir"))
    }

    pub async fn get_internal_edges(&self, node_ids: &[String]) -> Result<Vec<Edge>> {
        Err(pending("get_internal_edges"))
    }

    pub async fn collect_test_marker_ids(&self) -> Result<Vec<String>> {
        Err(pending("collect_test_marker_ids"))
    }

    pub async fn get_children_of_many(&self, parent_ids: &[String]) -> Result<Vec<Node>> {
        Err(pending("get_children_of_many"))
    }

    pub async fn get_trait_methods_for_impl_method(
        &self,
        impl_id: &str,
        method_name: &str,
    ) -> Result<Vec<Node>> {
        Err(pending("get_trait_methods_for_impl_method"))
    }

    pub async fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        Err(pending("get_nodes_by_kind"))
    }

    pub async fn get_all_nodes(&self) -> Result<Vec<Node>> {
        Err(pending("get_all_nodes"))
    }

    pub async fn replace_ambiguous_calls(
        &self,
        files: &[String],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        Err(pending("replace_ambiguous_calls"))
    }

    pub async fn replace_ambiguous_calls_for_refs(
        &self,
        refs: &[AmbiguityRefKey],
        calls: &[AmbiguousCall],
    ) -> Result<()> {
        Err(pending("replace_ambiguous_calls_for_refs"))
    }

    pub async fn get_ambiguous_calls(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AmbiguousCall>> {
        Err(pending("get_ambiguous_calls"))
    }

    pub async fn ambiguous_candidate_ids(&self) -> Result<HashSet<String>> {
        Err(pending("ambiguous_candidate_ids"))
    }

    pub async fn get_ambiguous_calls_naming(&self, node_id: &str) -> Result<Vec<AmbiguousCall>> {
        Err(pending("get_ambiguous_calls_naming"))
    }

    pub async fn get_node_paths(&self) -> Result<HashMap<String, String>> {
        Err(pending("get_node_paths"))
    }

    pub async fn get_nodes_filtered(&self, filter: &NodeFilter) -> Result<Vec<Node>> {
        Err(pending("get_nodes_filtered"))
    }

    pub async fn get_all_nodes_for_resolution(&self) -> Result<Vec<Node>> {
        Err(pending("get_all_nodes_for_resolution"))
    }

    pub async fn touched_nodes_by_file(&self, file_path: &str) -> Result<Vec<TouchedNode>> {
        Err(pending("touched_nodes_by_file"))
    }

    pub async fn rails_route_status(&self) -> Result<Option<serde_json::Value>> {
        Err(pending("rails_route_status"))
    }

    pub(crate) async fn invalidate_rails_routes(&self) -> Result<()> {
        Err(pending("invalidate_rails_routes"))
    }

    pub async fn rebuild_rails_routes(&self) -> Result<()> {
        Err(pending("rebuild_rails_routes"))
    }

    pub(crate) async fn rails_route_rename_blocker(&self, target: &Node) -> Result<Option<String>> {
        Err(pending("rails_route_rename_blocker"))
    }

    pub async fn insert_executable_body_documents(
        &self,
        documents: &[ExecutableBodyDocument],
    ) -> Result<()> {
        Err(pending("insert_executable_body_documents"))
    }

    pub async fn search_executable_bodies(
        &self,
        terms: &[String],
        candidate_limit: usize,
    ) -> Result<Vec<(Node, usize)>> {
        Err(pending("search_executable_bodies"))
    }

    pub async fn search_nodes_phrase(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        Err(pending("search_nodes_phrase"))
    }

    pub async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        Err(pending("search_nodes"))
    }

    pub async fn search_nodes_bounded(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        Err(pending("search_nodes_bounded"))
    }

    pub(crate) async fn search_nodes_fts(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        Err(pending("search_nodes_fts"))
    }

    pub async fn batch_incoming_call_counts(
        &self,
        node_ids: &[String],
    ) -> Result<std::collections::HashMap<String, u64>> {
        Err(pending("batch_incoming_call_counts"))
    }

    pub async fn search_nodes_by_exact_name(
        &self,
        names: &[String],
        limit: usize,
    ) -> Result<Vec<Node>> {
        Err(pending("search_nodes_by_exact_name"))
    }

    pub async fn get_stats(&self) -> Result<GraphStats> {
        Err(pending("get_stats"))
    }

    pub(crate) async fn get_stats_with_languages(
        &self,
        language_override: impl Fn(&str) -> Option<String>,
    ) -> Result<GraphStats> {
        Err(pending("get_stats_with_languages"))
    }

    pub async fn last_index_time(&self) -> Result<i64> {
        Err(pending("last_index_time"))
    }

    pub async fn get_gdscript_typed_refs(&self) -> Result<Vec<UnresolvedRef>> {
        Err(pending("get_gdscript_typed_refs"))
    }

    pub async fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        Err(pending("insert_unresolved_ref"))
    }

    pub async fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        Err(pending("insert_unresolved_refs"))
    }

    pub async fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        Err(pending("get_unresolved_refs"))
    }

    pub async fn count_unresolved_refs(&self) -> Result<usize> {
        Err(pending("count_unresolved_refs"))
    }

    pub async fn get_unresolved_refs_after(
        &self,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, UnresolvedRef)>> {
        Err(pending("get_unresolved_refs_after"))
    }

    pub async fn get_unresolved_refs_for_sources(
        &self,
        source_ids: &[String],
    ) -> Result<Vec<UnresolvedRef>> {
        Err(pending("get_unresolved_refs_for_sources"))
    }

    pub async fn clear_unresolved_refs(&self) -> Result<()> {
        Err(pending("clear_unresolved_refs"))
    }
}
