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

use crate::errors::Result;
use crate::global_db::GlobalDb;
use crate::tokensave::TokenSave;

use super::tools::{explore_call_budget, get_tool_definitions_with_budget, handle_tool_call};
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

/// Cache duration for version checks (15 minutes).
const VERSION_CHECK_INTERVAL: Duration = Duration::from_mins(15);

/// Hand-maintained schema documentation for the `tokensave://schema` resource.
/// Mirrors `src/db/migrations.rs::create_schema`. Update both together.
const SCHEMA_MARKDOWN: &str = r"# tokensave SQLite schema

The on-disk database lives at `.tokensave/tokensave.db` (per-branch variants
under multi-branch mode). All tables are plain SQLite; safe to query with any
client. WAL mode is used, so readers do not block writers.

## Tables

### `nodes` — every indexed symbol
- `id` TEXT PRIMARY KEY — content-hashed identifier (changes when symbol moves or renames)
- `kind` TEXT — e.g. `function`, `struct`, `trait`, `impl`, `method`, `module`, `file`
- `name` TEXT — local identifier
- `qualified_name` TEXT — language-style path (e.g. `crate::module::Type::method`)
- `file_path` TEXT — relative to the project root
- `start_line`, `end_line` INTEGER — 1-based inclusive line range of the symbol
- `start_column`, `end_column` INTEGER — 0-based column range
- `attrs_start_line` INTEGER — first line of leading doc-comments / attributes (or `start_line` if none)
- `signature` TEXT NULL — extracted source-level signature
- `docstring` TEXT NULL — leading doc-comment
- `visibility` TEXT — one of `public`, `pub_crate`, `pub_super`, `private`
- `is_async` INTEGER (0/1)
- `branches`, `loops`, `returns`, `max_nesting`, `unsafe_blocks`, `unchecked_calls`, `assertions` INTEGER — complexity metrics
- `updated_at` INTEGER — UNIX epoch seconds

Indexes: `kind`, `name`, `qualified_name`, `file_path`, `(file_path,start_line)`, `lower(name)`.

### `edges` — directed relationships between nodes
- `id` INTEGER PRIMARY KEY AUTOINCREMENT
- `source` TEXT — FK → `nodes.id` (CASCADE DELETE)
- `target` TEXT — FK → `nodes.id` (CASCADE DELETE)
- `kind` TEXT — one of `contains`, `calls`, `returns`, `type_of`, `uses`, `implements`, `extends`, `annotates`, `derives_macro`, `receives`
- `line` INTEGER NULL — source line of the relationship

Unique constraint: `(source, target, kind, COALESCE(line, -1))`. Indexes on `source`, `target`, `kind`, `(source,kind)`, `(target,kind)`.

### `files` — index bookkeeping
- `path` TEXT PRIMARY KEY
- `content_hash` TEXT — sha256 of file contents at index time
- `size` INTEGER — file size in bytes
- `modified_at`, `indexed_at` INTEGER — UNIX epoch seconds
- `node_count` INTEGER — number of nodes extracted from this file

### `unresolved_refs` — references the resolver could not bind
- `from_node_id` FK → `nodes.id`
- `reference_name` TEXT
- `reference_kind` TEXT
- `line`, `col` INTEGER
- `file_path` TEXT

### `vectors` — optional embeddings (semantic search backend)
- `node_id` PRIMARY KEY FK → `nodes.id`
- `embedding` BLOB
- `model` TEXT, `created_at` INTEGER

### `metadata` — key/value store
Common keys: `tokens_saved`, schema-version markers.

### `memory_decisions`, `memory_code_areas`
Hand-recorded notes from `tokensave_record_decision` / `tokensave_record_code_area`. FTS5 mirror tables exist for `nodes` (`nodes_fts`) and `memory_decisions` (`memory_decisions_fts`).

## Recipes

### Find every impl block of a trait
```sql
SELECT n.id, n.qualified_name, n.file_path, n.start_line
FROM nodes n
JOIN edges e ON e.source = n.id
WHERE e.kind = 'implements'
  AND e.target IN (SELECT id FROM nodes WHERE qualified_name = ?1);
```

