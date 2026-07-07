//! Session memory (decisions, code areas).
use super::*;

// ---------------------------------------------------------------------------
// Session memory
// ---------------------------------------------------------------------------

pub(crate) const MAX_RECALL_LIMIT: usize = 200;
pub(crate) const MAX_CODE_AREAS_LIMIT: usize = 200;

/// Half-life (in seconds) of the recency-decay weight applied to decisions in
/// the no-explicit-query recall path.
///
/// A decision's weight halves every `RECALL_DECAY_HALF_LIFE_SECS`. Chosen as
/// 14 days: recent decisions clearly outrank stale ones within a typical
/// multi-week work cycle, while the decay stays gentle enough that month-old
/// context still carries meaningful weight. This is a *ranking* knob only —
/// the weight asymptotically approaches zero but never reaches it, so no
/// decision is ever dropped or expired (issue #122: decay, not TTL).
pub(crate) const RECALL_DECAY_HALF_LIFE_SECS: f64 = 14.0 * 24.0 * 60.0 * 60.0;

/// Maximum number of decisions summarised in the `session_start` delta.
///
/// Keeps the injected "here's where we left off" summary within a small token
/// budget. Each entry is additionally truncated (see [`DELTA_TEXT_MAX_CHARS`]).
pub(crate) const SESSION_DELTA_MAX_DECISIONS: usize = 5;

/// Maximum number of code areas summarised in the `session_start` delta.
pub(crate) const SESSION_DELTA_MAX_CODE_AREAS: usize = 5;

/// Maximum character length of any single text field in the session delta.
pub(crate) const DELTA_TEXT_MAX_CHARS: usize = 120;

/// Recency-decay weight for a decision recorded at `created_at`, evaluated at
/// `now`.
///
/// Returns `2^(-age / half_life)`, an exponential decay in `[0, 1]` that is
/// `1.0` for a just-recorded decision and halves every
/// [`RECALL_DECAY_HALF_LIFE_SECS`]. The weight is strictly positive for all
/// finite ages, so older decisions sink in ranking but are never excluded.
///
/// Future timestamps (clock skew) clamp to weight `1.0`.
pub(crate) fn recency_decay_weight(created_at: i64, now: i64) -> f64 {
    let age = (now - created_at).max(0) as f64;
    2.0_f64.powf(-age / RECALL_DECAY_HALF_LIFE_SECS)
}

/// Truncate `s` to at most `max_chars` characters, appending an ellipsis when
/// the string was shortened. Respects UTF-8 char boundaries.
pub(crate) fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let kept: String = s.chars().take(max_chars).collect();
    format!("{kept}…")
}

/// A compact, budget-bounded "where we left off" summary emitted at session
/// start. See [`TokenSave::session_delta`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionDelta {
    /// Highest-value recent decisions, newest-first, each truncated.
    pub recent_decisions: Vec<DeltaEntry>,
    /// Recently touched code areas, newest-first, each truncated.
    pub recent_code_areas: Vec<DeltaEntry>,
}

/// A single truncated line in a [`SessionDelta`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeltaEntry {
    /// Short, truncated headline (decision text or code-area path).
    pub summary: String,
    /// UNIX timestamp (seconds) the underlying memory was recorded/touched.
    pub at: i64,
}

