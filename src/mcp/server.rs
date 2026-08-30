// Rust guideline compliant 2025-10-17
//! MCP server that reads JSON-RPC 2.0 messages from stdin and writes
//! responses to stdout.
//!
//! The server exposes code graph tools via the Model Context Protocol,
//! allowing AI assistants to query the code graph interactively.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
#[cfg(feature = "test-transport")]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::global_db::GlobalDb;
use crate::tokensave::TokenSave;

use super::graph_scope::{
    collapse_worktree_roots, decode_selected_inputs, merge_federated_results, qualify_result,
    select_graph, validate_local_inputs, GraphSelector, FEDERATABLE_TOOLS,
};
use super::tools::{
    baseline_policy, cap_baseline, get_always_load_tool_definitions, get_tool_definitions,
    handle_tool_call, is_graph_scoped_tool, request_overhead_tokens, schema_overhead_tokens,
    settle_session_debt,
};
use super::transport::{ErrorCode, JsonRpcRequest, JsonRpcResponse};

/// Selector-less local graph tools refused after tracked-branch drift.
pub(crate) const LOCAL_GRAPH_TOOLS_NOT_SUPPORTING_SELECTORS: &[&str] = &[
    "tokensave_affected",
    "tokensave_diff_context",
    "tokensave_simplify_scan",
    "tokensave_redundancy",
    "tokensave_diagnostics",
    "tokensave_diagnose",
];

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

/// How often a running server bothers to *consider* a worldwide-counter upload.
///
/// This is a re-entry guard, not the upload cadence: it only keeps a busy
/// server from reading the user config on every tool call. Whether a request is
/// actually made is `cloud::upload_is_due`, which is daily.
const FLUSH_CHECK_INTERVAL_SECS: i64 = 30;

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
- `start_line`, `end_line` INTEGER — 0-based inclusive line range of the symbol (raw tree-sitter rows; MCP tool responses convert to 1-based editor lines)
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
WHERE kind IN ('function', 'method', 'singleton_method')
ORDER BY lines DESC
LIMIT 20;
```

## Gotchas
- `nodes.id` is a content hash, so it changes when the symbol moves. For cross-run lookups use `qualified_name` (or `tokensave_by_qualified_name`).
- `edges.kind = 'calls'` may reference a *trait method* node rather than the resolved concrete impl — trait dispatch is not currently rewritten.
- `derives_macro` edges record `#[derive(...)]` usage but generated impls are not in the graph.
";

/// Build the per-file staleness banner inserted at the top of any tool
/// response that referenced files the in-line sync couldn't refresh.
///
/// The shape mimics codegraph's #428 banner: name each pending file with
/// its edit age (how long since the on-disk mtime), and direct the agent
/// to `Read` those specific files. The rest of the response is treated
/// as authoritative — distinct from the previous binary "STALE INDEX"
/// warning that asked the agent to distrust the whole answer.
fn format_per_file_staleness_banner(
    project_root: &std::path::Path,
    stale_files: &[String],
) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut lines = Vec::with_capacity(stale_files.len() + 2);
    lines.push(format!(
        "WARNING: {} file(s) referenced below were edited after the last sync. \
         Read these directly; the rest of this response reflects the current index:",
        stale_files.len()
    ));
    for path in stale_files {
        let age = file_mtime_secs(project_root, path).map_or(0, |m| now_secs.saturating_sub(m));
        lines.push(format!("  - {path} (edited {})", humanize_age(age)));
    }
    lines.push("Run `tokensave sync` to refresh the index.".to_string());
    lines.join("\n")
}

/// Read the on-disk mtime (UNIX seconds) for `relative_path` joined onto
/// `project_root`. Returns `None` when the file is missing or stat fails.
fn file_mtime_secs(project_root: &std::path::Path, relative_path: &str) -> Option<i64> {
    let abs = project_root.join(relative_path);
    let meta = std::fs::metadata(&abs).ok()?;
    let modified = meta.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(secs)
}

/// Render a duration in seconds as a compact phrase: `"5s ago"`,
/// `"3m ago"`, `"2h ago"`, `"4d ago"`. Used in the staleness banner so
/// the agent can judge how stale "still stale" actually is.
fn humanize_age(secs: i64) -> String {
    if secs < 60 {
        format!("{}s ago", secs.max(0))
    } else if secs < 3_600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3_600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

fn quote_inert_value(value: &str) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|_| "\"<unavailable>\"".to_string())
        .replace('`', "\\u0060")
}

fn format_selected_fallback_guidance(root: &str, branch: Option<&str>) -> String {
    let root = quote_inert_value(root);
    let guidance = branch.map_or_else(
        || format!("Refresh the selected graph while working from selected project root {root}."),
        |branch| {
            let branch = quote_inert_value(branch);
            format!(
                "Add or refresh selected branch {branch} while working from \
                 selected project root {root}."
            )
        },
    );
    format!("WARNING: Selected graph at {root} is using a fallback index. {guidance}")
}

fn format_selected_fallback_warning(selected: &super::graph_scope::SelectedGraph) -> String {
    format_selected_fallback_guidance(&selected.provenance_root, selected.cg.active_branch())
}

fn format_selected_index_age_warning(
    selected: &super::graph_scope::SelectedGraph,
    age_secs: i64,
) -> String {
    let hours = age_secs / 3600;
    let mins = (age_secs % 3600) / 60;
    let age = if hours >= 24 {
        format!("{}d {}h", hours / 24, hours % 24)
    } else {
        format!("{hours}h {mins}m")
    };
    let root = quote_inert_value(&selected.provenance_root);
    format!(
        "WARNING: Selected graph at {root} was last synced {age} ago. \
         Run Tokensave synchronization from selected project root {root}."
    )
}

/// Cached result of a latest-version check against GitHub releases.
struct VersionCheckState {
    latest: Option<String>,
    checked_at: Option<Instant>,
}

#[cfg(feature = "test-transport")]
struct AccountingTaskGuard(Arc<AtomicUsize>);

#[cfg(feature = "test-transport")]
impl Drop for AccountingTaskGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The MCP server wrapping a `TokenSave` instance.
// Lock ordering: file_token_map -> tool_call_counts (never nested)
pub struct McpServer {
    cg: TokenSave,
    graph_scoped_tools: HashSet<String>,
    stats: ServerStats,
    tool_call_counts: std::sync::Mutex<HashMap<String, u64>>,
    /// Approximate token count per indexed file (`file_path` -> tokens).
    file_token_map: std::sync::Mutex<HashMap<String, u64>>,
    /// One-time approximate token cost of the full tool schema listing
    /// (`tools/list`), charged into `after` on the first *successful*
    /// `tools/call` after [`schema_served`](Self::schema_served) goes true —
    /// see [`handle_tools_call`](Self::handle_tools_call).
    schema_overhead_tokens: u64,
    /// Whether [`handle_tools_list`](Self::handle_tools_list) has served the
    /// schema this session. `schema_overhead_tokens` approximates the
    /// context cost of that payload, so it would be wrong to charge it
    /// against a client that invoked a known tool directly and never
    /// fetched the schema at all. Gates `schema_overhead_charged` below —
    /// a call before `tools/list` has been served leaves the overhead
    /// un-debited so it can still be charged on the next accounted call
    /// once `tools/list` does run.
    schema_served: AtomicBool,
    /// Whether [`schema_overhead_tokens`](Self::schema_overhead_tokens) has
    /// already been charged this session. Tracked independently of
    /// `stats.tool_calls` (which counts every dispatch attempt, including
    /// ones that error before any accounting runs) so a failing first call
    /// can't permanently skip the charge.
    schema_overhead_charged: AtomicBool,
    /// Session-scoped carry-forward debt for [`add_tokens_saved`](Self::add_tokens_saved):
    /// the unpaid shortfall from calls whose `after` exceeded `before` — most
    /// notably the one call that absorbs the one-time schema charge — which
    /// later calls' surplus pays down before anything more is credited to
    /// the persisted counter. See
    /// [`settle_against_session_debt`](Self::settle_against_session_debt).
    session_debt: AtomicI64,
    /// Running total of tokens saved by serving from the graph.
    tokens_saved: AtomicU64,
    /// Tokens already flushed to the worldwide counter this session.
    last_flushed_tokens: AtomicU64,
    /// UNIX timestamp of last worldwide flush (0 = never).
    last_flush_at: AtomicI64,
    /// User-level database tracking all projects (best-effort).
    global_db: Option<GlobalDb>,
    /// Initialized projects sitting directly beside the served root, snapshotted
    /// at startup and named in the `initialize` instructions so a session knows
    /// which other graphs `graph_root` can reach (#375).
    sibling_projects: Vec<String>,
    /// Cached latest-version check result.
    version_cache: std::sync::Mutex<VersionCheckState>,
    /// Pending JSON-RPC notifications to send before the next response.
    pending_notifications: std::sync::Mutex<Vec<Value>>,
    /// When the MCP server was started from a subdirectory of the project root,
    /// this holds the relative path prefix (e.g. `"src/mcp"`). Listing tools
    /// use it as the default path filter. `None` when cwd == project root.
    scope_prefix: Option<String>,
    /// Set to `true` after `shutdown` runs once; makes shutdown idempotent so
    /// callers can invoke it explicitly after `run` returns without re-running
    /// persistence logic.
    shutdown_done: AtomicBool,
    /// Set when [`Self::run`] starts. This guards against accidentally running
    /// the same server loop twice without confusing a replayed `initialize`
    /// request with a previous run: `serve` legitimately calls
    /// [`Self::handle_and_write`] once before entering the loop.
    run_started: AtomicBool,
    /// When true, every `tools/call` response gains a `_meta.duration_us`
    /// field measuring the handler's pure execution time. Toggled by
    /// `tokensave serve --timings`. Off by default to keep responses clean.
    timings_enabled: AtomicBool,
    /// UNIX timestamp (secs) of the most recent staleness check started by
    /// the server. Read-modify-update via `compare_exchange` in
    /// [`maybe_sync_if_stale`](Self::maybe_sync_if_stale) so concurrent
    /// tool calls don't pile on the same walk.
    last_staleness_check_at: AtomicI64,
    /// Cached worktree-vs-index mismatch detection for this session. `None`
    /// when no mismatch exists (the common case) or detection was skipped
    /// (not a git repo / git missing). Computed once at startup so we
    /// spawn at most one pair of `git rev-parse` per session no matter how
    /// many tool calls fire. See [`crate::worktree`] and #312.
    worktree_mismatch: Option<crate::worktree::WorktreeIndexMismatch>,
    /// Flipped to `true` once [`Self::run_startup_catch_up_sync`] finishes
    /// (#414). Production code never reads this; tests poll it via
    /// [`Self::wait_for_startup_catch_up`] so they can race-free assert on
    /// the index state after the detached catch-up task completes.
    startup_catch_up_done: AtomicBool,
    /// Once-gate for the version-aware forced reindex. Flipped to `true` the
    /// first time a `tools/call` evaluates the upgrade check, so the (possibly
    /// expensive) reindex is spawned at most once per session.
    version_reindex_started: AtomicBool,
    /// Flipped to `true` once the version-aware reindex evaluation settles —
    /// either the background reindex finished, the marker was advanced, or no
    /// action was needed. Production code never reads this; tests poll it via
    /// [`Self::wait_for_version_reindex`].
    version_reindex_done: AtomicBool,
    /// Number of global-ledger persistence tasks spawned by this server.
    #[cfg(feature = "test-transport")]
    accounting_tasks_started: AtomicUsize,
    /// Number of spawned global-ledger persistence tasks still running.
    #[cfg(feature = "test-transport")]
    accounting_tasks_pending: Arc<AtomicUsize>,
}

