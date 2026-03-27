//! MCP tool call handlers.
//!
//! Each `handle_*` function implements one MCP tool: it deserializes
//! the JSON arguments, calls the appropriate `TokenSave` method, and
//! formats the result.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::tokensave::TokenSave;
use crate::context::format_context_as_markdown;
use crate::errors::{TokenSaveError, Result};
use crate::types::{BuildContextOptions, NodeKind, Visibility};

use super::{ToolResult, MAX_RESPONSE_CHARS};

/// Dispatches a tool call to the appropriate handler.
///
/// Returns the tool result and touched file paths, or an error if the tool
/// name is unknown or the handler fails. The optional `server_stats` value
/// is included in `tokensave_status` responses when provided.
pub async fn handle_tool_call(
    cg: &TokenSave,
    tool_name: &str,
    args: Value,
    server_stats: Option<Value>,
) -> Result<ToolResult> {
    debug_assert!(!tool_name.is_empty(), "handle_tool_call called with empty tool_name");
    debug_assert!(tool_name.starts_with("tokensave_"), "tool_name must start with 'tokensave_' prefix");
    match tool_name {
        "tokensave_search" => handle_search(cg, args).await,
        "tokensave_context" => handle_context(cg, args).await,
        "tokensave_callers" => handle_callers(cg, args).await,
        "tokensave_callees" => handle_callees(cg, args).await,
        "tokensave_impact" => handle_impact(cg, args).await,
        "tokensave_node" => handle_node(cg, args).await,
        "tokensave_status" => handle_status(cg, server_stats).await,
        "tokensave_files" => handle_files(cg, args).await,
        "tokensave_affected" => handle_affected(cg, args).await,
        "tokensave_dead_code" => handle_dead_code(cg, args).await,
        "tokensave_diff_context" => handle_diff_context(cg, args).await,
        "tokensave_module_api" => handle_module_api(cg, args).await,
        "tokensave_circular" => handle_circular(cg, args).await,
        "tokensave_hotspots" => handle_hotspots(cg, args).await,
        "tokensave_similar" => handle_similar(cg, args).await,
        "tokensave_rename_preview" => handle_rename_preview(cg, args).await,
        "tokensave_unused_imports" => handle_unused_imports(cg, args).await,
        "tokensave_rank" => handle_rank(cg, args).await,
        "tokensave_largest" => handle_largest(cg, args).await,
        "tokensave_coupling" => handle_coupling(cg, args).await,
        "tokensave_inheritance_depth" => handle_inheritance_depth(cg, args).await,
        "tokensave_distribution" => handle_distribution(cg, args).await,
        "tokensave_recursion" => handle_recursion(cg, args).await,
        "tokensave_complexity" => handle_complexity(cg, args).await,
        "tokensave_doc_coverage" => handle_doc_coverage(cg, args).await,
        "tokensave_god_class" => handle_god_class(cg, args).await,
        "tokensave_changelog" => handle_changelog(cg, args).await,
        "tokensave_port_status" => handle_port_status(cg, args).await,
        "tokensave_port_order" => handle_port_order(cg, args).await,
        _ => Err(TokenSaveError::Config {
            message: format!("unknown tool: {}", tool_name),
        }),
    }
}

/// Deduplicates an iterator of file path strings into a `Vec<String>`.
fn unique_file_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for p in paths {
        if seen.insert(p) {
            result.push(p.to_string());
        }
    }
    result
}

/// Truncates a string to the maximum response character limit, appending
/// a truncation notice if necessary.
fn truncate_response(s: &str) -> String {
    debug_assert!(!s.is_empty(), "truncate_response called with empty string");
    if s.len() <= MAX_RESPONSE_CHARS {
        s.to_string()
    } else {
        // Find a valid UTF-8 character boundary at or before MAX_RESPONSE_CHARS
        let mut end = MAX_RESPONSE_CHARS;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}\n\n[... truncated at {} chars]", &s[..end], end)
    }
}

