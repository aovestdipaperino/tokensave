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

    /// Checkpoints the WAL. Best-effort.
    pub async fn checkpoint(&self) {
        let _ = self
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .await;
    }
}
