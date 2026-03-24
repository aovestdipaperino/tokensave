// Rust guideline compliant 2025-10-17
//! MCP server that reads JSON-RPC 2.0 messages from stdin and writes
//! responses to stdout.
//!
//! The server exposes code graph tools via the Model Context Protocol,
//! allowing AI assistants to query the code graph interactively.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::tokensave::TokenSave;
use crate::errors::Result;

use super::tools::{get_tool_definitions, handle_tool_call};
use super::transport::{ErrorCode, JsonRpcRequest, JsonRpcResponse};

/// Runtime statistics for the MCP server.
pub struct ServerStats {
    started_at: Instant,
    total_requests: AtomicU64,
    tool_calls: AtomicU64,
    errors: AtomicU64,
}

impl ServerStats {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            total_requests: AtomicU64::new(0),
            tool_calls: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }
}

/// The MCP server wrapping a `TokenSave` instance.
// Lock ordering: file_token_map -> tool_call_counts (never nested)
pub struct McpServer {
    cg: TokenSave,
    stats: ServerStats,
    tool_call_counts: std::sync::Mutex<HashMap<String, u64>>,
    /// Approximate token count per indexed file (file_path -> tokens).
    file_token_map: std::sync::Mutex<HashMap<String, u64>>,
    /// Running total of tokens saved by serving from the graph.
    tokens_saved: AtomicU64,
}

impl McpServer {
    /// Creates a new MCP server backed by the given code graph.
    pub async fn new(cg: TokenSave) -> Self {
        let file_token_map = cg.get_file_token_map().await.unwrap_or_default();
        let persisted = cg.get_tokens_saved().await.unwrap_or(0);
        Self {
            cg,
            stats: ServerStats::new(),
            tool_call_counts: std::sync::Mutex::new(HashMap::new()),
            file_token_map: std::sync::Mutex::new(file_token_map),
            tokens_saved: AtomicU64::new(persisted),
        }
    }

    /// Adds the approximate token count for the given file paths to the
    /// running saved-tokens counter and persists it to the database.
    async fn accumulate_tokens_saved(&self, file_paths: &[String]) {
        if file_paths.is_empty() {
            return;
        }
        let delta = {
            let map = match self.file_token_map.lock() {
                Ok(m) => m,
                Err(_) => return,
            };
            let mut total: u64 = 0;
            for path in file_paths {
                if let Some(&tokens) = map.get(path.as_str()) {
                    total += tokens;
                }
            }
            total
        };
        if delta > 0 {
            let new_total = self.tokens_saved.fetch_add(delta, Ordering::Relaxed) + delta;
            // Persist to DB (best-effort, don't block on failure)
            let _ = self.cg.set_tokens_saved(new_total).await;
        }
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
                Ok(request) => self.handle_request(&request).await,
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
    async fn handle_request(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);
        let id = request.id.clone();

        let result = match request.method.as_str() {
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
            "tools/call" => Some(self.handle_tools_call(id, &request.params).await),
            "ping" => Some(JsonRpcResponse::success(id, json!({}))),
            _ => Some(JsonRpcResponse::error(
                id,
                ErrorCode::MethodNotFound,
                format!("method not found: {}", request.method),
            )),
        };

        // Track errors
        if let Some(ref resp) = result {
            if resp.error.is_some() {
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
            }
        }

        result
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
                    "name": "tokensave",
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
    async fn handle_tools_call(&self, id: Value, params: &Option<Value>) -> JsonRpcResponse {
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

        self.stats.tool_calls.fetch_add(1, Ordering::Relaxed);
        eprintln!("[tokensave] tool call: {}", tool_name);
        if let Ok(mut counts) = self.tool_call_counts.lock() {
            *counts.entry(tool_name.to_string()).or_insert(0) += 1;
        }

        let server_stats = if tool_name == "tokensave_status" {
            Some(self.server_stats_json())
        } else {
            None
        };

        match handle_tool_call(&self.cg, tool_name, arguments, server_stats).await {
            Ok(result) => {
                self.accumulate_tokens_saved(&result.touched_files).await;
                JsonRpcResponse::success(id, result.value)
            }
            Err(e) => JsonRpcResponse::error(
                id,
                ErrorCode::InternalError,
                format!("tool execution failed: {}", e),
            ),
        }
    }

    /// Returns the current server runtime statistics as a JSON value.
    pub fn server_stats_json(&self) -> Value {
        let uptime = self.stats.started_at.elapsed();
        let tool_counts: Value = self
            .tool_call_counts
            .lock()
            .map(|counts| json!(*counts))
            .unwrap_or(json!({}));

        json!({
            "uptime_secs": uptime.as_secs(),
            "total_requests": self.stats.total_requests.load(Ordering::Relaxed),
            "tool_calls": self.stats.tool_calls.load(Ordering::Relaxed),
            "errors": self.stats.errors.load(Ordering::Relaxed),
            "tool_call_counts": tool_counts,
            "approx_tokens_saved": self.tokens_saved.load(Ordering::Relaxed),
        })
    }
}
