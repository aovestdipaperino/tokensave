//! Health, test risk, sessions, gini, dependency depth, DSM, and test map
//! tool handlers.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::graph::health::{
    acyclicity_score, compute_composite_health, dependency_depth, depth_score, gini_coefficient,
    gini_label, modularity_score, HealthDimensions,
};
use crate::graph::queries::GraphQueryManager;
use crate::tokensave::TokenSave;
use crate::types::{EdgeKind, NodeKind};

use super::super::ToolResult;
use super::{effective_path, truncate_response, unique_file_paths};

// ---------------------------------------------------------------------------
// Shared health computation helper
// ---------------------------------------------------------------------------

pub(super) struct HealthSnapshot {
    pub(super) quality_signal: u32,
    pub(super) files_analyzed: usize,
    pub(super) acyclicity: f64,
    pub(super) depth: f64,
    pub(super) equality: f64,
    pub(super) redundancy: f64,
    pub(super) modularity: f64,
    pub(super) coverage_discipline: f64,
}

/// Computes all 5 health dimensions and the composite signal for a given scope.
pub(super) async fn compute_health_snapshot(
    cg: &TokenSave,
    path_prefix: Option<&str>,
) -> Result<HealthSnapshot> {
    let adj = GraphQueryManager::new(cg.db())
        .build_file_adjacency(path_prefix)
        .await?;
    let files_analyzed = adj.len();

    let (acyclicity, _) = acyclicity_score(&adj);
    let depth_result = dependency_depth(&adj, 1);
    let depth = depth_score(depth_result.max_depth, depth_result.ideal_depth);

    let all_nodes = cg.get_all_nodes().await?;
    let nodes: Vec<_> = all_nodes
        .iter()
        .filter(|n| {
            path_prefix.is_none_or(|pfx| {
                let with_slash = if pfx.ends_with('/') {
                    pfx.to_string()
                } else {
                    format!("{pfx}/")
                };
                n.file_path.starts_with(&with_slash) || n.file_path == pfx
            })
        })
        .collect();

    let mut per_file_complexity: HashMap<String, f64> = HashMap::new();
    for n in &nodes {
        let c = f64::from(n.branches) * 2.0
            + f64::from(n.loops) * 2.0
            + f64::from(n.max_nesting) * 3.0
            + f64::from(n.end_line.saturating_sub(n.start_line) + 1);
        *per_file_complexity
            .entry(n.file_path.clone())
            .or_insert(0.0) += c;
    }
    let complexity_values: Vec<f64> = per_file_complexity.values().copied().collect();
    let gini = gini_coefficient(&complexity_values);
    let equality = (1.0 - gini).clamp(0.0, 1.0);

    let dead = cg
        .find_dead_code(&[NodeKind::Function, NodeKind::Method], false)
        .await?;
    let dead_in_scope = dead.iter().filter(|n| {
        path_prefix.is_none_or(|pfx| {
            let with_slash = if pfx.ends_with('/') {
                pfx.to_string()
            } else {
                format!("{pfx}/")
            };
            n.file_path.starts_with(&with_slash) || n.file_path == pfx
        })
    });
    let dead_count = dead_in_scope.count();
    let total_fns = nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Method))
        .count();
    let redundancy = if total_fns == 0 {
        1.0
    } else {
        (1.0 - dead_count as f64 / total_fns as f64).clamp(0.0, 1.0)
    };

    let (modularity, _) = modularity_score(&adj);

    // coverage_discipline: penalise overuse of skip-test-coverage annotations.
    let skip_coverage = cg
        .get_skip_test_coverage_node_ids()
        .await
        .unwrap_or_default();
    let skipped_in_scope = nodes
        .iter()
        .filter(|n| {
            matches!(n.kind, NodeKind::Function | NodeKind::Method) && skip_coverage.contains(&n.id)
        })
        .count();
    let coverage_discipline = if total_fns == 0 {
        1.0
    } else {
        (1.0 - skipped_in_scope as f64 / total_fns as f64).clamp(0.0, 1.0)
    };

    let dims = HealthDimensions {
        acyclicity,
        depth,
        equality,
        redundancy,
        modularity,
        coverage_discipline,
    };
    let quality_signal = compute_composite_health(&dims);

    Ok(HealthSnapshot {
        quality_signal,
        files_analyzed,
        acyclicity,
        depth,
        equality,
        redundancy,
        modularity,
        coverage_discipline,
    })
}