/// Explains why an automatic sync declined to run, for the server's stderr.
///
/// Automatic syncs are the ones the user did not ask for, so a refusal has to
/// say what was skipped and how to do it deliberately — silently serving a
/// stale (or empty) index is the failure mode that made #396 and #393 hard to
/// diagnose from the outside.
fn auto_sync_refusal(scope: &crate::tokensave::AutoSyncScope) -> String {
    match scope {
        crate::tokensave::AutoSyncScope::Uninitialized => {
            "skipping automatic sync: this project has no indexed files yet. \
             Run `tokensave init` to build the index — a background sync will \
             not index a project from scratch (#396)."
                .to_string()
        }
        crate::tokensave::AutoSyncScope::BranchDrifted(drift) => format!(
            "skipping automatic sync: this server is serving branch '{}' but the \
             working tree is now on '{}'. Syncing would write '{}' files into \
             '{}'s index. Restart the MCP server to pick up the branch (#400).",
            drift.serving, drift.working_tree, drift.working_tree, drift.serving
        ),
        crate::tokensave::AutoSyncScope::TooManyStale { count, limit } => format!(
            "skipping automatic sync: {count} files are stale, over the \
             {limit}-file limit for a background sync. Run `tokensave sync` to \
             index them, or raise `max_auto_sync_files` in .tokensave/config.json."
        ),
        // Not a refusal; kept total so a new variant cannot silently become
        // an empty message.
        crate::tokensave::AutoSyncScope::Sync(_) => "automatic sync proceeding".to_string(),
    }
}

/// The deferral rule for [`McpServer::startup_work_in_flight`], as a pure
/// function of the three flags so the truth table can be tested directly.
///
/// The asymmetry between the two jobs is deliberate. Startup catch-up is
/// spawned unconditionally, so its `done` flag always settles and "not done"
/// really does mean "still running". A version reindex may never be triggered
/// at all, so it defers only while *started and not finished* — keying on
/// `!done` alone would make every ordinary session defer forever and the
/// option would silently do nothing.
fn defer_idle_exit(catch_up_done: bool, reindex_started: bool, reindex_done: bool) -> bool {
    !catch_up_done || (reindex_started && !reindex_done)
}

/// Sleep until `d` elapses, or never when there is no deadline.
///
/// Created fresh at each park, so the window always starts whole and never
/// spans request handling.
async fn idle_deadline(d: Option<std::time::Duration>) {
    match d {
        Some(d) => tokio::time::sleep(d).await,
        None => std::future::pending::<()>().await,
    }
}

/// Say why the server is leaving. An idle exit is indistinguishable from a
/// crash in a host's log otherwise, and the whole point of the option is that
/// an operator turned it on and wants to see it working.
fn report_idle_exit(d: Option<std::time::Duration>) {
    if let Some(d) = d {
        eprintln!(
            "[tokensave] idle for {}s with no request — exiting (--idle-timeout-secs)",
            d.as_secs()
        );
    }
}

impl McpServer {
    /// Creates a new MCP server backed by the given code graph.
    ///
    /// Index freshness is maintained by a lazy staleness check
    /// ([`maybe_sync_if_stale`](Self::maybe_sync_if_stale)) invoked at the
    /// start of every `tools/call` and gated by a 30 s cooldown — there
    /// is no background watcher task. This replaces the
    /// `notify-debouncer-full` watcher removed in v6.x (#80), which was
    /// the source of severe CPU and memory pressure on large monorepos
    /// where nested ignored directories (`apps/*/node_modules`,
    /// `packages/*/target`) drove unbounded event traffic and `FileId`
    /// cache growth.
    pub async fn new(cg: TokenSave, scope_prefix: Option<String>) -> Arc<Self> {
        Self::new_inner(cg, scope_prefix, true).await
    }

    /// [`Self::new`] for a server whose project root was named explicitly
    /// (`serve --path <dir>`) rather than discovered from the working
    /// directory. Serving a different repo than the CWD is the *point* of
    /// that mode, so the borrowed-worktree heads-up (#312) — which exists to
    /// catch CWD discovery silently resolving another worktree's index — is
    /// suppressed; its "run `tokensave init` here" remedy is wrong for a
    /// deliberate cross-repo serve (#201).
    pub async fn new_explicit_root(cg: TokenSave, scope_prefix: Option<String>) -> Arc<Self> {
        Self::new_inner(cg, scope_prefix, false).await
    }

