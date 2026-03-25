// Rust guideline compliant 2025-10-17
//! MCP tool definitions and dispatch for the code graph.
//!
//! Each tool maps to a `TokenSave` method. Tool definitions include JSON Schema
//! descriptions so that MCP clients can discover available capabilities.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tokensave::TokenSave;
use crate::context::format_context_as_markdown;
use crate::errors::{TokenSaveError, Result};
use crate::types::{BuildContextOptions, NodeKind, Visibility};

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
        ToolDefinition {
            name: "tokensave_dead_code".to_string(),
            description: "Find symbols with no incoming edges (potentially unreachable code). Excludes main, test functions, and public items.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "kinds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Node kinds to check (default: [\"function\", \"method\"])"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "tokensave_diff_context".to_string(),
            description: "Given changed file paths, return semantic context: which symbols were modified, what depends on them, and affected tests.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of changed file paths"
                    },
                    "depth": {
                        "type": "number",
                        "description": "Maximum impact traversal depth (default: 2)"
                    }
                },
                "required": ["files"]
            }),
        },
        ToolDefinition {
            name: "tokensave_module_api".to_string(),
            description: "Show the public API surface of a file or directory: all pub symbols sorted by file and line.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path or directory prefix to inspect"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "tokensave_circular".to_string(),
            description: "Detect circular dependencies between files in the code graph.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "max_depth": {
                        "type": "number",
                        "description": "Maximum cycle detection depth (default: 10)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "tokensave_hotspots".to_string(),
            description: "Find symbols with the highest connectivity (most incoming + outgoing edges).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of hotspots to return (default: 10)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "tokensave_similar".to_string(),
            description: "Find symbols with similar names using full-text search and substring matching.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Symbol name to find similar matches for"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of results (default: 10)"
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolDefinition {
            name: "tokensave_rename_preview".to_string(),
            description: "Show all references to a symbol — all edges where the node appears as source or target.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique node ID to find references for"
                    }
                },
                "required": ["node_id"]
            }),
        },
        ToolDefinition {
            name: "tokensave_unused_imports".to_string(),
            description: "Find import/use nodes that are never referenced by any other node.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "tokensave_changelog".to_string(),
            description: "Generate a semantic diff/changelog between two git refs, categorizing symbols as added, removed, or modified.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from_ref": {
                        "type": "string",
                        "description": "Starting git ref (commit, branch, tag)"
                    },
                    "to_ref": {
                        "type": "string",
                        "description": "Ending git ref (commit, branch, tag)"
                    }
                },
                "required": ["from_ref", "to_ref"]
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
        "tokensave_dead_code" => handle_dead_code(cg, args).await,
        "tokensave_diff_context" => handle_diff_context(cg, args).await,
        "tokensave_module_api" => handle_module_api(cg, args).await,
        "tokensave_circular" => handle_circular(cg, args).await,
        "tokensave_hotspots" => handle_hotspots(cg, args).await,
        "tokensave_similar" => handle_similar(cg, args).await,
        "tokensave_rename_preview" => handle_rename_preview(cg, args).await,
        "tokensave_unused_imports" => handle_unused_imports(cg, args).await,
        "tokensave_changelog" => handle_changelog(cg, args).await,
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
                    if is_test_file(&impacted.file_path) {
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
        if is_test_file(file) {
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
            if is_test_file(&dep) {
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

/// Handles `tokensave_changelog` tool calls.
async fn handle_changelog(cg: &TokenSave, args: Value) -> Result<ToolResult> {
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

    // Run git diff to get changed files
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", from_ref, to_ref])
        .current_dir(cg.project_root())
        .output();

    let changed_files: Vec<String> = match output {
        Ok(out) => {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Ok(ToolResult {
                    value: json!({
                        "content": [{ "type": "text", "text": format!("git diff failed: {}", stderr) }]
                    }),
                    touched_files: vec![],
                });
            }
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect()
        }
        Err(e) => {
            return Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": format!("failed to run git: {}", e) }]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_complete() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 18);

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
