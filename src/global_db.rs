//! User-level database that tracks all `TokenSave` projects and their saved tokens.
//!
//! Stored at `~/.tokensave/global.db`, this DB holds one row per project with
//! the project's DB path and its cumulative tokens-saved count. All operations
//! are best-effort: failures are silently ignored so they never block the main
//! MCP server loop.

use std::path::{Path, PathBuf};

use libsql::{params, Builder, Connection, Database as LibsqlDatabase};

/// Total savings + call count for a project (or all projects when `project` is None).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SavingsTotal {
    pub saved_tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SavingsDay {
    /// Start-of-day epoch seconds (UTC).
    pub day: i64,
    pub saved_tokens: u64,
    pub calls: u64,
}

/// Per-agent cost summary returned by `cost_by_agent_since`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentCostSummary {
    pub agent: String,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    /// None if any row in the group has NULL credits (i.e. Claude turns).
    pub credits: Option<u64>,
    pub turns: u64,
}

/// User-level database tracking all `TokenSave` projects.
pub struct GlobalDb {
    conn: Connection,
    _db: LibsqlDatabase,
}

/// Returns the path to the global database: `~/.tokensave/global.db`.
pub fn global_db_path() -> Option<PathBuf> {
    crate::agents::home_dir().map(|h| h.join(".tokensave").join("global.db"))
}

/// Resolves a project directory to the canonical string used as its global-DB key.
///
/// Every spelling of one directory must land on one row, so `.`, `./`, `../name`
/// and trailing separators are resolved away and symlinks followed. On Windows
/// the verbatim `\\?\` prefix that [`Path::canonicalize`] adds is stripped and
/// the drive letter upper-cased, so `d:\foo` and `D:\foo` share a row. Paths that
/// cannot be resolved on disk (a deleted project, or a synthetic path in a test)
/// fall back to a lexical absolute form, which is still stable for that caller.
///
/// # Examples
///
/// ```
/// use tokensave::global_db::normalize_project_key;
///
/// let cwd = std::env::current_dir().unwrap();
/// assert_eq!(normalize_project_key("."), normalize_project_key(&cwd));
/// ```
pub fn normalize_project_key(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let resolved = path
        .canonicalize()
        .unwrap_or_else(|_| lexical_absolute(path));
    normalize_key_string(&resolved.to_string_lossy())
}

/// Applies the textual half of [`normalize_project_key`] to an already-absolute path.
fn normalize_key_string(path: &str) -> String {
    let mut s = path.to_string();

    // Windows canonicalization yields verbatim paths (`\\?\C:\x`, `\\?\UNC\srv\s`),
    // which no other code path produces. Strip the prefix so keys stay comparable.
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        s = format!(r"\\{rest}");
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }

    // `d:\foo` and `D:\foo` are the same directory. Only a drive-letter prefix is
    // case-folded — the rest of the path is left alone, since a case-insensitive
    // filesystem is not something we can assume from the string.
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        s.replace_range(..1, &s[..1].to_ascii_uppercase());
    }

    trim_trailing_separators(&mut s);
    s
}