    async fn new_inner(
        cg: TokenSave,
        scope_prefix: Option<String>,
        check_worktree_mismatch: bool,
    ) -> Arc<Self> {
        // The DB stores `/`-separated paths on every platform, but the scope
        // prefix is derived from an OS path, so on Windows it arrives with
        // `\` separators and would never match any indexed path (#242).
        let scope_prefix = scope_prefix.map(|p| p.replace('\\', "/"));
        let file_token_map = cg.get_file_token_map().await.unwrap_or_default();
        let graph_scoped_tools = get_tool_definitions()
            .into_iter()
            .filter(is_graph_scoped_tool)
            .map(|definition| definition.name)
            .collect();
        // Approximates the schema payload the client actually loads into
        // context up front. Only the `anthropic/alwaysLoad` tools
        // (`tokensave_search`, `tokensave_context`, `tokensave_status`) are
        // resident from the start; the other ~80 are deferred and never
        // enter context unless the client fetches one on demand, so charging
        // the whole `tools/list` payload here over-stated the up-front cost
        // by more than an order of magnitude.
        let schema_overhead = schema_overhead_tokens(&get_always_load_tool_definitions());
        let persisted = cg.get_tokens_saved().await.unwrap_or(0);
        let global_db = GlobalDb::open().await;
        // Register this project in the global DB with its current tokens
        let mut sibling_projects = Vec::new();
        if let Some(ref gdb) = global_db {
            gdb.upsert(cg.project_root(), persisted).await;
            // Snapshot the neighbouring graphs once, for the initialize
            // instructions (#375). `tokensave_status` re-reads them live, so a
            // project indexed later in the session is still discoverable.
            sibling_projects = gdb.sibling_projects(cg.project_root()).await;
        }

        // Detect borrowed-worktree index once at startup so every read
        // tool can cheaply prefix a heads-up. Two git rev-parse spawns
        // worst case (#312). spawn_blocking because the underlying
        // `Command::output()` can sit on slow disks.
        let worktree_mismatch = if check_worktree_mismatch {
            let project_root = cg.project_root().to_path_buf();
            tokio::task::spawn_blocking(move || {
                let cwd = std::env::current_dir().ok()?;
                crate::worktree::detect_worktree_index_mismatch(&cwd, &project_root)
            })
            .await
            .ok()
            .flatten()
        } else {
            None
        };

        let server = Arc::new(Self {
            cg,
            graph_scoped_tools,
            stats: ServerStats::new(),
            tool_call_counts: std::sync::Mutex::new(HashMap::new()),
            file_token_map: std::sync::Mutex::new(file_token_map),
            schema_overhead_tokens: schema_overhead,
            schema_served: AtomicBool::new(false),
            schema_overhead_charged: AtomicBool::new(false),
            session_debt: AtomicI64::new(0),
            tokens_saved: AtomicU64::new(persisted),
            last_flushed_tokens: AtomicU64::new(persisted),
            last_flush_at: AtomicI64::new(0),
            global_db,
            sibling_projects,
            version_cache: std::sync::Mutex::new(VersionCheckState {
                latest: None,
                checked_at: None,
            }),
            pending_notifications: std::sync::Mutex::new(Vec::new()),
            scope_prefix,
            shutdown_done: AtomicBool::new(false),
            run_started: AtomicBool::new(false),
            timings_enabled: AtomicBool::new(false),
            last_staleness_check_at: AtomicI64::new(0),
            worktree_mismatch,
            startup_catch_up_done: AtomicBool::new(false),
            version_reindex_started: AtomicBool::new(false),
            version_reindex_done: AtomicBool::new(false),
            #[cfg(feature = "test-transport")]
            accounting_tasks_started: AtomicUsize::new(0),
            #[cfg(feature = "test-transport")]
            accounting_tasks_pending: Arc::new(AtomicUsize::new(0)),
        });

        // Catch-up sync (#414): pick up changes made while the server
        // was down — terminal `git pull`, IDE edits before the agent
        // launched, files touched by another tool. Detached and holding
        // only a `Weak`, so a server dropped before the task starts is
        // not resurrected by it; non-blocking so MCP `initialize` doesn't
        // wait on the walk. Note the upgrade below does hold a strong
        // reference for the duration of the sync — the server cannot drop
        // mid-sync — and nothing cancels an in-flight sync when the read
        // loop exits (#396). The scope bound in `find_stale_files_bounded`
        // caps how long that window can be; cooperative cancellation is
        // tracked separately.
        {
            let weak = Arc::downgrade(&server);
            tokio::spawn(async move {
                if let Some(s) = weak.upgrade() {
                    s.run_startup_catch_up_sync().await;
                }
            });
        }

        server
    }

