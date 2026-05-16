//! Status, files, `type_hierarchy`, body, todos, `simplify_scan`, `port_status`,
//! `port_order` tool handlers.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::{NodeKind, Visibility};

use super::super::ToolResult;
use super::{effective_path, require_node_id, truncate_response, unique_file_paths};

/// Handles `tokensave_status` tool calls.
pub(super) async fn handle_status(
    cg: &TokenSave,
    server_stats: Option<Value>,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let stats = cg.get_stats().await?;
    let mut output: Value = serde_json::to_value(&stats).unwrap_or(json!({}));
    if let Some(ss) = server_stats {
        output["server"] = ss;
    }

    // Branch info
    if let Some(branch) = cg.active_branch() {
        output["active_branch"] = json!(branch);
        let ts_dir = crate::config::get_tokensave_dir(cg.project_root());
        if let Some(meta) = crate::branch_meta::load_branch_meta(&ts_dir) {
            if let Some(entry) = meta.branches.get(branch) {
                if let Some(ref parent) = entry.parent {
                    output["parent_branch"] = json!(parent);
                }
            }
        }
    }
    if cg.is_fallback() {
        output["branch_fallback"] = json!(true);
        if let Some(warning) = cg.fallback_warning() {
            output["branch_warning"] = json!(warning);
        }
    }

    // Git commit staleness: count commits since last index
    let stale_commit_count = cg.git_commits_since(stats.last_updated as i64);
    if stale_commit_count > 0 {
        output["stale_commits"] = json!(stale_commit_count);
        output["stale_warning"] = json!(format!(
            "{} commit(s) since last sync. Run `tokensave sync` to update the index.",
            stale_commit_count
        ));
    }

    // File-level staleness summary (sample up to 100 files for efficiency)
    let all_files = cg.get_all_files().await.unwrap_or_default();
    let sample_paths: Vec<String> = all_files.iter().take(100).map(|f| f.path.clone()).collect();
    let stale_files = cg.check_file_staleness(&sample_paths).await;
    if !stale_files.is_empty() {
        output["stale_files"] = json!(stale_files.len());
    }

    if let Some(prefix) = scope_prefix {
        output["scope_prefix"] = json!(prefix);
    }

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_files` tool calls.
pub(super) async fn handle_files(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_files expects an object argument");
    let mut files = cg.get_all_files().await?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    // Apply directory prefix filter
    if let Some(dir) = effective_path(&args, scope_prefix) {
        let prefix = if dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{dir}/")
        };
        files.retain(|f| f.path.starts_with(&prefix) || f.path == dir);
    }

    // Apply glob pattern filter
    if let Some(pat) = args.get("pattern").and_then(|v| v.as_str()) {
        if let Ok(glob) = glob::Pattern::new(pat) {
            files.retain(|f| glob.matches(&f.path));
        }
    }

    // Listing files is metadata-only — no source code is served, so no tokens saved.
    let touched_files = vec![];

    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("grouped");

    let output = if format == "flat" {
        files
            .iter()
            .map(|f| format!("{} ({} symbols, {} bytes)", f.path, f.node_count, f.size))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Grouped by directory
        let mut groups: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for f in &files {
            let dir = f.path.rfind('/').map_or(".", |i| &f.path[..i]).to_string();
            #[allow(clippy::map_unwrap_or)]
            let name = f
                .path
                .rfind('/')
                .map(|i| &f.path[i + 1..])
                .unwrap_or(&f.path);
            groups
                .entry(dir)
                .or_default()
                .push(format!("{} ({} symbols)", name, f.node_count));
        }
        let mut lines = Vec::new();
        lines.push(format!("{} indexed files", files.len()));
        for (dir, entries) in &groups {
            lines.push(format!("\n{}/ ({} files)", dir, entries.len()));
            for entry in entries {
                lines.push(format!("  {entry}"));
            }
        }
        lines.join("\n")
    };

    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Default node kinds for port comparisons.
const PORT_DEFAULT_KINDS: &[&str] = &[
    "function",
    "method",
    "class",
    "struct",
    "interface",
    "trait",
    "enum",
    "module",
];

/// Returns the compatibility group for a node kind string used in port matching.
///
/// Kinds in the same group are considered cross-language equivalents:
/// - group 0: class, struct (cross-language data type)
/// - group 1: function
/// - group 2: method
/// - group 3: interface, trait
/// - group 4: enum
/// - group 5: module
fn kind_compat_group(kind: &str) -> u8 {
    match kind {
        "class" | "struct" => 0,
        "function" => 1,
        "method" => 2,
        "interface" | "trait" => 3,
        "enum" => 4,
        "module" => 5,
        _ => 255,
    }
}

/// Composite match key used by `handle_port_status`.
///
/// Combines the lowercased name, an optional parent qualifier (for methods,
/// fields, and variants), and a kind compatibility group, so siblings whose
/// names happen to collide (`Biquad::new` vs `Adaa::new`) do not cross-match.
type PortKey = (String, Option<String>, u8);

/// Returns true for kinds that conceptually have a parent type/owner whose
/// identity matters for matching (methods, fields, variants, etc.). Top-level
/// items (struct, function, …) return false — their parent in `qualified_name`
/// is just the file path and is not useful for cross-port matching.
fn port_kind_has_parent(kind: &str) -> bool {
    matches!(
        kind,
        "method"
            | "field"
            | "enum_variant"
            | "struct_method"
            | "abstract_method"
            | "constructor"
            | "csharp_property"
            | "property"
            | "val"
            | "var"
    )
}

/// Extracts the parent qualifier from a node's `qualified_name`, stripping
/// generic parameters so `Biquad<T>::new` and `Biquad::new` share the same
/// parent. Returns `None` for kinds where the parent qualifier is not the
/// containing type (e.g. top-level structs whose parent is the file path).
fn port_parent_qualifier(node: &crate::types::Node) -> Option<String> {
    if !port_kind_has_parent(node.kind.as_str()) {
        return None;
    }
    let parts: Vec<&str> = node.qualified_name.split("::").collect();
    if parts.len() < 2 {
        return None;
    }
    let parent = parts[parts.len() - 2];
    // Strip generic parameters: `Biquad<T>` -> `Biquad`.
    let parent_no_generics = parent.split('<').next().unwrap_or(parent);
    Some(parent_no_generics.trim().to_string())
}

/// Handles `tokensave_port_status` tool calls.
pub(super) async fn handle_port_status(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(
        args.is_object(),
        "handle_port_status expects an object argument"
    );

    let source_dir = args
        .get("source_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: source_dir".to_string(),
        })?;

    let target_dir = args
        .get("target_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: target_dir".to_string(),
        })?;

    let kind_strs: Vec<String> = args.get("kinds").and_then(|v| v.as_array()).map_or_else(
        || {
            PORT_DEFAULT_KINDS
                .iter()
                .map(std::string::ToString::to_string)
                .collect()
        },
        |arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect()
        },
    );

    let kinds: Vec<NodeKind> = kind_strs
        .iter()
        .filter_map(|s| NodeKind::from_str(s))
        .collect();

    if kinds.is_empty() {
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": "No valid node kinds specified." }]
            }),
            touched_files: vec![],
        });
    }

    let source_nodes = cg.get_nodes_by_dir(source_dir, &kinds).await?;
    let target_nodes = cg.get_nodes_by_dir(target_dir, &kinds).await?;

    // Match key includes the parent qualifier (e.g. enclosing struct/class) for
    // kinds that have one, so `Biquad::new` does NOT collide with `Adaa::new`.
    // Top-level kinds (struct, function, …) keep using name-only matching.
    let mut target_map: HashMap<PortKey, Vec<&crate::types::Node>> = HashMap::new();
    for node in &target_nodes {
        let key: PortKey = (
            node.name.to_lowercase(),
            port_parent_qualifier(node).map(|s| s.to_lowercase()),
            kind_compat_group(node.kind.as_str()),
        );
        target_map.entry(key).or_default().push(node);
    }

    let mut matched_symbols: Vec<Value> = Vec::new();
    let mut matched_target_ids: HashSet<String> = HashSet::new();
    let mut unmatched_by_file: HashMap<String, Vec<Value>> = HashMap::new();

    for src_node in &source_nodes {
        let key: PortKey = (
            src_node.name.to_lowercase(),
            port_parent_qualifier(src_node).map(|s| s.to_lowercase()),
            kind_compat_group(src_node.kind.as_str()),
        );
        if let Some(targets) = target_map.get(&key) {
            // Take the first match
            let tgt = targets[0];
            matched_symbols.push(json!({
                "name": src_node.name,
                "source_kind": src_node.kind.as_str(),
                "target_kind": tgt.kind.as_str(),
                "source_file": src_node.file_path,
                "target_file": tgt.file_path,
            }));
            matched_target_ids.insert(tgt.id.clone());
        } else {
            unmatched_by_file
                .entry(src_node.file_path.clone())
                .or_default()
                .push(json!({
                    "name": src_node.name,
                    "kind": src_node.kind.as_str(),
                    "line": src_node.start_line,
                }));
        }
    }

    // Target-only symbols (in target but no source match)
    let target_only: Vec<Value> = target_nodes
        .iter()
        .filter(|n| !matched_target_ids.contains(&n.id))
        .map(|n| {
            json!({
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
            })
        })
        .collect();

    let source_count = source_nodes.len();
    let matched_count = matched_symbols.len();
    let unmatched_count = source_count - matched_count;
    let coverage = if source_count > 0 {
        (matched_count as f64 / source_count as f64) * 100.0
    } else {
        0.0
    };

    let touched_files = unique_file_paths(
        source_nodes
            .iter()
            .chain(target_nodes.iter())
            .map(|n| n.file_path.as_str()),
    );

    let result = json!({
        "source_dir": source_dir,
        "target_dir": target_dir,
        "source_count": source_count,
        "target_count": target_nodes.len(),
        "matched": matched_count,
        "unmatched": unmatched_count,
        "target_only": target_only.len(),
        "coverage_percent": (coverage * 10.0).round() / 10.0,
        "unmatched_by_file": unmatched_by_file,
        "matched_symbols": matched_symbols,
        "target_only_symbols": target_only,
    });

    let formatted = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_port_order` tool calls.
pub(super) async fn handle_port_order(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(
        args.is_object(),
        "handle_port_order expects an object argument"
    );

    let source_dir = args
        .get("source_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: source_dir".to_string(),
        })?;

    let kind_strs: Vec<String> = args.get("kinds").and_then(|v| v.as_array()).map_or_else(
        || {
            PORT_DEFAULT_KINDS
                .iter()
                .map(std::string::ToString::to_string)
                .collect()
        },
        |arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect()
        },
    );

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(50, |v| v.min(500) as usize);

    let kinds: Vec<NodeKind> = kind_strs
        .iter()
        .filter_map(|s| NodeKind::from_str(s))
        .collect();

    if kinds.is_empty() {
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": "No valid node kinds specified." }]
            }),
            touched_files: vec![],
        });
    }

    let nodes = cg.get_nodes_by_dir(source_dir, &kinds).await?;
    let total_symbols = nodes.len();

    if nodes.is_empty() {
        let result = json!({
            "source_dir": source_dir,
            "total_symbols": 0,
            "returned": 0,
            "levels": [],
            "cycles": [],
        });
        let formatted = serde_json::to_string_pretty(&result).unwrap_or_default();
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": formatted }]
            }),
            touched_files: vec![],
        });
    }

    // Build node ID lookup
    let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let node_map: HashMap<&str, &crate::types::Node> =
        nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let id_set: HashSet<&str> = node_ids.iter().map(std::string::String::as_str).collect();

    // Get internal edges (dependency edges between these nodes)
    let edges = cg.get_internal_edges(&node_ids).await?;

    // Build adjacency list and in-degree map for Kahn's algorithm.
    // Edge direction: source depends on target (source calls/uses target),
    // so in the dependency graph, source -> target means "source needs target".
    // For topological sort, we want nodes with in_degree 0 (nothing depends on
    // them internally, OR they have no dependencies). Actually, for porting
    // order we want leaves first = nodes that DON'T depend on other internal
    // nodes. So in-degree in the dependency DAG = number of things this node
    // depends on = outgoing edges in the call/uses graph.
    //
    // Reframe: dependency_graph[A] = {B, C} means A depends on B and C.
    // in_degree[A] = number of nodes A depends on.
    // Kahn's starts with in_degree 0 = nodes with no dependencies = safe to port first.
    let dep_edge_kinds: HashSet<&str> = ["calls", "uses", "extends", "implements"]
        .iter()
        .copied()
        .collect();

    let mut dep_graph: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();

    // Initialize all nodes
    for id in &node_ids {
        dep_graph.entry(id.as_str()).or_default();
        in_degree.entry(id.as_str()).or_insert(0);
    }

    // reverse_dep_graph[B] = list of nodes that depend on B.
    // When B is sorted, we decrement in_degree for each of its reverse deps.
    let mut reverse_dep_graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for id in &node_ids {
        reverse_dep_graph.entry(id.as_str()).or_default();
    }

    for edge in &edges {
        if !dep_edge_kinds.contains(edge.kind.as_str()) {
            continue;
        }
        if !id_set.contains(edge.source.as_str()) || !id_set.contains(edge.target.as_str()) {
            continue;
        }
        // Self-edges are common resolver artifacts for methods with generic
        // names (`push`, `new`, `clamp`, `num_rows`) where a call on another
        // receiver fuzzy-binds back to the current method. They also make a
        // single symbol unsortable in Kahn's algorithm, producing noisy
        // singleton cycles instead of useful porting order. Mutual cycles are
        // still reported below.
        if edge.source == edge.target {
            continue;
        }
        // source depends on target: add dependency source -> target
        dep_graph
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
        // reverse: target is depended on by source
        reverse_dep_graph
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
        *in_degree.entry(edge.source.as_str()).or_insert(0) += 1;
    }

    // Kahn's algorithm (BFS topological sort)
    let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::new();
    for (&id, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(id);
        }
    }

    let mut levels: Vec<Vec<&str>> = Vec::new();
    let mut sorted_set: HashSet<&str> = HashSet::new();
    let mut emitted = 0usize;

    while !queue.is_empty() && emitted < limit {
        let mut current_level: Vec<&str> = Vec::new();
        let level_size = queue.len();
        for _ in 0..level_size {
            // Safety: we checked queue is non-empty above and iterate exactly level_size times
            let Some(id) = queue.pop_front() else { break };
            if sorted_set.contains(id) {
                continue;
            }
            sorted_set.insert(id);
            current_level.push(id);
            emitted += 1;
            if emitted >= limit {
                break;
            }
        }

        // For each sorted node, decrement in-degree of nodes that depend on it.
        for &sorted_id in &current_level {
            if let Some(dependents) = reverse_dep_graph.get(sorted_id) {
                for &dep_id in dependents {
                    if sorted_set.contains(dep_id) {
                        continue;
                    }
                    let deg = in_degree.entry(dep_id).or_insert(0);
                    if *deg > 0 {
                        *deg -= 1;
                    }
                    if *deg == 0 {
                        queue.push_back(dep_id);
                    }
                }
            }
        }

        if !current_level.is_empty() {
            levels.push(current_level);
        }
    }

    // Detect cycles: any unsorted nodes form cycles.
    let cycle_node_ids: HashSet<&str> = node_ids
        .iter()
        .map(std::string::String::as_str)
        .filter(|id| !sorted_set.contains(id))
        .collect();

    // Group cycles into SCCs so multiple disjoint mutually-recursive
    // groups don't collapse into one mega-cycle. Each non-trivial SCC
    // becomes its own entry with the files forming it surfaced — gives
    // the user a clear "break this cycle" target instead of a 200+
    // symbol blob.
    let mut cycle_adj: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (&node_id, neighbors) in &dep_graph {
        if !cycle_node_ids.contains(node_id) {
            continue;
        }
        let kept: HashSet<&str> = neighbors
            .iter()
            .copied()
            .filter(|n| cycle_node_ids.contains(n))
            .collect();
        cycle_adj.insert(node_id, kept);
    }
    let sccs = crate::graph::scc::tarjan_scc(&cycle_adj);

    let mut cycles_json: Vec<Value> = Vec::new();
    for scc in sccs {
        if !crate::graph::scc::is_cyclic_scc(&scc, &cycle_adj) {
            continue;
        }
        let scc_set: HashSet<&str> = scc.iter().copied().collect();
        // Rank symbols within the SCC by in-cycle out-degree (how many
        // *other* SCC members this symbol depends on). The symbol with the
        // smallest out-degree is the leaf-most node inside the cycle and is
        // the natural starting point: porting it requires stubbing the
        // fewest peers. The symbol with the largest out-degree is the
        // "hub" — the best candidate to break the cycle by refactoring its
        // call sites.
        let mut ranked: Vec<(&str, usize, usize)> = scc
            .iter()
            .map(|id| {
                let out_in_cycle = cycle_adj.get(id).map_or(0, |neighbors| {
                    neighbors.iter().filter(|n| scc_set.contains(*n)).count()
                });
                // In-degree (within the cycle) — how many SCC members
                // depend on this symbol. High in-degree = "many callers
                // inside the cycle", which is another useful break-point
                // signal.
                let mut in_in_cycle = 0;
                for (&src, neighbors) in &cycle_adj {
                    if !scc_set.contains(src) || src == *id {
                        continue;
                    }
                    if neighbors.contains(id) {
                        in_in_cycle += 1;
                    }
                }
                (*id, out_in_cycle, in_in_cycle)
            })
            .collect();
        // Ascending by out-degree → entry-point first; ties broken by
        // descending in-degree (hub-iness) so the most-referenced "leaf"
        // surfaces just after the cleanest leaf.
        ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));

        let symbols_detailed: Vec<Value> = ranked
            .iter()
            .filter_map(|(id, out_deg, in_deg)| {
                let node = node_map.get(id)?;
                Some(json!({
                    "name": node.name,
                    "kind": node.kind.as_str(),
                    "file": node.file_path,
                    "line": node.start_line,
                    "in_cycle_out_degree": out_deg,
                    "in_cycle_in_degree": in_deg,
                }))
            })
            .collect();

        // Rank files by how many cycle members each contains — the file
        // with the most members is the best refactor target.
        let mut file_counts: HashMap<&str, usize> = HashMap::new();
        for id in &scc {
            if let Some(n) = node_map.get(id) {
                *file_counts.entry(n.file_path.as_str()).or_insert(0) += 1;
            }
        }
        let mut files_ranked: Vec<(&str, usize)> = file_counts.into_iter().collect();
        files_ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let files_json: Vec<Value> = files_ranked
            .iter()
            .map(|(path, count)| json!({"file": path, "members_in_cycle": count}))
            .collect();

        let entry_point = ranked.first().and_then(|(id, _, _)| node_map.get(id));
        let hub = ranked
            .iter()
            .max_by_key(|(_, _out, in_deg)| *in_deg)
            .and_then(|(id, _, _)| node_map.get(id));

        cycles_json.push(json!({
            "size": scc.len(),
            "files": files_json,
            "symbols": symbols_detailed,
            "entry_point": entry_point.map(|n| json!({
                "name": n.name, "file": n.file_path, "line": n.start_line,
            })),
            "break_point_candidate": hub.map(|n| json!({
                "name": n.name, "file": n.file_path, "line": n.start_line,
                "rationale": "Highest in-cycle in-degree — refactoring its callers is the most effective way to fragment this SCC.",
            })),
            "note": "Mutual dependency — port together, starting at `entry_point` and refactoring `break_point_candidate` to split the cycle.",
        }));
    }

    // Build output levels
    let levels_json: Vec<Value> = levels
        .iter()
        .enumerate()
        .map(|(i, level_ids)| {
            let description = if i == 0 {
                "No internal dependencies — port these first".to_string()
            } else {
                format!("Depends only on levels 0–{}", i - 1)
            };

            let symbols: Vec<Value> = level_ids
                .iter()
                .filter_map(|id| {
                    let node = node_map.get(id)?;
                    // Find what this node depends on (for depends_on field)
                    let deps: Vec<&str> = dep_graph
                        .get(id)
                        .map(|d| {
                            d.iter()
                                .filter_map(|dep_id| node_map.get(dep_id).map(|n| n.name.as_str()))
                                .collect()
                        })
                        .unwrap_or_default();

                    let mut sym = json!({
                        "name": node.name,
                        "kind": node.kind.as_str(),
                        "file": node.file_path,
                        "line": node.start_line,
                    });
                    if !deps.is_empty() {
                        sym["depends_on"] = json!(deps);
                    }
                    Some(sym)
                })
                .collect();

            json!({
                "level": i,
                "description": description,
                "symbols": symbols,
            })
        })
        .collect();

    let touched_files = unique_file_paths(nodes.iter().map(|n| n.file_path.as_str()));

    let result = json!({
        "source_dir": source_dir,
        "total_symbols": total_symbols,
        "returned": emitted,
        "levels": levels_json,
        "cycles": cycles_json,
    });

    let formatted = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_simplify_scan` tool calls.
pub(super) async fn handle_simplify_scan(
    cg: &TokenSave,
    args: Value,
    _scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: files (array of strings)".to_string(),
        })?;

    let mut duplications: Vec<Value> = Vec::new();
    let mut dead_introductions: Vec<Value> = Vec::new();
    let mut complexity_warnings: Vec<Value> = Vec::new();
    let mut coupling_warnings: Vec<Value> = Vec::new();

    for file in &files {
        let nodes = cg.get_nodes_by_file(file).await.unwrap_or_default();

        for node in &nodes {
            // 1. Duplication: find similar symbols elsewhere
            if matches!(node.kind, NodeKind::Function | NodeKind::Method) {
                let similar = cg.search(&node.name, 5).await.unwrap_or_default();
                let dupes: Vec<Value> = similar
                    .iter()
                    .filter(|s| {
                        s.node.id != node.id && s.score > 0.8 && s.node.file_path != node.file_path
                    })
                    .map(|d| {
                        json!({
                            "name": d.node.name,
                            "file": d.node.file_path,
                            "line": d.node.start_line,
                            "score": d.score,
                        })
                    })
                    .collect();
                if !dupes.is_empty() {
                    duplications.push(json!({
                        "symbol": node.name,
                        "file": node.file_path,
                        "line": node.start_line,
                        "similar_to": dupes,
                    }));
                }
            }

            // 2. Dead code: function/method with no incoming edges
            if matches!(node.kind, NodeKind::Function | NodeKind::Method)
                && node.visibility != Visibility::Pub
                && node.name != "main"
                && !node.name.starts_with("test_")
            {
                let incoming = cg.get_incoming_edges(&node.id).await.unwrap_or_default();
                if incoming.is_empty() {
                    dead_introductions.push(json!({
                        "symbol": node.name,
                        "file": node.file_path,
                        "line": node.start_line,
                        "reason": "no incoming edges (unreferenced)",
                    }));
                }
            }

            // 3. Complexity: check if function exceeds threshold
            if matches!(node.kind, NodeKind::Function | NodeKind::Method) {
                let lines = node.end_line.saturating_sub(node.start_line) as usize;
                let fan_out = cg
                    .get_outgoing_edges(&node.id)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .filter(|e| matches!(e.kind, crate::types::EdgeKind::Calls))
                    .count();
                let score = lines + fan_out * 3;
                if score > 100 {
                    complexity_warnings.push(json!({
                        "symbol": node.name,
                        "file": node.file_path,
                        "line": node.start_line,
                        "lines": lines,
                        "fan_out": fan_out,
                        "score": score,
                    }));
                }
            }
        }

        // 4. Coupling: check file fan_in
        let file_deps = cg.get_file_dependents(file).await.unwrap_or_default();
        if file_deps.len() > 15 {
            coupling_warnings.push(json!({
                "file": file,
                "fan_in": file_deps.len(),
                "warning": "high fan-in — changes here affect many dependents",
            }));
        }
    }

    let output = json!({
        "duplications": duplications,
        "dead_introductions": dead_introductions,
        "complexity_warnings": complexity_warnings,
        "coupling_warnings": coupling_warnings,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: files,
    })
}

/// Handles `tokensave_type_hierarchy` tool calls.
pub(super) async fn handle_type_hierarchy(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;
    let max_depth = args
        .get("max_depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(5, |v| v.min(10) as usize);

    let root = cg
        .get_node(node_id)
        .await?
        .ok_or_else(|| TokenSaveError::Config {
            message: format!("node not found: {node_id}"),
        })?;

    let mut output = format!(
        "{} ({}) -- {}:{}\n",
        root.name,
        root.kind.as_str(),
        root.file_path,
        root.start_line
    );
    let mut all_files: Vec<String> = vec![root.file_path.clone()];

    // Recursively build the hierarchy
    build_type_tree(cg, &root.id, max_depth, 0, &mut output, &mut all_files).await;

    let touched_files = unique_file_paths(all_files.iter().map(std::string::String::as_str));
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&output)}]}),
        touched_files,
    })
}

/// Recursively appends type hierarchy lines to the output string.
fn build_type_tree<'a>(
    cg: &'a TokenSave,
    node_id: &'a str,
    max_depth: usize,
    depth: usize,
    output: &'a mut String,
    all_files: &'a mut Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        if depth >= max_depth {
            return;
        }

        let incoming = cg.get_incoming_edges(node_id).await.unwrap_or_default();
        let pad = "  ".repeat(depth);

        for edge in &incoming {
            if !matches!(
                edge.kind,
                crate::types::EdgeKind::Implements | crate::types::EdgeKind::Extends
            ) {
                continue;
            }
            if let Ok(Some(child)) = cg.get_node(&edge.source).await {
                let _ = writeln!(
                    output,
                    "{}|- {} {} ({}) -- {}:{}",
                    pad,
                    edge.kind.as_str(),
                    child.name,
                    child.kind.as_str(),
                    child.file_path,
                    child.start_line,
                );
                all_files.push(child.file_path.clone());
                build_type_tree(cg, &child.id, max_depth, depth + 1, output, all_files).await;
            }
        }
    })
}

/// Extract the source spanning tree-sitter rows `start_line..=end_line`
/// (0-based, inclusive) from `source`. Node line fields are stored as the
/// raw tree-sitter row index, so the caller passes them through unchanged.
/// Returns the empty string if the range is out of bounds.
pub(super) fn extract_lines(source: &str, start_line: u32, end_line: u32) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = start_line as usize;
    let end = (end_line as usize).saturating_add(1).min(lines.len());
    if start >= lines.len() || start >= end {
        return String::new();
    }
    lines[start..end].join("\n")
}

/// Handles `tokensave_body` tool calls.
pub(super) async fn handle_body(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let symbol =
        args.get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: symbol".to_string(),
            })?;

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(3, |v| v.clamp(1, 20) as usize);

    // First try an exact-name lookup against the DB — this avoids the BM25
    // ranker's tendency to bury a definition under unrelated noise when the
    // bare name is common (e.g. `gmres` exists as both a `pub fn` and a
    // struct field). Falls back to suffix / name match inside
    // `get_nodes_by_qualified_name`.
    let exact_nodes = cg.get_nodes_by_qualified_name(symbol).await?;
    let exact_nodes = super::filter_by_scope(exact_nodes, scope_prefix, |n| &n.file_path);

    // Wrap as SearchResult so the existing scoring/rendering path works.
    let mut candidates: Vec<crate::types::SearchResult> = exact_nodes
        .into_iter()
        .map(|node| crate::types::SearchResult { node, score: 0.0 })
        .collect();

    // If exact lookup returned nothing, fall back to BM25 search.
    if candidates.is_empty() {
        let raw = cg.search(symbol, (limit * 4).max(20)).await?;
        candidates = super::filter_by_scope(raw, scope_prefix, |r| &r.node.file_path);
    }

    // Whether the matches came from the exact lookup or the search fallback,
    // sort by `body_kind_preference` so callable / type definitions surface
    // above fields, variants, uses, etc. This is the bug-#1 fix: when both a
    // function and a same-named field exist, the function wins.
    candidates.sort_by_key(|r| body_kind_preference(&r.node.kind));
    let chosen: Vec<_> = candidates.iter().take(limit).collect();

    if chosen.is_empty() {
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": format!("No symbol named '{symbol}' found.") }]
            }),
            touched_files: vec![],
        });
    }

    let project_root = cg.project_root();
    let mut matches: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();

    for result in &chosen {
        let n = &result.node;
        let abs_path = project_root.join(&n.file_path);
        let body = match crate::sync::read_source_file(&abs_path) {
            Ok(source) => extract_lines(&source, n.start_line, n.end_line),
            Err(_) => String::from("<file unreadable>"),
        };
        if !touched.contains(&n.file_path) {
            touched.push(n.file_path.clone());
        }
        matches.push(json!({
            "id": n.id,
            "name": n.name,
            "qualified_name": n.qualified_name,
            "kind": n.kind.as_str(),
            "file": n.file_path,
            "start_line": n.start_line.saturating_add(1),
            "end_line": n.end_line.saturating_add(1),
            "signature": n.signature,
            "body": body,
        }));
    }

    let output = json!({
        "match_count": matches.len(),
        "matches": matches,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

/// Ordering key used by `handle_body` to choose between same-named symbols.
/// Lower number = higher preference (sorted ascending). Callable kinds rank
/// best because the user almost always asks for "show me the body of X"
/// expecting a function or method; type definitions are next; fields,
/// variants, use statements come last.
fn body_kind_preference(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::Function
        | NodeKind::Method
        | NodeKind::StructMethod
        | NodeKind::Constructor
        | NodeKind::AbstractMethod
        | NodeKind::ArrowFunction
        | NodeKind::Procedure => 0,
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
        | NodeKind::Typedef => 1,
        NodeKind::Impl => 2,
        NodeKind::Const | NodeKind::Static | NodeKind::Macro | NodeKind::PreprocessorDef => 3,
        NodeKind::Field
        | NodeKind::ValField
        | NodeKind::VarField
        | NodeKind::Property
        | NodeKind::CSharpProperty
        | NodeKind::EnumVariant => 4,
        NodeKind::Use | NodeKind::Include => 5,
        _ => 6,
    }
}

/// Default marker kinds recognised by `tokensave_todos`.
const DEFAULT_TODO_KINDS: &[&str] = &[
    "TODO",
    "FIXME",
    "XXX",
    "HACK",
    "WIP",
    "NOTE",
    "UNIMPLEMENTED",
];

/// True if `text` contains `marker` as a standalone uppercase word
/// (case-insensitive, surrounded by non-alphanumeric characters or string ends).
fn contains_marker_word(text: &str, marker: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let marker_lower = marker.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mlen = marker_lower.len();
    let mut idx = 0;
    while idx + mlen <= bytes.len() {
        if &bytes[idx..idx + mlen] == marker_lower.as_bytes() {
            let before_ok =
                idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric() && bytes[idx - 1] != b'_';
            let after_ok = idx + mlen == bytes.len()
                || (!bytes[idx + mlen].is_ascii_alphanumeric() && bytes[idx + mlen] != b'_');
            if before_ok && after_ok {
                return Some(idx);
            }
        }
        idx += 1;
    }
    None
}

/// Handles `tokensave_todos` tool calls.
pub(super) async fn handle_todos(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let kinds: Vec<String> = args
        .get("kinds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_uppercase))
                .collect::<Vec<_>>()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_TODO_KINDS
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        });

    let path = effective_path(&args, scope_prefix);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(200, |v| v.min(2000) as usize);

    let project_root = cg.project_root();
    let files = cg.get_all_files().await?;
    let mut markers: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    let mut by_kind: HashMap<String, u64> = HashMap::new();

    'outer: for file in &files {
        if let Some(prefix) = path {
            let with_slash = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            if !file.path.starts_with(&with_slash) && file.path != prefix {
                continue;
            }
        }
        let abs_path = project_root.join(&file.path);
        let Ok(source) = crate::sync::read_source_file(&abs_path) else {
            continue;
        };
        // Cache nodes per file so enclosing-symbol lookup is one DB call per file.
        let nodes = cg.get_nodes_by_file(&file.path).await.unwrap_or_default();

        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx as u32) + 1;
            for kind in &kinds {
                if contains_marker_word(line, kind).is_some() {
                    let enclosing = nodes
                        .iter()
                        .filter(|n| n.start_line <= line_no && line_no <= n.end_line)
                        .min_by_key(|n| n.end_line.saturating_sub(n.start_line))
                        .map(|n| n.qualified_name.clone());
                    *by_kind.entry(kind.clone()).or_insert(0) += 1;
                    markers.push(json!({
                        "kind": kind,
                        "file": file.path,
                        "line": line_no,
                        "text": line.trim(),
                        "enclosing": enclosing,
                    }));
                    if !touched.contains(&file.path) {
                        touched.push(file.path.clone());
                    }
                    if markers.len() >= limit {
                        break 'outer;
                    }
                    break; // one marker per line is enough
                }
            }
        }
    }

    let counts = serde_json::to_value(&by_kind).unwrap_or(json!({}));
    let output = json!({
        "match_count": markers.len(),
        "by_kind": counts,
        "markers": markers,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

/// Handles `tokensave_read` — mode-aware file read with cross-session cache.
pub(super) async fn handle_read(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    use crate::context::read_cache::{self, GLOBAL_SESSION};
    use crate::context::read_modes::{
        self, render_full, render_lines, render_map, render_signatures, LineRange, ReadMode,
    };

    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: file".to_string(),
        })?;

    let mode_str = args.get("mode").and_then(|v| v.as_str()).unwrap_or("full");
    let mode = ReadMode::parse(mode_str).ok_or_else(|| TokenSaveError::Config {
        message: format!("unknown mode '{mode_str}'; expected one of full, lines, map, signatures"),
    })?;

    let line_range = if mode == ReadMode::Lines {
        let raw =
            args.get("lines")
                .and_then(|v| v.as_str())
                .ok_or_else(|| TokenSaveError::Config {
                    message: "mode='lines' requires the 'lines' argument (e.g. '120-180')"
                        .to_string(),
                })?;
        Some(LineRange::parse(raw).ok_or_else(|| TokenSaveError::Config {
            message: format!("invalid 'lines' value '{raw}'; expected 'A' or 'A-B'"),
        })?)
    } else {
        None
    };

    let project_root = cg.project_root().to_path_buf();
    let project_id = project_root.to_string_lossy().to_string();
    let rel_path = file.trim_start_matches('/').to_string();
    let abs_path = if std::path::Path::new(file).is_absolute() {
        std::path::PathBuf::from(file)
    } else {
        project_root.join(&rel_path)
    };
    let display_file = if abs_path.starts_with(&project_root) {
        abs_path
            .strip_prefix(&project_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(rel_path.clone())
    } else {
        rel_path.clone()
    };

    let mtime_ns = read_cache::file_mtime_ns(&abs_path).map_err(|e| TokenSaveError::Config {
        message: format!("cannot read file metadata for '{file}': {e}"),
    })?;

    let last_sync_at = match mode {
        ReadMode::Map | ReadMode::Signatures => {
            cg.db().get_metadata("last_sync_at").await.unwrap_or(None)
        }
        _ => None,
    };
    let hash_input = json!({
        "lines": args.get("lines").cloned(),
        "last_sync_at": last_sync_at,
    });
    let args_hash = read_cache::args_hash(&hash_input);

    let conn = cg.db().conn();

    if let Some(cached) = read_cache::get(
        conn,
        &project_id,
        GLOBAL_SESSION,
        &display_file,
        mode.as_str(),
        &args_hash,
        mtime_ns,
    )
    .await?
    {
        let stub = json!({
            "unchanged": true,
            "file": display_file,
            "mode": mode.as_str(),
            "mtime_ns": cached.mtime_ns,
            "digest": cached.digest,
            "token_count": cached.token_count,
        });
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&stub).unwrap_or_default() }]
            }),
            touched_files: vec![display_file],
        });
    }

    let body_text = match mode {
        ReadMode::Full => {
            let source =
                crate::sync::read_source_file(&abs_path).map_err(|e| TokenSaveError::Config {
                    message: format!("cannot read '{file}': {e}"),
                })?;
            render_full(&source)
        }
        ReadMode::Lines => {
            let range = line_range.ok_or_else(|| TokenSaveError::Config {
                message: "internal error: lines mode reached without a parsed range".to_string(),
            })?;
            let source =
                crate::sync::read_source_file(&abs_path).map_err(|e| TokenSaveError::Config {
                    message: format!("cannot read '{file}': {e}"),
                })?;
            render_lines(&source, range)
        }
        ReadMode::Map => {
            let v = render_map(cg.db(), &display_file, None).await?;
            serde_json::to_string_pretty(&v).unwrap_or_default()
        }
        ReadMode::Signatures => {
            let v = render_signatures(cg.db(), &display_file).await?;
            serde_json::to_string_pretty(&v).unwrap_or_default()
        }
    };

    let token_count = read_modes::estimate_tokens(&body_text);
    let digest = read_cache::digest_bytes(body_text.as_bytes());

    read_cache::put(
        conn,
        &project_id,
        GLOBAL_SESSION,
        &display_file,
        mtime_ns,
        mode.as_str(),
        &args_hash,
        &digest,
        body_text.as_bytes(),
        token_count,
    )
    .await?;

    let payload = json!({
        "file": display_file,
        "mode": mode.as_str(),
        "mtime_ns": mtime_ns,
        "digest": digest,
        "token_count": token_count,
        "body": body_text,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();

    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![display_file],
    })
}

/// Handles `tokensave_outline` — flat symbol map for a file with optional
/// `kinds` filter.
pub(super) async fn handle_outline(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    use crate::context::read_modes::render_map;

    let file = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: file".to_string(),
        })?;

    let kinds: Option<Vec<String>> = args.get("kinds").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    });

    let project_root = cg.project_root();
    let rel_path = file.trim_start_matches('/').to_string();
    let abs_path = if std::path::Path::new(file).is_absolute() {
        std::path::PathBuf::from(file)
    } else {
        project_root.join(&rel_path)
    };
    let display_file = if abs_path.starts_with(project_root) {
        abs_path
            .strip_prefix(project_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(rel_path.clone())
    } else {
        rel_path.clone()
    };

    let kinds_slice: Option<&[String]> = kinds.as_deref();
    let value = render_map(cg.db(), &display_file, kinds_slice).await?;
    let formatted = serde_json::to_string_pretty(&value).unwrap_or_default();

    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![display_file],
    })
}

/// Handles `tokensave_config` — structured TOML / JSON queries by dotted
/// key path.
pub(super) async fn handle_config(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let key = args
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: key".to_string(),
        })?;
    let path = args.get("path").and_then(|v| v.as_str());
    let glob_pat = args.get("glob").and_then(|v| v.as_str());

    if path.is_none() && glob_pat.is_none() {
        return Err(TokenSaveError::Config {
            message: "tokensave_config requires either 'path' or 'glob'".to_string(),
        });
    }
    if path.is_some() && glob_pat.is_some() {
        return Err(TokenSaveError::Config {
            message: "tokensave_config: 'path' and 'glob' are mutually exclusive".to_string(),
        });
    }

    let project_root = cg.project_root().to_path_buf();
    let mut files: Vec<String> = Vec::new();
    if let Some(p) = path {
        files.push(p.to_string());
    } else if let Some(pat) = glob_pat {
        let combined = project_root.join(pat);
        let walker =
            glob::glob(&combined.to_string_lossy()).map_err(|e| TokenSaveError::Config {
                message: format!("invalid glob '{pat}': {e}"),
            })?;
        for entry in walker.flatten() {
            if let Ok(rel) = entry.strip_prefix(&project_root) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
        files.sort();
    }

    let mut matches: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    for rel in &files {
        let abs = project_root.join(rel);
        let Ok(contents) = std::fs::read_to_string(&abs) else {
            continue;
        };
        let parsed = match config_format(rel) {
            Some(ConfigFormat::Toml) => match toml::from_str::<toml::Value>(&contents) {
                Ok(v) => toml_to_json(&v),
                Err(e) => {
                    matches.push(json!({
                        "file": rel,
                        "error": format!("toml parse error: {e}"),
                    }));
                    continue;
                }
            },
            Some(ConfigFormat::Json) => match serde_json::from_str::<Value>(&contents) {
                Ok(v) => v,
                Err(e) => {
                    matches.push(json!({
                        "file": rel,
                        "error": format!("json parse error: {e}"),
                    }));
                    continue;
                }
            },
            None => continue,
        };

        let value = lookup_dotted(&parsed, key);
        let line = match &value {
            Some(_) => find_key_line(&contents, key),
            None => None,
        };

        if !touched.contains(rel) {
            touched.push(rel.clone());
        }

        matches.push(match value {
            Some(v) => json!({
                "file": rel,
                "key": key,
                "value": v,
                "line": line,
            }),
            None => json!({
                "file": rel,
                "key": key,
                "value": Value::Null,
                "found": false,
            }),
        });
    }

    let payload = json!({
        "match_count": matches.iter().filter(|m| m.get("found") != Some(&Value::Bool(false))).count(),
        "matches": matches,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

#[derive(Debug, Clone, Copy)]
enum ConfigFormat {
    Toml,
    Json,
}

fn config_format(path: &str) -> Option<ConfigFormat> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".toml") {
        Some(ConfigFormat::Toml)
    } else if lower.ends_with(".json") {
        Some(ConfigFormat::Json)
    } else {
        None
    }
}

fn lookup_dotted(value: &Value, key: &str) -> Option<Value> {
    let mut cursor = value.clone();
    for segment in key.split('.') {
        cursor = match cursor {
            Value::Object(map) => map.get(segment).cloned()?,
            Value::Array(items) => {
                let idx: usize = segment.parse().ok()?;
                items.get(idx).cloned()?
            }
            _ => return None,
        };
    }
    Some(cursor)
}

fn toml_to_json(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(items) => Value::Array(items.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            let mut map = serde_json::Map::with_capacity(t.len());
            for (k, child) in t {
                map.insert(k.clone(), toml_to_json(child));
            }
            Value::Object(map)
        }
    }
}

fn find_key_line(contents: &str, key: &str) -> Option<u32> {
    let last = key.rsplit('.').next()?;
    let toml_form_eq = format!("{last} =");
    let toml_form_quoted = format!("\"{last}\" =");
    let json_form = format!("\"{last}\":");
    for (idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&toml_form_eq)
            || trimmed.starts_with(&toml_form_quoted)
            || trimmed.starts_with(&json_form)
        {
            return Some((idx as u32) + 1);
        }
    }
    None
}

/// Handles `tokensave_signature_search` — substring search across the
/// cached `signature` column on every Function/Method node.
pub(super) async fn handle_signature_search(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let returns = args.get("returns").and_then(|v| v.as_str());
    let params: Vec<String> = args
        .get("params")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let want_async = args.get("async").and_then(serde_json::Value::as_bool);
    let path_filter = args.get("path").and_then(|v| v.as_str()).or(scope_prefix);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(50, |v| v.clamp(1, 500) as usize);

    if returns.is_none() && params.is_empty() && want_async.is_none() {
        return Err(TokenSaveError::Config {
            message: "tokensave_signature_search requires at least one of returns / params / async"
                .to_string(),
        });
    }

    let function_nodes = cg.db().get_nodes_by_kind(NodeKind::Function).await?;
    let method_nodes = cg.db().get_nodes_by_kind(NodeKind::Method).await?;

    let mut entries: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    for node in function_nodes.iter().chain(method_nodes.iter()) {
        if let Some(prefix) = path_filter {
            let with_slash = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            if !node.file_path.starts_with(&with_slash) && node.file_path != prefix {
                continue;
            }
        }

        if let Some(want) = want_async {
            if node.is_async != want {
                continue;
            }
        }

        let Some(sig) = node.signature.as_deref() else {
            continue;
        };

        if let Some(ret_pat) = returns {
            if !returns_substring(sig).contains(ret_pat) {
                continue;
            }
        }

        if !params.is_empty() {
            let param_region = params_substring(sig);
            if !params.iter().all(|p| param_region.contains(p.as_str())) {
                continue;
            }
        }

        if !touched.contains(&node.file_path) {
            touched.push(node.file_path.clone());
        }
        entries.push(json!({
            "name": node.name,
            "qualified_name": node.qualified_name,
            "kind": node.kind.as_str(),
            "file": node.file_path,
            "line": node.start_line,
            "is_async": node.is_async,
            "signature": sig,
        }));
        if entries.len() >= limit {
            break;
        }
    }

    let payload = json!({
        "match_count": entries.len(),
        "matches": entries,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

fn returns_substring(signature: &str) -> &str {
    match signature.find("->") {
        Some(pos) => signature[pos + 2..].trim_start(),
        None => signature,
    }
}

fn params_substring(signature: &str) -> &str {
    let bytes = signature.as_bytes();
    let Some(open) = signature.find('(') else {
        return signature;
    };
    let mut depth = 0i32;
    for (i, b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &signature[open + 1..i];
                }
            }
            _ => {}
        }
    }
    signature
}
