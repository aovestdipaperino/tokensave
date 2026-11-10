//! Whole-graph analyses: rankings, annotations, dead-code candidates,
//! file coupling and the edge audit.
//!
//! Each method loads the nodes and edges it needs with one label scan and
//! evaluates the libSQL query's predicates, grouping and ordering in Rust.
//! Ties the SQL leaves unordered are broken by insertion order.
use std::collections::{BTreeSet, HashMap, HashSet};

use super::nodes::sort_by_location;
use super::{contains_ci, ends_with_ci, starts_with_ci, under_path, Database, SYMBOL};
use crate::db::model::{AuditEdgeRow, EdgeEndpoint, ImportEdgeRow};
use crate::errors::Result;
use crate::types::{Edge, EdgeKind, Node, NodeKind, Visibility};

/// Kinds that conventionally carry per-declaration documentation.
const DOC_COVERAGE_KINDS: &[&str] = &[
    "function",
    "method",
    "singleton_method",
    "class",
    "interface",
    "trait",
    "struct",
    "enum",
    "module",
    "field",
    "enum_variant",
    "const",
    "static",
    "type_alias",
    "property",
    "csharp_property",
    "record",
    "data_class",
    "sealed_class",
    "object",
    "case_class",
    "kotlin_object",
    "inner_class",
    "abstract_method",
    "constructor",
    "struct_method",
    "val",
    "var",
    "mixin",
    "extension",
    "union",
    "typedef",
];

/// Edge kinds that count as a real use site for dead-code analysis.
const USE_SITE_KINDS: &[EdgeKind] = &[
    EdgeKind::Calls,
    EdgeKind::Implements,
    EdgeKind::Extends,
    EdgeKind::TypeOf,
    EdgeKind::Returns,
    EdgeKind::Receives,
    EdgeKind::Uses,
];

/// Maximum `extends` hops followed from one class, as the libSQL query.
const MAX_INHERITANCE_DEPTH: u64 = 50;

impl Database {
    /// Calls `f(file_path, name)` for every `use` node.
    pub async fn for_each_use_node(&self, mut f: impl FnMut(String, String)) -> Result<()> {
        for node in self.all_symbols("edge_audit")? {
            if node.kind == NodeKind::Use {
                f(node.file_path, node.name);
            }
        }
        Ok(())
    }

    /// Calls `f(name, count)` for every name shared by symbols that are not
    /// `file` or `use` nodes.
    pub async fn for_each_symbol_name_count(&self, mut f: impl FnMut(String, i64)) -> Result<()> {
        let mut counts: HashMap<String, i64> = HashMap::new();
        for node in self.all_symbols("edge_audit")? {
            if !matches!(node.kind, NodeKind::File | NodeKind::Use) {
                *counts.entry(node.name).or_insert(0) += 1;
            }
        }
        for (name, count) in counts {
            f(name, count);
        }
        Ok(())
    }

    /// Calls `f(id, name)` for every node.
    pub async fn for_each_node_name(&self, mut f: impl FnMut(String, String)) -> Result<()> {
        for node in self.all_symbols("edge_audit")? {
            f(node.id, node.name);
        }
        Ok(())
    }