/// Handles `tokensave_search` tool calls.
async fn handle_search(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let query =
        args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: query".to_string(),
            })?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(500) as usize)
        .unwrap_or(10);

    let results = cg.search(query, limit).await?;

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
async fn handle_context(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: task".to_string(),
        })?;

    let max_nodes = args
        .get("max_nodes")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(20);

    let options = BuildContextOptions {
        max_nodes,
        ..Default::default()
    };

    let context = cg.build_context(task, &options).await?;
    let touched_files = unique_file_paths(
        context
            .subgraph
            .nodes
            .iter()
            .map(|n| n.file_path.as_str())
            .chain(context.related_files.iter().map(|s| s.as_str())),
    );
    let output = format_context_as_markdown(&context);

    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_callers` tool calls.
async fn handle_callers(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(10) as usize)
        .unwrap_or(3);

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
async fn handle_callees(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(10) as usize)
        .unwrap_or(3);

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
async fn handle_impact(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(10) as usize)
        .unwrap_or(3);

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
async fn handle_node(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

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

/// Handles `tokensave_status` tool calls.
async fn handle_status(cg: &TokenSave, server_stats: Option<Value>) -> Result<ToolResult> {
    let stats = cg.get_stats().await?;
    let mut output: Value = serde_json::to_value(&stats).unwrap_or(json!({}));
    if let Some(ss) = server_stats {
        output["server"] = ss;
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
    let sample_paths: Vec<String> = all_files
        .iter()
        .take(100)
        .map(|f| f.path.clone())
        .collect();
    let stale_files = cg.check_file_staleness(&sample_paths).await;
    if !stale_files.is_empty() {
        output["stale_files"] = json!(stale_files.len());
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
async fn handle_files(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_files expects an object argument");
    let mut files = cg.get_all_files().await?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    // Apply directory prefix filter
    if let Some(dir) = args.get("path").and_then(|v| v.as_str()) {
        let prefix = if dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{}/", dir)
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
            let dir = f
                .path
                .rfind('/')
                .map(|i| &f.path[..i])
                .unwrap_or(".")
                .to_string();
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
                lines.push(format!("  {}", entry));
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

/// Handles `tokensave_affected` tool calls.
async fn handle_affected(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: files (array of strings)".to_string(),
        })?;

    let max_depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(10) as usize)
        .unwrap_or(5);

    let custom_filter = args.get("filter").and_then(|v| v.as_str());
    let custom_glob = custom_filter.and_then(|p| glob::Pattern::new(p).ok());

    let matches_test = |path: &str| -> bool {
        if let Some(ref g) = custom_glob {
            g.matches(path)
        } else {
            crate::tokensave::is_test_file(path)
        }
    };

    let mut affected: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();

    for file in &files {
        if matches_test(file) {
            affected.insert(file.clone());
        }
        if visited.insert(file.clone()) {
            queue.push_back((file.clone(), 0));
        }
    }

    while let Some((file, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let dependents = cg.get_file_dependents(&file).await?;
        for dep in dependents {
            if !visited.insert(dep.clone()) {
                continue;
            }
            if matches_test(&dep) {
                affected.insert(dep.clone());
            } else {
                queue.push_back((dep, depth + 1));
            }
        }
    }

    let mut result: Vec<String> = affected.into_iter().collect();
    result.sort();

    let touched_files = result.clone();
    let output = json!({
        "changed_files": files,
        "affected_tests": result,
        "count": result.len(),
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_dead_code` tool calls.
async fn handle_dead_code(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let kinds: Vec<NodeKind> = args
        .get("kinds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(NodeKind::from_str))
                .collect()
        })
        .unwrap_or_else(|| vec![NodeKind::Function, NodeKind::Method]);

    let dead = cg.find_dead_code(&kinds).await?;

    let touched_files = unique_file_paths(dead.iter().map(|n| n.file_path.as_str()));

    let items: Vec<Value> = dead
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
                "signature": n.signature,
            })
        })
        .collect();

    let output = json!({
        "dead_code_count": items.len(),
        "symbols": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_diff_context` tool calls.
async fn handle_diff_context(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_diff_context expects an object argument");
    let files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: files (array of strings)".to_string(),
        })?;

    let depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(10) as usize)
        .unwrap_or(2);

    let mut modified_symbols: Vec<Value> = Vec::new();
    let mut impacted_symbols: Vec<Value> = Vec::new();
    let mut affected_tests: HashSet<String> = HashSet::new();
    let mut all_touched_files: Vec<String> = Vec::new();

    for file in &files {
        let nodes = cg.get_nodes_by_file(file).await?;
        for node in &nodes {
            all_touched_files.push(node.file_path.clone());
            modified_symbols.push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
            }));

            // Get impact radius for each modified symbol
            let impact = cg.get_impact_radius(&node.id, depth).await?;
            for impacted in &impact.nodes {
                if impacted.id != node.id {
                    impacted_symbols.push(json!({
                        "id": impacted.id,
                        "name": impacted.name,
                        "kind": impacted.kind.as_str(),
                        "file": impacted.file_path,
                        "line": impacted.start_line,
                    }));
                    if crate::tokensave::is_test_file(&impacted.file_path) {
                        affected_tests.insert(impacted.file_path.clone());
                    }
                }
            }
        }
    }

    // Also run affected-tests BFS at file level
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
    for file in &files {
        if crate::tokensave::is_test_file(file) {
            affected_tests.insert(file.clone());
        }
        if visited.insert(file.clone()) {
            queue.push_back((file.clone(), 0));
        }
    }
    while let Some((file, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        let dependents = cg.get_file_dependents(&file).await?;
        for dep in dependents {
            if !visited.insert(dep.clone()) {
                continue;
            }
            if crate::tokensave::is_test_file(&dep) {
                affected_tests.insert(dep.clone());
            } else {
                queue.push_back((dep, d + 1));
            }
        }
    }

    let mut tests_sorted: Vec<String> = affected_tests.into_iter().collect();
    tests_sorted.sort();

    let touched_files = unique_file_paths(
        all_touched_files.iter().map(|s| s.as_str()).chain(files.iter().map(|s| s.as_str())),
    );

    let output = json!({
        "changed_files": files,
        "modified_symbols": modified_symbols,
        "impacted_symbols_count": impacted_symbols.len(),
        "impacted_symbols": impacted_symbols,
        "affected_tests": tests_sorted,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_module_api` tool calls.
async fn handle_module_api(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;

    let all_nodes = cg.get_all_nodes().await?;

    // Filter to nodes in matching files (exact path or directory prefix)
    let prefix = if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{}/", path)
    };

    let mut pub_nodes: Vec<&crate::types::Node> = all_nodes
        .iter()
        .filter(|n| {
            n.visibility == Visibility::Pub
                && (n.file_path == path || n.file_path.starts_with(&prefix))
        })
        .collect();

    pub_nodes.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.start_line.cmp(&b.start_line))
    });

    let touched_files = unique_file_paths(pub_nodes.iter().map(|n| n.file_path.as_str()));

    let items: Vec<Value> = pub_nodes
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
                "signature": n.signature,
            })
        })
        .collect();

    let output = json!({
        "path": path,
        "public_symbol_count": items.len(),
        "symbols": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_circular` tool calls.
async fn handle_circular(cg: &TokenSave, _args: Value) -> Result<ToolResult> {
    let cycles = cg.find_circular_dependencies().await?;

    let items: Vec<Value> = cycles
        .iter()
        .map(|cycle| json!(cycle))
        .collect();

    let output = json!({
        "cycle_count": cycles.len(),
        "cycles": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_hotspots` tool calls.
async fn handle_hotspots(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);
    debug_assert!(limit > 0, "handle_hotspots limit must be positive");

    let all_edges = cg.get_all_edges().await?;

    // Count incoming + outgoing edges per node
    let mut connectivity: HashMap<String, (usize, usize)> = HashMap::new();
    for edge in &all_edges {
        connectivity
            .entry(edge.source.clone())
            .or_insert((0, 0))
            .1 += 1; // outgoing
        connectivity
            .entry(edge.target.clone())
            .or_insert((0, 0))
            .0 += 1; // incoming
    }

    // Sort by total connectivity descending
    let mut sorted: Vec<(String, usize, usize)> = connectivity
        .into_iter()
        .map(|(id, (inc, out))| (id, inc, out))
        .collect();
    sorted.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
    sorted.truncate(limit);

    // Resolve node details
    let mut items: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    for (node_id, incoming, outgoing) in &sorted {
        if let Some(node) = cg.get_node(node_id).await? {
            touched.push(node.file_path.clone());
            items.push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "incoming": incoming,
                "outgoing": outgoing,
                "total": incoming + outgoing,
            }));
        }
    }

    let touched_files = unique_file_paths(touched.iter().map(|s| s.as_str()));

    let output = json!({
        "hotspot_count": items.len(),
        "hotspots": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_similar` tool calls.
async fn handle_similar(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_similar expects an object argument");
    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: symbol".to_string(),
        })?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

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
                        || n.qualified_name.to_ascii_lowercase().contains(&lower_symbol))
            })
            .map(|n| crate::types::SearchResult { node: n, score: 0.5 })
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
async fn handle_rename_preview(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

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

    let touched_files = unique_file_paths(touched.iter().map(|s| s.as_str()));

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

/// Handles `tokensave_unused_imports` tool calls.
async fn handle_unused_imports(cg: &TokenSave, _args: Value) -> Result<ToolResult> {
    let all_nodes = cg.get_all_nodes().await?;

    // Find all Use nodes
    let use_nodes: Vec<&crate::types::Node> = all_nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();

    let mut unused: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();

    for use_node in &use_nodes {
        // Check if this use node has any outgoing edges (it references something)
        // or if any other node references it via incoming edges
        let incoming = cg.get_incoming_edges(&use_node.id).await?;
        let outgoing = cg.get_outgoing_edges(&use_node.id).await?;

        // A use node is "unused" if nothing references it (no incoming edges)
        // and it doesn't create any connections (no outgoing edges beyond contains)
        let has_meaningful_outgoing = outgoing.iter().any(|e| {
            e.kind != crate::types::EdgeKind::Contains
        });

        if incoming.is_empty() && !has_meaningful_outgoing {
            touched.push(use_node.file_path.clone());
            unused.push(json!({
                "id": use_node.id,
                "name": use_node.name,
                "file": use_node.file_path,
                "line": use_node.start_line,
            }));
        }
    }

    let touched_files = unique_file_paths(touched.iter().map(|s| s.as_str()));

    let output = json!({
        "unused_import_count": unused.len(),
        "imports": unused,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_rank` tool calls.
async fn handle_rank(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_rank expects an object argument");
    use crate::types::EdgeKind;

    let edge_kind_str =
        args.get("edge_kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: edge_kind".to_string(),
            })?;

    let edge_kind = EdgeKind::from_str(edge_kind_str).ok_or_else(|| TokenSaveError::Config {
        message: format!(
            "invalid edge_kind '{}'. Valid values: implements, extends, calls, uses, contains, annotates, derives_macro",
            edge_kind_str
        ),
    })?;

    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("incoming");

    let incoming = match direction {
        "incoming" => true,
        "outgoing" => false,
        _ => {
            return Err(TokenSaveError::Config {
                message: format!("invalid direction '{}'. Valid values: incoming, outgoing", direction),
            });
        }
    };

    let node_kind = args
        .get("node_kind")
        .and_then(|v| v.as_str())
        .and_then(|s| NodeKind::from_str(s));

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

    let results = cg
        .get_ranked_nodes_by_edge_kind(&edge_kind, node_kind.as_ref(), incoming, limit)
        .await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, count)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "count": count,
            })
        })
        .collect();

    let output = json!({
        "edge_kind": edge_kind_str,
        "direction": direction,
        "node_kind_filter": args.get("node_kind").and_then(|v| v.as_str()),
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_largest` tool calls.
async fn handle_largest(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_kind = args
        .get("node_kind")
        .and_then(|v| v.as_str())
        .and_then(|s| NodeKind::from_str(s));

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

    let results = cg.get_largest_nodes(node_kind.as_ref(), limit).await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, lines)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "start_line": node.start_line,
                "end_line": node.end_line,
                "lines": lines,
            })
        })
        .collect();

    let output = json!({
        "node_kind_filter": args.get("node_kind").and_then(|v| v.as_str()),
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_coupling` tool calls.
async fn handle_coupling(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("fan_in");

    let fan_in = match direction {
        "fan_in" => true,
        "fan_out" => false,
        _ => {
            return Err(TokenSaveError::Config {
                message: format!(
                    "invalid direction '{}'. Valid values: fan_in, fan_out",
                    direction
                ),
            });
        }
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

    let results = cg.get_file_coupling(fan_in, limit).await?;

    let items: Vec<Value> = results
        .iter()
        .map(|(file, count)| {
            json!({
                "file": file,
                "coupled_files": count,
            })
        })
        .collect();

    let output = json!({
        "direction": direction,
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_inheritance_depth` tool calls.
async fn handle_inheritance_depth(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

    let results = cg.get_inheritance_depth(limit).await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, depth)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "depth": depth,
            })
        })
        .collect();

    let output = json!({
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_distribution` tool calls.
async fn handle_distribution(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_distribution expects an object argument");
    let path_prefix = args.get("path").and_then(|v| v.as_str());
    let summary = args
        .get("summary")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let results = cg.get_node_distribution(path_prefix).await?;

    let output = if summary {
        // Aggregate counts across all files
        let mut totals: HashMap<String, u64> = HashMap::new();
        for (_file, kind, count) in &results {
            *totals.entry(kind.clone()).or_insert(0) += count;
        }
        let mut sorted: Vec<(String, u64)> = totals.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        let items: Vec<Value> = sorted
            .iter()
            .map(|(kind, count)| json!({ "kind": kind, "count": count }))
            .collect();

        json!({
            "path_filter": path_prefix,
            "mode": "summary",
            "total_kinds": items.len(),
            "distribution": items,
        })
    } else {
        // Per-file breakdown, grouped by file
        let mut by_file: Vec<(String, Vec<Value>)> = Vec::new();
        let mut current_file = String::new();
        for (file, kind, count) in &results {
            if *file != current_file {
                current_file = file.clone();
                by_file.push((file.clone(), Vec::new()));
            }
            if let Some(last) = by_file.last_mut() {
                last.1.push(json!({ "kind": kind, "count": count }));
            }
        }

        let items: Vec<Value> = by_file
            .iter()
            .map(|(file, kinds)| json!({ "file": file, "kinds": kinds }))
            .collect();

        json!({
            "path_filter": path_prefix,
            "mode": "per_file",
            "file_count": items.len(),
            "files": items,
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

/// Handles `tokensave_recursion` tool calls.
///
/// Detects cycles in the call graph using iterative DFS on the calls-only
/// edge subgraph. Each cycle is a vec of node IDs forming the loop.
async fn handle_recursion(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);
    debug_assert!(limit > 0, "handle_recursion limit must be positive");

    let call_edges = cg.get_call_edges().await?;

    // Build adjacency list
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for (src, tgt) in &call_edges {
        adj.entry(src.clone()).or_default().push(tgt.clone());
    }

    // Iterative DFS cycle detection
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut on_stack: HashSet<String> = HashSet::new();

    let all_nodes: Vec<String> = adj.keys().cloned().collect();

    for start in &all_nodes {
        if visited.contains(start) {
            continue;
        }
        // Iterative DFS: stack of (node, neighbor_list, index, path_so_far)
        let mut stack: Vec<(String, Vec<String>, usize)> = Vec::new();
        let mut path: Vec<String> = Vec::new();

        let neighbors = adj.get(start).cloned().unwrap_or_default();
        visited.insert(start.clone());
        on_stack.insert(start.clone());
        path.push(start.clone());
        stack.push((start.clone(), neighbors, 0));

        while let Some(frame) = stack.last_mut() {
            let idx = frame.2;
            if idx >= frame.1.len() {
                let Some((node, _, _)) = stack.pop() else {
                    break;
                };
                path.pop();
                on_stack.remove(&node);
                continue;
            }
            frame.2 += 1;
            let neighbor = frame.1[idx].clone();

            if !visited.contains(&neighbor) {
                let nb_neighbors = adj.get(&neighbor).cloned().unwrap_or_default();
                visited.insert(neighbor.clone());
                on_stack.insert(neighbor.clone());
                path.push(neighbor.clone());
                stack.push((neighbor, nb_neighbors, 0));
            } else if on_stack.contains(&neighbor) {
                // Found a cycle
                let mut cycle = Vec::new();
                let mut found = false;
                for item in &path {
                    if *item == neighbor {
                        found = true;
                    }
                    if found {
                        cycle.push(item.clone());
                    }
                }
                cycle.push(neighbor.clone());
                cycles.push(cycle);
                if cycles.len() >= limit {
                    break;
                }
            }
        }
        if cycles.len() >= limit {
            break;
        }
    }

    // Resolve node details for each cycle
    let mut cycle_items: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    for cycle in &cycles {
        let mut chain: Vec<Value> = Vec::new();
        for node_id in cycle {
            if let Some(node) = cg.get_node(node_id).await? {
                touched.push(node.file_path.clone());
                chain.push(json!({
                    "id": node.id,
                    "name": node.name,
                    "kind": node.kind.as_str(),
                    "file": node.file_path,
                    "line": node.start_line,
                }));
            } else {
                chain.push(json!({ "id": node_id }));
            }
        }
        cycle_items.push(json!({
            "length": cycle.len() - 1,
            "chain": chain,
        }));
    }

    let touched_files = unique_file_paths(touched.iter().map(|s| s.as_str()));

    let output = json!({
        "cycle_count": cycle_items.len(),
        "cycles": cycle_items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_complexity` tool calls.
async fn handle_complexity(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let node_kind = args
        .get("node_kind")
        .and_then(|v| v.as_str())
        .and_then(|s| NodeKind::from_str(s));

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

    let results = cg
        .get_complexity_ranked(node_kind.as_ref(), limit)
        .await?;

    let touched_files =
        unique_file_paths(results.iter().map(|(n, _, _, _, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, lines, fan_out, fan_in, score)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "lines": lines,
                "cyclomatic_complexity": node.branches + 1,
                "branches": node.branches,
                "loops": node.loops,
                "returns": node.returns,
                "max_nesting": node.max_nesting,
                "unsafe_blocks": node.unsafe_blocks,
                "unchecked_calls": node.unchecked_calls,
                "assertions": node.assertions,
                "fan_out": fan_out,
                "fan_in": fan_in,
                "score": score,
            })
        })
        .collect();

    let output = json!({
        "formula": "lines + (fan_out × 3) + fan_in",
        "note": "cyclomatic_complexity = branches + 1 (computed from AST during extraction)",
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_doc_coverage` tool calls.
async fn handle_doc_coverage(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path_prefix = args.get("path").and_then(|v| v.as_str());

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(500) as usize)
        .unwrap_or(50);

    let results = cg
        .get_undocumented_public_symbols(path_prefix, limit)
        .await?;

    let touched_files = unique_file_paths(results.iter().map(|n| n.file_path.as_str()));

    // Group by file for readability
    let mut by_file: HashMap<String, Vec<Value>> = HashMap::new();
    for node in &results {
        by_file
            .entry(node.file_path.clone())
            .or_default()
            .push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "line": node.start_line,
                "signature": node.signature,
            }));
    }

    let mut file_items: Vec<Value> = by_file
        .into_iter()
        .map(|(file, symbols)| {
            json!({
                "file": file,
                "count": symbols.len(),
                "symbols": symbols,
            })
        })
        .collect();
    file_items.sort_by(|a, b| {
        b["count"].as_u64().cmp(&a["count"].as_u64())
    });

    let output = json!({
        "path_filter": path_prefix,
        "total_undocumented": results.len(),
        "file_count": file_items.len(),
        "files": file_items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_god_class` tool calls.
async fn handle_god_class(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as usize)
        .unwrap_or(10);

    let results = cg.get_god_classes(limit).await?;

    let touched_files =
        unique_file_paths(results.iter().map(|(n, _, _, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, methods, fields, total)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "methods": methods,
                "fields": fields,
                "total_members": total,
            })
        })
        .collect();

    let output = json!({
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_changelog` tool calls.
async fn handle_changelog(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_changelog expects an object argument");
    let from_ref = args
        .get("from_ref")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: from_ref".to_string(),
        })?;

    let to_ref = args
        .get("to_ref")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: to_ref".to_string(),
        })?;

    // Use gix to diff the two trees
    let changed_files: Vec<String> = match git_diff_files(cg.project_root(), from_ref, to_ref) {
        Ok(files) => files,
        Err(e) => {
            return Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": format!("git diff failed: {}", e) }]
                }),
                touched_files: vec![],
            });
        }
    };

    // For each changed file, get current symbols from the graph
    let mut added: Vec<Value> = Vec::new();
    let mut modified: Vec<Value> = Vec::new();
    let mut file_symbols: HashMap<String, Vec<Value>> = HashMap::new();

    for file in &changed_files {
        let nodes = cg.get_nodes_by_file(file).await?;
        let symbols: Vec<Value> = nodes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "name": n.name,
                    "kind": n.kind.as_str(),
                    "file": n.file_path,
                    "line": n.start_line,
                    "signature": n.signature,
                })
            })
            .collect();

        if symbols.is_empty() {
            // File was likely removed or not indexed
            modified.push(json!({
                "file": file,
                "status": "removed_or_not_indexed",
            }));
        } else {
            for sym in &symbols {
                added.push(sym.clone());
            }
        }
        file_symbols.insert(file.clone(), symbols);
    }

    let touched_files: Vec<String> = changed_files.clone();

    let result = json!({
        "from_ref": from_ref,
        "to_ref": to_ref,
        "changed_file_count": changed_files.len(),
        "changed_files": changed_files,
        "symbols_in_changed_files": added,
        "files_not_indexed": modified,
    });

    let formatted = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Default node kinds for port comparisons.