/// Handles `tokensave_gini` tool calls.
pub(super) async fn handle_gini(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let metric = args
        .get("metric")
        .and_then(|v| v.as_str())
        .unwrap_or("complexity");
    let scope = args.get("scope").and_then(|v| v.as_str()).unwrap_or("file");
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);
    let path_prefix = effective_path(&args, scope_prefix);

    let all_nodes = cg.get_all_nodes().await?;
    let all_edges = if metric == "fan_in" || metric == "fan_out" {
        cg.get_all_edges().await?
    } else {
        vec![]
    };

    // Apply path filter
    let nodes: Vec<_> = all_nodes
        .into_iter()
        .filter(|n| {
            path_prefix.is_none_or(|pfx| {
                let with_slash = if pfx.ends_with('/') {
                    pfx.to_string()
                } else {
                    format!("{pfx}/")
                };
                n.file_path.starts_with(&with_slash) || n.file_path == pfx
            })
        })
        .collect();

    // Build named_values per metric+scope
    let named_values: Vec<(String, f64)> = match (metric, scope) {
        ("complexity", "file") => {
            let mut per_file: HashMap<String, f64> = HashMap::new();
            for n in &nodes {
                let c = f64::from(n.branches + n.loops + n.returns + n.max_nesting);
                *per_file.entry(n.file_path.clone()).or_insert(0.0) += c;
            }
            per_file.into_iter().collect()
        }
        ("lines", "file") => {
            let mut per_file: HashMap<String, f64> = HashMap::new();
            for n in &nodes {
                let lines = f64::from(n.end_line.saturating_sub(n.start_line) + 1);
                *per_file.entry(n.file_path.clone()).or_insert(0.0) += lines;
            }
            per_file.into_iter().collect()
        }
        ("fan_in", "file") => {
            let node_to_file: HashMap<String, String> = nodes
                .iter()
                .map(|n| (n.id.clone(), n.file_path.clone()))
                .collect();
            let mut per_file: HashMap<String, f64> = HashMap::new();
            // Initialize all files
            for n in &nodes {
                per_file.entry(n.file_path.clone()).or_insert(0.0);
            }
            for e in &all_edges {
                if let (Some(src_file), Some(tgt_file)) =
                    (node_to_file.get(&e.source), node_to_file.get(&e.target))
                {
                    if src_file != tgt_file {
                        *per_file.entry(tgt_file.clone()).or_insert(0.0) += 1.0;
                    }
                }
            }
            per_file.into_iter().collect()
        }
        ("fan_out", "file") => {
            let node_to_file: HashMap<String, String> = nodes
                .iter()
                .map(|n| (n.id.clone(), n.file_path.clone()))
                .collect();
            let mut per_file: HashMap<String, f64> = HashMap::new();
            for n in &nodes {
                per_file.entry(n.file_path.clone()).or_insert(0.0);
            }
            for e in &all_edges {
                if let (Some(src_file), Some(tgt_file)) =
                    (node_to_file.get(&e.source), node_to_file.get(&e.target))
                {
                    if src_file != tgt_file {
                        *per_file.entry(src_file.clone()).or_insert(0.0) += 1.0;
                    }
                }
            }
            per_file.into_iter().collect()
        }
        ("members", _) => {
            // Count members of each Class/Struct via parent_id (v9+).
            let class_nodes: HashSet<String> = nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Class | NodeKind::Struct))
                .map(|n| n.id.clone())
                .collect();
            let mut per_class: HashMap<String, (String, f64)> = nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Class | NodeKind::Struct))
                .map(|n| (n.id.clone(), (n.name.clone(), 0.0)))
                .collect();
            for n in &nodes {
                if let Some(parent) = n.parent_id.as_deref() {
                    if class_nodes.contains(parent) {
                        if let Some(entry) = per_class.get_mut(parent) {
                            entry.1 += 1.0;
                        }
                    }
                }
            }
            per_class.into_values().collect()
        }
        (_, "symbol") => {
            // Per-function/method complexity
            nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Method))
                .map(|n| {
                    let c = f64::from(n.branches + n.loops + n.returns + n.max_nesting);
                    (format!("{}:{}", n.file_path, n.name), c)
                })
                .collect()
        }
        _ => {
            // Default: file-level complexity
            let mut per_file: HashMap<String, f64> = HashMap::new();
            for n in &nodes {
                let c = f64::from(n.branches + n.loops + n.returns + n.max_nesting);
                *per_file.entry(n.file_path.clone()).or_insert(0.0) += c;
            }
            per_file.into_iter().collect()
        }
    };

    let values: Vec<f64> = named_values.iter().map(|(_, v)| *v).collect();
    let gini = gini_coefficient(&values);
    let interpretation = gini_label(gini);

    // Sort descending, take top limit as outliers with percentiles
    let total_items = named_values.len();
    let mut sorted = named_values;
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);

    let max_val = sorted.first().map_or(0.0, |(_, v)| *v);
    let outliers: Vec<Value> = sorted
        .iter()
        .map(|(name, val)| {
            let pct = if max_val > 0.0 {
                (val / max_val * 100.0).round()
            } else {
                0.0
            };
            json!({
                "name": name,
                "value": val,
                "pct_of_max": pct,
            })
        })
        .collect();

    let output = json!({
        "gini": (gini * 10000.0).round() / 10000.0,
        "interpretation": interpretation,
        "total_items": total_items,
        "metric": metric,
        "scope": scope,
        "outliers": outliers,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_dependency_depth` tool calls.
pub(super) async fn handle_dependency_depth(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);
    let path_prefix = effective_path(&args, scope_prefix);

    let adj = GraphQueryManager::new(cg.db())
        .build_file_adjacency(path_prefix)
        .await?;

    let result = dependency_depth(&adj, limit);
    let score = depth_score(result.max_depth, result.ideal_depth);

    let chains: Vec<Value> = result
        .chains
        .iter()
        .map(|ch| {
            json!({
                "file": ch.file,
                "depth": ch.depth,
                "chain": ch.chain,
            })
        })
        .collect();

    let output = json!({
        "max_depth": result.max_depth,
        "ideal_depth": result.ideal_depth,
        "depth_score": (score * 10000.0).round() / 10000.0,
        "chains": chains,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_health` tool calls.
pub(super) async fn handle_health(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path_prefix = effective_path(&args, scope_prefix);
    let details = args
        .get("details")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let snap = compute_health_snapshot(cg, path_prefix).await?;

    let output = if details {
        json!({
            "quality_signal": snap.quality_signal,
            "files_analyzed": snap.files_analyzed,
            "dimensions": {
                "acyclicity": (snap.acyclicity * 10000.0).round() / 10000.0,
                "depth": (snap.depth * 10000.0).round() / 10000.0,
                "equality": (snap.equality * 10000.0).round() / 10000.0,
                "redundancy": (snap.redundancy * 10000.0).round() / 10000.0,
                "modularity": (snap.modularity * 10000.0).round() / 10000.0,
                "coverage_discipline": snap.coverage_discipline,
            }
        })
    } else {
        json!({
            "quality_signal": snap.quality_signal,
            "files_analyzed": snap.files_analyzed,
        })
    };

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_dsm` tool calls.
pub(super) async fn handle_dsm(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path_prefix = effective_path(&args, scope_prefix);
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("stats");
    let max_files = args
        .get("max_files")
        .and_then(serde_json::Value::as_u64)
        .map_or(30, |v| v.min(200) as usize);

    let adj = GraphQueryManager::new(cg.db())
        .build_file_adjacency(path_prefix)
        .await?;

    let file_count = adj.len();
    let edge_count: usize = adj.values().map(std::collections::HashSet::len).sum();
    let density = if file_count > 1 {
        edge_count as f64 / (file_count * (file_count - 1)) as f64
    } else {
        0.0
    };

    // Group files by parent directory
    let mut dir_to_files: HashMap<String, Vec<String>> = HashMap::new();
    for file in adj.keys() {
        let dir = file
            .rfind('/')
            .map_or_else(|| ".".to_string(), |i| file[..i].to_string());
        dir_to_files.entry(dir).or_default().push(file.clone());
    }

    let output = match format {
        "clusters" => {
            // For each dir, compute internal/outgoing/incoming edges
            let mut clusters: Vec<Value> = dir_to_files
                .iter()
                .map(|(dir, files)| {
                    let file_set: HashSet<&str> = files.iter().map(String::as_str).collect();
                    let mut internal = 0usize;
                    let mut outgoing = 0usize;
                    let mut incoming = 0usize;
                    for file in files {
                        if let Some(targets) = adj.get(file) {
                            for tgt in targets {
                                if file_set.contains(tgt.as_str()) {
                                    internal += 1;
                                } else {
                                    outgoing += 1;
                                }
                            }
                        }
                        // Incoming: edges pointing to this file from outside the cluster
                        for (src, targets) in &adj {
                            if !file_set.contains(src.as_str()) && targets.contains(file) {
                                incoming += 1;
                            }
                        }
                    }
                    json!({
                        "directory": dir,
                        "file_count": files.len(),
                        "internal_edges": internal,
                        "outgoing_edges": outgoing,
                        "incoming_edges": incoming,
                    })
                })
                .collect();
            clusters.sort_by_key(|c| std::cmp::Reverse(c["file_count"].as_u64().unwrap_or(0)));
            json!({ "clusters": clusters })
        }
        "matrix" => {
            // Select top max_files by total edge count
            let mut file_edge_counts: Vec<(String, usize)> = adj
                .iter()
                .map(|(f, targets)| {
                    let out = targets.len();
                    let inc = adj.values().filter(|t| t.contains(f)).count();
                    (f.clone(), out + inc)
                })
                .collect();
            file_edge_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            file_edge_counts.truncate(max_files);

            let selected: Vec<String> = file_edge_counts.into_iter().map(|(f, _)| f).collect();
            let _selected_set: HashSet<&str> = selected.iter().map(String::as_str).collect();

            // Build short filenames (last component)
            let short_names: Vec<String> = selected
                .iter()
                .map(|f| {
                    f.rfind('/')
                        .map_or_else(|| f.clone(), |i| f[i + 1..].to_string())
                })
                .collect();

            // Build NxN matrix
            let n = selected.len();
            let mut matrix: Vec<Vec<u8>> = vec![vec![0u8; n]; n];
            for (i, src) in selected.iter().enumerate() {
                if let Some(targets) = adj.get(src) {
                    for (j, tgt) in selected.iter().enumerate() {
                        if i != j && targets.contains(tgt) {
                            matrix[i][j] = 1;
                        }
                    }
                }
            }

            json!({
                "files": short_names,
                "matrix": matrix,
                "note": format!("Top {} files by edge count shown", n),
            })
        }
        _ => {
            // stats (default)
            let largest_cluster = dir_to_files.values().map(Vec::len).max().unwrap_or(0);
            json!({
                "files": file_count,
                "edges": edge_count,
                "density": (density * 10000.0).round() / 10000.0,
                "clusters": dir_to_files.len(),
                "largest_cluster": largest_cluster,
            })
        }
    };

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

struct RiskEntry {
    id: String,
    name: String,
    file: String,
    line: u32,
    complexity: u32,
    fan_in: usize,
    has_test: bool,
    risk: f64,
    churn: usize,
}

/// Handles `tokensave_test_risk` tool calls.
pub(super) async fn handle_test_risk(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |v| v.min(200) as usize);
    let path_prefix = effective_path(&args, scope_prefix);
    let include_tested = args
        .get("include_tested")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let all_nodes = cg.get_all_nodes().await?;
    let all_edges = cg.get_all_edges().await?;

    // Build a map from node_id to file_path for fast lookup
    let node_to_file: HashMap<String, String> = all_nodes
        .iter()
        .map(|n| (n.id.clone(), n.file_path.clone()))
        .collect();

    // Collect all function/method IDs to check for #[test] annotations.
    let fn_ids: Vec<String> = all_nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Method))
        .map(|n| n.id.clone())
        .collect();
    let test_annotated_fns = cg
        .get_test_annotated_node_ids(&fn_ids)
        .await
        .unwrap_or_default();
    let skip_coverage = cg
        .get_skip_test_coverage_node_ids()
        .await
        .unwrap_or_default();

    // Source functions/methods (exclude test files, test-named nodes,
    // #[test]-annotated functions, functions inside #[cfg(test)] modules,
    // and functions marked with `/// skip-test-coverage`).
    let source_fns: Vec<_> = all_nodes
        .iter()
        .filter(|n| {
            matches!(n.kind, NodeKind::Function | NodeKind::Method)
                && !crate::tokensave::is_test_file(&n.file_path)
                && !n.name.starts_with("test_")
                && !n.name.starts_with("test")
                && !n.file_path.contains("/test")
                && !test_annotated_fns.contains(&n.id)
                && !skip_coverage.contains(&n.id)
                && !n.qualified_name.contains("::tests::")
        })
        .filter(|n| {
            path_prefix.is_none_or(|pfx| {
                let with_slash = if pfx.ends_with('/') {
                    pfx.to_string()
                } else {
                    format!("{pfx}/")
                };
                n.file_path.starts_with(&with_slash) || n.file_path == pfx
            })
        })
        .collect();

    // Count fan_in (calls edges targeting each node)
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    for e in &all_edges {
        if e.kind == EdgeKind::Calls {
            *fan_in.entry(e.target.clone()).or_insert(0) += 1;
        }
    }

    // Determine which nodes are tested: called by nodes in test files or
    // by #[test]-annotated functions (inline test modules).
    let call_source_ids: Vec<String> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .map(|e| e.source.clone())
        .collect();
    let test_annotated_callers = cg
        .get_test_annotated_node_ids(&call_source_ids)
        .await
        .unwrap_or_default();
    let mut tested: HashSet<String> = HashSet::new();
    for e in &all_edges {
        if e.kind == EdgeKind::Calls {
            let is_test = node_to_file
                .get(&e.source)
                .is_some_and(|f| crate::tokensave::is_test_file(f))
                || test_annotated_callers.contains(&e.source);
            if is_test {
                tested.insert(e.target.clone());
            }
        }
    }

    let total_functions = source_fns.len();
    let tested_count = source_fns.iter().filter(|n| tested.contains(&n.id)).count();
    let skipped_count = all_nodes
        .iter()
        .filter(|n| {
            matches!(n.kind, NodeKind::Function | NodeKind::Method)
                && skip_coverage.contains(&n.id)
                && !crate::tokensave::is_test_file(&n.file_path)
                && !n.qualified_name.contains("::tests::")
        })
        .count();

    // Compute risk scores
    let mut risks: Vec<RiskEntry> = source_fns
        .iter()
        .map(|n| {
            let complexity = n.branches + n.loops + n.returns + n.max_nesting;
            let fi = *fan_in.get(&n.id).unwrap_or(&0);
            let has_test = tested.contains(&n.id);
            let multiplier = if has_test { 0.1 } else { 1.0 };
            let risk = (f64::from(complexity) + 1.0) * (fi as f64 + 1.0) * multiplier;
            RiskEntry {
                id: n.id.clone(),
                name: n.name.clone(),
                file: n.file_path.clone(),
                line: n.start_line,
                complexity,
                fan_in: fi,
                has_test,
                risk,
                churn: 0,
            }
        })
        .filter(|r| include_tested || !r.has_test)
        .collect();

    // Overlay git churn data: multiply risk by log2(churn + 1) for churned files
    let churn_map = crate::graph::git::file_churn(cg.project_root(), 90)
        .await
        .unwrap_or_default();
    for r in &mut risks {
        let churn = churn_map.get(&r.file).copied().unwrap_or(0);
        r.churn = churn;
        if churn > 0 {
            r.risk *= (churn as f64 + 1.0).log2();
        }
    }

    risks.sort_by(|a, b| {
        b.risk
            .partial_cmp(&a.risk)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let top_risk_untested = risks
        .iter()
        .find(|r| !r.has_test)
        .map(|r| r.name.clone())
        .unwrap_or_default();

    let coverage_pct = if total_functions == 0 {
        0.0
    } else {
        (tested_count as f64 / total_functions as f64 * 100.0).round()
    };

    risks.truncate(limit);

    let risk_items: Vec<Value> = risks
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": r.name,
                "file": r.file,
                "line": r.line,
                "complexity": r.complexity,
                "fan_in": r.fan_in,
                "has_test": r.has_test,
                "risk": (r.risk * 100.0).round() / 100.0,
                "churn": r.churn,
            })
        })
        .collect();

    let output = json!({
        "risks": risk_items,
        "summary": {
            "total_functions": total_functions,
            "tested": tested_count,
            "skipped": skipped_count,
            "coverage_pct": coverage_pct,
            "top_risk_untested": top_risk_untested,
        }
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_test_map` tool calls.
pub(super) async fn handle_test_map(
    cg: &TokenSave,
    args: Value,
    _scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let source_nodes = if let Some(file) = args.get("file").and_then(|v| v.as_str()) {
        cg.get_nodes_by_file(file).await?
    } else if let Some(node_id) = args
        .get("node_id")
        .or(args.get("id"))
        .and_then(|v| v.as_str())
    {
        cg.get_node(node_id).await?.into_iter().collect()
    } else {
        return Err(TokenSaveError::Config {
            message: "provide either 'file' or 'node_id'".to_string(),
        });
    };

    let mut coverage_map: Vec<Value> = Vec::new();
    let mut uncovered: Vec<Value> = Vec::new();
    let mut all_test_files: HashSet<String> = HashSet::new();

    for node in &source_nodes {
        if !matches!(node.kind, NodeKind::Function | NodeKind::Method) {
            continue;
        }

        let callers = cg.get_callers(&node.id, 3).await.unwrap_or_default();
        // Batch-check which callers have #[test] annotations (inline test modules).
        let caller_ids: Vec<String> = callers.iter().map(|(n, _)| n.id.clone()).collect();
        let test_annotated = cg
            .get_test_annotated_node_ids(&caller_ids)
            .await
            .unwrap_or_default();
        let test_callers: Vec<Value> = callers
            .iter()
            .filter(|(n, _)| {
                crate::tokensave::is_test_file(&n.file_path) || test_annotated.contains(&n.id)
            })
            .map(|(n, _)| {
                all_test_files.insert(n.file_path.clone());
                json!({
                    "test_name": n.name,
                    "test_file": n.file_path,
                    "test_line": n.start_line,
                })
            })
            .collect();

        if test_callers.is_empty() {
            uncovered.push(json!({
                "id": node.id,
                "name": node.name,
                "file": node.file_path,
                "line": node.start_line,
            }));
        } else {
            coverage_map.push(json!({
                "source_name": node.name,
                "source_id": node.id,
                "source_file": node.file_path,
                "source_line": node.start_line,
                "tests": test_callers,
            }));
        }
    }

    let mut test_file_list: Vec<String> = all_test_files.into_iter().collect();
    test_file_list.sort();

    let output = json!({
        "covered_symbols": coverage_map.len(),
        "uncovered_symbols": uncovered.len(),
        "test_files": test_file_list,
        "coverage": coverage_map,
        "uncovered": uncovered,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    let touched_files = unique_file_paths(source_nodes.iter().map(|n| n.file_path.as_str()));
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files,
    })
}

// ---------------------------------------------------------------------------
// Session start / end handlers
// ---------------------------------------------------------------------------

/// Handles `tokensave_session_start` tool calls.
pub(super) async fn handle_session_start(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path_prefix = effective_path(&args, scope_prefix);
    let snap = compute_health_snapshot(cg, path_prefix).await?;

    let baseline = json!({
        "quality_signal": snap.quality_signal,
        "files_analyzed": snap.files_analyzed,
        "dimensions": {
            "acyclicity": snap.acyclicity,
            "depth": snap.depth,
            "equality": snap.equality,
            "redundancy": snap.redundancy,
            "modularity": snap.modularity,
            "coverage_discipline": snap.coverage_discipline,
        },
        "timestamp": crate::tokensave::current_timestamp(),
    });

    // Write baseline to .tokensave/session_baseline.json
    let tokensave_dir = crate::config::get_tokensave_dir(cg.project_root());
    std::fs::create_dir_all(&tokensave_dir).map_err(|e| crate::errors::TokenSaveError::Config {
        message: format!("failed to create .tokensave dir: {e}"),
    })?;
    let baseline_path = tokensave_dir.join("session_baseline.json");
    std::fs::write(
        &baseline_path,
        serde_json::to_string_pretty(&baseline).unwrap_or_default(),
    )
    .map_err(|e| crate::errors::TokenSaveError::Config {
        message: format!("failed to write session baseline: {e}"),
    })?;

    let output = json!({
        "status": "baseline_saved",
        "quality_signal": snap.quality_signal,
        "files_analyzed": snap.files_analyzed,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_session_end` tool calls.
pub(super) async fn handle_session_end(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let tokensave_dir = crate::config::get_tokensave_dir(cg.project_root());
    let baseline_path = tokensave_dir.join("session_baseline.json");

    // Check if baseline exists
    if !baseline_path.exists() {
        let output = json!({
            "status": "no_baseline",
            "message": "No session baseline found. Call tokensave_session_start first.",
        });
        let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": truncate_response(&formatted) }]
            }),
            touched_files: vec![],
        });
    }

    // Read baseline
    let baseline_raw = std::fs::read_to_string(&baseline_path).map_err(|e| {
        crate::errors::TokenSaveError::Config {
            message: format!("failed to read session baseline: {e}"),
        }
    })?;
    let baseline: Value =
        serde_json::from_str(&baseline_raw).map_err(|e| crate::errors::TokenSaveError::Config {
            message: format!("failed to parse session baseline: {e}"),
        })?;

    let signal_before = baseline["quality_signal"].as_u64().unwrap_or(0) as u32;
    let dims_before = &baseline["dimensions"];

    // Recompute current health
    let path_prefix = effective_path(&args, scope_prefix);
    let snap = compute_health_snapshot(cg, path_prefix).await?;

    // Remove the baseline file
    let _ = std::fs::remove_file(&baseline_path);

    let signal_after = snap.quality_signal;
    let delta = i64::from(signal_after) - i64::from(signal_before);
    let pass = signal_after >= signal_before;

    // Compute per-dimension deltas
    let dim_names = [
        "acyclicity",
        "depth",
        "equality",
        "redundancy",
        "modularity",
        "coverage_discipline",
    ];
    let after_vals = [
        snap.acyclicity,
        snap.depth,
        snap.equality,
        snap.redundancy,
        snap.modularity,
        snap.coverage_discipline,
    ];

    let mut dimensions = serde_json::Map::new();
    let mut degraded_dimensions: Vec<String> = vec![];

    for (name, after_val) in dim_names.iter().zip(after_vals.iter()) {
        let before_val = dims_before[name].as_f64().unwrap_or(0.0);
        let dim_delta = after_val - before_val;
        let status = if dim_delta > 0.001 {
            "improved"
        } else if dim_delta < -0.001 {
            degraded_dimensions.push((*name).to_string());
            "degraded"
        } else {
            "unchanged"
        };
        dimensions.insert(
            (*name).to_string(),
            json!({
                "before": (before_val * 10000.0).round() / 10000.0,
                "after": (after_val * 10000.0).round() / 10000.0,
                "delta": (dim_delta * 10000.0).round() / 10000.0,
                "status": status,
            }),
        );
    }

    let output = json!({
        "pass": pass,
        "signal_before": signal_before,
        "signal_after": signal_after,
        "delta": delta,
        "files_analyzed": snap.files_analyzed,
        "degraded_dimensions": degraded_dimensions,
        "dimensions": dimensions,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}
