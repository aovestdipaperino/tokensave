//! MCP tool definitions and dispatch for the code graph.
//!
//! Split into two sub-modules:
//! - `definitions`: JSON Schema tool descriptors (`def_*` functions)
//! - `handlers`: tool call implementations (`handle_*` functions)

mod definitions;
mod handlers;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use definitions::get_tool_definitions;
pub use handlers::handle_tool_call;

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
    /// MCP tool annotations (readOnlyHint, title, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    /// MCP tool metadata (e.g. anthropic/alwaysLoad).
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// The result of a tool call, including the JSON response and the file
/// paths that were touched (used to track saved tokens).
pub struct ToolResult {
    /// The JSON-RPC result payload.
    pub value: Value,
    /// Unique file paths referenced in the result.
    pub touched_files: Vec<String>,
}
