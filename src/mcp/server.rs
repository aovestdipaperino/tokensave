// Rust guideline compliant 2025-10-17
//! MCP server that reads JSON-RPC 2.0 messages from stdin and writes
//! responses to stdout.
//!
//! The server exposes code graph tools via the Model Context Protocol,
//! allowing AI assistants to query the code graph interactively.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::global_db::GlobalDb;
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

/// Cache duration for version checks (5 minutes).
const VERSION_CHECK_INTERVAL: Duration = Duration::from_secs(300);

/// Cached result of a latest-version check against GitHub releases.
struct VersionCheckState {
    latest: Option<String>,
    checked_at: Option<Instant>,
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
    /// Tokens already flushed to the worldwide counter this session.
    last_flushed_tokens: AtomicU64,
    /// UNIX timestamp of last worldwide flush (0 = never).
    last_flush_at: AtomicI64,
    /// User-level database tracking all projects (best-effort).
    global_db: Option<GlobalDb>,
    /// Cached latest-version check result.
    version_cache: std::sync::Mutex<VersionCheckState>,
    /// Pending JSON-RPC notifications to send before the next response.
    pending_notifications: std::sync::Mutex<Vec<Value>>,
}

impl McpServer {
    /// Creates a new MCP server backed by the given code graph.
    pub async fn new(cg: TokenSave) -> Self {
        let file_token_map = cg.get_file_token_map().await.unwrap_or_default();
        let persisted = cg.get_tokens_saved().await.unwrap_or(0);
        let global_db = GlobalDb::open().await;
        // Register this project in the global DB with its current tokens
        if let Some(ref gdb) = global_db {
            gdb.upsert(cg.project_root(), persisted).await;
        }
        Self {
            cg,
            stats: ServerStats::new(),
            tool_call_counts: std::sync::Mutex::new(HashMap::new()),
            file_token_map: std::sync::Mutex::new(file_token_map),
            tokens_saved: AtomicU64::new(persisted),
            last_flushed_tokens: AtomicU64::new(persisted),
            last_flush_at: AtomicI64::new(0),
            global_db,
            version_cache: std::sync::Mutex::new(VersionCheckState {
                latest: None,
                checked_at: None,
            }),
            pending_notifications: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Adds the approximate token count for the given file paths to the
    /// running saved-tokens counter and persists it to the database.
    async fn accumulate_tokens_saved(&self, file_paths: &[String]) {
        if file_paths.is_empty() {
            return;
        }
        debug_assert!(file_paths.iter().all(|p| !p.is_empty()), "accumulate_tokens_saved received empty file path");
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
            // Best-effort update to global DB
            if let Some(ref gdb) = self.global_db {
                gdb.upsert(self.cg.project_root(), new_total).await;
            }
        }
    }

    /// Flushes pending tokens to the worldwide counter if at least 30 seconds
    /// have elapsed since the last flush. Best-effort, never blocks for long.
    async fn maybe_flush_worldwide(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let last = self.last_flush_at.load(Ordering::Relaxed);
        if now - last < 30 {
            return;
        }
        // Mark as attempted immediately to prevent re-entry.
        self.last_flush_at.store(now, Ordering::Relaxed);

        let current = self.tokens_saved.load(Ordering::Relaxed);
        let last_flushed = self.last_flushed_tokens.load(Ordering::Relaxed);
        if current <= last_flushed {
            return;
        }
        let delta = current - last_flushed;

        let success = tokio::task::spawn_blocking(move || {
            let mut config = crate::user_config::UserConfig::load();
            config.pending_upload += delta;
            if config.upload_enabled {
                if crate::cloud::flush_pending(config.pending_upload).is_some() {
                    config.pending_upload = 0;
                    config.last_upload_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    config.save();
                    return true;
                }
            }
            config.save();
            false
        })
        .await
        .unwrap_or(false);

        if success {
            self.last_flushed_tokens.store(current, Ordering::Relaxed);
        }
    }

    /// Returns a version-update warning if a newer release is available.
    /// Results are cached for `VERSION_CHECK_INTERVAL` (5 minutes).
    async fn check_version_update(&self) -> Option<String> {
        let current = env!("CARGO_PKG_VERSION");

        // Fast path: serve from cache if still fresh.
        {
            let cache = self.version_cache.lock().ok()?;
            if let Some(checked_at) = cache.checked_at {
                if checked_at.elapsed() < VERSION_CHECK_INTERVAL {
                    let latest = cache.latest.as_deref()?;
                    return if crate::cloud::is_newer_version(current, latest) {
                        let method = crate::cloud::detect_install_method();
                        let cmd = crate::cloud::upgrade_command(&method);
                        Some(format!(
                            "⚠️ tokensave v{current} is installed, but v{latest} is available. \
                             Run `{cmd}` to upgrade."
                        ))
                    } else {
                        None
                    };
                }
            }
        }

        // Cache miss or expired – fetch from GitHub (best-effort, 1 s timeout).
        let latest = tokio::task::spawn_blocking(crate::cloud::fetch_latest_version)
            .await
            .ok()
            .flatten();

        // Update cache regardless of fetch outcome so we don't retry immediately.
        if let Ok(mut cache) = self.version_cache.lock() {
            cache.latest = latest.clone();
            cache.checked_at = Some(Instant::now());
        }

        let latest = latest?;
        if crate::cloud::is_newer_version(current, &latest) {
            let method = crate::cloud::detect_install_method();
            let cmd = crate::cloud::upgrade_command(&method);
            Some(format!(
                "⚠️ tokensave v{current} is installed, but v{latest} is available. \
                 Run `{cmd}` to upgrade."
            ))
        } else {
            None
        }
    }

    /// Runs the server, reading JSON-RPC requests from stdin and writing
    /// responses to stdout. Runs until stdin is closed or a shutdown signal
    /// (SIGINT/SIGTERM) is received, then performs graceful cleanup.
    pub async fn run(&self) -> Result<()> {
        debug_assert!(self.stats.total_requests.load(Ordering::Relaxed) == 0,
            "server run() called on an already-used server");
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        loop {
            let line: String = {
                #[cfg(unix)]
                {
                    let mut sigterm = tokio::signal::unix::signal(
                        tokio::signal::unix::SignalKind::terminate(),
                    )
                    .expect("failed to register SIGTERM handler");
                    tokio::select! {
                        result = lines.next_line() => {
                            match result {
                                Ok(Some(line)) => line,
                                _ => break,
                            }
                        }
                        _ = tokio::signal::ctrl_c() => break,
                        _ = sigterm.recv() => break,
                    }
                }
                #[cfg(not(unix))]
                {
                    tokio::select! {
                        result = lines.next_line() => {
                            match result {
                                Ok(Some(line)) => line,
                                _ => break,
                            }
                        }
                        _ = tokio::signal::ctrl_c() => break,
                    }
                }
            };

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

            // Drain and write any pending notifications (e.g., version warnings).
            {
                let notifications: Vec<Value> = self
                    .pending_notifications
                    .lock()
                    .map(|mut p| p.drain(..).collect())
                    .unwrap_or_default();
                for notification in notifications {
                    if let Ok(s) = serde_json::to_string(&notification) {
                        let _ = stdout.write_all(format!("{}\n", s).as_bytes()).await;
                        let _ = stdout.flush().await;
                    }
                }
            }

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

        self.shutdown().await;
        Ok(())
    }

    /// Performs graceful shutdown: persists the tokens-saved counter,
    /// flushes pending tokens to the worldwide counter, checkpoints the WAL,
    /// and logs a session summary.
    async fn shutdown(&self) {
        let uptime = self.stats.started_at.elapsed();
        let tool_calls = self.stats.tool_calls.load(Ordering::Relaxed);
        let tokens_saved = self.tokens_saved.load(Ordering::Relaxed);

        // Persist final tokens-saved value
        if let Err(e) = self.cg.set_tokens_saved(tokens_saved).await {
            eprintln!("[tokensave] warning: failed to persist tokens_saved on shutdown: {e}");
        }

        // Update global DB with final count and checkpoint it
        if let Some(ref gdb) = self.global_db {
            gdb.upsert(self.cg.project_root(), tokens_saved).await;
            gdb.checkpoint().await;
        }

        // Flush remaining delta to worldwide counter (what periodic flushes missed)
        let last_flushed = self.last_flushed_tokens.load(Ordering::Relaxed);
        if tokens_saved > last_flushed {
            let delta = tokens_saved - last_flushed;
            let mut config = crate::user_config::UserConfig::load();
            config.pending_upload += delta;
            if config.upload_enabled {
                if let Some(_total) = crate::cloud::flush_pending(config.pending_upload) {
                    config.pending_upload = 0;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    config.last_upload_at = now;
                }
            }
            config.save();
        }

        // Checkpoint WAL to merge it into the main database file
        if let Err(e) = self.cg.checkpoint().await {
            eprintln!("[tokensave] warning: failed to checkpoint WAL on shutdown: {e}");
        }

        eprintln!(
            "[tokensave] shutdown: {} tool calls, ~{} tokens saved, uptime {}s",
            tool_calls, tokens_saved, uptime.as_secs()
        );
    }

    /// Dispatches a parsed JSON-RPC request to the appropriate handler.
    ///
    /// Returns `None` for notifications (requests without an `id`).
    async fn handle_request(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        debug_assert!(!request.method.is_empty(), "handle_request called with empty method");
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
                    "tools": {},
                    "logging": {}
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
        debug_assert!(!id.is_null(), "handle_tools_call called with null request id");
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
            Some(self.server_stats_json().await)
        } else {
            None
        };

        match handle_tool_call(&self.cg, tool_name, arguments, server_stats).await {
            Ok(mut result) => {
                self.accumulate_tokens_saved(&result.touched_files).await;
                self.maybe_flush_worldwide().await;

                // Prepend version-update warning + queue logging notification.
                if let Some(warning) = self.check_version_update().await {
                    if let Some(content) = result
                        .value
                        .get_mut("content")
                        .and_then(|c| c.as_array_mut())
                    {
                        content.insert(0, json!({"type": "text", "text": &warning}));
                    }
                    if let Ok(mut pending) = self.pending_notifications.lock() {
                        pending.push(json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/message",
                            "params": {
                                "level": "warning",
                                "logger": "tokensave",
                                "data": warning
                            }
                        }));
                    }
                }

                // Check per-file staleness for files touched by this tool call.
                if !result.touched_files.is_empty() {
                    let stale_files = self.cg.check_file_staleness(&result.touched_files).await;
                    if !stale_files.is_empty() {
                        let warning = format!(
                            "WARNING: STALE INDEX — {} file(s) modified since last sync: {}. Run `tokensave sync` to update.",
                            stale_files.len(),
                            stale_files.join(", ")
                        );
                        if let Some(content) = result.value.get_mut("content").and_then(|c| c.as_array_mut()) {
                            content.insert(0, json!({"type": "text", "text": &warning}));
                        }
                    }
                }

                // Check overall index age (warn if older than 1 hour).
                if let Ok(last_time) = self.cg.last_index_time().await {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let age_secs = now - last_time;
                    if age_secs > 3600 {
                        let hours = age_secs / 3600;
                        let mins = (age_secs % 3600) / 60;
                        let warning = if hours >= 24 {
                            format!(
                                "WARNING: Index last synced {}d {}h ago. Run `tokensave sync` to update.",
                                hours / 24, hours % 24
                            )
                        } else {
                            format!(
                                "WARNING: Index last synced {}h {}m ago. Run `tokensave sync` to update.",
                                hours, mins
                            )
                        };
                        if let Some(content) = result.value.get_mut("content").and_then(|c| c.as_array_mut()) {
                            content.insert(0, json!({"type": "text", "text": &warning}));
                        }
                    }
                }

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
    pub async fn server_stats_json(&self) -> Value {
        let uptime = self.stats.started_at.elapsed();
        let tool_counts: Value = self
            .tool_call_counts
            .lock()
            .map(|counts| json!(*counts))
            .unwrap_or(json!({}));

        let mut stats = json!({
            "uptime_secs": uptime.as_secs(),
            "total_requests": self.stats.total_requests.load(Ordering::Relaxed),
            "tool_calls": self.stats.tool_calls.load(Ordering::Relaxed),
            "errors": self.stats.errors.load(Ordering::Relaxed),
            "tool_call_counts": tool_counts,
            "approx_tokens_saved": self.tokens_saved.load(Ordering::Relaxed),
        });

        if let Some(ref gdb) = self.global_db {
            if let Some(global_total) = gdb.global_tokens_saved().await {
                stats["global_tokens_saved"] = json!(global_total);
            }
        }

        stats
    }
}
