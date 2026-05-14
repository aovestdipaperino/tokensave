//! Graph traversal tool handlers: search, context, callers, callees,
//! impact, node, similar, rename_preview, callers_for, by_qualified_name.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::context::format_context_as_markdown;
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::{BuildContextOptions, EdgeKind, NodeKind, Visibility};

use super::super::ToolResult;
use super::{
    effective_path, filter_by_scope, require_node_id, truncate_response, unique_file_paths,
};

/// Handles `tokensave_search` tool calls.
pub(super) async fn handle_search(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let query =
        args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: query".to_string(),
            })?;

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(500) as usize);

    let results = cg.search(query, limit).await?;
    let results = filter_by_scope(results, scope_prefix, |r| &r.node.file_path);

    let touched_files = unique_file_paths(results.iter().map(|r| r.node.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "id": r.node.id,
                "name": r.node.name,
                "kind": r.node.kind.as_str(),
                "file": r.node.file_path,
                "line": r.node.start_line,
                "signature": r.node.signature,
                "score": r.score,
            })
        })
        .collect();

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_context` tool calls.
pub(super) async fn handle_context(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: task".to_string(),
        })?;

    let max_nodes = args
        .get("max_nodes")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |v| v.min(100) as usize);

    let include_code = args
        .get("include_code")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let max_code_blocks = args
        .get("max_code_blocks")
        .and_then(serde_json::Value::as_u64)
        .map_or(5, |v| v.min(20) as usize);

    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("explore");

    let extra_keywords: Vec<String> = args
        .get("keywords")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let exclude_node_ids: std::collections::HashSet<String> = args
        .get("exclude_node_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let merge_adjacent = args
        .get("merge_adjacent")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let max_per_file: Option<usize> = args
        .get("max_per_file")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize)
        .or(Some((max_nodes / 3).max(3)));

    let path_prefix = effective_path(&args, scope_prefix).map(String::from);

    let options = BuildContextOptions {
        max_nodes,
        max_code_blocks,
        include_code,
        extra_keywords,
        exclude_node_ids,
        merge_adjacent,
        max_per_file,
        path_prefix,
        ..Default::default()
    };

    let context = cg.build_context(task, &options).await?;
    let touched_files = unique_file_paths(
        context
            .subgraph
            .nodes
            .iter()
            .map(|n| n.file_path.as_str())
            .chain(
                context
                    .related_files
                    .iter()
                    .map(std::string::String::as_str),
            ),
    );
    let mut output = format_context_as_markdown(&context);

    // Plan mode: append extension points, test coverage, and dependency info
    if mode == "plan" {
        output.push_str("\n### Extension Points\n");
        let mut found_extension = false;
        for node in &context.subgraph.nodes {
            if matches!(node.kind, NodeKind::Trait | NodeKind::Interface)
                && node.visibility == Visibility::Pub
            {
                let implementors = cg.get_callers(&node.id, 1).await.unwrap_or_default();
                let impl_count = implementors
                    .iter()
                    .filter(|(_, e)| matches!(e.kind, crate::types::EdgeKind::Implements))
                    .count();
                let _ = writeln!(
                    output,
                    "- **{}** ({}) - {}:{} ({} implementors)",
                    node.name,
                    node.kind.as_str(),
                    node.file_path,
                    node.start_line,
                    impl_count,
                );
                found_extension = true;
            }
        }
        if !found_extension {
            output.push_str("_No public traits/interfaces found in context._\n");
        }

        // Test coverage for related files
        let file_paths: Vec<String> = context
            .subgraph
            .nodes
            .iter()
            .map(|n| n.file_path.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if !file_paths.is_empty() {
            output.push_str("\n### Test Coverage\n");
            let mut test_files: HashSet<String> = HashSet::new();
            for file in &file_paths {
                let nodes = cg.get_nodes_by_file(file).await.unwrap_or_default();
                for node in &nodes {
                    let callers = cg.get_callers(&node.id, 2).await.unwrap_or_default();
                    let caller_ids: Vec<String> =
                        callers.iter().map(|(n, _)| n.id.clone()).collect();
                    let test_annotated = cg
                        .get_test_annotated_node_ids(&caller_ids)
                        .await
                        .unwrap_or_default();
                    for (caller, _) in &callers {
                        if crate::tokensave::is_test_file(&caller.file_path)
                            || test_annotated.contains(&caller.id)
                        {
                            test_files.insert(caller.file_path.clone());
                        }
                    }
                }
            }
            if test_files.is_empty() {
                output.push_str("_No test files found covering these modules._\n");
            } else {
                let mut sorted: Vec<_> = test_files.into_iter().collect();
                sorted.sort();
                for tf in &sorted {
                    let _ = writeln!(output, "- {tf}");
                }
            }
        }
    }

    if !context.seen_node_ids.is_empty() {
        let _ = write!(
            output,
            "\nseen_node_ids: {}\n",
            serde_json::to_string(&context.seen_node_ids).unwrap_or_default()
        );
    }

    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_callers` tool calls.
