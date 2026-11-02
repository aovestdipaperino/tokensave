//! Lightweight probes that read an index file without opening a full
//! [`Database`](super::Database).
//!
//! Hooks run on every agent tool call, so they skip the channel checks,
//! migrations and write pragmas that `Database::open` performs and open the
//! file read-only instead.
use std::path::Path;

use crate::errors::Result;

/// Returns whether the index at `db_path` holds a node whose name matches
/// one of `names`, compared case-insensitively. An empty `names` is `false`.
pub(crate) async fn any_node_named(db_path: &Path, names: &[String]) -> Result<bool> {
    if names.is_empty() {
        return Ok(false);
    }
    let placeholders = vec!["?"; names.len()].join(",");
    let sql = format!("SELECT 1 FROM nodes WHERE LOWER(name) IN ({placeholders}) LIMIT 1");
    let params: Vec<libsql::Value> = names
        .iter()
        .map(|s| libsql::Value::Text(s.to_lowercase()))
        .collect();
    let db = libsql::Builder::new_local(db_path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await?;
    let conn = db.connect()?;
    let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
    Ok(rows.next().await?.is_some())
}