    /// Calls `f` for every edge whose source and target nodes both exist.
    pub async fn for_each_audit_edge(&self, mut f: impl FnMut(AuditEdgeRow)) -> Result<()> {
        let nodes = self.symbol_map("edge_audit")?;
        for edge in self.scan_edges(&[], "edge_audit")? {
            let (Some(src), Some(dst)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            f(AuditEdgeRow {
                src_file: src.file_path.clone(),
                dst_id: dst.id.clone(),
                dst_name: dst.name.clone(),
                dst_file: dst.file_path.clone(),
                dst_kind: dst.kind.as_str().to_string(),
                dst_line: i64::from(dst.start_line),
                dst_parent: dst.parent_id.clone(),
            });
        }
        Ok(())
    }

    /// Returns the subset of `candidate_ids` annotated with `#[test]`.
    pub async fn get_test_annotated_node_ids(
        &self,
        candidate_ids: &[String],
    ) -> Result<HashSet<String>> {
        let op = "get_test_annotated_node_ids";
        let mut out = HashSet::new();
        for id in candidate_ids {
            for edge in self.edges_of(
                id,
                grafeo::store::Direction::Incoming,
                &[EdgeKind::Annotates],
                op,
            )? {
                if let Some(source) = self.node_by_id(&edge.source, op)? {
                    if source.kind == NodeKind::AnnotationUsage && source.name == "test" {
                        out.insert(id.clone());
                        break;
                    }
                }
            }
        }
        Ok(out)
    }

    /// Histogram of annotation usages as `(name, count)`, by count
    /// descending then name.
    pub async fn get_annotation_histogram(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, u64)>> {
        let mut counts: HashMap<String, u64> = HashMap::new();
        for node in self.all_symbols("get_annotation_histogram")? {
            if node.kind == NodeKind::AnnotationUsage
                && path_prefix.is_none_or(|p| under_path(&node.file_path, p))
            {
                *counts.entry(node.name).or_insert(0) += 1;
            }
        }
        let mut out: Vec<(String, u64)> = counts
            .into_iter()
            .filter(|(name, _)| !name.is_empty())
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(out)
    }

    /// Sites where annotations attach to targets, as JSON rows.
    pub async fn get_annotation_sites(
        &self,
        name: Option<&str>,
        path_prefix: Option<&str>,
        target_kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        let op = "get_annotation_sites";
        let nodes = self.symbol_map(op)?;
        let mut sites: Vec<(&Node, &Node)> = Vec::new();
        for edge in self.scan_edges(&[EdgeKind::Annotates], op)? {
            let (Some(a), Some(t)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            if a.kind != NodeKind::AnnotationUsage
                || name.is_some_and(|n| a.name != n)
                || path_prefix.is_some_and(|p| !under_path(&t.file_path, p))
                || target_kind.is_some_and(|k| t.kind.as_str() != k)
            {
                continue;
            }
            sites.push((a, t));
        }
        sites.sort_by(|(a1, t1), (a2, t2)| {
            a1.name
                .cmp(&a2.name)
                .then(t1.file_path.cmp(&t2.file_path))
                .then(t1.start_line.cmp(&t2.start_line))
        });
        Ok(sites
            .into_iter()
            .take(limit)
            .map(|(a, t)| {
                serde_json::json!({
                    "annotation": a.name,
                    "annotation_file": a.file_path,
                    "annotation_line": a.start_line,
                    "target": {
                        "id": t.id,
                        "name": t.name,
                        "kind": t.kind.as_str(),
                        "file": t.file_path,
                        "line": t.start_line,
                        "qualified_name": t.qualified_name,
                    },
                })
            })
            .collect())
    }

    /// Returns the files holding a function or method annotated `#[test]`.
    pub async fn get_files_with_test_annotations(&self) -> Result<HashSet<String>> {
        let op = "get_files_with_test_annotations";
        let nodes = self.symbol_map(op)?;
        let mut out = HashSet::new();
        for edge in self.scan_edges(&[EdgeKind::Annotates], op)? {
            let (Some(a), Some(t)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            if a.kind == NodeKind::AnnotationUsage
                && a.name == "test"
                && matches!(
                    t.kind,
                    NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
                )
            {
                out.insert(t.file_path.clone());
            }
        }
        Ok(out)
    }

    /// Returns the ids of nodes whose docstring contains `skip-test-coverage`.
    pub async fn get_skip_test_coverage_node_ids(&self) -> Result<HashSet<String>> {
        Ok(self
            .all_symbols("get_skip_test_coverage_node_ids")?
            .into_iter()
            .filter(|n| {
                n.docstring
                    .as_deref()
                    .is_some_and(|d| contains_ci(d, "skip-test-coverage"))
            })
            .map(|n| n.id)
            .collect())
    }

    /// Returns nodes ranked by how many `edge_kind` edges enter (`incoming`)
    /// or leave them, by count descending then id.
    pub async fn get_ranked_nodes_by_edge_kind(
        &self,
        edge_kind: &EdgeKind,
        node_kind: Option<&NodeKind>,
        incoming: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        let op = "get_ranked_nodes_by_edge_kind";
        let nodes = self.symbol_map(op)?;
        let mut counts: HashMap<&str, u64> = HashMap::new();
        let edges = self.scan_edges(std::slice::from_ref(edge_kind), op)?;
        for edge in &edges {
            let id = if incoming { &edge.target } else { &edge.source };
            let Some(node) = nodes.get(id) else {
                continue;
            };
            if node_kind.is_some_and(|k| node.kind != *k)
                || path_prefix.is_some_and(|p| !under_path(&node.file_path, p))
            {
                continue;
            }
            *counts.entry(id.as_str()).or_insert(0) += 1;
        }
        let mut ranked: Vec<(&str, u64)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        Ok(ranked
            .into_iter()
            .take(limit)
            .filter_map(|(id, count)| nodes.get(id).map(|n| (n.clone(), count)))
            .collect())
    }

    /// Returns nodes ranked by line span, largest first.
    pub async fn get_largest_nodes(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32)>> {
        let mut sized: Vec<(Node, i64)> = self
            .all_symbols("get_largest_nodes")?
            .into_iter()
            .filter(|n| {
                node_kind.is_none_or(|k| n.kind == *k)
                    && path_prefix.is_none_or(|p| under_path(&n.file_path, p))
            })
            .map(|n| {
                let lines = line_span(&n);
                (n, lines)
            })
            .collect();
        sized.sort_by_key(|(_, lines)| std::cmp::Reverse(*lines));
        Ok(sized
            .into_iter()
            .take(limit)
            .map(|(n, lines)| (n, u32::try_from(lines).unwrap_or(0)))
            .collect())
    }

    /// Returns files ranked by how many distinct other files they depend on
    /// (`fan_in == false`) or are depended on by, through `calls`, `uses`,
    /// `implements` and `extends` edges.
    pub async fn get_file_coupling(
        &self,
        fan_in: bool,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, u64)>> {
        let op = "get_file_coupling";
        let nodes = self.symbol_map(op)?;
        let mut coupled: HashMap<&str, HashSet<&str>> = HashMap::new();
        let edges = self.scan_edges(
            &[
                EdgeKind::Calls,
                EdgeKind::Uses,
                EdgeKind::Implements,
                EdgeKind::Extends,
            ],
            op,
        )?;
        for edge in &edges {
            let (Some(src), Some(tgt)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            if src.file_path == tgt.file_path {
                continue;
            }
            let (group, other) = if fan_in { (tgt, src) } else { (src, tgt) };
            if path_prefix.is_some_and(|p| !under_path(&group.file_path, p)) {
                continue;
            }
            coupled
                .entry(group.file_path.as_str())
                .or_default()
                .insert(other.file_path.as_str());
        }
        let mut out: Vec<(String, u64)> = coupled
            .into_iter()
            .map(|(file, others)| (file.to_string(), others.len() as u64))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.truncate(limit);
        Ok(out)
    }

    /// Returns each class's deepest chain of `extends` hops, deepest first.
    /// Cycles are cut and chains stop at 50 hops, as in the libSQL query.
    pub async fn get_inheritance_depth(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64)>> {
        let op = "get_inheritance_depth";
        let nodes = self.symbol_map(op)?;
        let mut parents: HashMap<String, Vec<String>> = HashMap::new();
        for edge in self.scan_edges(&[EdgeKind::Extends], op)? {
            parents.entry(edge.source).or_default().push(edge.target);
        }
        let mut depths: Vec<(&str, u64)> = Vec::new();
        for leaf in parents.keys() {
            if let Some(prefix) = path_prefix {
                if !nodes
                    .get(leaf)
                    .is_some_and(|n| under_path(&n.file_path, prefix))
                {
                    continue;
                }
            }
            let depth = leaf_depth(&parents, leaf);
            if depth > 0 {
                depths.push((leaf.as_str(), depth));
            }
        }
        depths.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        Ok(depths
            .into_iter()
            .filter_map(|(id, depth)| nodes.get(id).map(|n| (n.clone(), depth)))
            .take(limit)
            .collect())
    }

    /// Returns node counts by `(file, kind)`, by file then count descending.
    pub async fn get_node_distribution(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, u64)>> {
        let mut counts: HashMap<(String, &'static str), u64> = HashMap::new();
        for node in self.all_symbols("get_node_distribution")? {
            if path_prefix.is_none_or(|p| under_path(&node.file_path, p)) {
                *counts
                    .entry((node.file_path, node.kind.as_str()))
                    .or_insert(0) += 1;
            }
        }
        let mut out: Vec<(String, String, u64)> = counts
            .into_iter()
            .map(|((file, kind), count)| (file, kind.to_string(), count))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)).then(a.1.cmp(&b.1)));
        Ok(out)
    }

    /// Returns every `calls` edge as `(source, target)`, from sources under
    /// `path_prefix` when given.
    pub async fn get_call_edges(&self, path_prefix: Option<&str>) -> Result<Vec<(String, String)>> {
        Ok(self
            .call_edges_from(path_prefix, "get_call_edges")?
            .into_iter()
            .map(|e| (e.source, e.target))
            .collect())
    }

    /// Returns every `calls` edge as `(source, target, line)`, from sources
    /// under `path_prefix` when given.
    pub async fn get_call_edges_with_lines(
        &self,
        path_prefix: Option<&str>,
    ) -> Result<Vec<(String, String, Option<u32>)>> {
        Ok(self
            .call_edges_from(path_prefix, "get_call_edges_with_lines")?
            .into_iter()
            .map(|e| (e.source, e.target, e.line))
            .collect())
    }

    /// Returns callables ranked by `lines + 3 * fan_out + fan_in`, as
    /// `(node, lines, fan_out, fan_in, score)`.
    pub async fn get_complexity_ranked(
        &self,
        node_kind: Option<&NodeKind>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u32, u64, u64, u64)>> {
        let op = "get_complexity_ranked";
        let mut fan_out: HashMap<String, u64> = HashMap::new();
        let mut fan_in: HashMap<String, u64> = HashMap::new();
        for edge in self.scan_edges(&[EdgeKind::Calls], op)? {
            *fan_out.entry(edge.source).or_insert(0) += 1;
            *fan_in.entry(edge.target).or_insert(0) += 1;
        }
        let mut scored: Vec<(Node, i64, u64, u64, i64)> = self
            .all_symbols(op)?
            .into_iter()
            .filter(|n| match node_kind {
                Some(k) => n.kind == *k,
                None => matches!(
                    n.kind,
                    NodeKind::Function | NodeKind::Method | NodeKind::SingletonMethod
                ),
            })
            .filter(|n| path_prefix.is_none_or(|p| under_path(&n.file_path, p)))
            .map(|n| {
                let lines = line_span(&n);
                let out = fan_out.get(&n.id).copied().unwrap_or(0);
                let inc = fan_in.get(&n.id).copied().unwrap_or(0);
                let score = lines
                    .saturating_add(i64::try_from(out.saturating_mul(3)).unwrap_or(i64::MAX))
                    .saturating_add(i64::try_from(inc).unwrap_or(i64::MAX));
                (n, lines, out, inc, score)
            })
            .collect();
        scored.sort_by(|a, b| b.4.cmp(&a.4).then(a.0.id.cmp(&b.0.id)));
        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(n, lines, out, inc, score)| {
                (
                    n,
                    u32::try_from(lines).unwrap_or(0),
                    out,
                    inc,
                    u64::try_from(score).unwrap_or(0),
                )
            })
            .collect())
    }

