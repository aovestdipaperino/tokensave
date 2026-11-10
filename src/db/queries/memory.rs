//! Session-memory storage: agent decisions and touched code areas.
//!
//! Ranking, clamping and timestamps are decided by the caller in
//! `crate::tokensave`; this module stores and reads rows. The `files` and
//! `tags` lists are stored as JSON arrays.
use super::*;
use crate::types::{CodeAreaRecord, DecisionRecord};

impl Database {
    /// Inserts a decision recorded at `created_at`. Returns the new row id.
    pub async fn insert_decision(
        &self,
        text: &str,
        reason: Option<&str>,
        files: &[String],
        tags: &[String],
        created_at: i64,
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
            params![text, reason, created_at, files_json, tags_json],
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
    /// `query` is free text matched against the decision text and reason; a
    /// query with no searchable terms matches every decision. `since` keeps
    /// decisions created at or after that timestamp.
    pub async fn get_decisions(
        &self,
        query: Option<&str>,
        since: Option<i64>,
        limit: i64,
    ) -> Result<Vec<DecisionRecord>> {
        // FTS5 parses the bound MATCH string as a query expression even
        // through a bound parameter, so raw terms containing `-`, `.`, `/`
        // etc. (`data-api`) are syntax errors (#218). Escape into quoted
        // prefix terms first; a query with no tokenizable content degrades
        // to the unfiltered recency-ordered arms.
        let fts_query = query.and_then(to_fts_match_query);
        let conn = self.conn();
        let db_err = |e: libsql::Error| TokenSaveError::Database {
            message: format!("session_recall query failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut rows = match (fts_query.as_deref(), since) {
            (Some(q), Some(ts)) => conn
                .query(
                    "SELECT d.id, d.text, d.reason, d.created_at, d.files, d.tags \
                     FROM memory_decisions d \
                     JOIN memory_decisions_fts f ON f.rowid = d.id \
                     WHERE memory_decisions_fts MATCH ?1 AND d.created_at >= ?2 \
                     ORDER BY d.created_at DESC LIMIT ?3",
                    params![q, ts, limit],
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
                    params![q, limit],
                )
                .await
                .map_err(db_err)?,
            (None, Some(ts)) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions WHERE created_at >= ?1 \
                     ORDER BY created_at DESC LIMIT ?2",
                    params![ts, limit],
                )
                .await
                .map_err(db_err)?,
            (None, None) => conn
                .query(
                    "SELECT id, text, reason, created_at, files, tags \
                     FROM memory_decisions ORDER BY created_at DESC LIMIT ?1",
                    params![limit],
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

    /// Inserts a code area touched at `touched_at`, or bumps its touch count.
    /// A `None` description keeps the stored one.
    pub async fn upsert_code_area(
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
                params![path, description, touched_at],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("record_code_area upsert failed: {e}"),
                operation: "record_code_area".to_string(),
            })?;
        Ok(())
    }

    /// Stores `area` as given, replacing any area with the same path, so its
    /// touch count and timestamps survive a storage switch (#946). The
    /// record's `id` is not kept; the table assigns one.
    pub async fn import_code_area(&self, area: &CodeAreaRecord) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO memory_code_areas (path, description, last_touched_at, touch_count) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(path) DO UPDATE SET \
                    description = excluded.description, \
                    last_touched_at = excluded.last_touched_at, \
                    touch_count = excluded.touch_count",
                params![
                    area.path.as_str(),
                    area.description.as_deref(),
                    area.last_touched_at,
                    i64::from(area.touch_count)
                ],
            )
            .await
            .map_err(|e| TokenSaveError::Database {
                message: format!("import_code_area upsert failed: {e}"),
                operation: "import_code_area".to_string(),
            })?;
        Ok(())
    }

    /// Returns up to `limit` code areas, most recently touched first.
    pub async fn get_code_areas(&self, limit: i64) -> Result<Vec<CodeAreaRecord>> {
        let mut rows = self
            .conn()
            .query(
                "SELECT id, path, description, last_touched_at, touch_count \
                 FROM memory_code_areas ORDER BY last_touched_at DESC LIMIT ?1",
                params![limit],
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