### Top callers of a node
```sql
SELECT n.qualified_name, COUNT(*) AS call_count
FROM edges e
JOIN nodes n ON n.id = e.source
WHERE e.target = ?1 AND e.kind = 'calls'
GROUP BY n.qualified_name
ORDER BY call_count DESC
LIMIT 20;
```

### Files modified since last index
Compare `files.modified_at` against the live filesystem mtime — `tokensave_affected` does this with extra git plumbing.

### Largest functions by line span
```sql
SELECT qualified_name, file_path, end_line - start_line + 1 AS lines
FROM nodes
WHERE kind IN ('function', 'method')
ORDER BY lines DESC
LIMIT 20;
```

## Gotchas
- `nodes.id` is a content hash, so it changes when the symbol moves. For cross-run lookups use `qualified_name` (or `tokensave_by_qualified_name`).
- `edges.kind = 'calls'` may reference a *trait method* node rather than the resolved concrete impl — trait dispatch is not currently rewritten.
- `derives_macro` edges record `#[derive(...)]` usage but generated impls are not in the graph.
";

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
    /// Approximate token count per indexed file (`file_path` -> tokens).
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
    /// When the MCP server was started from a subdirectory of the project root,
    /// this holds the relative path prefix (e.g. `"src/mcp"`). Listing tools
    /// use it as the default path filter. `None` when cwd == project root.
    scope_prefix: Option<String>,
}

