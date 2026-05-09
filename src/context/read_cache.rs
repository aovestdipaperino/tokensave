// Rust guideline compliant 2025-10-17
//! Cross-session response cache for `tokensave_read`.
//!
//! Cached entries are keyed by `(project_id, file_path, mode, args_hash)` and
//! survive across MCP sessions. The `mtime_ns` column on each row is the
//! source-of-truth for freshness: a row is served only if the file's current
//! `mtime_ns` matches what was cached. Any mismatch triggers a recomputation
//! and replaces the row.
//!
//! Per-session entries are still possible (set `session_id` to a real session
//! identifier), but the canonical 5.0 mode passes `GLOBAL_SESSION` so a single
//! row backs all sessions on the same project.
//!
//! The cache lives in the same libSQL database as the code graph and is wiped
//! by the v8 schema's `sweep` helper after `MAX_AGE_SECS` of inactivity.

use libsql::{params, Connection};
use sha2::{Digest, Sha256};

use crate::errors::{Result, TokenSaveError};

/// Sentinel session id used for cross-session cache rows. Picked so it cannot
/// collide with a real session UUID.
pub const GLOBAL_SESSION: &str = "global";

/// Rows older than this are evicted by the periodic sweep.
const MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

/// A cached read response.
#[derive(Debug, Clone)]
pub struct CachedRead {
    pub mtime_ns: i64,
    pub digest: String,
    pub body: Vec<u8>,
    pub token_count: u32,
}

/// Computes a stable hash of the per-call arguments that affect output. Used
/// as the `args_hash` cache-key component so two calls with different `lines`
/// or `limit` values map to distinct rows.
pub fn args_hash(args: &serde_json::Value) -> String {
    let canonical = canonicalize(args);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Sorts JSON object keys recursively so two semantically-equal arg objects
/// hash identically regardless of key insertion order.
fn canonicalize(v: &serde_json::Value) -> String {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by_key(|(k, _)| k.as_str());
            let inner = entries
                .into_iter()
                .map(|(k, v)| format!("\"{k}\":{}", canonicalize(v)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{inner}}}")
        }
        Value::Array(items) => {
            let inner = items.iter().map(canonicalize).collect::<Vec<_>>().join(",");
            format!("[{inner}]")
        }
        other => other.to_string(),
    }
}

/// SHA-256 of arbitrary bytes, hex-encoded. Used as the body digest so callers
/// can detect content changes even when only the cache layer changed.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Looks up a cached row. Returns `Some` only when the row exists *and* its
/// `mtime_ns` matches `current_mtime_ns`. A stale row (file changed since the
/// cache was written) is reported as a miss; the caller is expected to
/// recompute and `put` a fresh row, which replaces the stale one via the
/// primary-key `INSERT OR REPLACE`.
pub async fn get(
    conn: &Connection,
    project_id: &str,
    session_id: &str,
    file_path: &str,
    mode: &str,
    args_hash: &str,
    current_mtime_ns: i64,
) -> Result<Option<CachedRead>> {
    let mut rows = conn
        .query(
            "SELECT mtime_ns, digest, body, token_count
             FROM read_cache
             WHERE project_id = ?1
               AND session_id = ?2
               AND file_path  = ?3
               AND mode       = ?4
               AND args_hash  = ?5",
            params![project_id, session_id, file_path, mode, args_hash],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("read_cache lookup failed: {e}"),
            operation: "read_cache::get".to_string(),
        })?;

    let row = rows.next().await.map_err(|e| TokenSaveError::Database {
        message: format!("read_cache row fetch failed: {e}"),
        operation: "read_cache::get".to_string(),
    })?;

    let Some(row) = row else { return Ok(None) };

    let cached_mtime: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
        message: format!("read_cache column 0: {e}"),
        operation: "read_cache::get".to_string(),
    })?;

    if cached_mtime != current_mtime_ns {
        return Ok(None);
    }

    let digest: String = row.get(1).map_err(|e| TokenSaveError::Database {
        message: format!("read_cache column 1: {e}"),
        operation: "read_cache::get".to_string(),
    })?;
    let body: Vec<u8> = row.get(2).map_err(|e| TokenSaveError::Database {
        message: format!("read_cache column 2: {e}"),
        operation: "read_cache::get".to_string(),
    })?;
    let token_count: i64 = row.get(3).map_err(|e| TokenSaveError::Database {
        message: format!("read_cache column 3: {e}"),
        operation: "read_cache::get".to_string(),
    })?;

    Ok(Some(CachedRead {
        mtime_ns: cached_mtime,
        digest,
        body,
        token_count: token_count.max(0) as u32,
    }))
}

/// Inserts or replaces a cache row. The primary key is
/// `(project_id, session_id, file_path, mode, args_hash)`; a re-`put` with
/// matching keys replaces the prior row, which is how stale entries (mtime
/// mismatch) get evicted.
#[allow(clippy::too_many_arguments)]
pub async fn put(
    conn: &Connection,
    project_id: &str,
    session_id: &str,
    file_path: &str,
    mtime_ns: i64,
    mode: &str,
    args_hash: &str,
    digest: &str,
    body: &[u8],
    token_count: u32,
) -> Result<()> {
    let now = unix_seconds();
    conn.execute(
        "INSERT OR REPLACE INTO read_cache
            (project_id, session_id, file_path, mtime_ns, mode, args_hash,
             digest, body, token_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            project_id,
            session_id,
            file_path,
            mtime_ns,
            mode,
            args_hash,
            digest,
            body,
            i64::from(token_count),
            now
        ],
    )
    .await
    .map_err(|e| TokenSaveError::Database {
        message: format!("read_cache insert failed: {e}"),
        operation: "read_cache::put".to_string(),
    })?;
    Ok(())
}

/// Deletes rows older than [`MAX_AGE_SECS`]. Returns the number of rows
/// removed. Safe to call from any context.
pub async fn sweep(conn: &Connection) -> Result<u64> {
    let cutoff = unix_seconds() - MAX_AGE_SECS;
    let removed = conn
        .execute(
            "DELETE FROM read_cache WHERE created_at < ?1",
            params![cutoff],
        )
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("read_cache sweep failed: {e}"),
            operation: "read_cache::sweep".to_string(),
        })?;
    Ok(removed)
}

fn unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Reads a file's modification time, normalised to nanoseconds since the
/// UNIX epoch. Used as the freshness key for cache lookups.
pub fn file_mtime_ns(path: &std::path::Path) -> std::io::Result<i64> {
    use std::time::UNIX_EPOCH;
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata.modified()?;
    let dur = mtime
        .duration_since(UNIX_EPOCH)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "mtime before epoch"))?;
    let nanos = i128::from(dur.as_secs()) * 1_000_000_000 + i128::from(dur.subsec_nanos());
    Ok(nanos.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
}
