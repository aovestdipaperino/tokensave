//! MCP server that reads JSON-RPC 2.0 messages from stdin and writes
//! responses to stdout.
//!
//! The server exposes code graph tools via the Model Context Protocol,
//! allowing AI assistants to query the code graph interactively.

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::codegraph::CodeGraph;
use crate::errors::Result;

use super::tools::{get_tool_definitions, handle_tool_call};
use super::transport::{ErrorCode, JsonRpcRequest, JsonRpcResponse};

/// The MCP server wrapping a `CodeGraph` instance.
pub struct McpServer {
    cg: CodeGraph,
}

impl McpServer {
    /// Creates a new MCP server backed by the given code graph.
    pub fn new(cg: CodeGraph) -> Self {
        Self { cg }
    }

    /// Runs the server, reading JSON-RPC requests from stdin and writing
    /// responses to stdout. Runs until stdin is closed.
    pub async fn run(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            // Parse the incoming JSON
            let parsed: std::result::Result<JsonRpcRequest, _> = serde_json::from_str(&line);

            let response = match parsed {
                Ok(request) => self.handle_request(&request),
                Err(e) => Some(JsonRpcResponse::error(
                    Value::Null,
                    ErrorCode::ParseError,
                    format!("failed to parse JSON-RPC request: {}", e),
                )),
            };

            // Write response (if any) as a single line to stdout
            if let Some(resp) = response {
                let json_line = match serde_json::to_string(&resp) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("failed to serialize response: {}", e);
                        continue;
                    }
                };
                let output = format!("{}\n", json_line);
                if let Err(e) = stdout.write_all(output.as_bytes()).await {
                    eprintln!("failed to write response: {}", e);
                    break;
                }
                if let Err(e) = stdout.flush().await {
                    eprintln!("failed to flush stdout: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Dispatches a parsed JSON-RPC request to the appropriate handler.
    ///
    /// Returns `None` for notifications (requests without an `id`).
    fn handle_request(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = request.id.clone();

        match request.method.as_str() {
            "initialize" => Some(self.handle_initialize(id)),
            "initialized" => {
                // Notification - no response required
                None
            }
            "notifications/initialized" => {
                // Alternative notification path - no response required
                None
            }
            "tools/list" => Some(self.handle_tools_list(id)),
            "tools/call" => Some(self.handle_tools_call(id, &request.params)),
            "ping" => Some(JsonRpcResponse::success(id, json!({}))),
            _ => Some(JsonRpcResponse::error(
                id,
                ErrorCode::MethodNotFound,
                format!("method not found: {}", request.method),
            )),
        }
    }

    /// Handles the `initialize` method, returning server capabilities.
    fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "codegraph",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    /// Handles the `tools/list` method, returning all available tool definitions.
    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let tools = get_tool_definitions();
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    /// Handles the `tools/call` method, dispatching to the appropriate tool handler.
    fn handle_tools_call(&self, id: Value, params: &Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    ErrorCode::InvalidParams,
                    "missing params for tools/call".to_string(),
                );
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => {
                return JsonRpcResponse::error(
                    id,
                    ErrorCode::InvalidParams,
                    "missing 'name' in tools/call params".to_string(),
                );
            }
        };

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        match handle_tool_call(&self.cg, tool_name, arguments) {
            Ok(result) => JsonRpcResponse::success(id, result),
            Err(e) => JsonRpcResponse::error(
                id,
                ErrorCode::InternalError,
                format!("tool execution failed: {}", e),
            ),
        }
    }
}
