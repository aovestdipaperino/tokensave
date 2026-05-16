//! Graph traversal tool handlers: `search`, `context`, `callers`, `callees`,
//! `impact`, `node`, `similar`, `rename_preview`, `callers_for`, `by_qualified_name`,
//! `signature`.

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
///
/// Beyond the direct `Calls` edges, this handler also surfaces *trait
/// dispatch targets*: when a callee is a method whose enclosing scope is a
/// trait, the concrete impl methods reachable through that trait are added
/// to the result list and tagged with `dispatch_via_trait: true`. The
/// original trait-method entry is preserved so callers can still see what
/// they statically called.
///
/// Dispatch resolution skipped when `resolve_dispatch=false` is passed.
pub(super) async fn handle_callees(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = require_node_id(&args)?;

    let max_depth = args
        .get("max_depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(3, |v| v.min(10) as usize);

    let resolve_dispatch = args
        .get("resolve_dispatch")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    let results = cg.get_callees(node_id, max_depth).await?;
    let mut seen: HashSet<String> = results.iter().map(|(n, _)| n.id.clone()).collect();

    let mut items: Vec<Value> = results
        .iter()
        .map(|(node, edge)| {
            json!({
                "node_id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "edge_kind": edge.kind.as_str(),
                "dispatch_via_trait": false,
            })
        })
        .collect();

    if resolve_dispatch {
        for (callee, _) in &results {
            let impls = cg.get_trait_dispatch_targets(callee).await?;
            for impl_method in impls {
                if !seen.insert(impl_method.id.clone()) {
                    continue;
                }
                items.push(json!({
                    "node_id": impl_method.id,
                    "name": impl_method.name,
                    "kind": impl_method.kind.as_str(),
                    "file": impl_method.file_path,
                    "line": impl_method.start_line,
                    "edge_kind": "calls",
                    "dispatch_via_trait": true,
                    "dispatch_from": callee.id.clone(),
                }));
            }
        }
    }

    let touched_files = unique_file_paths(
        items
            .iter()
            .filter_map(|v| v.get("file").and_then(Value::as_str)),
    );

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
            let file_size_bytes = cg.get_file_size_bytes(&n.file_path).await;
            // For type-kind nodes, also surface the `#[derive(...)]` macros
            // attached. Costs one extra edge query per node lookup; skipped
            // for non-type kinds where derives never apply.
            let derives: Vec<Value> = if matches!(
                n.kind,
                NodeKind::Struct
                    | NodeKind::Enum
                    | NodeKind::Union
                    | NodeKind::CaseClass
                    | NodeKind::DataClass
                    | NodeKind::Record
                    | NodeKind::PascalRecord
            ) {
                cg.get_derives_for_node(&n.id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|name| {
                        let look = crate::derive_table::enrich(&name);
                        json!({
                            "derive": look.derive_name,
                            "trait": look.known.as_ref().map(|k| k.trait_path),
                            "methods": look.known.as_ref().map(|k| k.methods.to_vec()),
                            "well_known": look.known.is_some(),
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
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
                "cost_to_expand": cost_to_expand(&n, file_size_bytes),
                "derives": derives,
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

/// Handles `tokensave_signature` — signature-only lookup (no body) by
/// qualified name or node ID. Returns the public-API surface of a symbol so
/// callers can avoid reading the source file just to inspect the signature.
pub(super) async fn handle_signature(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let qname = args.get("qualified_name").and_then(|v| v.as_str());
    let node_id = args
        .get("node_id")
        .or_else(|| args.get("id"))
        .and_then(|v| v.as_str());

    if qname.is_none() && node_id.is_none() {
        return Err(TokenSaveError::Config {
            message: "missing required parameter: qualified_name or node_id".to_string(),
        });
    }

    let nodes = if let Some(id) = node_id {
        match cg.get_node(id).await? {
            Some(n) => vec![n],
            None => vec![],
        }
    } else if let Some(q) = qname {
        cg.get_nodes_by_qualified_name(q).await?
    } else {
        vec![]
    };

    let touched_files = unique_file_paths(nodes.iter().map(|n| n.file_path.as_str()));

    let mut items: Vec<Value> = Vec::with_capacity(nodes.len());
    for n in &nodes {
        let file_size_bytes = cg.get_file_size_bytes(&n.file_path).await;
        items.push(json!({
            "node_id": n.id,
            "name": n.name,
            "qualified_name": n.qualified_name,
            "kind": n.kind.as_str(),
            "visibility": n.visibility.as_str(),
            "is_async": n.is_async,
            "signature": n.signature,
            "docstring": n.docstring,
            "file": n.file_path,
            "start_line": n.start_line,
            "attrs_start_line": n.attrs_start_line,
            "end_line": n.end_line,
            "cost_to_expand": cost_to_expand(n, file_size_bytes),
        }));
    }

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_impls` — index of `impl Trait for Type` blocks.
///
/// Both `trait` and `type` arguments are optional. With neither, every impl
/// in the graph is returned (capped by `limit`). Surfaces trait-dispatch
/// information that is otherwise hidden behind raw `Implements` edges.
pub(super) async fn handle_impls(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let trait_filter = args.get("trait").and_then(|v| v.as_str());
    let type_filter = args.get("type").and_then(|v| v.as_str());
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(100, |v| v.min(1000) as usize);

    let mut results = cg.get_impls(trait_filter, type_filter).await?;
    let truncated = results.len() > limit;
    results.truncate(limit);

    let touched_files = unique_file_paths(
        results
            .iter()
            .map(|(impl_node, _)| impl_node.file_path.as_str()),
    );

    let items: Vec<Value> = results
        .iter()
        .map(|(impl_node, trait_node)| {
            json!({
                "impl_id": impl_node.id,
                "type": impl_node.name,
                "qualified_name": impl_node.qualified_name,
                "trait": trait_node.as_ref().map(|t| t.name.clone()),
                "trait_qualified_name": trait_node.as_ref().map(|t| t.qualified_name.clone()),
                "trait_id": trait_node.as_ref().map(|t| t.id.clone()),
                "file": impl_node.file_path,
                "start_line": impl_node.start_line,
                "end_line": impl_node.end_line,
                "signature": impl_node.signature,
            })
        })
        .collect();

    let output = json!({
        "count": items.len(),
        "truncated": truncated,
        "impls": items,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_derives` — lists `#[derive(...)]` macros on a type
/// and the trait + method names each one synthesizes (per the static
/// `derive_table`). Accepts either `node_id` or `qualified_name`.
pub(super) async fn handle_derives(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let qname = args.get("qualified_name").and_then(|v| v.as_str());
    let node_id = args
        .get("node_id")
        .or_else(|| args.get("id"))
        .and_then(|v| v.as_str());
    if qname.is_none() && node_id.is_none() {
        return Err(TokenSaveError::Config {
            message: "missing required parameter: qualified_name or node_id".to_string(),
        });
    }

    let nodes = if let Some(id) = node_id {
        match cg.get_node(id).await? {
            Some(n) => vec![n],
            None => vec![],
        }
    } else if let Some(q) = qname {
        cg.get_nodes_by_qualified_name(q).await?
    } else {
        vec![]
    };

    let touched_files = unique_file_paths(nodes.iter().map(|n| n.file_path.as_str()));

    let mut items: Vec<Value> = Vec::with_capacity(nodes.len());
    for n in &nodes {
        let derive_names = cg.get_derives_for_node(&n.id).await?;
        let derives: Vec<Value> = derive_names
            .iter()
            .map(|name| {
                let look = crate::derive_table::enrich(name);
                json!({
                    "derive": look.derive_name,
                    "trait": look.known.as_ref().map(|k| k.trait_path),
                    "methods": look.known.as_ref().map(|k| k.methods.to_vec()),
                    "source": look.known.as_ref().map(|k| k.source),
                    "well_known": look.known.is_some(),
                })
            })
            .collect();
        items.push(json!({
            "node_id": n.id,
            "name": n.name,
            "kind": n.kind.as_str(),
            "qualified_name": n.qualified_name,
            "file": n.file_path,
            "start_line": n.start_line,
            "derives": derives,
        }));
    }

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Approximate token cost of expanding a node's body and its full file.
///
/// `body` uses ~20 tokens/line (≈80 chars/line at 4 chars/token), tuned for
/// Rust source — denser languages like Haskell or Python will be over-estimated
/// by ~2-3x and ultra-terse declarations (one-line `use`, single-line `pub fn`)
/// resolve to the single-line floor of 20 tokens. Good enough to decide whether
/// to set `include_code=true`; not a reliable absolute count.
/// `full_file` uses `size_bytes / 4` from the indexed `files.size`.
pub(super) fn cost_to_expand(node: &crate::types::Node, file_size_bytes: u64) -> Value {
    let line_count = node
        .end_line
        .saturating_sub(node.start_line)
        .saturating_add(1);
    let body_tokens = u64::from(line_count) * 20;
    let full_file_tokens = file_size_bytes / 4;
    json!({
        "body": body_tokens,
        "full_file": full_file_tokens,
    })
}

/// Handles `tokensave_implementations` — trait / method implementor lookup.
pub(super) async fn handle_implementations(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let trait_name = args.get("trait").and_then(|v| v.as_str());
    let method_name = args.get("method").and_then(|v| v.as_str());

    if trait_name.is_none() && method_name.is_none() {
        return Err(TokenSaveError::Config {
            message: "tokensave_implementations requires either 'trait' or 'method'".to_string(),
        });
    }
    if trait_name.is_some() && method_name.is_some() {
        return Err(TokenSaveError::Config {
            message: "tokensave_implementations: 'trait' and 'method' are mutually exclusive"
                .to_string(),
        });
    }

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |v| v.clamp(1, 200) as usize);

    let project_root = cg.project_root().to_path_buf();
    let mut entries: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();

    if let Some(name) = trait_name {
        let candidates = cg
            .db()
            .search_nodes_by_exact_name(&[name.to_string()], 50)
            .await?;
        let trait_nodes: Vec<&crate::types::Node> = candidates
            .iter()
            .filter(|n| {
                matches!(
                    n.kind,
                    NodeKind::Trait | NodeKind::Interface | NodeKind::InterfaceType
                )
            })
            .collect();
        if trait_nodes.is_empty() {
            return Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": format!("No trait or interface named '{name}' found.") }]
                }),
                touched_files: vec![],
            });
        }

        for trait_node in trait_nodes {
            let implementors = cg
                .db()
                .get_incoming_edges(&trait_node.id, &[EdgeKind::Implements])
                .await?;
            for edge in implementors {
                let Some(impl_node) = cg.db().get_node_by_id(&edge.source).await? else {
                    continue;
                };
                if scope_prefix.is_some_and(|p| !impl_node.file_path.starts_with(p)) {
                    continue;
                }
                let methods = collect_method_bodies(cg, &impl_node, &project_root).await?;
                if !touched.contains(&impl_node.file_path) {
                    touched.push(impl_node.file_path.clone());
                }
                entries.push(json!({
                    "type": impl_node.name,
                    "qualified_name": impl_node.qualified_name,
                    "kind": impl_node.kind.as_str(),
                    "file": impl_node.file_path,
                    "line": impl_node.start_line,
                    "trait": trait_node.qualified_name,
                    "methods": methods,
                }));
                if entries.len() >= limit {
                    break;
                }
            }
            if entries.len() >= limit {
                break;
            }
        }
    } else if let Some(name) = method_name {
        let nodes = cg
            .db()
            .search_nodes_by_exact_name(&[name.to_string()], limit * 4)
            .await?;
        let method_nodes: Vec<&crate::types::Node> = nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Method))
            .filter(|n| scope_prefix.is_none_or(|p| n.file_path.starts_with(p)))
            .take(limit)
            .collect();
        if method_nodes.is_empty() {
            return Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": format!("No function or method named '{name}' found.") }]
                }),
                touched_files: vec![],
            });
        }
        for n in method_nodes {
            let abs_path = project_root.join(&n.file_path);
            let body = match crate::sync::read_source_file(&abs_path) {
                Ok(source) => super::info::extract_lines(&source, n.start_line, n.end_line),
                Err(_) => String::from("<file unreadable>"),
            };
            if !touched.contains(&n.file_path) {
                touched.push(n.file_path.clone());
            }
            entries.push(json!({
                "name": n.name,
                "qualified_name": n.qualified_name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
                "end_line": n.end_line,
                "signature": n.signature,
                "body": body,
            }));
        }
    }

    let payload = json!({
        "match_count": entries.len(),
        "implementations": entries,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();

    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

async fn collect_method_bodies(
    cg: &TokenSave,
    impl_node: &crate::types::Node,
    project_root: &std::path::Path,
) -> Result<Vec<Value>> {
    let children = cg.db().get_children_of(&impl_node.id).await?;
    let mut out: Vec<Value> = Vec::new();
    for child in children {
        if !matches!(child.kind, NodeKind::Method | NodeKind::Function) {
            continue;
        }
        let abs_path = project_root.join(&child.file_path);
        let body = match crate::sync::read_source_file(&abs_path) {
            Ok(source) => super::info::extract_lines(&source, child.start_line, child.end_line),
            Err(_) => String::from("<file unreadable>"),
        };
        out.push(json!({
            "name": child.name,
            "kind": child.kind.as_str(),
            "line": child.start_line,
            "signature": child.signature,
            "body": body,
        }));
    }
    Ok(out)
}
