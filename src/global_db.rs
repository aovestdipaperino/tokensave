//! User-level database that tracks all TokenSave projects and their saved tokens.
//!
//! Stored at `~/.tokensave/global.db`, this DB holds one row per project with
//! the project's DB path and its cumulative tokens-saved count. All operations
//! are best-effort: failures are silently ignored so they never block the main
//! MCP server loop.

use std::path::{Path, PathBuf};

use libsql::{Builder, Connection, Database as LibsqlDatabase, params};

/// User-level database tracking all TokenSave projects.
pub struct GlobalDb {
    conn: Connection,
    _db: LibsqlDatabase,
}

/// Returns the path to the global database: `~/.tokensave/global.db`.
pub fn global_db_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave").join("global.db"))
}

impl GlobalDb {
    /// Opens (or creates) the global database. Returns `None` if the home
    /// directory cannot be determined or the DB fails to open.
    pub async fn open() -> Option<Self> {
        let db_path = global_db_path()?;

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok()?;
        }

        let db = Builder::new_local(&db_path).build().await.ok()?;
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
                tool_names TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_turns_timestamp ON turns(timestamp);
            CREATE INDEX IF NOT EXISTS idx_turns_project ON turns(project_hash);
            CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);
            CREATE TABLE IF NOT EXISTS parse_offsets (
                file_path TEXT PRIMARY KEY,
                byte_offset INTEGER NOT NULL,
                mtime INTEGER NOT NULL
            )",
        )
        .await
        .ok()?;

        Some(Self { conn, _db: db })
    }

    /// Registers or updates a project's tokens-saved count. Best-effort.
    pub async fn upsert(&self, project_path: &Path, tokens_saved: u64) {
        let path_str = project_path.to_string_lossy().to_string();
        let _ = self
            .conn
            .execute(
                "INSERT INTO projects (path, tokens_saved) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET tokens_saved = ?2",
                params![path_str, tokens_saved as i64],
            )
            .await;
    }

    /// Returns the stored tokens_saved count for a specific project, or 0 if not found.
    pub async fn get_project_tokens(&self, project_path: &Path) -> u64 {
        let path_str = project_path.to_string_lossy().to_string();
        let mut rows = match self
            .conn
            .query(
                "SELECT tokens_saved FROM projects WHERE path = ?1",
                params![path_str],
            )
            .await
        {
            Ok(r) => r,
            Err(_) => return 0,
        };
        match rows.next().await {
            Ok(Some(row)) => row.get::<i64>(0).unwrap_or(0) as u64,
            _ => 0,
        }
    }

    /// Returns the sum of tokens_saved across all tracked projects.
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

    /// Returns all tracked project paths.
    pub async fn list_project_paths(&self) -> Vec<String> {
        let mut rows = match self.conn.query("SELECT path FROM projects", ()).await {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mut paths = Vec::new();
        loop {
            match rows.next().await {
                Ok(Some(row)) => {
                    if let Ok(path) = row.get::<String>(0) {
                        paths.push(path);
                    }
                }
                _ => break,
            }
        }
        paths
    }

    // ── Accounting: turns table ──────────────────────────────────────

    /// Insert a parsed turn. Returns `true` if inserted, `false` if duplicate.
    pub async fn insert_turn(&self, turn: &crate::accounting::parser::CostTurn) -> bool {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO turns
                 (message_id, project_hash, session_id, model, timestamp,
                  input_tokens, output_tokens, cache_write_tokens, cache_read_tokens,
                  cost_usd, category, tool_names)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                ],
            )
            .await
            .map(|n| n > 0)
            .unwrap_or(false)
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

    /// Token breakdown (input, output, cache_read) since a given timestamp.
    pub async fn token_breakdown_since(&self, since: u64) -> Option<(u64, u64, u64)> {
        let mut rows = self
            .conn
            .query(
                "SELECT COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0)
                 FROM turns WHERE timestamp >= ?1",
                params![since as i64],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        Some((
            row.get::<i64>(0).unwrap_or(0) as u64,
            row.get::<i64>(1).unwrap_or(0) as u64,
            row.get::<i64>(2).unwrap_or(0) as u64,
        ))
    }

    /// Cost grouped by model since a given timestamp.
    /// Returns `(model, cost, total_tokens)`.
    pub async fn cost_by_model_since(&self, since: u64) -> Vec<(String, f64, u64)> {
        let mut rows = match self
            .conn
            .query(
                "SELECT model, SUM(cost_usd), SUM(input_tokens + output_tokens)
                 FROM turns WHERE timestamp >= ?1
                 GROUP BY model ORDER BY SUM(cost_usd) DESC",
                params![since as i64],
            )
            .await
        {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        loop {
            match rows.next().await {
                Ok(Some(row)) => {
                    let model: String = row.get(0).unwrap_or_default();
                    let cost: f64 = row.get(1).unwrap_or(0.0);
                    let tokens: i64 = row.get(2).unwrap_or(0);
                    out.push((model, cost, tokens as u64));
                }
                _ => break,
            }
        }
        out
    }

    /// Cost grouped by category since a given timestamp.
    /// Returns `(category, cost, turn_count)`.
    pub async fn cost_by_category_since(&self, since: u64) -> Vec<(String, f64, u64)> {
        let mut rows = match self
            .conn
            .query(
                "SELECT category, SUM(cost_usd), COUNT(*)
                 FROM turns WHERE timestamp >= ?1
                 GROUP BY category ORDER BY SUM(cost_usd) DESC",
                params![since as i64],
            )
            .await
        {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        loop {
            match rows.next().await {
                Ok(Some(row)) => {
                    let cat: String = row.get(0).unwrap_or_default();
                    let cost: f64 = row.get(1).unwrap_or(0.0);
                    let count: i64 = row.get(2).unwrap_or(0);
                    out.push((cat, cost, count as u64));
                }
                _ => break,
            }
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