impl TokenSave {
    /// Record an agent decision. Returns the new row id.
    pub async fn record_decision(
        &self,
        text: &str,
        reason: Option<&str>,
        files: &[String],
        tags: &[String],
    ) -> crate::errors::Result<i64> {
        debug_assert!(!text.is_empty(), "decision text must not be empty");
        let files_json =
            serde_json::to_string(files).map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("record_decision files serialization failed: {e}"),
                operation: "record_decision".to_string(),
            })?;
        let tags_json =
            serde_json::to_string(tags).map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("record_decision tags serialization failed: {e}"),
                operation: "record_decision".to_string(),
            })?;
        let now = current_timestamp();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO memory_decisions (text, reason, created_at, files, tags) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            libsql::params![text, reason, now, files_json, tags_json],
        )
        .await
        .map_err(|e| crate::errors::TokenSaveError::Database {
            message: format!("record_decision insert failed: {e}"),
            operation: "record_decision".to_string(),
        })?;
        Ok(conn.last_insert_rowid())
    }

    /// Recall decisions. With `query`, runs FTS5 MATCH against text+reason
    /// ordered by relevance (recency). Without `query`, orders by a
    /// recency-decay weight ([`recency_decay_weight`]) so older decisions
    /// rank lower but are never dropped — there is no TTL or hard expiry.
    pub async fn session_recall(
        &self,
        query: Option<&str>,
        since: Option<i64>,
        limit: usize,
    ) -> crate::errors::Result<Vec<DecisionRecord>> {
        let limit = limit.clamp(1, MAX_RECALL_LIMIT) as i64;
        let conn = self.db.conn();

        let db_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall query failed: {e}"),
            operation: "session_recall".to_string(),
        };

        let mut rows = match (query, since) {
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

        let row_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
            message: format!("session_recall row read failed: {e}"),
            operation: "session_recall".to_string(),
        };
        let json_err = |e: serde_json::Error| crate::errors::TokenSaveError::Database {
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

        // Recency-decay ranking for the no-explicit-query path. The SQL above
        // already LIMITed to the newest N rows (which, because the decay is
        // monotonic in `created_at`, are exactly the N highest-weighted rows),
        // so we only need to (re)order the selected set by decay weight here.
        // Higher weight first; ties keep newest-first. No row is dropped — old
        // decisions still surface, just lower down.
        if query.is_none() {
            let now = current_timestamp();
            out.sort_by(|a, b| {
                let wa = recency_decay_weight(a.created_at, now);
                let wb = recency_decay_weight(b.created_at, now);
                wb.partial_cmp(&wa)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.created_at.cmp(&a.created_at))
            });
        }
        Ok(out)
    }

    /// Build a compact, budget-bounded "where we left off" delta for session
    /// start.
    ///
    /// Returns the highest-value *recent* memories — the most recent decisions
    /// (ranked by the same recency-decay weight as [`session_recall`]) and the
    /// most recently touched code areas — each truncated to a short headline.
    /// Entry counts are capped ([`SESSION_DELTA_MAX_DECISIONS`],
    /// [`SESSION_DELTA_MAX_CODE_AREAS`]) so the result stays cheap to inject at
    /// session start. This never deletes or expires any memory; it is a view.
    ///
    /// # Errors
    /// Returns a database error if the underlying recall queries fail.
    pub async fn session_delta(&self) -> crate::errors::Result<SessionDelta> {
        let decisions = self
            .session_recall(None, None, SESSION_DELTA_MAX_DECISIONS)
            .await?;
        let recent_decisions = decisions
            .into_iter()
            .take(SESSION_DELTA_MAX_DECISIONS)
            .map(|d| DeltaEntry {
                summary: truncate_chars(&d.text, DELTA_TEXT_MAX_CHARS),
                at: d.created_at,
            })
            .collect();

        let areas = self.list_code_areas(SESSION_DELTA_MAX_CODE_AREAS).await?;
        let recent_code_areas = areas
            .into_iter()
            .take(SESSION_DELTA_MAX_CODE_AREAS)
            .map(|a| DeltaEntry {
                summary: truncate_chars(&a.path, DELTA_TEXT_MAX_CHARS),
                at: a.last_touched_at,
            })
            .collect();

        Ok(SessionDelta {
            recent_decisions,
            recent_code_areas,
        })
    }

    /// Record (or update) a code area the agent worked in. Increments `touch_count`
    /// on re-touch. Description is set on first insert; subsequent `None` values
    /// preserve the existing description.
    pub async fn record_code_area(
        &self,
        path: &str,
        description: Option<&str>,
    ) -> crate::errors::Result<()> {
        debug_assert!(!path.is_empty(), "code area path must not be empty");
        let now = current_timestamp();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO memory_code_areas (path, description, last_touched_at, touch_count) \
             VALUES (?1, ?2, ?3, 1) \
             ON CONFLICT(path) DO UPDATE SET \
                description = COALESCE(excluded.description, memory_code_areas.description), \
                last_touched_at = excluded.last_touched_at, \
                touch_count = memory_code_areas.touch_count + 1",
            libsql::params![path, description, now],
        )
        .await
        .map_err(|e| crate::errors::TokenSaveError::Database {
            message: format!("record_code_area upsert failed: {e}"),
            operation: "record_code_area".to_string(),
        })?;
        Ok(())
    }

    /// List code areas, most-recently-touched first.
    pub async fn list_code_areas(
        &self,
        limit: usize,
    ) -> crate::errors::Result<Vec<CodeAreaRecord>> {
        let limit = limit.clamp(1, MAX_CODE_AREAS_LIMIT) as i64;
        let conn = self.db.conn();
        let mut rows = conn
            .query(
                "SELECT id, path, description, last_touched_at, touch_count \
                 FROM memory_code_areas ORDER BY last_touched_at DESC LIMIT ?1",
                libsql::params![limit],
            )
            .await
            .map_err(|e| crate::errors::TokenSaveError::Database {
                message: format!("list_code_areas query failed: {e}"),
                operation: "list_code_areas".to_string(),
            })?;
        let row_err = |e: libsql::Error| crate::errors::TokenSaveError::Database {
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