    /// Tools that stay callable when strict mode refuses everything else.
    ///
    /// Refusing *every* tool would leave an agent unable to discover why it was
    /// refused from inside the session that hit it. The exemption is deliberately
    /// one tool: `tokensave_status` reports the server's own state — the root and
    /// branch being served, whether a fallback is active — which is precisely
    /// what a refused caller needs, and it reads no graph content, so it cannot
    /// carry a wrong-tree answer.
    ///
    /// Three tools whose names suggest they belong here do not:
    ///
    /// - `tokensave_config` queries arbitrary TOML/JSON files by key path. It
    ///   has nothing to do with tokensave's own configuration, and knowing a
    ///   `Cargo.toml` value does not help diagnose a refusal.
    /// - `tokensave_diagnose` maps `cargo check`/`clippy` output onto the
    ///   smallest containing **graph node**, with callers attached.
    /// - `tokensave_diagnostics` runs the type-checker and resolves each
    ///   diagnostic's **enclosing graph node**.
    ///
    /// The last two are graph reads wearing diagnostic names: under a wrong-tree
    /// index they would attribute a real compiler error to a node from another
    /// tree, which is the exact failure strict mode exists to prevent.
    const STRICT_MODE_DIAGNOSTIC_TOOLS: &'static [&'static str] = &["tokensave_status"];

    /// Why this call must be refused under strict mode, or `None` to proceed.
    ///
    /// Strict mode (`strict_tree`, default off — #372 §2) turns the two
    /// existing wrong-tree detections from advisory into refusals: a borrowed
    /// worktree index (#312) and a branch that drifted under a running server
    /// (#400). Nothing new is detected here; this only decides what a detection
    /// does.
    ///
    /// The reporter's case for it: every tool built on tokensave inherits a
    /// wrong-tree answer with no signal, and an empty result reads as "no such
    /// symbol" rather than "wrong tree". The case for it staying opt-in: a
    /// shared index across a family of worktrees is a legitimate setup, and
    /// hard-erroring that would be a bad surprise in a point release.
    ///
    /// The message names both trees or branches and the setting responsible, so
    /// the refusal is actionable without reading the docs.
    fn strict_tree_refusal(&self, tool_name: &str) -> Option<String> {
        if !self.cg.get_config().strict_tree {
            return None;
        }
        if Self::STRICT_MODE_DIAGNOSTIC_TOOLS.contains(&tool_name) {
            return None;
        }

        if let Some(m) = &self.worktree_mismatch {
            return Some(format!(
                "refusing: strict_tree is enabled and this index belongs to a different git \
                 working tree. Running in '{}', index from '{}'. Run `tokensave init` here for \
                 a worktree-local index, or set strict_tree to false to answer with a warning \
                 instead.",
                m.worktree_root.display(),
                m.index_root.display()
            ));
        }

        // Re-checked per call, unlike the worktree mismatch above: drift is
        // caused by a `git checkout` during the session, so a value computed
        // at startup would always report none.
        if let Some(drift) = self.cg.branch_drift() {
            return Some(format!(
                "refusing: strict_tree is enabled and this server is serving branch '{}' while \
                 the working tree is on '{}'. Restart the MCP server to serve this branch, or \
                 set strict_tree to false to answer with a warning instead.",
                drift.serving, drift.working_tree
            ));
        }

        None
    }

    /// Refuse local graph calls when a live multi-branch server has drifted.
    ///
    /// Checked per call (a `git checkout` under a running server is what
    /// drifts), before freshness work and dispatch, and only for local graph
    /// tools: `tokensave_status` stays callable for diagnosis, tools without
    /// graph reads are unaffected, and explicit external `graph_root`
    /// selections never reach this gate.
    fn branch_drift_refusal(&self, tool_name: &str) -> Option<String> {
        if tool_name == "tokensave_status"
            || (!self.graph_scoped_tools.contains(tool_name)
                && !LOCAL_GRAPH_TOOLS_NOT_SUPPORTING_SELECTORS.contains(&tool_name))
        {
            return None;
        }

        let drift = self.cg.branch_drift()?;
        Some(format!(
            "refusing: MCP server serves branch '{}' while the working tree is on '{}'. \
             Restart the MCP server to serve this branch, or reopen it for the working branch.",
            drift.serving, drift.working_tree
        ))
    }

    /// Answers one query across several `graph_root`s (#376).
    ///
    /// Runs the ordinary selected-graph pipeline once per root — open, decode
    /// inputs, dispatch, qualify — then interleaves the per-root payloads
    /// round-robin by rank. Scores are BM25-derived per database and are not
    /// calibrated between them, so sorting them together would compare numbers
    /// that do not share a scale; rank is the only ordering both roots agree on.
    ///
    /// Roots that are worktrees of a repository already named are collapsed
    /// first, and the response says which and why. Without that, a caller with
    /// a repo and its worktrees among their roots gets a result set full of the
    /// same symbol at slightly different line numbers, and a per-root cap does
    /// not help because each worktree is its own root.
    ///
    /// A root that fails to open is reported inline rather than failing the
    /// whole call: with several roots named, one unreadable index should not
    /// cost the caller the answers from the others.
    async fn handle_federated_call(
        self: &Arc<Self>,
        id: Value,
        tool_name: &str,
        arguments: &Value,
        selection: &super::graph_scope::GraphSelection,
    ) -> JsonRpcResponse {
        let (kept, collapsed) = collapse_worktree_roots(selection.roots.clone());
        let branch = selection.branch.clone();

        let mut parts: Vec<(String, crate::mcp::tools::ToolResult)> = Vec::new();
        let mut failures: Vec<String> = Vec::new();

        for root in kept {
            let selector = GraphSelector {
                root: root.clone(),
                branch: branch.clone(),
            };
            let selected = match select_graph(selector, self.cg.project_root()).await {
                Ok(selected) => selected,
                Err(error) => {
                    failures.push(format!("{}: {error}", root.display()));
                    continue;
                }
            };
            let mut root_args = arguments.clone();
            if let Err(error) = decode_selected_inputs(&selected, &mut root_args) {
                failures.push(format!("{}: {error}", root.display()));
                continue;
            }
            let outcome = handle_tool_call(&selected.cg, tool_name, root_args, None, None).await;
            let mut result = match outcome {
                Ok(result) => result,
                Err(error) => {
                    failures.push(format!("{}: {error}", root.display()));
                    continue;
                }
            };
            if let Err(error) = qualify_result(&selected, &mut result).await {
                failures.push(format!("{}: {error}", root.display()));
                continue;
            }
            parts.push((selected.provenance_root.clone(), result));
        }

        if parts.is_empty() {
            return JsonRpcResponse::error(
                id,
                ErrorCode::InvalidParams,
                format!("no graph_root could be queried: {}", failures.join("; ")),
            );
        }

        let mut merged = merge_federated_results(parts, &collapsed);
        if !failures.is_empty() {
            if let Some(content) = merged
                .value
                .get_mut("content")
                .and_then(|c| c.as_array_mut())
            {
                content.push(json!({
                    "type": "text",
                    "text": format!(
                        "WARNING: {} root(s) could not be queried and are absent from these \
                         results: {}",
                        failures.len(),
                        failures.join("; ")
                    )
                }));
            }
        }
        JsonRpcResponse::success(id, merged.value)
    }

    /// Returns the active scope prefix, if the server was launched from a subdirectory.
    pub fn scope_prefix(&self) -> Option<&str> {
        self.scope_prefix.as_deref()
    }

    /// Enables or disables per-call timing reporting. When enabled, every
    /// `tools/call` response gains a `_meta.duration_us` field with the
    /// handler's pure execution time in microseconds. Useful for profiling
    /// where time is spent inside the index vs. on the JSON-RPC/stdio
    /// transport. Safe to flip at any time — the next call observes the
    /// new setting.
    pub fn set_timings_enabled(&self, enabled: bool) {
        self.timings_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    /// Returns whether timing reporting is currently enabled.
    pub fn timings_enabled(&self) -> bool {
        self.timings_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Test-only accessor for the backing `TokenSave`. Exposed so
    /// integration tests can drive the staleness pipeline directly,
    /// bypassing the 30 s cooldown in
    /// [`maybe_sync_if_stale`](Self::maybe_sync_if_stale).
    #[doc(hidden)]
    pub fn cg(&self) -> &TokenSave {
        &self.cg
    }

    /// Sums the raw (full-file) approximate token weight of the given files
    /// from the cached `file_token_map`. This is the *unadjusted* baseline —
    /// callers must run it through [`cap_baseline`] per the tool's
    /// [`baseline_policy`] before treating it as a "before" figure, since
    /// most tools return references rather than full file content.
    fn touched_file_tokens(&self, file_paths: &[String]) -> u64 {
        if file_paths.is_empty() {
            return 0;
        }
        debug_assert!(
            file_paths.iter().all(|p| !p.is_empty()),
            "touched_file_tokens received empty file path"
        );
        let Ok(map) = self.file_token_map.lock() else {
            return 0;
        };
        file_paths
            .iter()
            .filter_map(|path| map.get(path.as_str()))
            .sum()
    }

    /// Settles a call's raw signed savings (`before as i64 - after as i64`)
    /// against `session_debt` via [`settle_session_debt`], returning the
    /// non-negative amount that should actually be credited to the
    /// persisted counter this call. Only this aggregate is saturated this
    /// way — the per-call displayed `saved=` figure and the ledger's
    /// persisted rows are unaffected.
    fn settle_against_session_debt(&self, raw_delta: i64) -> u64 {
        let mut credited = 0u64;
        let _ = self
            .session_debt
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |debt| {
                let (new_debt, c) = settle_session_debt(debt, raw_delta);
                credited = c;
                Some(new_debt)
            });
        credited
    }

    /// Adds `net` (already `before - after`, baseline-capped and
    /// overhead-inclusive) to the running saved-tokens counter and persists
    /// it to the database. No-op when `net` is 0 (nothing was saved, or the
    /// call cost more than its baseline).
    async fn add_tokens_saved(&self, net: u64) {
        if net == 0 {
            return;
        }
        let new_total = self.tokens_saved.fetch_add(net, Ordering::Relaxed) + net;
        // Persist to DB (best-effort, don't block on failure)
        let _ = self.cg.set_tokens_saved(new_total).await;
        // Also increment the resettable local counter
        let _ = self.cg.add_local_counter(net).await;
        // Best-effort update to global DB
        if let Some(ref gdb) = self.global_db {
            gdb.upsert(self.cg.project_root(), new_total).await;
        }
    }

    /// Re-read the file-to-token-count map from the DB and swap it into the
    /// cached `file_token_map`. Called after each lazy sync triggered by
    /// [`maybe_sync_if_stale`](Self::maybe_sync_if_stale) so the accounting
    /// tracks newly indexed / removed files.
    pub async fn refresh_file_token_map(&self) {
        // best-effort; leave stale map in place if the DB read fails
        let Ok(fresh) = self.cg.get_file_token_map().await else {
            return;
        };
        if let Ok(mut guard) = self.file_token_map.lock() {
            *guard = fresh;
        }
    }

    /// Catch-up sync run once at startup (#414). Bypasses the 30 s
    /// cooldown in [`Self::maybe_sync_if_stale`] so changes made while
    /// the server was down — a terminal `git pull`, IDE edits before
    /// the agent launched, files touched by another tool — are
    /// reconciled by the time the first MCP tool call arrives. The
    /// staleness-check stamp is updated on the way out so the first
    /// tool call doesn't re-walk the tree.
    ///
    /// The completion flag is flipped on every exit path (including
    /// errors) so [`Self::wait_for_startup_catch_up`] never hangs.
    pub async fn run_startup_catch_up_sync(&self) {
        let stale = match self.cg.find_stale_files_bounded().await {
            crate::tokensave::AutoSyncScope::Sync(stale) => stale,
            scope => {
                eprintln!("[tokensave] {}", auto_sync_refusal(&scope));
                self.startup_catch_up_done.store(true, Ordering::Release);
                return;
            }
        };
        if !stale.is_empty() {
            if let Err(e) = self.cg.sync_if_stale_silent(&stale).await {
                eprintln!("[tokensave] startup catch-up sync failed: {e}");
                self.startup_catch_up_done.store(true, Ordering::Release);
                return;
            }
        }
        self.refresh_file_token_map().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.last_staleness_check_at.store(now, Ordering::Release);
        self.startup_catch_up_done.store(true, Ordering::Release);
    }

    /// Returns `true` once the detached
    /// [`Self::run_startup_catch_up_sync`] task has finished (success
    /// or error). Production code never needs this — the MCP loop runs
    /// regardless of catch-up state — but tests poll it to avoid
    /// racing the catch-up task against later DB assertions.
    pub fn startup_catch_up_done(&self) -> bool {
        self.startup_catch_up_done.load(Ordering::Acquire)
    }

    /// Polls [`Self::startup_catch_up_done`] with a 25 ms interval up
    /// to `timeout`, returning `true` if catch-up completed within the
    /// budget. Tests use this to make the otherwise-detached #414
    /// task observable.
    pub async fn wait_for_startup_catch_up(&self, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while !self.startup_catch_up_done() {
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        true
    }

    /// Walk the project tree, sync any stale files, and refresh the
    /// file-to-token-count map — but only if at least 30 s have passed
    /// since the last successful sync. The cooldown is the gate: while
    /// it holds, this returns immediately, so dropping it into every
    /// `tools/call` handler is cheap.
    ///
    /// Concurrent callers are serialized via
    /// [`Self::last_staleness_check_at`]: the first caller stamps `now`
    /// into the field with `compare_exchange`; later callers within the
    /// same window see the stamp and bail. If the actual sync work
    /// fails, the stamp still advances — failure to walk the tree
    /// should not cause every subsequent tool call to retry.
    pub async fn maybe_sync_if_stale(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let last_sync = self.cg.last_sync_timestamp().await;
        if now.saturating_sub(last_sync) < 30 {
            return;
        }

        let previous = self.last_staleness_check_at.load(Ordering::Acquire);
        if now.saturating_sub(previous) < 30 {
            return;
        }
        if self
            .last_staleness_check_at
            .compare_exchange(previous, now, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let stale = match self.cg.find_stale_files_bounded().await {
            crate::tokensave::AutoSyncScope::Sync(stale) => stale,
            scope => {
                eprintln!("[tokensave] {}", auto_sync_refusal(&scope));
                return;
            }
        };
        if !stale.is_empty() {
            if let Err(e) = self.cg.sync_if_stale_silent(&stale).await {
                eprintln!("[tokensave] lazy sync failed: {e}");
                return;
            }
        }
        // Always refresh: a sibling MCP peer may have synced the DB
        // between our cooldown windows, in which case `stale` is empty
        // here but our in-memory `file_token_map` is still pre-sync.
        self.refresh_file_token_map().await;
    }

    /// On the first `tools/call` of the session, force a per-project reindex if
    /// a major version bump or a stale DB schema requires it.
    ///
    /// "Needed" is true when the recorded `last_indexed_version` → running
    /// transition classifies as [`crate::cloud::BumpKind::Major`] **or** the
    /// project DB schema is older than this build's latest. An empty
    /// `last_indexed_version` (pre-7.0 projects) is treated as needing a reindex
    /// so the latest schema columns are backfilled.
    ///
    /// When needed, a forced full reindex runs in a detached background task so
    /// the triggering tool response is never blocked. On success the marker is
    /// advanced to the running version and the project config is saved. When not
    /// needed but the marker is merely behind, the marker is advanced without a
    /// reindex. The whole evaluation runs at most once per session, gated by
    /// [`Self::version_reindex_started`].
    fn maybe_reindex_on_version_bump(self: &Arc<Self>) {
        // Once-gate: only the first caller proceeds.
        if self
            .version_reindex_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let Some(server) = weak.upgrade() else {
                return;
            };
            server.run_version_reindex().await;
            server.version_reindex_done.store(true, Ordering::Release);
        });
    }

    /// Evaluates and, if required, performs the version-aware forced reindex.
    ///
    /// Best-effort: any failure is logged, never panics, and still advances the
    /// session gate so the work is not retried in a tight loop.
    async fn run_version_reindex(&self) {
        let running = env!("CARGO_PKG_VERSION");
        let project_root = self.cg.project_root().to_path_buf();

        let mut config = match crate::config::load_config(&project_root) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[tokensave] version reindex: failed to load config: {e}");
                return;
            }
        };

        let bump = crate::cloud::bump_kind(&config.last_indexed_version, running);
        let needs_reindex =
            bump == crate::cloud::BumpKind::Major || self.cg.needs_schema_upgrade().await;

        if needs_reindex {
            eprintln!(
                "[tokensave] major upgrade or schema change detected \
                 (indexed by {:?}, running {running}) — forcing project reindex…",
                config.last_indexed_version
            );
            if let Err(e) = self.cg.index_all().await {
                eprintln!("[tokensave] version reindex failed: {e}");
                return;
            }
            self.refresh_file_token_map().await;
            config.last_indexed_version = running.to_string();
            if let Err(e) = crate::config::save_config(&project_root, &config) {
                eprintln!("[tokensave] version reindex: failed to save config: {e}");
            }
        } else if config.last_indexed_version != running {
            // No reindex needed (patch/minor/none) but advance the marker so we
            // don't keep re-evaluating across sessions.
            config.last_indexed_version = running.to_string();
            if let Err(e) = crate::config::save_config(&project_root, &config) {
                eprintln!("[tokensave] version marker advance: failed to save config: {e}");
            }
        }
    }

    /// Returns `true` once the version-aware reindex evaluation has settled.
    ///
    /// Production code never needs this; tests poll it to make the otherwise
    /// detached background task observable.
    pub fn version_reindex_done(&self) -> bool {
        self.version_reindex_done.load(Ordering::Acquire)
    }

    /// Returns whether the version-aware reindex gate has been evaluated.
    #[cfg(feature = "test-transport")]
    #[doc(hidden)]
    pub fn version_reindex_started(&self) -> bool {
        self.version_reindex_started.load(Ordering::Acquire)
    }

    /// Polls [`Self::version_reindex_done`] every 25 ms up to `timeout`.
    ///
    /// Returns `true` if the evaluation settled within the budget.
    pub async fn wait_for_version_reindex(&self, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while !self.version_reindex_done() {
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        true
    }

    /// Returns the number of global-ledger persistence tasks spawned.
    #[cfg(feature = "test-transport")]
    #[doc(hidden)]
    pub fn accounting_tasks_started(&self) -> usize {
        self.accounting_tasks_started.load(Ordering::Acquire)
    }

    /// Returns the number of global-ledger persistence tasks still running.
    #[cfg(feature = "test-transport")]
    #[doc(hidden)]
    pub fn accounting_tasks_pending(&self) -> usize {
        self.accounting_tasks_pending.load(Ordering::Acquire)
    }

    /// Waits until all spawned global-ledger persistence tasks have settled.
    #[cfg(feature = "test-transport")]
    #[doc(hidden)]
    pub async fn wait_for_accounting_idle(&self, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while self.accounting_tasks_pending() != 0 {
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::task::yield_now().await;
        }
        true
    }

    /// Internal: snapshot of the current `file_token_map`. Exposed for
    /// integration tests only; not part of the stable public API.
    #[doc(hidden)]
    pub fn file_token_map_snapshot(&self) -> HashMap<String, u64> {
        self.file_token_map
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Uploads the accumulated saved-token delta to the worldwide counter, at
    /// most once a day. Best-effort, never blocks for long.
    ///
    /// Two gates, doing different jobs. The in-process one keeps a busy server
    /// from loading the user config on every single tool call; the daily one in
    /// `cloud::upload_is_due` decides whether a request is actually made, and is
    /// shared with the CLI so both cadences are the same decision.
    ///
    /// Nothing is lost by declining to upload: `last_flushed_tokens` advances
    /// only on success, so the delta is re-derived from the running total next
    /// time. That is also why the accumulated total is not *persisted* when the
    /// upload is skipped — writing it while `last_flushed_tokens` stays put
    /// would count the same tokens twice.
    async fn maybe_flush_worldwide(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let last = self.last_flush_at.load(Ordering::Relaxed);
        if now - last < FLUSH_CHECK_INTERVAL_SECS {
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
            if !crate::cloud::upload_is_due(&config, now) {
                // Deliberately not saved: see the note above on double counting.
                return false;
            }
            config.last_flush_attempt_at = now;
            if crate::cloud::flush_pending(config.pending_upload).is_some() {
                config.pending_upload = 0;
                config.last_upload_at = now;
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
        if !crate::cloud::update_check_enabled() {
            return None;
        }
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
        self: &Arc<Self>,
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
            // `write_line` expects the caller to terminate the line. Without
            // the `\n` a line-framed MCP client never sees the handshake
            // response complete and hangs waiting for the rest of the line.
            let _ = transport.write_line(&format!("{json_str}\n")).await;
            let _ = transport.flush().await;
        }
    }

    /// Runs the server, reading JSON-RPC requests from stdin and writing
    /// responses to stdout. Runs until stdin is closed or a shutdown signal
    /// (SIGINT/SIGTERM) is received, then performs graceful cleanup.
    pub async fn run(
        self: &Arc<Self>,
        transport: &mut impl super::transport::McpTransport,
    ) -> Result<()> {
        self.run_with_idle_timeout(transport, None).await
    }

    /// Is a detached startup job still running?
    ///
    /// The idle deadline must not cut one of these off. Both are spawned
    /// rather than awaited, so a server can look idle — no request in flight,
    /// nothing on stdin — while it is still doing the work a client is about
    /// to depend on. Startup catch-up is always spawned, so its `done` flag
    /// settles either way; the version reindex is checked as
    /// started-and-not-finished, because a session that never triggers one
    /// must not be treated as forever busy.
    fn startup_work_in_flight(&self) -> bool {
        defer_idle_exit(
            self.startup_catch_up_done.load(Ordering::Acquire),
            self.version_reindex_started.load(Ordering::Acquire),
            self.version_reindex_done.load(Ordering::Acquire),
        )
    }

    /// Run the MCP loop, optionally exiting after `idle_timeout` passes with
    /// no request (#436).
    ///
    /// A host that keeps a finished subagent's server alive never closes its
    /// stdin, so the EOF that would normally stop the server never arrives and
    /// one server accumulates per subagent, each holding its index open. That
    /// is the host's bug to fix — the servers are all children of the same
    /// still-live supervisor, so there is no dead-parent signal to key on
    /// either — but a deadline bounds the damage without waiting for it.
    ///
    /// Off unless asked for. Whether this is safe depends on the host starting
    /// a fresh server when a tool is called after an idle exit, which varies by
    /// host and is not something tokensave can detect, so the default stays
    /// today's indefinite lifetime.
    ///
    /// The deadline is evaluated **only** while parked waiting for the next
    /// line, never during request handling: the timer is created fresh each
    /// time the loop parks, so a request that takes longer than the timeout
    /// cannot be interrupted by it, and the window after it starts whole.
    /// Requests are handled serially, so no in-flight counter is needed.
    pub async fn run_with_idle_timeout(
        self: &Arc<Self>,
        transport: &mut impl super::transport::McpTransport,
        idle_timeout: Option<std::time::Duration>,
    ) -> Result<()> {
        let already_started = self.run_started.swap(true, Ordering::Relaxed);
        debug_assert!(
            !already_started,
            "server run() called on an already-used server"
        );

        // Registered once, for the life of the loop, rather than per
        // iteration (#450/#436).
        //
        // Creating the stream inside the loop meant it existed only while
        // `select!` awaited the next line, and was dropped before
        // `handle_request` ran. Tokio's registration is process-global and is
        // *not* undone on drop, so the default disposition — terminate — was
        // permanently replaced after the first iteration, while for most of a
        // busy server's life nothing was listening. A SIGTERM delivered during
        // request handling therefore neither killed the process nor reached
        // the loop: the next iteration built a fresh stream, which cannot see
        // an event delivered before it existed. That is the reported
        // behaviour, a server that ignores `kill` and needs `SIGKILL`.
        //
        // Held across the whole loop, the stream coalesces and retains a
        // signal delivered while it is not being polled, so a SIGTERM arriving
        // mid-request is observed at the top of the next iteration and the
        // server leaves through the normal graceful shutdown. Note this waits
        // for the in-flight request to finish; interrupting a long sync
        // mid-flight is a separate decision, tracked on #450.
        #[cfg(unix)]
        #[allow(clippy::expect_used)]
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");

        'serve: loop {
            // The inner loop exists only so a deferred idle expiry can re-arm:
            // a deadline that lands while a detached startup job is still
            // running is not evidence the server is finished with, so it waits
            // out another whole window rather than exiting or being ignored.
            let line: String = 'wait: loop {
                #[cfg(unix)]
                {
                    tokio::select! {
                        result = transport.read_line() => {
                            match result {
                                Ok(Some(line)) => break 'wait line,
                                _ => break 'serve,
                            }
                        }
                        _ = tokio::signal::ctrl_c() => break 'serve,
                        _ = sigterm.recv() => break 'serve,
                        // Set by the process-wide handler in `cancel`, which
                        // observes a signal the moment it lands rather than
                        // only while this loop is parked here — and by the
                        // orphan watchdog, which has no signal to deliver at
                        // all (#450).
                        () = crate::cancel::cancelled() => break 'serve,
                        () = idle_deadline(idle_timeout) => {
                            if self.startup_work_in_flight() {
                                continue 'wait;
                            }
                            report_idle_exit(idle_timeout);
                            break 'serve;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    tokio::select! {
                        result = transport.read_line() => {
                            match result {
                                Ok(Some(line)) => break 'wait line,
                                _ => break 'serve,
                            }
                        }
                        _ = tokio::signal::ctrl_c() => break 'serve,
                        () = crate::cancel::cancelled() => break 'serve,
                        () = idle_deadline(idle_timeout) => {
                            if self.startup_work_in_flight() {
                                continue 'wait;
                            }
                            report_idle_exit(idle_timeout);
                            break 'serve;
                        }
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
                    break 'serve;
                }
                if let Err(e) = transport.flush().await {
                    eprintln!("failed to flush stdout: {e}");
                    break 'serve;
                }
            }
        }

        self.shutdown().await;
        Ok(())
    }

    /// Persists the tokens-saved counter, flushes pending tokens to the
    /// worldwide counter, checkpoints the WAL, and logs a session summary.
    ///
    /// Idempotent — safe to call multiple times. `run` invokes it once when
    /// its main loop exits; callers (e.g. `main.rs`, tests) may invoke it
    /// explicitly afterwards without re-running the persistence logic.
    pub async fn shutdown(&self) {
        // Idempotency guard: only run the persistence path once.
        if self.shutdown_done.swap(true, Ordering::SeqCst) {
            return;
        }

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

        // Record the remaining delta the periodic flushes did not upload, and
        // upload it only if a day has passed. Unlike the periodic path this
        // always *persists* the accumulated total: the process is ending, so
        // `last_flushed_tokens` is about to be lost and the config file is the
        // only thing that will still remember these tokens.
        let last_flushed = self.last_flushed_tokens.load(Ordering::Relaxed);
        if tokens_saved > last_flushed {
            let delta = tokens_saved - last_flushed;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let mut config = crate::user_config::UserConfig::load();
            config.pending_upload += delta;
            if crate::cloud::upload_is_due(&config, now) {
                config.last_flush_attempt_at = now;
                if crate::cloud::flush_pending(config.pending_upload).is_some() {
                    config.pending_upload = 0;
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
    pub(crate) async fn handle_request(
        self: &Arc<Self>,
        request: &JsonRpcRequest,
    ) -> Option<JsonRpcResponse> {
        debug_assert!(
            !request.method.is_empty(),
            "handle_request called with empty method"
        );
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);
        let id = request.id.clone();

        let result = match request.method.as_str() {
            "initialize" => Some(Self::handle_initialize(
                id,
                self.cg.report_savings(),
                &self.sibling_projects,
            )),
            "initialized" => {
                // Notification - no response required
                None
            }
            "notifications/initialized" => {
                // Alternative notification path - no response required
                None
            }
            "tools/list" => Some(self.handle_tools_list(id)),
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
    ///
    /// `report_savings` gates the closing sentence only. Asking the agent to
    /// report savings makes it spend output tokens narrating them on nearly
    /// every turn, which for some users offsets the input-token win the server
    /// exists to deliver (#356); when the setting is off the sentence is
    /// omitted, and `handle_tools_call` correspondingly stops appending the
    /// `tokensave_metrics:` line it refers to.
    fn handle_initialize(
        id: Value,
        report_savings: bool,
        sibling_projects: &[String],
    ) -> JsonRpcResponse {
        const BASE_INSTRUCTIONS: &str = "tokensave is a code-graph MCP server. \
            Start with tokensave_context for any code exploration task \
            — it returns relevant symbols, relationships, and code \
            snippets for a natural-language query. Use tokensave_search \
            to find specific symbols by name. Discovery and analysis \
            tools are read-only and safe to call in parallel. Edit \
            and session-memory tools can mutate local project state \
            and declare readOnlyHint=false.";
        const REPORT_SAVINGS_INSTRUCTION: &str =
            " When a tool result contains a `tokensave_metrics:` line, \
             report the savings to the user (e.g. 'TokenSave'd ~N tokens').";

        let mut instructions = BASE_INSTRUCTIONS.to_string();
        if report_savings {
            instructions.push_str(REPORT_SAVINGS_INSTRUCTION);
        }
        // Sibling checkouts are queryable through `graph_root` but nothing else
        // reveals that they exist, so a cross-repo session reads an empty result
        // as "no such symbol" instead of retrying next door (#375).
        if !sibling_projects.is_empty() {
            let _ = write!(
                instructions,
                " These other initialized projects sit beside this one and can be \
                 queried by passing graph_root: {}.",
                sibling_projects.join(", ")
            );
        }

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
                "instructions": instructions
            }),
        )
    }

    /// Handles the `tools/list` method, returning all available tool definitions.
    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let tools = get_tool_definitions();
        // Marks the schema as actually delivered so `handle_tools_call` knows
        // it's fair to debit `schema_overhead_tokens` against this session —
        // see `schema_served`.
        self.schema_served.store(true, Ordering::Relaxed);
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
    async fn handle_tools_call(
        self: &Arc<Self>,
        id: Value,
        params: Option<&Value>,
    ) -> JsonRpcResponse {
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

        let mut arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        // Request-side cost of this call (tool name + arguments + JSON-RPC
        // framing) — computed before graph selectors or qualified IDs are
        // removed from the mutable dispatch arguments below. Folded into
        // `after` alongside the response text, since the model pays for
        // sending the original call.
        let request_overhead = request_overhead_tokens(tool_name, &arguments);

        // Count every named tool attempt before selector validation. Rejected
        // selectors are still deliberate calls, but they must not reach
        // freshness work or handler dispatch.
        self.stats.tool_calls.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut counts) = self.tool_call_counts.lock() {
            *counts.entry(tool_name.to_string()).or_insert(0) += 1;
        }

        let has_selector = arguments.as_object().is_some_and(|object| {
            object.contains_key("graph_root") || object.contains_key("graph_branch")
        });
        if has_selector && !self.graph_scoped_tools.contains(tool_name) {
            return JsonRpcResponse::error(
                id,
                ErrorCode::InvalidParams,
                format!("tool '{tool_name}' does not support graph_root or graph_branch selectors"),
            );
        }

        let selection = if has_selector {
            match GraphSelector::take(&mut arguments) {
                Ok(Some(selection)) => Some(selection),
                Ok(None) => unreachable!("has_selector guarantees a selector field"),
                Err(error) => {
                    return JsonRpcResponse::error(
                        id,
                        ErrorCode::InvalidParams,
                        format!("invalid graph selector: {error}"),
                    );
                }
            }
        } else {
            None
        };

        // Federation (#376). Handled before the single-graph pipeline because
        // it runs that pipeline once per root and merges. Selected calls are
        // already outside savings accounting (#363), so an early return here
        // loses nothing a single-root selected call would have recorded.
        if let Some(selection) = selection.as_ref() {
            if selection.is_federated() {
                if !FEDERATABLE_TOOLS.contains(&tool_name) {
                    return JsonRpcResponse::error(
                        id,
                        ErrorCode::InvalidParams,
                        format!(
                            "tool '{tool_name}' answers about a single graph, so it accepts one \
                             graph_root rather than an array; a union of two graphs is not a \
                             larger answer. Federation is available for: {}",
                            FEDERATABLE_TOOLS.join(", ")
                        ),
                    );
                }
                return self
                    .handle_federated_call(id, tool_name, &arguments, selection)
                    .await;
            }
        }

        let selected = if let Some(selection) = selection {
            let selector = selection
                .selectors()
                .into_iter()
                .next()
                .unwrap_or_else(|| unreachable!("a selection always names at least one root"));
            let selected = match select_graph(selector, self.cg.project_root()).await {
                Ok(selected) => selected,
                Err(TokenSaveError::Config { message }) => {
                    return JsonRpcResponse::error(
                        id,
                        ErrorCode::InvalidParams,
                        format!("invalid graph selector: {message}"),
                    );
                }
                Err(error) => {
                    return JsonRpcResponse::error(
                        id,
                        ErrorCode::InternalError,
                        format!("failed to open selected graph: {error}"),
                    );
                }
            };
            if let Err(error) = decode_selected_inputs(&selected, &mut arguments) {
                return JsonRpcResponse::error(
                    id,
                    ErrorCode::InvalidParams,
                    format!("invalid selected graph arguments: {error}"),
                );
            }
            Some(selected)
        } else {
            if let Err(error) = validate_local_inputs(&arguments) {
                return JsonRpcResponse::error(
                    id,
                    ErrorCode::InvalidParams,
                    format!("invalid local graph arguments: {error}"),
                );
            }
            None
        };

        if selected.is_none() {
            if let Some(reason) = self.branch_drift_refusal(tool_name) {
                return JsonRpcResponse::error(id, ErrorCode::InvalidRequest, reason);
            }

            // Strict mode (#372 §2): refuse rather than answer from a tree the
            // user is not in. Checked before freshness and before dispatch, so
            // a refused call does no work. Selected graphs are exempt — naming
            // `graph_root` is an explicit request for another project's
            // snapshot, so "this isn't your working tree" is the point.
            if let Some(reason) = self.strict_tree_refusal(tool_name) {
                return JsonRpcResponse::error(id, ErrorCode::InvalidRequest, reason);
            }

            // Notification-free freshness: walk the local tree and resync any
            // stale files, gated by a 30 s cooldown. Explicitly selected
            // graphs are read-only snapshots and never run local repair.
            self.maybe_sync_if_stale().await;

            // Evaluate the local project's version-aware reindex on every
            // local path. The atomic once-gate keeps this at most once per
            // session without letting a selected call consume the check.
            self.maybe_reindex_on_version_bump();
        }

        eprintln!("[tokensave] tool call: {tool_name}");

        let server_stats = if selected.is_none() && tool_name == "tokensave_status" {
            Some(self.server_stats_json().await)
        } else {
            None
        };

        let timings_enabled = self.timings_enabled();
        let handler_start = if timings_enabled {
            Some(std::time::Instant::now())
        } else {
            None
        };
        let (dispatch_graph, scope_prefix) = selected.as_ref().map_or_else(
            || (&self.cg, self.scope_prefix()),
            |selected| (&selected.cg, None),
        );
        let dispatch_outcome = handle_tool_call(
            dispatch_graph,
            tool_name,
            arguments,
            server_stats,
            scope_prefix,
        )
        .await;
        let handler_elapsed_us = handler_start.map(|t| t.elapsed().as_micros() as u64);
        match dispatch_outcome {
            Ok(mut result) => {
                if let Some(us) = handler_elapsed_us {
                    let obj = result.value.as_object_mut();
                    if let Some(map) = obj {
                        let meta = map.entry("_meta").or_insert_with(|| json!({}));
                        if let Some(meta_obj) = meta.as_object_mut() {
                            meta_obj.insert("duration_us".to_string(), json!(us));
                        }
                    }
                }
                if let Some(selected) = selected.as_ref() {
                    if let Err(error) = qualify_result(selected, &mut result).await {
                        return JsonRpcResponse::error(
                            id,
                            ErrorCode::InternalError,
                            format!("tool execution failed: {error}"),
                        );
                    }

                    // Selected graphs are read-only and intentionally skip
                    // all local repair, accounting, and current-project
                    // warnings. Only warnings recorded by the selected graph
                    // itself are relevant to this response.
                    if selected.cg.fallback_warning().is_some() {
                        let warning = format_selected_fallback_warning(selected);
                        if let Some(content) = result
                            .value
                            .get_mut("content")
                            .and_then(|content| content.as_array_mut())
                        {
                            content.insert(0, json!({"type": "text", "text": warning}));
                        }
                    }
                    let last_time = selected.cg.last_sync_timestamp().await;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let age_secs = now - last_time;
                    if last_time > 0 && age_secs > 3600 {
                        let warning = format_selected_index_age_warning(selected, age_secs);
                        if let Some(content) = result
                            .value
                            .get_mut("content")
                            .and_then(|content| content.as_array_mut())
                        {
                            content.insert(0, json!({"type": "text", "text": warning}));
                        }
                    }

                    crate::memstats::record(tool_name);
                    return JsonRpcResponse::success(id, result.value);
                }

                // Estimate approximate token count of the tool's own answer,
                // before any of the warnings/banners below are attached.
                // Used as the baseline-cap basis (see `before_tokens` below)
                // so a large staleness banner can't loosen the cap on a
                // `Reference` tool's savings — the cap should track what the
                // *answer* delivered, not incidental warning text.
                let tool_response_tokens: u64 = result
                    .value
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map_or(0, |arr| {
                        let total_bytes: usize = arr
                            .iter()
                            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                            .map(str::len)
                            .sum();
                        (total_bytes / 4) as u64
                    });

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

                // Per-file staleness banner (#428 design): files this response
                // referenced that are still pending after the in-line sync
                // attempt get a focused banner naming them with edit ages,
                // telling the agent to Read THOSE files directly while
                // treating the rest of the response as authoritative.
                // Replaces the previous all-or-nothing "STALE INDEX"
                // warning that made agents distrust the entire answer.
                if !result.touched_files.is_empty() {
                    let stale_files = self.cg.check_file_staleness(&result.touched_files).await;
                    if !stale_files.is_empty() {
                        let still_stale = match self.cg.sync_if_stale(&stale_files).await {
                            Ok(false) => false,        // sync completed; files now fresh
                            Ok(true) | Err(_) => true, // still stale (lock contention / sync error)
                        };
                        if still_stale {
                            let banner = format_per_file_staleness_banner(
                                self.cg.project_root(),
                                &stale_files,
                            );
                            // Machine-readable marker. Same shape as before
                            // so existing scrapers keep working.
                            let stale_json = serde_json::to_string(&stale_files)
                                .unwrap_or_else(|_| "[]".to_string());
                            let marker = format!("\ntokensave_graph_stale: {stale_json}");
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
                                content.insert(0, json!({"type": "text", "text": &banner}));
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
                // Uses `last_sync_timestamp` (sync execution time) not the
                // max file `indexed_at` — a no-change sync still updates the
                // sync metadata even though no file gets a fresh `indexed_at`,
                // so a per-file fallback fires the warning forever on quiet
                // repos (#86).
                {
                    let last_time = self.cg.last_sync_timestamp().await;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let age_secs = now - last_time;
                    if last_time > 0 && age_secs > 3600 {
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

                // Borrowed-worktree heads-up (#312). Inserted LAST so it
                // appears FIRST in the response — the index serving the
                // wrong branch is the most serious of these warnings to
                // surface to the agent.
                if let Some(ref m) = self.worktree_mismatch {
                    let notice = crate::worktree::worktree_mismatch_notice(m);
                    if let Some(content) = result
                        .value
                        .get_mut("content")
                        .and_then(|c| c.as_array_mut())
                    {
                        content.insert(0, json!({"type": "text", "text": notice}));
                    }
                }

                // Branch drift (#400). Unlike the worktree mismatch above,
                // this is re-checked per call rather than cached at startup:
                // it is caused by a `git checkout` *during* the session, so a
                // value computed once would always say "no drift". Same
                // failure shape though — answers about a tree the user is not
                // in — so it rides the same channel.
                if let Some(drift) = self.cg.branch_drift() {
                    let notice = format!(
                        "WARNING: tokensave results below come from branch '{}', but your \
                         working tree is on '{}' — symbols that exist only on '{}' are \
                         missing, and symbols shown may not exist on it. Restart the MCP \
                         server to serve this branch.",
                        drift.serving, drift.working_tree, drift.working_tree
                    );
                    if let Some(content) = result
                        .value
                        .get_mut("content")
                        .and_then(|c| c.as_array_mut())
                    {
                        content.insert(0, json!({"type": "text", "text": notice}));
                    }
                }

                // Every warning/banner above has now been attached to
                // `content` — re-measure the *whole* response so `after`
                // reflects what the model actually receives, not just the
                // tool's own answer. Version warnings and staleness banners
                // in particular can be substantial text; measuring before
                // they were attached (the original bug) left `after`
                // understating real cost and `saved` correspondingly
                // inflated.
                let full_response_tokens: u64 = result
                    .value
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map_or(0, |arr| {
                        let total_bytes: usize = arr
                            .iter()
                            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                            .map(str::len)
                            .sum();
                        (total_bytes / 4) as u64
                    });

                // `after`: everything the model actually paid for this call —
                // the full response text (answer + any banners), the request
                // it sent (name + args + JSON-RPC framing), and, once per
                // session, the tool schema it loaded up front via
                // `tools/list`. The original metric only counted the tool's
                // own answer text, so overhead that's very real to the
                // model — 80+ tool schemas, the call itself, warning banners
                // — was invisible. Debited only once `schema_served` is
                // true (i.e. `tools/list` actually ran): a client that
                // invokes a known tool directly never paid for the schema
                // listing, so charging it here would be a real cost with no
                // matching event. If `tools/list` hasn't run yet,
                // `schema_overhead_charged` is left unset so the overhead
                // still gets debited on the first accounted call after it
                // eventually does.
                let schema_overhead = if self.schema_served.load(Ordering::Relaxed)
                    && !self.schema_overhead_charged.swap(true, Ordering::Relaxed)
                {
                    self.schema_overhead_tokens
                } else {
                    0
                };
                let after_pre_metrics = full_response_tokens + request_overhead + schema_overhead;

                // `before`: the full weight of every touched file, capped per
                // the tool's baseline policy. Most tools return references
                // (file paths, symbol names) rather than file content, so an
                // agent would never have read every touched file in full —
                // charging that full weight as the counterfactual wildly
                // overstates savings for e.g. a dead-code scan touching 50
                // files, or a cached/partial `tokensave_read` claiming a
                // large file's full weight for a stub. The cap scales with
                // `tool_response_tokens` alone (not `after`) — schema/
                // request/banner overhead are real costs but say nothing
                // about how much source the answer stood in for, and mixing
                // them in would loosen the cap most on a session's first
                // call, where the schema overhead is largest. It also never
                // binds on a genuine full-file response, since that response
                // is always at least as large as the source it wraps — see
                // `accounting::baseline_policy`.
                let full_file_tokens = self.touched_file_tokens(&result.touched_files);
                let before_tokens = cap_baseline(
                    baseline_policy(tool_name),
                    full_file_tokens,
                    tool_response_tokens,
                );

                // The metrics line itself is appended to `content` below, so
                // it too costs the model tokens. Two-pass: render it once
                // against the pre-metrics `after` to measure its own byte
                // cost, then fold that in and re-render with the final
                // numbers. `saved`'s digit count essentially never changes
                // between the two passes, so this converges in one extra
                // pass.
                // The line is only ever appended to `content` when
                // `before_tokens > 0` (see the shared `emit_metrics_line`
                // below) — so its own token cost only belongs in `after` in
                // that same case. Charging for it unconditionally would
                // record phantom response tokens for a line that was never
                // sent, and would also make a schema charge landing on a
                // `before == 0` call invisible in both the response and
                // what's persisted. One shared bool drives both decisions so
                // they can't drift apart again.
                // `report_savings = false` suppresses the line outright (#356).
                // It flows through the same bool as the `before_tokens > 0`
                // case, so the line's own token cost is not charged for a line
                // that was never sent — and accounting to the global DB below
                // is deliberately left outside this gate, so `tokensave gain`
                // still sees every call.
                let emit_metrics_line = before_tokens > 0 && self.cg.report_savings();
                let render_metrics = |before: u64, after: u64, saved: u64| -> String {
                    format!("\ntokensave_metrics: before={before} after={after} saved={saved}")
                };
                let metrics_line_tokens = if emit_metrics_line {
                    let provisional = render_metrics(
                        before_tokens,
                        after_pre_metrics,
                        before_tokens.saturating_sub(after_pre_metrics),
                    );
                    (provisional.len() / 4) as u64
                } else {
                    0
                };
                let after_tokens = after_pre_metrics + metrics_line_tokens;

                // Per-call net, saturating: what this call alone reports as
                // saved, never negative. Drives the displayed metrics line
                // and the monitor log — unlike the persisted counter below,
                // these stay simple, per-call figures with no carry-forward.
                let net_saved = before_tokens.saturating_sub(after_tokens);

                // What actually accrues to the persisted counter. Unlike
                // `net_saved` above, a call whose `after` exceeds `before` —
                // most notably the one that absorbs the one-time schema
                // charge — doesn't just lose the excess: it's carried
                // forward as session debt and paid down out of later calls'
                // surplus first. See `settle_against_session_debt`.
                let raw_delta = before_tokens as i64 - after_tokens as i64;
                let credited = self.settle_against_session_debt(raw_delta);
                self.add_tokens_saved(credited).await;
                crate::monitor::write_entry(
                    self.cg.project_root(),
                    "tokensave",
                    tool_name,
                    net_saved,
                    before_tokens,
                );
                // Memory instrumentation for #253: one RSS sample per
                // handled tool call, attributed to the tool name. This
                // generically covers every handler that materializes the
                // graph (get_all_nodes callers included) at the single
                // dispatch point.
                crate::memstats::record(tool_name);
                self.maybe_flush_worldwide().await;

                // Append per-call token savings to the response content.
                if emit_metrics_line {
                    if let Some(content) = result
                        .value
                        .get_mut("content")
                        .and_then(|c| c.as_array_mut())
                    {
                        content.push(json!({
                            "type": "text",
                            "text": render_metrics(before_tokens, after_tokens, net_saved)
                        }));
                    }
                }

                // Persist to the cross-project savings ledger (best-effort, non-blocking).
                {
                    let project_path_str = self.cg.project_root().to_string_lossy().to_string();
                    let tool_name_owned = tool_name.to_string();
                    let ts = crate::tokensave::current_timestamp();
                    #[cfg(feature = "test-transport")]
                    let accounting_tasks_pending = {
                        self.accounting_tasks_started.fetch_add(1, Ordering::AcqRel);
                        self.accounting_tasks_pending.fetch_add(1, Ordering::AcqRel);
                        Arc::clone(&self.accounting_tasks_pending)
                    };
                    tokio::spawn(async move {
                        #[cfg(feature = "test-transport")]
                        let _guard = AccountingTaskGuard(accounting_tasks_pending);
                        if let Some(gdb) = crate::global_db::GlobalDb::open().await {
                            gdb.record_savings(
                                &project_path_str,
                                &tool_name_owned,
                                before_tokens,
                                after_tokens,
                                ts,
                            )
                            .await;
                        }
                    });
                }

                JsonRpcResponse::success(id, result.value)
            }
            Err(TokenSaveError::Config { message }) if selected.is_some() => {
                JsonRpcResponse::error(
                    id,
                    ErrorCode::InvalidParams,
                    format!("invalid selected graph arguments: {message}"),
                )
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

        // Surface the verbose worktree-mismatch warning when present, so
        // `tokensave_status` is the one tool whose output is loud about
        // serving a borrowed index (#312).
        if let Some(ref m) = self.worktree_mismatch {
            stats["worktree_mismatch"] = json!({
                "worktree_root": m.worktree_root.display().to_string(),
                "index_root": m.index_root.display().to_string(),
                "warning": crate::worktree::worktree_mismatch_warning(m),
            });
        }

        stats
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod staleness_banner_tests {
    use super::{
        format_per_file_staleness_banner, format_selected_fallback_guidance, humanize_age,
    };
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn humanize_age_picks_right_unit() {
        assert_eq!(humanize_age(0), "0s ago");
        assert_eq!(humanize_age(45), "45s ago");
        assert_eq!(humanize_age(125), "2m ago");
        assert_eq!(humanize_age(3_700), "1h ago");
        assert_eq!(humanize_age(90_000), "1d ago");
    }

    #[test]
    fn banner_lists_stale_files_with_age() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.rs"), "fn a() {}").unwrap();
        fs::write(root.join("src/b.rs"), "fn b() {}").unwrap();

        let stale = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let banner = format_per_file_staleness_banner(root, &stale);
        assert!(banner.contains("2 file(s) referenced below were edited"));
        assert!(banner.contains("src/a.rs ("));
        assert!(banner.contains("src/b.rs ("));
        assert!(banner.contains("ago)"));
        assert!(banner.contains("tokensave sync"));
        // Critical UX shift: should NOT say "STALE INDEX" — the whole
        // point of #428 is to scope the warning, not blanket-distrust
        // the entire response.
        assert!(!banner.contains("STALE INDEX"));
    }

    #[test]
    fn banner_handles_missing_file_gracefully() {
        let tmp = tempdir().unwrap();
        let stale = vec!["does/not/exist.rs".to_string()];
        let banner = format_per_file_staleness_banner(tmp.path(), &stale);
        // Missing files still get listed (e.g. file deleted between
        // sync and tool response). Age falls back to 0s.
        assert!(banner.contains("does/not/exist.rs"));
    }

    #[test]
    fn selected_fallback_guidance_treats_values_as_inert_text() {
        let warning =
            format_selected_fallback_guidance("/tmp/project $(touch pwned)", Some("review; id"));

        assert!(warning.contains("\"/tmp/project $(touch pwned)\""));
        assert!(warning.contains("\"review; id\""));
        assert!(warning.contains("is using a fallback index"));
        assert!(!warning.contains("branch fallback"));
        assert!(!warning.contains('`'));
        assert!(!warning.contains("tokensave branch add"));
        assert!(!warning.contains("tokensave sync --path"));
    }
}

#[cfg(test)]
mod idle_timeout_tests {
    use super::defer_idle_exit;

    /// Startup catch-up is spawned unconditionally, so "not done" means it is
    /// genuinely still walking the tree — and cutting that off would abandon
    /// work the next client request depends on.
    #[test]
    fn a_running_startup_catch_up_defers_the_exit() {
        assert!(defer_idle_exit(false, false, false));
        assert!(defer_idle_exit(false, true, true));
    }

    /// The case that would have made the whole option a no-op: a session that
    /// never triggers a version reindex leaves `reindex_done` false forever, so
    /// keying on that alone would defer every expiry on every server.
    #[test]
    fn a_reindex_that_never_started_does_not_defer_forever() {
        assert!(
            !defer_idle_exit(true, false, false),
            "an ordinary idle server must be allowed to exit"
        );
    }

    #[test]
    fn a_running_version_reindex_defers_the_exit() {
        assert!(defer_idle_exit(true, true, false));
    }

    #[test]
    fn a_finished_reindex_stops_deferring() {
        assert!(!defer_idle_exit(true, true, true));
    }
}
