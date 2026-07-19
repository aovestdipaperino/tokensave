//! Port-status and dependency-order MCP tool handlers.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::NodeKind;

use super::super::ToolResult;
use super::{truncate_response, unique_file_paths};

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
                    "line": super::display_line(src_node.start_line),
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
                "line": super::display_line(n.start_line),
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
                    "line": super::display_line(node.start_line),
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
                "name": n.name, "file": n.file_path, "line": super::display_line(n.start_line),
            })),
            "break_point_candidate": hub.map(|n| json!({
                "name": n.name, "file": n.file_path, "line": super::display_line(n.start_line),
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
                        "line": super::display_line(node.start_line),
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
