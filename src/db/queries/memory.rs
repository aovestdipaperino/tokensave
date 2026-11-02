//! Session memory storage: agent decisions and touched code areas.
use crate::db::Database;
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::{CodeAreaRecord, DecisionRecord};

impl Database {
    /// Inserts a decision and returns its row id. `files` and `tags` are
    /// stored as JSON arrays.
    pub async fn insert_memory_decision(
        &self,
        text: &str,
        reason: Option<&str>,
        created_at: i64,
        files: &[String],
        tags: &[String],
    ) -> Result<i64> {
        let files_json = serde_json::to_string(files).map_err(|e| TokenSaveError::Database {
            message: format!("record_decision files serialization failed: {e}"),
            operation: "record_decision".to_string(),
        })?;
        let tags_json = serde_json::to_string(tags).map_err(|e| TokenSaveError::Database {
            message: format!("record_decision tags serialization failed: {e}"),
            operation: "record_decision".to_string(),
        })?;
        let conn = self.conn();
        conn.execute(
            "INSERT INTO memory_decisions (text, reason, created_at, files, tags) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            libsql::params![text, reason, created_at, files_json, tags_json],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("record_decision insert failed: {e}"),
            operation: "record_decision".to_string(),
        })?;
        Ok(conn.last_insert_rowid())
    }

    /// Returns up to `limit` decisions, newest first.
    ///
    /// `fts_query` must already be an escaped FTS5 MATCH expression (see
    /// `to_fts_match_query`); when present only matching decisions are
    /// returned. `since` keeps decisions created at or after that timestamp.
    pub async fn query_memory_decisions(
        &self,
        fts_query: Option<&str>,
        since: Option<i64>,
        limit: i64,
    ) -> Result<Vec<DecisionRecord>> {
        let conn = self.conn();

        let db_err = |e: libsql::Error| TokenSaveError::Database {
            message: format!("session_recall query failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut rows = match (fts_query, since) {
            (Some(q), Some(ts)) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 AND d.created_at >= ?2 \
                     ORDER BY d.created_at DESC LIMIT ?3",
                    libsql::params![q, ts, limit],
                )
                .await
                .map_err(db_err)?,
            (Some(q), None) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 \
                     ORDER BY d.created_at DESC LIMIT ?2",
                    libsql::params![q, limit],
                )
                .await
                .map_err(db_err)?,
            (None, Some(ts)) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions WHERE created_at >= ?1 \
                     ORDER BY created_at DESC LIMIT ?2",
                    libsql::params![ts, limit],
                )
                .await
                .map_err(db_err)?,
            (None, None) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions ORDER BY created_at DESC LIMIT ?1",
                    libsql::params![limit],
                )
                .await
                .map_err(db_err)?,
        };

        let row_err = |e: libsql::Error| TokenSaveError::Database {
            message: format!("session_recall row read failed: {e}"),
            operation: "session_recall".to_string(),
        };
        let json_err = |e: serde_json::Error| TokenSaveError::Database {
            message: format!("session_recall JSON parse failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(row_err)? {
            let files_json: String = row.get(4).map_err(row_err)?;
            let tags_json: String = row.get(5).map_err(row_err)?;
            out.push(DecisionRecord {
                id: row.get(0).map_err(row_err)?,
                text: row.get(1).map_err(row_err)?,
                reason: row.get::<Option<String>>(2).map_err(row_err)?,
                created_at: row.get(3).map_err(row_err)?,
                files: serde_json::from_str(&files_json).map_err(json_err)?,
                tags: serde_json::from_str(&tags_json).map_err(json_err)?,
            });
        }
        Ok(out)
    }

    /// Inserts a code area or, if `path` exists, bumps its `touch_count` and
    /// `last_touched_at`. A `None` description keeps the stored one.
    pub async fn upsert_memory_code_area(
        &self,
        path: &str,
        description: Option<&str>,
        touched_at: i64,
    ) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO memory_code_areas (path, description, last_touched_at, touch_count) \
                 VALUES (?1, ?2, ?3, 1) \
                 ON CONFLICT(path) DO UPDATE SET \
                    description = COALESCE(excluded.description, memory_code_areas.description), \
                    last_touched_at = excluded.last_touched_at, \
                    touch_count = memory_code_areas.touch_count + 1",
                libsql::params![path, description, touched_at],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("record_code_area upsert failed: {e}"),
                operation: "record_code_area".to_string(),
            })?;
        Ok(())
    }

    /// Returns up to `limit` code areas, most recently touched first.
    pub async fn list_memory_code_areas(&self, limit: i64) -> Result<Vec<CodeAreaRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, path, description, last_touched_at, touch_count \
                 FROM memory_code_areas ORDER BY last_touched_at DESC LIMIT ?1",
                libsql::params![limit],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("list_code_areas query failed: {e}"),
                operation: "list_code_areas".to_string(),
            })?;
        let row_err = |e: libsql::Error| TokenSaveError::Database {
            message: format!("list_code_areas row read failed: {e}"),
            operation: "list_code_areas".to_string(),
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(row_err)? {
            out.push(CodeAreaRecord {
                id: row.get(0).map_err(row_err)?,
                path: row.get(1).map_err(row_err)?,
                description: row.get::<Option<String>>(2).map_err(row_err)?,
                last_touched_at: row.get(3).map_err(row_err)?,
                touch_count: row.get::<i64>(4).map_err(row_err)? as u32,
            });
        }
        Ok(out)
    }
}
