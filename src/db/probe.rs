//! Lightweight read-only probes that skip [`super::Database::open`].
//!
//! The grep hook runs on every agent search, so it opens the index file
//! directly: no migrations, compatibility preflight or write pragmas.

use std::path::Path;

use crate::errors::Result;

/// Returns whether the index at `db_path` holds a node whose name matches
/// one of `names`, compared case-insensitively.
///
/// Returns `Ok(false)` for an empty `names`.
pub async fn any_name_indexed(db_path: &Path, names: &[String]) -> Result<bool> {
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
