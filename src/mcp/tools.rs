//! MCP tool definitions and dispatch for the code graph.
//!
//! Each tool maps to a `CodeGraph` method. Tool definitions include JSON Schema
//! descriptions so that MCP clients can discover available capabilities.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::codegraph::CodeGraph;
use crate::context::format_context_as_markdown;
use crate::errors::{CodeGraphError, Result};
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
            name: "codegraph_search".to_string(),
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
            name: "codegraph_context".to_string(),
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
            name: "codegraph_callers".to_string(),
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
            name: "codegraph_callees".to_string(),
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
            name: "codegraph_impact".to_string(),
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
            name: "codegraph_node".to_string(),
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
            name: "codegraph_status".to_string(),
            description: "Return aggregate statistics about the code graph (node/edge/file counts, DB size, etc.).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

/// Dispatches a tool call to the appropriate handler.
///
/// Returns the tool result as a JSON value, or an error if the tool name
/// is unknown or the handler fails.
pub fn handle_tool_call(cg: &CodeGraph, tool_name: &str, args: Value) -> Result<Value> {
    match tool_name {
        "codegraph_search" => handle_search(cg, args),
        "codegraph_context" => handle_context(cg, args),
        "codegraph_callers" => handle_callers(cg, args),
        "codegraph_callees" => handle_callees(cg, args),
        "codegraph_impact" => handle_impact(cg, args),
        "codegraph_node" => handle_node(cg, args),
        "codegraph_status" => handle_status(cg),
        _ => Err(CodeGraphError::Config {
            message: format!("unknown tool: {}", tool_name),
        }),
    }
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

/// Handles `codegraph_search` tool calls.
fn handle_search(cg: &CodeGraph, args: Value) -> Result<Value> {
    let query =
        args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeGraphError::Config {
                message: "missing required parameter: query".to_string(),
            })?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(10);

    let results = cg.search(query, limit)?;

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
    Ok(json!({
        "content": [{ "type": "text", "text": truncate_response(&output) }]
    }))
}

/// Handles `codegraph_context` tool calls.
fn handle_context(cg: &CodeGraph, args: Value) -> Result<Value> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodeGraphError::Config {
            message: "missing required parameter: task".to_string(),
        })?;

    let max_nodes = args
        .get("max_nodes")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20);

    let options = BuildContextOptions {
        max_nodes,
        ..Default::default()
    };

    let context = cg.build_context(task, &options)?;
    let output = format_context_as_markdown(&context);

    Ok(json!({
        "content": [{ "type": "text", "text": truncate_response(&output) }]
    }))
}

/// Handles `codegraph_callers` tool calls.
fn handle_callers(cg: &CodeGraph, args: Value) -> Result<Value> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodeGraphError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(3);

    let results = cg.get_callers(node_id, max_depth)?;

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
    Ok(json!({
        "content": [{ "type": "text", "text": truncate_response(&output) }]
    }))
}

/// Handles `codegraph_callees` tool calls.
fn handle_callees(cg: &CodeGraph, args: Value) -> Result<Value> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodeGraphError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(3);

    let results = cg.get_callees(node_id, max_depth)?;

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
    Ok(json!({
        "content": [{ "type": "text", "text": truncate_response(&output) }]
    }))
}

/// Handles `codegraph_impact` tool calls.
fn handle_impact(cg: &CodeGraph, args: Value) -> Result<Value> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodeGraphError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(3);

    let subgraph = cg.get_impact_radius(node_id, max_depth)?;

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
    Ok(json!({
        "content": [{ "type": "text", "text": truncate_response(&formatted) }]
    }))
}

/// Handles `codegraph_node` tool calls.
fn handle_node(cg: &CodeGraph, args: Value) -> Result<Value> {
    let node_id = args
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodeGraphError::Config {
            message: "missing required parameter: node_id".to_string(),
        })?;

    let node = cg.get_node(node_id)?;

    match node {
        Some(n) => {
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
            Ok(json!({
                "content": [{ "type": "text", "text": truncate_response(&formatted) }]
            }))
        }
        None => Ok(json!({
            "content": [{ "type": "text", "text": format!("Node not found: {}", node_id) }]
        })),
    }
}

/// Handles `codegraph_status` tool calls.
fn handle_status(cg: &CodeGraph) -> Result<Value> {
    let stats = cg.get_stats()?;
    let output = serde_json::to_string_pretty(&stats).unwrap_or_default();
    Ok(json!({
        "content": [{ "type": "text", "text": truncate_response(&output) }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_complete() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 7);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"codegraph_search"));
        assert!(tool_names.contains(&"codegraph_context"));
        assert!(tool_names.contains(&"codegraph_callers"));
        assert!(tool_names.contains(&"codegraph_callees"));
        assert!(tool_names.contains(&"codegraph_impact"));
        assert!(tool_names.contains(&"codegraph_node"));
        assert!(tool_names.contains(&"codegraph_status"));
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
        assert!(json.contains("codegraph_search"));
        assert!(json.contains("codegraph_status"));
    }
}