    /// Returns public symbols missing a docstring, by file and line.
    pub async fn get_undocumented_public_symbols(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Node>> {
        let mut out: Vec<Node> = self
            .all_symbols("get_undocumented_public_symbols")?
            .into_iter()
            .filter(|n| {
                n.visibility.as_str() == "public"
                    && n.docstring.as_deref().is_none_or(str::is_empty)
                    && DOC_COVERAGE_KINDS.contains(&n.kind.as_str())
                    && path_prefix.is_none_or(|p| under_path(&n.file_path, p))
            })
            .collect();
        sort_by_location(&mut out);
        out.truncate(limit);
        Ok(out)
    }

    /// Returns classes and structs ranked by member count, as
    /// `(node, methods, fields, total)`.
    pub async fn get_god_classes(
        &self,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Node, u64, u64, u64)>> {
        let nodes = self.all_symbols("get_god_classes")?;
        let mut members: HashMap<&str, (u64, u64, u64)> = HashMap::new();
        for child in &nodes {
            let Some(parent) = child.parent_id.as_deref() else {
                continue;
            };
            let entry = members.entry(parent).or_insert((0, 0, 0));
            if matches!(
                child.kind,
                NodeKind::Method
                    | NodeKind::SingletonMethod
                    | NodeKind::AbstractMethod
                    | NodeKind::Constructor
            ) {
                entry.0 += 1;
            }
            if child.kind == NodeKind::Field {
                entry.1 += 1;
            }
            entry.2 += 1;
        }
        let mut ranked: Vec<(Node, u64, u64, u64)> = nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.kind,
                    NodeKind::Class
                        | NodeKind::Struct
                        | NodeKind::InnerClass
                        | NodeKind::ScalaObject
                ) && path_prefix.is_none_or(|p| under_path(&n.file_path, p))
            })
            .filter_map(|n| {
                members
                    .get(n.id.as_str())
                    .map(|(methods, fields, total)| (n.clone(), *methods, *fields, *total))
            })
            .collect();
        ranked.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.id.cmp(&b.0.id)));
        ranked.truncate(limit);
        Ok(ranked)
    }

    /// Rust build-variant candidates: `(group key, node id)` for callables
    /// in `.rs` files annotated with `cfg` or `cfg_attr` (#481).
    pub async fn variant_group_candidates(&self) -> Result<Vec<(String, String)>> {
        let op = "variant_group_candidates";
        let nodes = self.symbol_map(op)?;
        let mut out = Vec::new();
        for edge in self.scan_edges(&[EdgeKind::Annotates], op)? {
            let (Some(a), Some(n)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            if a.kind == NodeKind::AnnotationUsage
                && (a.name == "cfg" || a.name == "cfg_attr")
                && ends_with_ci(&n.file_path, ".rs")
                && crate::resolution::CALLABLE_KIND_NAMES.contains(&n.kind.as_str())
            {
                out.push((format!("rs\u{1}{}", n.qualified_name), n.id.clone()));
            }
        }
        Ok(out)
    }

    /// Go package-level functions whose name repeats somewhere in the graph:
    /// `(file_path, name, node id)` (#481).
    pub async fn go_variant_candidates(&self) -> Result<Vec<(String, String, String)>> {
        let functions: Vec<Node> = self
            .all_symbols("go_variant_candidates")?
            .into_iter()
            .filter(|n| n.kind == NodeKind::Function && ends_with_ci(&n.file_path, ".go"))
            .collect();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for node in &functions {
            *counts.entry(node.name.as_str()).or_insert(0) += 1;
        }
        Ok(functions
            .iter()
            .filter(|n| counts.get(n.name.as_str()).is_some_and(|c| *c >= 2))
            .map(|n| (n.file_path.clone(), n.name.clone(), n.id.clone()))
            .collect())
    }

    /// Returns `(node_id, in_degree, out_degree)` for the `limit` nodes with
    /// the highest combined degree, ignoring self-edges; ties by id.
    pub async fn get_top_degree_nodes(&self, limit: usize) -> Result<Vec<(String, u32, u32)>> {
        let mut degrees: HashMap<String, (u32, u32)> = HashMap::new();
        for edge in self.scan_edges(&[], "get_top_degree_nodes")? {
            if edge.source == edge.target {
                continue;
            }
            let target = degrees.entry(edge.target).or_insert((0, 0));
            target.0 = target.0.saturating_add(1);
            let source = degrees.entry(edge.source).or_insert((0, 0));
            source.1 = source.1.saturating_add(1);
        }
        let mut out: Vec<(String, u32, u32)> = degrees
            .into_iter()
            .map(|(id, (inc, out))| (id, inc, out))
            .collect();
        out.sort_by(|a, b| {
            (u64::from(b.1) + u64::from(b.2))
                .cmp(&(u64::from(a.1) + u64::from(a.2)))
                .then(a.0.cmp(&b.0))
        });
        out.truncate(limit);
        Ok(out)
    }

    /// Returns nodes with no incoming use-site edge, the raw candidates for
    /// dead-code analysis, with the libSQL query's exemptions (see the
    /// libSQL backend for why each exists).
    pub async fn find_dead_code_candidates(
        &self,
        kinds: &[NodeKind],
        include_public: bool,
        include_trait_impls: bool,
        godot_virtuals: &[&str],
    ) -> Result<Vec<Node>> {
        let op = "find_dead_code";
        // One narrow scan supplies both the candidates and the test markers;
        // full nodes are read only for the candidates that survive.
        let nodes = self.brief_symbols(op)?;
        let markers: HashSet<&str> = nodes
            .iter()
            .filter(|n| is_test_marker(*n))
            .map(|n| n.id.as_str())
            .collect();
        let mut used: HashSet<String> = HashSet::new();
        let mut test_annotated: HashSet<String> = HashSet::new();
        let mut implementing: HashSet<String> = HashSet::new();
        for edge in self.scan_edges(&[], op)? {
            if USE_SITE_KINDS.contains(&edge.kind) {
                if edge.kind == EdgeKind::Implements {
                    implementing.insert(edge.source.clone());
                }
                used.insert(edge.target);
            } else if edge.kind == EdgeKind::Annotates && markers.contains(edge.source.as_str()) {
                test_annotated.insert(edge.target);
            }
        }
        let mut trait_impls: HashSet<&str> = HashSet::new();
        if !include_trait_impls {
            for p in nodes.iter().filter(|p| p.kind == NodeKind::Impl) {
                let is_trait_impl = implementing.contains(&p.id)
                    || self
                        .node_by_id(&p.id, op)?
                        .and_then(|full| full.signature)
                        .is_some_and(|s| contains_ci(&s, " for "));
                if is_trait_impl {
                    trait_impls.insert(p.id.as_str());
                }
            }
        }
        let survivors: Vec<&BriefNode> = nodes
            .iter()
            .filter(|n| {
                n.kind != NodeKind::Route
                    && n.name != "main"
                    && n.name != "_bind_methods"
                    && !starts_with_ci(&n.name, "test")
                    && !(n.name == "init" && ends_with_ci(&n.file_path, ".go"))
                    && !(ends_with_ci(&n.file_path, ".gd")
                        && godot_virtuals.contains(&n.name.as_str()))
                    && (include_public || n.visibility != Visibility::Pub)
                    && (kinds.is_empty() || kinds.contains(&n.kind))
                    && n.parent_id
                        .as_deref()
                        .is_none_or(|p| !trait_impls.contains(p))
                    && !used.contains(&n.id)
                    && !test_annotated.contains(&n.id)
            })
            .collect();
        let mut out = Vec::with_capacity(survivors.len());
        for brief in survivors {
            if let Some(node) = self.node_by_id(&brief.id, op)? {
                out.push(Node {
                    // The libSQL query selects neither the parent nor the
                    // health columns; dead-code analysis reads neither.
                    parent_id: None,
                    cognitive_complexity: 0,
                    distinct_operators: 0,
                    distinct_operands: 0,
                    total_operators: 0,
                    total_operands: 0,
                    ..node
                });
            }
        }
        Ok(out)
    }

    /// The fields of every node that dead-code filtering reads, without the
    /// rest of each node.
    fn brief_symbols(&self, operation: &str) -> Result<Vec<BriefNode>> {
        let rows = self.query_rows(
            &format!(
                "MATCH (n:{SYMBOL}) \
                 RETURN n.id, n.kind, n.name, n.file_path, n.visibility, n.parent_id"
            ),
            operation,
        )?;
        let text = |v: &grafeo::Value| v.as_str().unwrap_or_default().to_string();
        Ok(rows
            .iter()
            .filter_map(|row| {
                let [id, kind, name, file_path, visibility, parent_id] = row.as_slice() else {
                    return None;
                };
                Some(BriefNode {
                    id: text(id),
                    kind: NodeKind::from_str(&text(kind)).unwrap_or(NodeKind::Function),
                    name: text(name),
                    file_path: text(file_path),
                    visibility: Visibility::from_str(&text(visibility)).unwrap_or_default(),
                    parent_id: parent_id.as_str().map(str::to_string),
                })
            })
            .collect())
    }

    /// `GDScript` methods that override a base-class method which is called
    /// or used, following `extends` transitively (#598).
    pub async fn gdscript_live_overrides(&self) -> Result<HashSet<String>> {
        let op = "gdscript_live_overrides";
        let nodes = self.all_symbols(op)?;
        let mut parents: HashMap<&str, Vec<String>> = HashMap::new();
        let extends = self.scan_edges(&[EdgeKind::Extends], op)?;
        for edge in &extends {
            parents
                .entry(edge.source.as_str())
                .or_default()
                .push(edge.target.clone());
        }
        let mut children: HashMap<&str, Vec<&Node>> = HashMap::new();
        for node in &nodes {
            if let Some(parent) = node.parent_id.as_deref() {
                if matches!(node.kind, NodeKind::Function | NodeKind::Method) {
                    children.entry(parent).or_default().push(node);
                }
            }
        }
        let mut live = HashSet::new();
        for class in nodes.iter().filter(|n| {
            ends_with_ci(&n.file_path, ".gd")
                && matches!(n.kind, NodeKind::Class | NodeKind::InnerClass)
        }) {
            let mut bases: BTreeSet<&str> = BTreeSet::new();
            let mut frontier: Vec<&str> = vec![class.id.as_str()];
            while let Some(current) = frontier.pop() {
                for base in parents.get(current).into_iter().flatten() {
                    if bases.insert(base.as_str()) {
                        frontier.push(base.as_str());
                    }
                }
            }
            for method in children.get(class.id.as_str()).into_iter().flatten() {
                for base in &bases {
                    for overridden in children.get(base).into_iter().flatten() {
                        if overridden.name == method.name
                            && !self
                                .edges_of(
                                    &overridden.id,
                                    grafeo::store::Direction::Incoming,
                                    &[EdgeKind::Calls, EdgeKind::Uses],
                                    op,
                                )?
                                .is_empty()
                        {
                            live.insert(method.id.clone());
                        }
                    }
                }
            }
        }
        Ok(live)
    }

    /// Returns the distinct far endpoints of `calls`/`uses` edges touching
    /// `node_ids`: their targets or their sources.
    pub async fn get_edge_endpoint_ids(
        &self,
        node_ids: &[String],
        endpoint: EdgeEndpoint,
    ) -> Result<Vec<String>> {
        let (direction, operation) = match endpoint {
            EdgeEndpoint::Targets => (grafeo::store::Direction::Outgoing, "get_file_dependencies"),
            EdgeEndpoint::Sources => (grafeo::store::Direction::Incoming, "get_file_dependents"),
        };
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for id in node_ids {
            for edge in
                self.edges_of(id, direction, &[EdgeKind::Uses, EdgeKind::Calls], operation)?
            {
                let far = match endpoint {
                    EdgeEndpoint::Targets => edge.target,
                    EdgeEndpoint::Sources => edge.source,
                };
                if seen.insert(far.clone()) {
                    out.push(far);
                }
            }
        }
        Ok(out)
    }

    /// Returns the distinct `(source file, target file)` pairs joined by a
    /// `calls` or `uses` edge between two different files.
    pub async fn get_cross_file_edge_pairs(&self) -> Result<Vec<(String, String)>> {
        let op = "build_file_adjacency";
        let nodes = self.symbol_map(op)?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for edge in self.scan_edges(&[EdgeKind::Calls, EdgeKind::Uses], op)? {
            let (Some(src), Some(tgt)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            if src.file_path != tgt.file_path {
                let pair = (src.file_path.clone(), tgt.file_path.clone());
                if seen.insert(pair.clone()) {
                    out.push(pair);
                }
            }
        }
        Ok(out)
    }

    /// Returns every resolved `uses` edge from a `use` node to a node in a
    /// different file, for the module import graph (#334).
    pub async fn get_cross_file_import_rows(&self) -> Result<Vec<ImportEdgeRow>> {
        let op = "build_module_import_graph";
        let nodes = self.symbol_map(op)?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for edge in self.scan_edges(&[EdgeKind::Uses], op)? {
            let (Some(n1), Some(n2)) = (nodes.get(&edge.source), nodes.get(&edge.target)) else {
                continue;
            };
            if n1.kind != NodeKind::Use || n1.file_path == n2.file_path {
                continue;
            }
            let parent_kind = n1
                .parent_id
                .as_deref()
                .and_then(|p| nodes.get(p))
                .map_or("file", |p| p.kind.as_str());
            let key = (
                n1.file_path.clone(),
                n1.start_line,
                n1.name.clone(),
                n1.signature.clone(),
                n2.file_path.clone(),
                parent_kind,
            );
            if !seen.insert(key) {
                continue;
            }
            out.push(ImportEdgeRow {
                file: n1.file_path.clone(),
                line: n1.start_line,
                imported: n1.name.clone(),
                statement: n1.signature.clone(),
                resolved_file: n2.file_path.clone(),
                parent_kind: parent_kind.to_string(),
            });
        }
        Ok(out)
    }

    /// Ids of `annotation_usage` nodes whose name marks a test (`test`,
    /// `tokio::test`, `wasm_bindgen_test`, …).
    pub async fn collect_test_marker_ids(&self) -> Result<Vec<String>> {
        Ok(self
            .symbols_where(
                "kind",
                NodeKind::AnnotationUsage.as_str(),
                "collect_test_marker_ids",
            )?
            .into_iter()
            .filter(is_test_marker)
            .map(|n| n.id)
            .collect())
    }

    /// Every node by id.
    fn symbol_map(&self, operation: &str) -> Result<HashMap<String, Node>> {
        Ok(self
            .all_symbols(operation)?
            .into_iter()
            .map(|n| (n.id.clone(), n))
            .collect())
    }

    /// `calls` edges whose source lies under `path_prefix`; all of them
    /// without a prefix.
    fn call_edges_from(&self, path_prefix: Option<&str>, operation: &str) -> Result<Vec<Edge>> {
        let edges = self.scan_edges(&[EdgeKind::Calls], operation)?;
        let Some(prefix) = path_prefix else {
            return Ok(edges);
        };
        let files = self.get_node_paths_sync(operation)?;
        Ok(edges
            .into_iter()
            .filter(|e| files.get(&e.source).is_some_and(|f| under_path(f, prefix)))
            .collect())
    }

    /// Every node's id paired with its file path.
    fn get_node_paths_sync(&self, operation: &str) -> Result<HashMap<String, String>> {
        Ok(self
            .scan_label(SYMBOL, operation)?
            .iter()
            .map(|p| {
                (
                    super::get_str(p.as_ref(), "id"),
                    super::get_str(p.as_ref(), "file_path"),
                )
            })
            .collect())
    }
}

/// The node fields dead-code filtering reads.
struct BriefNode {
    id: String,
    kind: NodeKind,
    name: String,
    file_path: String,
    visibility: Visibility,
    parent_id: Option<String>,
}

/// The fields [`is_test_marker`] reads.
trait MarkerFields {
    fn kind(&self) -> &NodeKind;
    fn name(&self) -> &str;
}

impl MarkerFields for Node {
    fn kind(&self) -> &NodeKind {
        &self.kind
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl MarkerFields for BriefNode {
    fn kind(&self) -> &NodeKind {
        &self.kind
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// Whether `node` is an annotation that marks a test (`test`,
/// `tokio::test`, `wasm_bindgen_test`, …).
fn is_test_marker(node: &impl MarkerFields) -> bool {
    let (kind, name) = (node.kind(), node.name());
    *kind == NodeKind::AnnotationUsage
        && (name == "test"
            || ends_with_ci(name, "::test")
            || name == "wasm_bindgen_test"
            || ends_with_ci(name, "::wasm_bindgen_test"))
}

/// `end_line - start_line + 1`, signed as SQL integer arithmetic is.
fn line_span(node: &Node) -> i64 {
    i64::from(node.end_line) - i64::from(node.start_line) + 1
}

/// The libSQL recursive query's depth for `leaf`: every `extends` edge of
/// the leaf is one hop, even to itself, and later hops skip classes already
/// on the path, up to [`MAX_INHERITANCE_DEPTH`].
fn leaf_depth(parents: &HashMap<String, Vec<String>>, leaf: &str) -> u64 {
    let mut deepest = 0;
    for base in parents.get(leaf).into_iter().flatten() {
        let mut path = vec![leaf.to_string(), base.clone()];
        deepest = deepest.max(deepest_chain(parents, &mut path, 1));
    }
    deepest
}

/// Hops in the longest cycle-free `extends` chain from the last class on
/// `path`, which is already `depth` hops deep.
fn deepest_chain(
    parents: &HashMap<String, Vec<String>>,
    path: &mut Vec<String>,
    depth: u64,
) -> u64 {
    if depth >= MAX_INHERITANCE_DEPTH {
        return depth;
    }
    let Some(current) = path.last().cloned() else {
        return depth;
    };
    let mut deepest = depth;
    for base in parents.get(&current).into_iter().flatten() {
        if path.contains(base) {
            continue;
        }
        path.push(base.clone());
        deepest = deepest.max(deepest_chain(parents, path, depth + 1));
        path.pop();
    }
    deepest
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::leaf_depth;

    #[test]
    fn leaf_depth_follows_the_longest_path_and_cuts_cycles() {
        let mut parents: HashMap<String, Vec<String>> = HashMap::new();
        parents.insert("a".into(), vec!["b".into(), "c".into()]);
        parents.insert("b".into(), vec!["d".into()]);
        parents.insert("d".into(), vec!["a".into()]);
        assert_eq!(leaf_depth(&parents, "a"), 2);
        assert_eq!(leaf_depth(&parents, "d"), 2);
    }

    #[test]
    fn leaf_depth_counts_a_self_extends_edge_as_one_hop() {
        let parents = HashMap::from([("a".to_string(), vec!["a".to_string()])]);
        assert_eq!(leaf_depth(&parents, "a"), 1);
    }
}