const PORT_DEFAULT_KINDS: &[&str] = &[
    "function", "method", "class", "struct", "interface", "trait", "enum", "module",
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

/// Handles `tokensave_port_status` tool calls.
async fn handle_port_status(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_port_status expects an object argument");

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

    let kind_strs: Vec<String> = args
        .get("kinds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| PORT_DEFAULT_KINDS.iter().map(|s| s.to_string()).collect());

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

    // Build target lookup: (lowercase_name, compat_group) -> Vec<&Node>
    let mut target_map: HashMap<(String, u8), Vec<&crate::types::Node>> = HashMap::new();
    for node in &target_nodes {
        let key = (node.name.to_lowercase(), kind_compat_group(node.kind.as_str()));
        target_map.entry(key).or_default().push(node);
    }

    let mut matched_symbols: Vec<Value> = Vec::new();
    let mut matched_target_ids: HashSet<String> = HashSet::new();
    let mut unmatched_by_file: HashMap<String, Vec<Value>> = HashMap::new();

    for src_node in &source_nodes {
        let key = (
            src_node.name.to_lowercase(),
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
async fn handle_port_order(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(args.is_object(), "handle_port_order expects an object argument");

    let source_dir = args
        .get("source_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: source_dir".to_string(),
        })?;

    let kind_strs: Vec<String> = args
        .get("kinds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| PORT_DEFAULT_KINDS.iter().map(|s| s.to_string()).collect());

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(500) as usize)
        .unwrap_or(50);

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
    let id_set: HashSet<&str> = node_ids.iter().map(|s| s.as_str()).collect();

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

    for edge in &edges {
        if !dep_edge_kinds.contains(edge.kind.as_str()) {
            continue;
        }
        if !id_set.contains(edge.source.as_str()) || !id_set.contains(edge.target.as_str()) {
            continue;
        }
        // source depends on target: add dependency source -> target
        dep_graph
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
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

        // For each node in this level, reduce in-degree of its reverse deps
        // (nodes that depend on it). We need to find who depends on each sorted node.
        // Build reverse: for each (A depends on B), when B is sorted, decrement in_degree[A].
        for &sorted_id in &current_level {
            for (&node_id, deps) in &dep_graph {
                if sorted_set.contains(node_id) {
                    continue;
                }
                if deps.contains(&sorted_id) {
                    let deg = in_degree.entry(node_id).or_insert(0);
                    if *deg > 0 {
                        *deg -= 1;
                    }
                    if *deg == 0 {
                        queue.push_back(node_id);
                    }
                }
            }
        }

        if !current_level.is_empty() {
            levels.push(current_level);
        }
    }

    // Detect cycles: any unsorted nodes form cycles
    let cycle_node_ids: Vec<&str> = node_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|id| !sorted_set.contains(id))
        .collect();

    // Group cycles: find strongly connected components among remaining nodes
    // For simplicity, report all cycle nodes as one group with a note.
    let mut cycles_json: Vec<Value> = Vec::new();
    if !cycle_node_ids.is_empty() {
        let cycle_names: Vec<&str> = cycle_node_ids
            .iter()
            .filter_map(|id| node_map.get(id).map(|n| n.name.as_str()))
            .collect();
        cycles_json.push(json!({
            "symbols": cycle_names,
            "note": "Mutual dependency — port together"
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
                                .filter_map(|dep_id| {
                                    node_map.get(dep_id).map(|n| n.name.as_str())
                                })
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

/// Diff two git refs and return the list of changed file paths.
fn git_diff_files(
    project_root: &std::path::Path,
    from_ref: &str,
    to_ref: &str,
) -> std::result::Result<Vec<String>, String> {
    let repo =
        gix::open(project_root).map_err(|e| format!("failed to open git repo: {e}"))?;

    let from_tree = repo
        .rev_parse_single(from_ref)
        .map_err(|e| format!("cannot resolve '{from_ref}': {e}"))?
        .object()
        .map_err(|e| format!("cannot read object for '{from_ref}': {e}"))?
        .peel_to_tree()
        .map_err(|e| format!("cannot peel '{from_ref}' to tree: {e}"))?;

    let to_tree = repo
        .rev_parse_single(to_ref)
        .map_err(|e| format!("cannot resolve '{to_ref}': {e}"))?
        .object()
        .map_err(|e| format!("cannot read object for '{to_ref}': {e}"))?
        .peel_to_tree()
        .map_err(|e| format!("cannot peel '{to_ref}' to tree: {e}"))?;

    let mut changed = Vec::new();
    from_tree
        .changes()
        .map_err(|e| format!("diff init failed: {e}"))?
        .for_each_to_obtain_tree(&to_tree, |change| {
            use gix::object::tree::diff::Change;
            match &change {
                Change::Addition { location, .. }
                | Change::Deletion { location, .. }
                | Change::Modification { location, .. } => {
                    changed.push(location.to_string());
                }
                Change::Rewrite {
                    source_location,
                    location,
                    ..
                } => {
                    changed.push(source_location.to_string());
                    changed.push(location.to_string());
                }
            }
            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
        })
        .map_err(|e| format!("tree diff failed: {e}"))?;

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::get_tool_definitions;

    #[test]
    fn test_tool_definitions_complete() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 29);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"tokensave_search"));
        assert!(tool_names.contains(&"tokensave_context"));
        assert!(tool_names.contains(&"tokensave_callers"));
        assert!(tool_names.contains(&"tokensave_callees"));
        assert!(tool_names.contains(&"tokensave_impact"));
        assert!(tool_names.contains(&"tokensave_node"));
        assert!(tool_names.contains(&"tokensave_status"));
        assert!(tool_names.contains(&"tokensave_files"));
        assert!(tool_names.contains(&"tokensave_affected"));
        assert!(tool_names.contains(&"tokensave_dead_code"));
        assert!(tool_names.contains(&"tokensave_diff_context"));
        assert!(tool_names.contains(&"tokensave_module_api"));
        assert!(tool_names.contains(&"tokensave_circular"));
        assert!(tool_names.contains(&"tokensave_hotspots"));
        assert!(tool_names.contains(&"tokensave_similar"));
        assert!(tool_names.contains(&"tokensave_rename_preview"));
        assert!(tool_names.contains(&"tokensave_unused_imports"));
        assert!(tool_names.contains(&"tokensave_changelog"));
        assert!(tool_names.contains(&"tokensave_rank"));
        assert!(tool_names.contains(&"tokensave_largest"));
        assert!(tool_names.contains(&"tokensave_coupling"));
        assert!(tool_names.contains(&"tokensave_inheritance_depth"));
        assert!(tool_names.contains(&"tokensave_distribution"));
        assert!(tool_names.contains(&"tokensave_recursion"));
        assert!(tool_names.contains(&"tokensave_complexity"));
        assert!(tool_names.contains(&"tokensave_doc_coverage"));
        assert!(tool_names.contains(&"tokensave_god_class"));
        assert!(tool_names.contains(&"tokensave_port_status"));
        assert!(tool_names.contains(&"tokensave_port_order"));
    }

    #[test]
    fn test_tool_definitions_have_schemas() {
        let tools = get_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(tool.input_schema.is_object());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn test_truncate_short_response() {
        let short = "hello world";
        assert_eq!(truncate_response(short), short);
    }

    #[test]
    fn test_truncate_long_response() {
        let long = "x".repeat(20_000);
        let result = truncate_response(&long);
        assert!(result.len() < 20_000);
        assert!(result.contains("[... truncated at 15000 chars]"));
    }

    #[test]
    fn test_tool_definitions_serializable() {
        let tools = get_tool_definitions();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("tokensave_search"));
        assert!(json.contains("tokensave_status"));
    }
}