impl McpServer {
    /// Creates a new MCP server backed by the given code graph.
    pub async fn new(cg: TokenSave, scope_prefix: Option<String>) -> Self {
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
            scope_prefix,
        }
    }

    /// Returns the active scope prefix, if the server was launched from a subdirectory.
    pub fn scope_prefix(&self) -> Option<&str> {
        self.scope_prefix.as_deref()
    }

    /// Adds the approximate token count for the given file paths to the
    /// running saved-tokens counter and persists it to the database.
    /// Returns the delta (tokens saved by this call).
    async fn accumulate_tokens_saved(&self, file_paths: &[String]) -> u64 {
        if file_paths.is_empty() {
            return 0;
        }
        debug_assert!(
            file_paths.iter().all(|p| !p.is_empty()),
            "accumulate_tokens_saved received empty file path"
        );
        let delta = {
            let Ok(map) = self.file_token_map.lock() else {
                return 0;
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
            // Also increment the resettable local counter
            let _ = self.cg.add_local_counter(delta).await;
            // Best-effort update to global DB
            if let Some(ref gdb) = self.global_db {
                gdb.upsert(self.cg.project_root(), new_total).await;
            }
        }
        delta
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
            if config.upload_enabled && crate::cloud::flush_pending(config.pending_upload).is_some()
            {
                config.pending_upload = 0;
                config.last_upload_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                config.save();
                return true;
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
    /// Results are cached for `VERSION_CHECK_INTERVAL` (15 minutes).
    async fn check_version_update(&self) -> Option<String> {
        let current = env!("CARGO_PKG_VERSION");

        // Fast path: serve from cache if still fresh.
        {
            let cache = self.version_cache.lock().ok()?;
            if let Some(checked_at) = cache.checked_at {
                if checked_at.elapsed() < VERSION_CHECK_INTERVAL {
                    let latest = cache.latest.as_deref()?;
                    return if crate::cloud::is_newer_minor_version(current, latest) {
                        Some(format!(
                            "⚠️ tokensave v{current} is installed, but v{latest} is available. \
                             Run `tokensave upgrade` to update."
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
            cache.latest.clone_from(&latest);
            cache.checked_at = Some(Instant::now());
        }

        let latest = latest?;
        if crate::cloud::is_newer_minor_version(current, &latest) {
            Some(format!(
                "⚠️ tokensave v{current} is installed, but v{latest} is available. \
                 Run `tokensave upgrade` to update."
            ))
        } else {
            None
        }
    }

    /// Process a single raw JSON-RPC line and write the response.
    /// Used to replay a peeked `initialize` message that was consumed before
    /// the server's main loop started.
    pub async fn handle_and_write(
        &self,
        line: &str,
        transport: &mut impl super::transport::McpTransport,
    ) {
        let parsed: std::result::Result<super::transport::JsonRpcRequest, _> =
            serde_json::from_str(line);
        let response = match parsed {
            Ok(request) => self.handle_request(&request).await,
            Err(e) => Some(super::transport::JsonRpcResponse::error(
                Value::Null,
                super::transport::ErrorCode::ParseError,
                format!("failed to parse JSON-RPC request: {e}"),
            )),
        };
        if let Some(resp) = response {
            let json_str = serde_json::to_string(&resp).unwrap_or_default();
            let _ = transport.write_line(&json_str).await;
            let _ = transport.flush().await;
        }
    }

    /// Runs the server, reading JSON-RPC requests from stdin and writing
    /// responses to stdout. Runs until stdin is closed or a shutdown signal
    /// (SIGINT/SIGTERM) is received, then performs graceful cleanup.
    pub async fn run(&self, transport: &mut impl super::transport::McpTransport) -> Result<()> {
        debug_assert!(
            self.stats.total_requests.load(Ordering::Relaxed) == 0,
            "server run() called on an already-used server"
        );

        loop {
            let line: String = {
                #[cfg(unix)]
                {
                    #[allow(clippy::expect_used)]
                    let mut sigterm =
                        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                            .expect("failed to register SIGTERM handler");
                    tokio::select! {
                        result = transport.read_line() => {
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
                        result = transport.read_line() => {
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
                    format!("failed to parse JSON-RPC request: {e}"),
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
                        let _ = transport.write_line(&format!("{s}\n")).await;
                        let _ = transport.flush().await;
                    }
                }
            }

            // Write response (if any) as a single line to stdout
            if let Some(resp) = response {
                let json_line = match serde_json::to_string(&resp) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("failed to serialize response: {e}");
                        continue;
                    }
                };
                let output = format!("{json_line}\n");
                if let Err(e) = transport.write_line(&output).await {
                    eprintln!("failed to write response: {e}");
                    break;
                }
                if let Err(e) = transport.flush().await {
                    eprintln!("failed to flush stdout: {e}");
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
            tool_calls,
            tokens_saved,
            uptime.as_secs()
        );
    }

    /// Dispatches a parsed JSON-RPC request to the appropriate handler.
    ///
    /// Returns `None` for notifications (requests without an `id`).
    pub(crate) async fn handle_request(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        debug_assert!(
            !request.method.is_empty(),
            "handle_request called with empty method"
        );
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);
        let id = request.id.clone();

        let result = match request.method.as_str() {
            "initialize" => Some(Self::handle_initialize(id)),
            "initialized" => {
                // Notification - no response required
                None
            }
            "notifications/initialized" => {
                // Alternative notification path - no response required
                None
            }
            "tools/list" => Some(self.handle_tools_list(id).await),
            "tools/call" => Some(self.handle_tools_call(id, request.params.as_ref()).await),
            "resources/list" => Some(Self::handle_resources_list(id)),
            "resources/read" => Some(
                self.handle_resources_read(id, request.params.as_ref())
                    .await,
            ),
            "ping" | "logging/setLevel" => Some(JsonRpcResponse::success(id, json!({}))),
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
    fn handle_initialize(id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "logging": {}
                },
                "serverInfo": {
                    "name": "tokensave",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": "tokensave is a code-graph MCP server. \
                    Start with tokensave_context for any code exploration task \
                    — it returns relevant symbols, relationships, and code \
                    snippets for a natural-language query. Use tokensave_search \
                    to find specific symbols by name. All tools are read-only \
                    and safe to call in parallel. \
                    When a tool result contains a `tokensave_metrics:` line, \
                    report the savings to the user (e.g. 'TokenSave\\'d ~N tokens')."
            }),
        )
    }

    /// Handles the `tools/list` method, returning all available tool definitions.
    async fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let node_count = self.cg.get_stats().await.map_or(0, |s| s.node_count);
        let budget = explore_call_budget(node_count);
        let tools = get_tool_definitions_with_budget(node_count, budget);
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    /// Handles the `resources/list` method, returning available resources.
    fn handle_resources_list(id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "resources": [
                    {
                        "uri": "tokensave://status",
                        "name": "Graph Status",
                        "description": "Code graph statistics: node/edge/file counts, languages, DB size, and index freshness.",
                        "mimeType": "application/json"
                    },
                    {
                        "uri": "tokensave://files",
                        "name": "File List",
                        "description": "All indexed project files grouped by directory with symbol counts.",
                        "mimeType": "text/plain"
                    },
                    {
                        "uri": "tokensave://overview",
                        "name": "Project Overview",
                        "description": "High-level project summary: language distribution, largest modules, and top entry points.",
                        "mimeType": "text/plain"
                    },
                    {
                        "uri": "tokensave://branches",
                        "name": "Tracked Branches",
                        "description": "List of tracked branches with DB sizes, parent branch, and last sync time. Empty if multi-branch is not active.",
                        "mimeType": "application/json"
                    },
                    {
                        "uri": "tokensave://schema",
                        "name": "SQLite Schema",
                        "description": "Documentation for the .tokensave/tokensave.db schema: tables, columns, indexes, and common query recipes. Use when MCP tools don't cover your query and you need to drop down to raw SQL.",
                        "mimeType": "text/markdown"
                    }
                ]
            }),
        )
    }

    /// Handles the `resources/read` method, returning resource contents.
    async fn handle_resources_read(&self, id: Value, params: Option<&Value>) -> JsonRpcResponse {
        let uri = params.and_then(|p| p.get("uri")).and_then(|v| v.as_str());

        let Some(uri) = uri else {
            return JsonRpcResponse::error(
                id,
                ErrorCode::InvalidParams,
                "missing 'uri' in resources/read params".to_string(),
            );
        };

        match uri {
            "tokensave://status" => self.read_resource_status(id).await,
            "tokensave://files" => self.read_resource_files(id).await,
            "tokensave://overview" => self.read_resource_overview(id).await,
            "tokensave://branches" => self.read_resource_branches(id),
            "tokensave://schema" => Self::read_resource_schema(id),
            _ => JsonRpcResponse::error(
                id,
                ErrorCode::InvalidParams,
                format!("unknown resource URI: {uri}"),
            ),
        }
    }

    /// Returns the `SQLite` schema documentation as a markdown resource.
    /// Sourced from `src/db/migrations.rs::create_schema` — keep in sync.
    fn read_resource_schema(id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "contents": [{
                    "uri": "tokensave://schema",
                    "mimeType": "text/markdown",
                    "text": SCHEMA_MARKDOWN
                }]
            }),
        )
    }

    /// Returns graph statistics as a JSON resource.
    async fn read_resource_status(&self, id: Value) -> JsonRpcResponse {
        match self.cg.get_stats().await {
            Ok(stats) => {
                let text = serde_json::to_string_pretty(&stats).unwrap_or_default();
                JsonRpcResponse::success(
                    id,
                    json!({
                        "contents": [{
                            "uri": "tokensave://status",
                            "mimeType": "application/json",
                            "text": text
                        }]
                    }),
                )
            }
            Err(e) => JsonRpcResponse::error(
                id,
                ErrorCode::InternalError,
                format!("failed to read graph stats: {e}"),
            ),
        }
    }

    /// Returns the file list as a text resource (grouped by directory).
    async fn read_resource_files(&self, id: Value) -> JsonRpcResponse {
        match self.cg.get_all_files().await {
            Ok(mut files) => {
                files.sort_by(|a, b| a.path.cmp(&b.path));
                let mut groups: std::collections::BTreeMap<String, Vec<String>> =
                    std::collections::BTreeMap::new();
                for f in &files {
                    let dir = f.path.rfind('/').map_or(".", |i| &f.path[..i]).to_string();
                    #[allow(clippy::map_unwrap_or)]
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
                        lines.push(format!("  {entry}"));
                    }
                }
                let text = lines.join("\n");
                JsonRpcResponse::success(
                    id,
                    json!({
                        "contents": [{
                            "uri": "tokensave://files",
                            "mimeType": "text/plain",
                            "text": text
                        }]
                    }),
                )
            }
            Err(e) => JsonRpcResponse::error(
                id,
                ErrorCode::InternalError,
                format!("failed to read file list: {e}"),
            ),
        }
    }

    /// Returns a high-level project overview as a text resource.
    async fn read_resource_overview(&self, id: Value) -> JsonRpcResponse {
        let stats = match self.cg.get_stats().await {
            Ok(s) => s,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    ErrorCode::InternalError,
                    format!("failed to read graph stats: {e}"),
                );
            }
        };

        let mut lines = Vec::new();
        lines.push(format!("Project: {}", self.cg.project_root().display()));
        lines.push(format!(
            "Graph: {} nodes, {} edges, {} files",
            stats.node_count, stats.edge_count, stats.file_count
        ));

        // Language distribution
        if !stats.files_by_language.is_empty() {
            lines.push("\nLanguages:".to_string());
            let mut langs: Vec<_> = stats.files_by_language.iter().collect();
            langs.sort_by(|a, b| b.1.cmp(a.1));
            for (lang, count) in &langs {
                lines.push(format!("  {lang} ({count} files)"));
            }
        }

        // Node kind distribution (top 10)
        if !stats.nodes_by_kind.is_empty() {
            lines.push("\nSymbol kinds:".to_string());
            let mut kinds: Vec<_> = stats.nodes_by_kind.iter().collect();
            kinds.sort_by(|a, b| b.1.cmp(a.1));
            for (kind, count) in kinds.iter().take(10) {
                lines.push(format!("  {kind} ({count})"));
            }
        }

        let text = lines.join("\n");
        JsonRpcResponse::success(
            id,
            json!({
                "contents": [{
                    "uri": "tokensave://overview",
                    "mimeType": "text/plain",
                    "text": text
                }]
            }),
        )
    }

    fn read_resource_branches(&self, id: Value) -> JsonRpcResponse {
        let tokensave_dir = crate::config::get_tokensave_dir(self.cg.project_root());
        let current = self.cg.active_branch();

        let branches: Vec<Value> = match crate::branch_meta::load_branch_meta(&tokensave_dir) {
            Some(meta) => meta
                .branches
                .iter()
                .map(|(name, entry)| {
                    let db_path = tokensave_dir.join(&entry.db_file);
                    let size_bytes = db_path.metadata().map_or(0, |m| m.len());
                    json!({
                        "name": name,
                        "db_file": entry.db_file,
                        "parent": entry.parent,
                        "size_bytes": size_bytes,
                        "last_synced_at": entry.last_synced_at,
                        "is_current": current == Some(name.as_str()),
                        "is_default": name == &meta.default_branch,
                    })
                })
                .collect(),
            None => vec![],
        };

        let output = json!({
            "branch_count": branches.len(),
            "branches": branches,
        });
        let text = serde_json::to_string_pretty(&output).unwrap_or_default();
        JsonRpcResponse::success(
            id,
            json!({
                "contents": [{
                    "uri": "tokensave://branches",
                    "mimeType": "application/json",
                    "text": text
                }]
            }),
        )
    }

    /// Handles the `tools/call` method, dispatching to the appropriate tool handler.
    async fn handle_tools_call(&self, id: Value, params: Option<&Value>) -> JsonRpcResponse {
        debug_assert!(
            !id.is_null(),
            "handle_tools_call called with null request id"
        );
        let Some(params) = params else {
            return JsonRpcResponse::error(
                id,
                ErrorCode::InvalidParams,
                "missing params for tools/call".to_string(),
            );
        };

        let Some(tool_name) = params.get("name").and_then(|v| v.as_str()) else {
            return JsonRpcResponse::error(
                id,
                ErrorCode::InvalidParams,
                "missing 'name' in tools/call params".to_string(),
            );
        };

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        self.stats.tool_calls.fetch_add(1, Ordering::Relaxed);
        eprintln!("[tokensave] tool call: {tool_name}");
        if let Ok(mut counts) = self.tool_call_counts.lock() {
            *counts.entry(tool_name.to_string()).or_insert(0) += 1;
        }

        let server_stats = if tool_name == "tokensave_status" {
            Some(self.server_stats_json().await)
        } else {
            None
        };

        match handle_tool_call(
            &self.cg,
            tool_name,
            arguments,
            server_stats,
            self.scope_prefix(),
        )
        .await
        {
            Ok(mut result) => {
                let raw_file_tokens = self.accumulate_tokens_saved(&result.touched_files).await;
                crate::monitor::write_entry(
                    self.cg.project_root(),
                    "tokensave",
                    tool_name,
                    raw_file_tokens,
                    raw_file_tokens,
                );
                self.maybe_flush_worldwide().await;

                // Estimate approximate token count of the graph response.
                let response_tokens: u64 = result
                    .value
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map_or(0, |arr| {
                        let total_chars: usize = arr
                            .iter()
                            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                            .map(str::len)
                            .sum();
                        (total_chars / 4) as u64
                    });

                // Append per-call token savings to the response content.
                if raw_file_tokens > 0 {
                    if let Some(content) = result
                        .value
                        .get_mut("content")
                        .and_then(|c| c.as_array_mut())
                    {
                        content.push(json!({"type": "text", "text": format!(
                            "\ntokensave_metrics: before={raw_file_tokens} after={response_tokens}"
                        )}));
                    }
                }

                // Persist to the cross-project savings ledger (best-effort, non-blocking).
                {
                    let project_path_str = self.cg.project_root().to_string_lossy().to_string();
                    let tool_name_owned = tool_name.to_string();
                    let ts = crate::tokensave::current_timestamp();
                    tokio::spawn(async move {
                        if let Some(gdb) = crate::global_db::GlobalDb::open().await {
                            gdb.record_savings(
                                &project_path_str,
                                &tool_name_owned,
                                raw_file_tokens,
                                response_tokens,
                                ts,
                            )
                            .await;
                        }
                    });
                }

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
                // If stale files exist, attempt an incremental sync first to keep
                // the index up-to-date before returning the response.
                if !result.touched_files.is_empty() {
                    let stale_files = self.cg.check_file_staleness(&result.touched_files).await;
                    if !stale_files.is_empty() {
                        // Try to sync before responding. If sync fails (e.g., lock
                        // held by another process), we still warn the user.
                        let still_stale = match self.cg.sync_if_stale(&stale_files).await {
                            Ok(false) => false,        // Sync completed and files are no longer stale
                            Ok(true) | Err(_) => true, // Files still stale or sync failed
                        };
                        if still_stale {
                            let warning = format!(
                                "WARNING: STALE INDEX — {} file(s) modified since last sync: {}. Run `tokensave sync` to update.",
                                stale_files.len(),
                                stale_files.join(", ")
                            );
                            // Machine-readable marker so callers can distrust
                            // any answer referencing these paths. Always
                            // emitted (as text + structured field) when
                            // post-sync stale files remain.
                            let stale_json = serde_json::to_string(&stale_files)
                                .unwrap_or_else(|_| "[]".to_string());
                            let marker = format!("\ntokensave_graph_stale: {stale_json}");
                            // Every handler returns an object with `content`;
                            // crash hard in debug if a future handler ever
                            // breaks that contract so we don't silently drop
                            // the structured staleness signal.
                            debug_assert!(
                                result.value.is_object(),
                                "tool result must be a JSON object so graph_stale can be attached"
                            );
                            if let Some(obj) = result.value.as_object_mut() {
                                obj.insert("graph_stale".to_string(), json!(stale_files));
                            }
                            if let Some(content) = result
                                .value
                                .get_mut("content")
                                .and_then(|c| c.as_array_mut())
                            {
                                content.insert(0, json!({"type": "text", "text": &warning}));
                                content.push(json!({"type": "text", "text": marker}));
                            }
                        }
                    }
                }

                // Warn if serving from a fallback (ancestor) branch DB.
                if let Some(warning) = self.cg.fallback_warning() {
                    let warning = format!("WARNING: {warning}");
                    if let Some(content) = result
                        .value
                        .get_mut("content")
                        .and_then(|c| c.as_array_mut())
                    {
                        content.insert(0, json!({"type": "text", "text": &warning}));
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
                                "WARNING: Index last synced {hours}h {mins}m ago. Run `tokensave sync` to update."
                            )
                        };
                        if let Some(content) = result
                            .value
                            .get_mut("content")
                            .and_then(|c| c.as_array_mut())
                        {
                            content.insert(0, json!({"type": "text", "text": &warning}));
                        }
                    }
                }

                JsonRpcResponse::success(id, result.value)
            }
            Err(e) => JsonRpcResponse::error(
                id,
                ErrorCode::InternalError,
                format!("tool execution failed: {e}"),
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
                let local = self.tokens_saved.load(Ordering::Relaxed);
                stats["global_tokens_saved"] = json!(global_total.saturating_sub(local));
            }
        }

        stats
    }
}