pub(super) async fn handle_callers(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;

    let max_depth = args
        .get("max_depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(3, |v| v.min(10) as usize);

    let results = cg.get_callers(node_id, max_depth).await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, edge)| {
            json!({
                "node_id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "edge_kind": edge.kind.as_str(),
            })
        })
        .collect();

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_callees` tool calls.
pub(super) async fn handle_callees(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;

    let max_depth = args
        .get("max_depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(3, |v| v.min(10) as usize);

    let results = cg.get_callees(node_id, max_depth).await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, edge)| {
            json!({
                "node_id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "edge_kind": edge.kind.as_str(),
            })
        })
        .collect();

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_impact` tool calls.
pub(super) async fn handle_impact(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;

    let max_depth = args
        .get("max_depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(3, |v| v.min(10) as usize);

    let subgraph = cg.get_impact_radius(node_id, max_depth).await?;

    let touched_files = unique_file_paths(subgraph.nodes.iter().map(|n| n.file_path.as_str()));

    let nodes: Vec<Value> = subgraph
        .nodes
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
            })
        })
        .collect();

    let output = json!({
        "node_count": subgraph.nodes.len(),
        "edge_count": subgraph.edges.len(),
        "nodes": nodes,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_node` tool calls.
pub(super) async fn handle_node(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;

    let node = cg.get_node(node_id).await?;

    match node {
        Some(n) => {
            let touched_files = vec![n.file_path.clone()];
            let output = json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "qualified_name": n.qualified_name,
                "file": n.file_path,
                "start_line": n.start_line,
                "end_line": n.end_line,
                "signature": n.signature,
                "docstring": n.docstring,
                "visibility": n.visibility.as_str(),
                "is_async": n.is_async,
                "branches": n.branches,
                "loops": n.loops,
                "returns": n.returns,
                "max_nesting": n.max_nesting,
                "unsafe_blocks": n.unsafe_blocks,
                "unchecked_calls": n.unchecked_calls,
                "assertions": n.assertions,
                "cyclomatic_complexity": n.branches + 1,
            });
            let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
            Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": truncate_response(&formatted) }]
                }),
                touched_files,
            })
        }
        None => Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": format!("Node not found: {}", node_id) }]
            }),
            touched_files: vec![],
        }),
    }
}

/// Handles `tokensave_similar` tool calls.
pub(super) async fn handle_similar(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(
        args.is_object(),
        "handle_similar expects an object argument"
    );
    let symbol =
        args.get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: symbol".to_string(),
            })?;

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    // Use FTS search first
    let mut results = cg.search(symbol, limit).await?;

    // If FTS didn't return enough, supplement with substring matching
    if results.len() < limit {
        let all_nodes = cg.get_all_nodes().await?;
        let lower_symbol = symbol.to_ascii_lowercase();
        let existing_ids: HashSet<String> = results.iter().map(|r| r.node.id.clone()).collect();

        let mut substring_matches: Vec<crate::types::SearchResult> = all_nodes
            .into_iter()
            .filter(|n| {
                !existing_ids.contains(&n.id)
                    && (n.name.to_ascii_lowercase().contains(&lower_symbol)
                        || n.qualified_name
                            .to_ascii_lowercase()
                            .contains(&lower_symbol))
            })
            .map(|n| crate::types::SearchResult {
                node: n,
                score: 0.5,
            })
            .collect();

        substring_matches.truncate(limit.saturating_sub(results.len()));
        results.extend(substring_matches);
    }

    let touched_files = unique_file_paths(results.iter().map(|r| r.node.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "id": r.node.id,
                "name": r.node.name,
                "kind": r.node.kind.as_str(),
                "file": r.node.file_path,
                "line": r.node.start_line,
                "signature": r.node.signature,
                "score": r.score,
            })
        })
        .collect();

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_rename_preview` tool calls.
pub(super) async fn handle_rename_preview(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;

    // Get the node itself
    let node = cg.get_node(node_id).await?;
    let node_info = match &node {
        Some(n) => json!({
            "id": n.id,
            "name": n.name,
            "kind": n.kind.as_str(),
            "file": n.file_path,
            "line": n.start_line,
        }),
        None => {
            return Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": format!("Node not found: {}", node_id) }]
                }),
                touched_files: vec![],
            });
        }
    };

    // Get all edges referencing this node
    let incoming = cg.get_incoming_edges(node_id).await?;
    let outgoing = cg.get_outgoing_edges(node_id).await?;

    let mut references: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();

    if let Some(ref n) = node {
        touched.push(n.file_path.clone());
    }

    // Incoming edges: other nodes that reference this node
    for edge in &incoming {
        if let Some(source_node) = cg.get_node(&edge.source).await? {
            touched.push(source_node.file_path.clone());
            references.push(json!({
                "direction": "incoming",
                "node_id": source_node.id,
                "name": source_node.name,
                "kind": source_node.kind.as_str(),
                "file": source_node.file_path,
                "line": source_node.start_line,
                "edge_kind": edge.kind.as_str(),
                "edge_line": edge.line,
            }));
        }
    }

    // Outgoing edges: nodes this node references
    for edge in &outgoing {
        if let Some(target_node) = cg.get_node(&edge.target).await? {
            touched.push(target_node.file_path.clone());
            references.push(json!({
                "direction": "outgoing",
                "node_id": target_node.id,
                "name": target_node.name,
                "kind": target_node.kind.as_str(),
                "file": target_node.file_path,
                "line": target_node.start_line,
                "edge_kind": edge.kind.as_str(),
                "edge_line": edge.line,
            }));
        }
    }

    let touched_files = unique_file_paths(touched.iter().map(std::string::String::as_str));

    let output = json!({
        "node": node_info,
        "reference_count": references.len(),
        "references": references,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_callers_for` tool calls — bulk caller lookup over many IDs.
pub(super) async fn handle_callers_for(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_ids: Vec<String> = args
        .get("node_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    if node_ids.is_empty() {
        return Err(TokenSaveError::Config {
            message: "callers_for requires non-empty node_ids".to_string(),
        });
    }

    // Default to "calls" but allow any kind (or empty string for all kinds).
    let kind_arg = args.get("kind").and_then(|v| v.as_str()).unwrap_or("calls");
    let kinds: Vec<EdgeKind> = if kind_arg.is_empty() {
        Vec::new()
    } else {
        match EdgeKind::from_str(kind_arg) {
            Some(k) => vec![k],
            None => {
                return Err(TokenSaveError::Config {
                    message: format!("unknown edge kind: {kind_arg}"),
                });
            }
        }
    };

    let max_per_item = args
        .get("max_per_item")
        .and_then(serde_json::Value::as_u64)
        .map_or(1000usize, |v| v.min(10_000) as usize);

    let edges = cg.get_incoming_edges_bulk(&node_ids, &kinds).await?;

    // Group source IDs by target. Cap each list at max_per_item.
    let mut by_target: HashMap<String, Vec<String>> = HashMap::new();
    let mut truncated = false;
    for edge in edges {
        let entry = by_target.entry(edge.target).or_default();
        if entry.len() < max_per_item {
            entry.push(edge.source);
        } else {
            truncated = true;
        }
    }

    // Ensure every requested ID appears in the response, even if no callers.
    let result_map: HashMap<&String, Vec<String>> = node_ids
        .iter()
        .map(|id| (id, by_target.remove(id).unwrap_or_default()))
        .collect();

    let output = json!({
        "callers": result_map,
        "truncated": truncated,
        "max_per_item": max_per_item,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_by_qualified_name` — cross-run node lookup by name.
pub(super) async fn handle_by_qualified_name(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let qname = args
        .get("qualified_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: qualified_name".to_string(),
        })?;

    let nodes = cg.get_nodes_by_qualified_name(qname).await?;
    let touched_files = unique_file_paths(nodes.iter().map(|n| n.file_path.as_str()));

    let items: Vec<Value> = nodes
        .iter()
        .map(|n| {
            json!({
                "node_id": n.id,
                "name": n.name,
                "qualified_name": n.qualified_name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "start_line": n.start_line,
                "attrs_start_line": n.attrs_start_line,
                "end_line": n.end_line,
            })
        })
        .collect();

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}
