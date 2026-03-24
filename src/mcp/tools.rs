// Rust guideline compliant 2025-10-17
//! MCP tool definitions and dispatch for the code graph.
//!
//! Each tool maps to a `TokenSave` method. Tool definitions include JSON Schema
//! descriptions so that MCP clients can discover available capabilities.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tokensave::TokenSave;
use crate::context::format_context_as_markdown;
use crate::errors::{TokenSaveError, Result};
use crate::types::BuildContextOptions;

/// Maximum character length for a tool response before truncation.
const MAX_RESPONSE_CHARS: usize = 15_000;

/// A tool definition exposed by the MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Returns the list of all tool definitions exposed by this MCP server.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "tokensave_search".to_string(),
            description: "Search for symbols (functions, structs, traits, etc.) in the code graph by name or keyword.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query string to match against symbol names"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of results to return (default: 10)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "tokensave_context".to_string(),
            description: "Build an AI-ready context for a task description. Returns relevant symbols, relationships, and code snippets.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Natural language description of the task or question"
                    },
                    "max_nodes": {
                        "type": "number",
                        "description": "Maximum number of symbols to include (default: 20)"
                    }
                },
                "required": ["task"]
            }),
        },
        ToolDefinition {
            name: "tokensave_callers".to_string(),
            description: "Find all callers of a given node (function, method, etc.) up to a specified depth.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique node ID to find callers for"
                    },
                    "max_depth": {
                        "type": "number",
                        "description": "Maximum traversal depth (default: 3)"
                    }
                },
                "required": ["node_id"]
            }),
        },
        ToolDefinition {
            name: "tokensave_callees".to_string(),
            description: "Find all callees of a given node (function, method, etc.) up to a specified depth.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique node ID to find callees for"
                    },
                    "max_depth": {
                        "type": "number",
                        "description": "Maximum traversal depth (default: 3)"
                    }
                },
                "required": ["node_id"]
            }),
        },
        ToolDefinition {
            name: "tokensave_impact".to_string(),
            description: "Compute the impact radius of a node: all symbols that directly or indirectly depend on it.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique node ID to compute impact for"
                    },
                    "max_depth": {
                        "type": "number",
                        "description": "Maximum traversal depth (default: 3)"
                    }
                },
                "required": ["node_id"]
            }),
        },
        ToolDefinition {
            name: "tokensave_node".to_string(),
            description: "Retrieve detailed information about a single node by its ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique node ID to retrieve"
                    }
                },
                "required": ["node_id"]
            }),
        },
        ToolDefinition {
            name: "tokensave_status".to_string(),
            description: "Return aggregate statistics about the code graph (node/edge/file counts, DB size, etc.).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "tokensave_files".to_string(),
            description: "List indexed project files. Use to explore file structure without reading file contents.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Filter to files under this directory path"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Filter files matching this glob pattern (e.g. '**/*.rs')"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["flat", "grouped"],
                        "description": "Output format: flat (one per line) or grouped by directory (default: grouped)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "tokensave_affected".to_string(),
            description: "Find test files affected by changed source files. BFS through file dependency graph to discover impacted tests.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of changed file paths to analyze"
                    },
                    "depth": {
                        "type": "number",
                        "description": "Maximum dependency traversal depth (default: 5)"
                    },
                    "filter": {
                        "type": "string",
                        "description": "Custom glob pattern for test files (default: common test patterns)"
                    }
                },
                "required": ["files"]
            }),
        },
    ]
}

/// The result of a tool call, including the JSON response and the file
/// paths that were touched (used to track saved tokens).
pub struct ToolResult {
    /// The JSON-RPC result payload.
    pub value: Value,
    /// Unique file paths referenced in the result.
    pub touched_files: Vec<String>,
}

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

    let touched_files = unique_file_paths(files.iter().map(|f| f.path.as_str()));

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

/// Returns `true` if the file path looks like a test file.
fn is_test_file(path: &str) -> bool {
    let test_segments = [
        "test/", "tests/", "__tests__/", "spec/", "e2e/",
        ".test.", ".spec.", "_test.", "_spec.",
    ];
    let lower = path.to_ascii_lowercase();
    test_segments.iter().any(|s| lower.contains(s))
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
            is_test_file(path)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_complete() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 9);

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