/// Makes a path absolute without touching the filesystem, resolving `.` and `..`.
fn lexical_absolute(path: &Path) -> PathBuf {
    use std::path::Component;

    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // A leading `..` on a relative fallback has nothing to pop; keep it
                // rather than silently changing which directory is meant.
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Strips trailing `/` and `\` without eating a root (`/`, `C:\`).
fn trim_trailing_separators(s: &mut String) {
    let root_len =
        if s.len() >= 3 && s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':' {
            3
        } else {
            1
        };
    while s.len() > root_len && (s.ends_with('/') || s.ends_with('\\')) {
        s.pop();
    }
}

/// Splits a normalized project key into its parent directory, or `None` at a root.
///
/// Both separators are accepted because a key may have been written by either
/// platform, and the string half of normalization does not rewrite separators.
fn parent_key(key: &str) -> Option<&str> {
    let cut = key.rfind(['/', '\\'])?;
    let parent = &key[..cut];
    // `/foo` and `C:\foo` sit directly under a root that the cut erased.
    if parent.is_empty() || parent.ends_with(':') {
        return None;
    }
    Some(parent)
}

/// Most siblings ever surfaced, however many share the parent directory.
///
/// A shared scratch or checkout directory can hold dozens of indexed projects.
/// Naming them all buries the useful ones and, since this list is embedded in
/// tool responses, can consume the whole response budget on its own.
pub const MAX_SIBLING_PROJECTS: usize = 5;

/// Selects the projects that sit directly beside `served`, from `all` known keys.
///
/// Only immediate siblings qualify: a nested project or an unrelated checkout
/// elsewhere on disk is not something the served session can be assumed to care
/// about, and a broader net would make the hint noisy enough to ignore. Inputs
/// are normalized here so callers may pass raw rows. The result is sorted — so
/// the surfaced list is stable across calls — and capped at
/// [`MAX_SIBLING_PROJECTS`].
///
/// # Examples
///
/// ```
/// use tokensave::global_db::sibling_project_keys;
///
/// let all = vec!["/w/svc".to_string(), "/w/lib".to_string(), "/other/x".to_string()];
/// assert_eq!(sibling_project_keys("/w/svc", &all), vec!["/w/lib".to_string()]);
/// ```
pub fn sibling_project_keys(served: &str, all: &[String]) -> Vec<String> {
    let served = normalize_key_string(served);
    let Some(parent) = parent_key(&served) else {
        return Vec::new();
    };

    let mut siblings: Vec<String> = all
        .iter()
        .map(|path| normalize_key_string(path))
        .filter(|key| *key != served && parent_key(key) == Some(parent))
        .collect();
    siblings.sort();
    siblings.dedup();
    siblings.truncate(MAX_SIBLING_PROJECTS);
    siblings
}

impl GlobalDb {
    /// Returns the initialized projects sitting directly beside `served_root`.
    ///
    /// Used to tell a session which other graphs it can reach with `graph_root`;
    /// see [`sibling_project_keys`] for the selection rule.
    pub async fn sibling_projects(&self, served_root: &Path) -> Vec<String> {
        let served = normalize_project_key(served_root);
        sibling_project_keys(&served, &self.list_project_paths().await)
    }

    /// Opens (or creates) the global database at an explicit path. Returns
    /// `None` if the directory cannot be created or the DB fails to open.
    pub async fn open_at(db_path: &std::path::Path) -> Option<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok()?;
        }

        let db = Builder::new_local(db_path).build().await.ok()?;
        let conn = db.connect().ok()?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;",
        )
        .await
        .ok()?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                path TEXT PRIMARY KEY,
                tokens_saved INTEGER NOT NULL DEFAULT 0
            )",
        )
        .await
        .ok()?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS turns (
                message_id TEXT PRIMARY KEY,
                project_hash TEXT NOT NULL,
                session_id TEXT NOT NULL,
                model TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL,
                category TEXT NOT NULL,
                tool_names TEXT NOT NULL DEFAULT '',
                agent TEXT NOT NULL DEFAULT 'claude',
                credits INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_turns_timestamp ON turns(timestamp);
            CREATE INDEX IF NOT EXISTS idx_turns_project ON turns(project_hash);
            CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);
            CREATE TABLE IF NOT EXISTS parse_offsets (
                file_path TEXT PRIMARY KEY,
                byte_offset INTEGER NOT NULL,
                mtime INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS savings_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                project_path TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                before_tokens INTEGER NOT NULL,
                after_tokens INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_savings_ledger_ts ON savings_ledger(ts);
            CREATE INDEX IF NOT EXISTS idx_savings_ledger_project ON savings_ledger(project_path)",
        )
        .await
        .ok()?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .await
        .ok()?;

        // Migrate existing DBs: add agent/credits columns if absent.
        Self::migrate_turns_columns(&conn).await;
        Self::migrate_project_paths(&conn).await;

        Some(Self { conn, _db: db })
    }

    /// Opens (or creates) the global database. Returns `None` if the home
    /// directory cannot be determined or the DB fails to open.
    pub async fn open() -> Option<Self> {
        let db_path = global_db_path()?;
        Self::open_at(&db_path).await
    }

    /// Add `agent` and `credits` columns to an existing turns table if absent.
    /// Best-effort: failures are silently ignored (matches `open_at` pattern).
    async fn migrate_turns_columns(conn: &Connection) {
        let Ok(mut rows) = conn.query("PRAGMA table_info(turns)", ()).await else {
            return;
        };
        let mut has_agent = false;
        let mut has_credits = false;
        while let Ok(Some(row)) = rows.next().await {
            if let Ok(name) = row.get::<String>(1) {
                match name.as_str() {
                    "agent" => has_agent = true,
                    "credits" => has_credits = true,
                    _ => {}
                }
            }
        }
        if !has_agent {
            let _ = conn
                .execute(
                    "ALTER TABLE turns ADD COLUMN agent TEXT NOT NULL DEFAULT 'claude'",
                    (),
                )
                .await;
        }
        if !has_credits {
            let _ = conn
                .execute("ALTER TABLE turns ADD COLUMN credits INTEGER", ())
                .await;
        }
    }

    /// Rewrites pre-existing project keys to their canonical form, once per DB.
    ///
    /// Rows written before keys were canonicalised can name the same directory
    /// several ways (`d:\p` vs `D:\p`), splitting one project's savings across
    /// duplicates; those are merged by summing. Rows keyed by a relative path
    /// (a literal `.`) name no recoverable directory and are dropped — they
    /// would otherwise be permanently unattributable and never purged as stale.
    /// Best-effort: failures leave the DB as it was and the flag unset, so the
    /// next open retries.
    async fn migrate_project_paths(conn: &Connection) {
        if let Ok(mut flag) = conn
            .query(
                "SELECT 1 FROM meta WHERE key = 'projects_path_normalized'",
                (),
            )
            .await
        {
            if matches!(flag.next().await, Ok(Some(_))) {
                return;
            }
        }

        let Ok(mut rows) = conn
            .query("SELECT path, tokens_saved FROM projects", ())
            .await
        else {
            return;
        };
        let mut existing: Vec<(String, i64)> = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let (Ok(path), Ok(tokens)) = (row.get::<String>(0), row.get::<i64>(1)) {
                existing.push((path, tokens));
            }
        }

        // old key -> canonical key, for the rows that actually move.
        let mut remapped: Vec<(String, String)> = Vec::new();
        let mut merged: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
        let mut dropped: Vec<String> = Vec::new();
        for (path, tokens) in existing {
            if !Path::new(&path).is_absolute() {
                dropped.push(path);
                continue;
            }
            let key = normalize_project_key(&path);
            if key != path {
                remapped.push((path, key.clone()));
            }
            *merged.entry(key).or_insert(0) += tokens;
        }

        if !remapped.is_empty() || !dropped.is_empty() {
            if conn.execute("DELETE FROM projects", ()).await.is_err() {
                return;
            }
            for (key, tokens) in &merged {
                let _ = conn
                    .execute(
                        "INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)",
                        params![key.as_str(), *tokens],
                    )
                    .await;
            }
            // Keep the ledger addressable by the same keys, so `gain` for a
            // project still sees the history recorded under its old spelling.
            for (old, new) in &remapped {
                let _ = conn
                    .execute(
                        "UPDATE savings_ledger SET project_path = ?1 WHERE project_path = ?2",
                        params![new.as_str(), old.as_str()],
                    )
                    .await;
            }
        }

        let _ = conn
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('projects_path_normalized', '1')",
                (),
            )
            .await;
    }

    /// Registers or updates a project's tokens-saved count. Best-effort.
    ///
    /// The path is canonicalised via [`normalize_project_key`] so that `.`,
    /// relative paths and case-variant drive letters all update one row.
    pub async fn upsert(&self, project_path: &Path, tokens_saved: u64) {
        let path_str = normalize_project_key(project_path);
        let _ = self
            .conn
            .execute(
                "INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET tokens_saved = ?2",
                params![path_str, tokens_saved as i64],
            )
            .await;
    }

    /// Returns the stored `tokens_saved` count for a specific project, or 0 if not found.
    pub async fn get_project_tokens(&self, project_path: &Path) -> u64 {
        let path_str = normalize_project_key(project_path);
        let Ok(mut rows) = self
            .conn
            .query(
                "SELECT tokens_saved FROM projects WHERE path = ?1",
                params![path_str],
            )
            .await
        else {
            return 0;
        };
        match rows.next().await {
            Ok(Some(row)) => row.get::<i64>(0).unwrap_or(0) as u64,
            _ => 0,
        }
    }

    /// Returns the sum of `tokens_saved` across all tracked projects.
    pub async fn global_tokens_saved(&self) -> Option<u64> {
        let mut rows = self
            .conn
            .query("SELECT COALESCE(SUM(tokens_saved), 0) FROM projects", ())
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        let total: i64 = row.get(0).ok()?;
        Some(total as u64)
    }

    /// Insert a new ledger row. Best-effort; errors are reported to stderr via eprintln
    /// but never propagated.
    pub async fn record_savings(
        &self,
        project_path: &str,
        tool_name: &str,
        before_tokens: u64,
        after_tokens: u64,
        ts: i64,
    ) {
        let project_path = normalize_project_key(project_path);
        let result = self
            .conn
            .execute(
                "INSERT INTO savings_ledger (ts, project_path, tool_name, before_tokens, after_tokens) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ts, project_path, tool_name, before_tokens as i64, after_tokens as i64],
            )
            .await;
        if let Err(e) = result {
            eprintln!("[tokensave] savings_ledger insert failed: {e}");
        }
    }

    /// Sum (before-after) across the ledger entries, with `ts >= since`. Optionally
    /// filter by exact project path. Returns zeros on any DB error.
    pub async fn sum_savings(&self, project: Option<&str>, since: i64) -> SavingsTotal {
        let sql_with_project =
            "SELECT COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), COUNT(*) \
             FROM savings_ledger WHERE project_path = ?1 AND ts >= ?2";
        let sql_all =
            "SELECT COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), COUNT(*) \
             FROM savings_ledger WHERE ts >= ?1";

        let rows = match project.map(normalize_project_key) {
            Some(p) => self.conn.query(sql_with_project, params![p, since]).await,
            None => self.conn.query(sql_all, params![since]).await,
        };
        let Ok(mut rows) = rows else {
            return SavingsTotal {
                saved_tokens: 0,
                calls: 0,
            };
        };
        match rows.next().await {
            Ok(Some(row)) => SavingsTotal {
                saved_tokens: row.get::<i64>(0).unwrap_or(0).max(0) as u64,
                calls: row.get::<i64>(1).unwrap_or(0).max(0) as u64,
            },
            _ => SavingsTotal {
                saved_tokens: 0,
                calls: 0,
            },
        }
    }

    /// Group ledger entries by UTC calendar day. Newest-first.
    pub async fn savings_history(&self, project: Option<&str>, since: i64) -> Vec<SavingsDay> {
        let sql_with_project =
            "SELECT (ts/86400)*86400 AS day, \
                    COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), \
                    COUNT(*) \
             FROM savings_ledger WHERE project_path = ?1 AND ts >= ?2 \
             GROUP BY day ORDER BY day DESC";
        let sql_all =
            "SELECT (ts/86400)*86400 AS day, \
                    COALESCE(SUM(CASE WHEN before_tokens > after_tokens THEN before_tokens - after_tokens ELSE 0 END), 0), \
                    COUNT(*) \
             FROM savings_ledger WHERE ts >= ?1 \
             GROUP BY day ORDER BY day DESC";

        let rows = match project.map(normalize_project_key) {
            Some(p) => self.conn.query(sql_with_project, params![p, since]).await,
            None => self.conn.query(sql_all, params![since]).await,
        };
        let Ok(mut rows) = rows else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            out.push(SavingsDay {
                day: row.get::<i64>(0).unwrap_or(0),
                saved_tokens: row.get::<i64>(1).unwrap_or(0).max(0) as u64,
                calls: row.get::<i64>(2).unwrap_or(0).max(0) as u64,
            });
        }
        out
    }

    /// Removes a project's row from the global DB. Best-effort.
    pub async fn delete_project(&self, project_path: &Path) {
        let path_str = normalize_project_key(project_path);
        let _ = self
            .conn
            .execute("DELETE FROM projects WHERE path = ?1", params![path_str])
            .await;
    }

    /// Removes many project rows in a single statement. Returns the number of
    /// rows actually deleted (0 on any error). Best-effort.
    ///
    /// Chunks the input at 256 paths per statement to stay well clear of
    /// `SQLite`'s default 999-parameter limit while still reducing N round trips
    /// to ⌈N/256⌉.
    pub async fn delete_projects(&self, project_paths: &[String]) -> usize {
        const CHUNK: usize = 256;
        let mut total: usize = 0;
        for chunk in project_paths.chunks(CHUNK) {
            if chunk.is_empty() {
                continue;
            }
            let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
            let sql = format!(
                "DELETE FROM projects WHERE path IN ({})",
                placeholders.join(",")
            );
            let values: Vec<libsql::Value> = chunk
                .iter()
                .map(|p| libsql::Value::Text(p.clone()))
                .collect();
            if let Ok(n) = self.conn.execute(&sql, values).await {
                total = total.saturating_add(n as usize);
            }
        }
        total
    }

    /// Returns all tracked project paths.
    pub async fn list_project_paths(&self) -> Vec<String> {
        let Ok(mut rows) = self.conn.query("SELECT path FROM projects", ()).await else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            if let Ok(path) = row.get::<String>(0) {
                paths.push(path);
            }
        }
        paths
    }

    // ── Accounting: turns table ──────────────────────────────────────

    /// Insert a parsed turn. Returns `true` if inserted, `false` if duplicate.
    pub async fn insert_turn(&self, turn: &crate::types::CostTurn) -> bool {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO turns
                 (message_id, project_hash, session_id, model, timestamp,
                  input_tokens, output_tokens, cache_write_tokens, cache_read_tokens,
                  cost_usd, category, tool_names, agent, credits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    turn.message_id.clone(),
                    turn.project_hash.clone(),
                    turn.session_id.clone(),
                    turn.model.clone(),
                    turn.timestamp as i64,
                    turn.input_tokens as i64,
                    turn.output_tokens as i64,
                    turn.cache_write_tokens as i64,
                    turn.cache_read_tokens as i64,
                    turn.cost_usd,
                    turn.category.clone(),
                    turn.tool_names.clone(),
                    turn.agent.clone(),
                    turn.credits.map(|c| c as i64),
                ],
            )
            .await
            .is_ok_and(|n| n > 0)
    }

    /// Upsert a Droid cumulative-snapshot turn (monotonic semantics).
    ///
    /// On conflict, the row is updated only when **every** token counter in the
    /// candidate is ≥ the stored value, credits do not regress, and **at least
    /// one** counter or credit strictly increases.  `timestamp` is never
    /// updated (it is the stable session-start bucket).
    ///
    /// Returns `true` if a row was inserted or updated, `false` otherwise.
    pub async fn upsert_droid_turn(&self, turn: &crate::types::CostTurn) -> bool {
        self.conn
            .execute(
                "INSERT INTO turns
                 (message_id, project_hash, session_id, model, timestamp,
                  input_tokens, output_tokens, cache_write_tokens, cache_read_tokens,
                  cost_usd, category, tool_names, agent, credits)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(message_id) DO UPDATE SET
                     project_hash        = excluded.project_hash,
                     session_id          = excluded.session_id,
                     model               = excluded.model,
                     input_tokens        = MAX(excluded.input_tokens,        turns.input_tokens),
                     output_tokens       = MAX(excluded.output_tokens,       turns.output_tokens),
                     cache_write_tokens  = MAX(excluded.cache_write_tokens,  turns.cache_write_tokens),
                     cache_read_tokens   = MAX(excluded.cache_read_tokens,   turns.cache_read_tokens),
                     cost_usd            = excluded.cost_usd,
                     category            = excluded.category,
                     tool_names          = excluded.tool_names,
                     agent               = excluded.agent,
                     credits             = COALESCE(excluded.credits, turns.credits)
                 WHERE
                     excluded.input_tokens        >= turns.input_tokens
                     AND excluded.output_tokens       >= turns.output_tokens
                     AND excluded.cache_write_tokens  >= turns.cache_write_tokens
                     AND excluded.cache_read_tokens   >= turns.cache_read_tokens
                     AND (excluded.credits IS NULL
                          OR turns.credits IS NULL
                          OR excluded.credits >= turns.credits)
                     AND (
                         excluded.input_tokens        > turns.input_tokens
                         OR excluded.output_tokens       > turns.output_tokens
                         OR excluded.cache_write_tokens  > turns.cache_write_tokens
                         OR excluded.cache_read_tokens   > turns.cache_read_tokens
                         OR (excluded.credits IS NOT NULL
                             AND (turns.credits IS NULL
                                  OR excluded.credits > turns.credits))
                     )",
                params![
                    turn.message_id.clone(),
                    turn.project_hash.clone(),
                    turn.session_id.clone(),
                    turn.model.clone(),
                    turn.timestamp as i64,
                    turn.input_tokens as i64,
                    turn.output_tokens as i64,
                    turn.cache_write_tokens as i64,
                    turn.cache_read_tokens as i64,
                    turn.cost_usd,
                    turn.category.clone(),
                    turn.tool_names.clone(),
                    turn.agent.clone(),
                    turn.credits.map(|c| c as i64),
                ],
            )
            .await
            .is_ok_and(|n| n > 0)
    }

    /// Total cost in USD since a given unix timestamp.
    pub async fn total_cost_since(&self, since: u64) -> Option<f64> {
        let mut rows = self
            .conn
            .query(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM turns WHERE timestamp >= ?1",
                params![since as i64],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        Some(row.get::<f64>(0).unwrap_or(0.0))
    }

    /// Total input + output tokens since a given unix timestamp.
    pub async fn total_tokens_since(&self, since: u64) -> Option<u64> {
        let mut rows = self
            .conn
            .query(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM turns WHERE timestamp >= ?1",
                params![since as i64],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        Some(row.get::<i64>(0).unwrap_or(0) as u64)
    }

    /// Token breakdown (input, output, `cache_read`) since a given timestamp.
    /// Claude-only: Droid turns are excluded so legacy USD views remain accurate.
    pub async fn token_breakdown_since(&self, since: u64) -> Option<(u64, u64, u64, u64)> {
        let mut rows = self
            .conn
            .query(
                "SELECT COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        COALESCE(SUM(cache_write_tokens), 0)
                 FROM turns WHERE timestamp >= ?1 AND agent = 'claude'",
                params![since as i64],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        Some((
            row.get::<i64>(0).unwrap_or(0) as u64,
            row.get::<i64>(1).unwrap_or(0) as u64,
            row.get::<i64>(2).unwrap_or(0) as u64,
            row.get::<i64>(3).unwrap_or(0) as u64,
        ))
    }

    /// Cost grouped by model since a given timestamp. Claude-only.
    /// Returns `(model, cost, total_tokens)`.
    ///
    /// `total_tokens` counts every category the cost was computed from, cache
    /// reads and cache writes included (#472). Summing only uncached input and
    /// output implied a price per million several times any published rate,
    /// because agent traffic is dominated by the cached context resent each
    /// turn — priced, but previously uncounted.
    pub async fn cost_by_model_since(&self, since: u64) -> Vec<(String, f64, u64)> {
        let Ok(mut rows) = self
            .conn
            .query(
                "SELECT model, SUM(cost_usd),
                        SUM(input_tokens + output_tokens + cache_read_tokens + cache_write_tokens)
                 FROM turns WHERE timestamp >= ?1 AND agent = 'claude'
                 GROUP BY model ORDER BY SUM(cost_usd) DESC",
                params![since as i64],
            )
            .await
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let model: String = row.get(0).unwrap_or_default();
            let cost: f64 = row.get(1).unwrap_or(0.0);
            let tokens: i64 = row.get(2).unwrap_or(0);
            out.push((model, cost, tokens as u64));
        }
        out
    }

    /// Cost grouped by category since a given timestamp. Claude-only.
    /// Returns `(category, cost, turn_count)`.
    pub async fn cost_by_category_since(&self, since: u64) -> Vec<(String, f64, u64)> {
        let Ok(mut rows) = self
            .conn
            .query(
                "SELECT category, SUM(cost_usd), COUNT(*)
                 FROM turns WHERE timestamp >= ?1 AND agent = 'claude'
                 GROUP BY category ORDER BY SUM(cost_usd) DESC",
                params![since as i64],
            )
            .await
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let cat: String = row.get(0).unwrap_or_default();
            let cost: f64 = row.get(1).unwrap_or(0.0);
            let count: i64 = row.get(2).unwrap_or(0);
            out.push((cat, cost, count as u64));
        }
        out
    }

    /// Fetch `(tool_names, input_tokens)` for every Claude turn since a timestamp.
    ///
    /// Claude-only: the discover analyzer works on Claude Code navigation patterns.
    /// Returns an empty vector on any DB error.
    pub async fn nav_turns_since(&self, since: u64) -> Vec<(String, u64)> {
        let Ok(mut rows) = self
            .conn
            .query(
                "SELECT tool_names, input_tokens FROM turns WHERE timestamp >= ?1 AND agent = 'claude'",
                params![since as i64],
            )
            .await
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let tool_names: String = row.get(0).unwrap_or_default();
            let input_tokens: i64 = row.get(1).unwrap_or(0);
            out.push((tool_names, input_tokens.max(0) as u64));
        }
        out
    }

    /// Cost grouped by agent since a given timestamp.
    ///
    /// Credits aggregate to `None` if any row in the group has a NULL credit value.
    pub async fn cost_by_agent_since(&self, since: u64) -> Vec<AgentCostSummary> {
        let Ok(mut rows) = self
            .conn
            .query(
                "SELECT agent,
                        COALESCE(SUM(cost_usd), 0.0),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_write_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        CASE WHEN COUNT(credits) = COUNT(*) THEN SUM(credits) ELSE NULL END,
                        COUNT(*)
                 FROM turns WHERE timestamp >= ?1
                 GROUP BY agent ORDER BY agent",
                params![since as i64],
            )
            .await
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let agent: String = row.get(0).unwrap_or_default();
            let cost_usd: f64 = row.get(1).unwrap_or(0.0);
            let input_tokens: i64 = row.get(2).unwrap_or(0);
            let output_tokens: i64 = row.get(3).unwrap_or(0);
            let cache_write_tokens: i64 = row.get(4).unwrap_or(0);
            let cache_read_tokens: i64 = row.get(5).unwrap_or(0);
            let credits: Option<i64> = row.get(6).ok();
            let turns: i64 = row.get(7).unwrap_or(0);
            out.push(AgentCostSummary {
                agent,
                cost_usd,
                input_tokens: input_tokens.max(0) as u64,
                output_tokens: output_tokens.max(0) as u64,
                cache_write_tokens: cache_write_tokens.max(0) as u64,
                cache_read_tokens: cache_read_tokens.max(0) as u64,
                credits: credits.map(|c| c.max(0) as u64),
                turns: turns.max(0) as u64,
            });
        }
        out
    }

    // ── Accounting: parse_offsets table ────────────────────────────────

    /// Get the saved parse offset for a JSONL file.
    /// Returns `(byte_offset, mtime)` or `None` if not tracked.
    pub async fn get_parse_offset(&self, path: &str) -> Option<(u64, u64)> {
        let mut rows = self
            .conn
            .query(
                "SELECT byte_offset, mtime FROM parse_offsets WHERE file_path = ?1",
                params![path],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        let offset: i64 = row.get(0).ok()?;
        let mtime: i64 = row.get(1).ok()?;
        Some((offset as u64, mtime as u64))
    }

    /// Save the parse offset for a JSONL file. Best-effort.
    pub async fn set_parse_offset(&self, path: &str, offset: u64, mtime: u64) {
        let _ = self
            .conn
            .execute(
                "INSERT INTO parse_offsets (file_path, byte_offset, mtime) VALUES (?1, ?2, ?3)
                 ON CONFLICT(file_path) DO UPDATE SET byte_offset = ?2, mtime = ?3",
                params![path, offset as i64, mtime as i64],
            )
            .await;
    }

    /// Checkpoints the WAL. Best-effort.
    pub async fn checkpoint(&self) {
        let _ = self
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .await;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn dot_resolves_to_the_same_key_as_the_absolute_path() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(normalize_project_key("."), normalize_project_key(&cwd));
        assert_eq!(normalize_project_key("./"), normalize_project_key(&cwd));
    }

    #[test]
    fn parent_relative_path_resolves_to_the_parent_directory() {
        let cwd = std::env::current_dir().unwrap();
        let parent = cwd.parent().unwrap();
        assert_eq!(normalize_project_key(".."), normalize_project_key(parent));
    }

    #[test]
    fn trailing_separators_do_not_create_a_second_key() {
        let cwd = std::env::current_dir().unwrap();
        let with_slash = format!("{}/", cwd.display());
        assert_eq!(
            normalize_project_key(&with_slash),
            normalize_project_key(&cwd)
        );
    }

    #[test]
    fn drive_letter_case_is_folded() {
        assert_eq!(
            normalize_key_string(r"d:\Work\ProjectA"),
            normalize_key_string(r"D:\Work\ProjectA")
        );
        assert_eq!(
            normalize_key_string(r"d:\Work\ProjectA"),
            r"D:\Work\ProjectA"
        );
    }

    #[test]
    fn windows_verbatim_prefixes_are_stripped() {
        assert_eq!(normalize_key_string(r"\\?\D:\Work\P"), r"D:\Work\P");
        assert_eq!(normalize_key_string(r"\\?\UNC\srv\share"), r"\\srv\share");
    }

    #[test]
    fn a_root_is_not_trimmed_away() {
        let mut unix_root = "/".to_string();
        trim_trailing_separators(&mut unix_root);
        assert_eq!(unix_root, "/");

        let mut drive_root = r"D:\".to_string();
        trim_trailing_separators(&mut drive_root);
        assert_eq!(drive_root, r"D:\");
    }

    #[test]
    fn unresolvable_absolute_paths_keep_a_stable_key() {
        // A deleted project must still map to one key, not to the cwd.
        let key = normalize_project_key("/definitely/not/here/../here");
        assert_eq!(key, normalize_project_key("/definitely/not/here"));
    }

    #[tokio::test]
    async fn relative_and_absolute_writes_share_one_row() {
        let dir = tempfile::tempdir().unwrap();
        let db = GlobalDb::open_at(&dir.path().join("global.db"))
            .await
            .unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir(&project).unwrap();

        db.upsert(&project, 100).await;
        db.upsert(&project.join("."), 250).await;

        assert_eq!(db.list_project_paths().await.len(), 1);
        assert_eq!(db.get_project_tokens(&project).await, 250);
    }

    #[tokio::test]
    async fn migration_merges_duplicates_and_drops_relative_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("global.db");
        let project = dir.path().join("proj");
        std::fs::create_dir(&project).unwrap();
        let canonical = normalize_project_key(&project);
        let trailing = format!("{canonical}/");

        {
            // Simulate a pre-canonicalisation DB: one duplicate pair and a `.` row.
            let db = GlobalDb::open_at(&db_path).await.unwrap();
            db.conn
                .execute_batch(
                    "DELETE FROM meta WHERE key = 'projects_path_normalized';
                     DELETE FROM projects;",
                )
                .await
                .unwrap();
            for (path, tokens) in [
                (canonical.as_str(), 14_000_i64),
                (trailing.as_str(), 12_000),
                (".", 999),
            ] {
                db.conn
                    .execute(
                        "INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)",
                        params![path, tokens],
                    )
                    .await
                    .unwrap();
            }
            // Raw insert: record_savings would canonicalise it for us.
            db.conn
                .execute(
                    "INSERT INTO savings_ledger (ts, project_path, tool_name, before_tokens, after_tokens) \
                     VALUES (42, ?1, 'tokensave_context', 1000, 100)",
                    params![trailing.as_str()],
                )
                .await
                .unwrap();
        }

        let db = GlobalDb::open_at(&db_path).await.unwrap();
        let paths = db.list_project_paths().await;
        assert_eq!(paths, vec![canonical.clone()]);
        assert_eq!(db.get_project_tokens(&project).await, 26_000);
        // The ledger row written under the old spelling still answers for the project.
        assert_eq!(db.sum_savings(Some(&canonical), 0).await.calls, 1);
    }
}
