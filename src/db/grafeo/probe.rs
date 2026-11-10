//! Lightweight read-only probes that skip [`super::Database::open`].

use std::path::Path;

use grafeo::{GrafeoDB, Value};

use super::SYMBOL;
use crate::errors::{Result, TokenSaveError};

/// Returns whether the index at `db_path` holds a node whose name matches
/// one of `names`, compared case-insensitively.
///
/// Returns `Ok(false)` for an empty `names`. Grafeo refuses a read-only
/// open while another process holds the index for writing; the error lets
/// the grep hook fail closed, as it does for any probe error.
pub async fn any_name_indexed(db_path: &Path, names: &[String]) -> Result<bool> {
    if names.is_empty() {
        return Ok(false);
    }
    let error = |e: grafeo::Error| TokenSaveError::Database {
        message: e.to_string(),
        operation: "any_name_indexed".to_string(),
    };
    let db = GrafeoDB::open_read_only(db_path).map_err(error)?;
    // An exact-case hit is found through the name index without a scan.
    for name in names {
        if !db
            .find_nodes_by_property("name", &Value::from(name.as_str()))
            .map_err(error)?
            .is_empty()
        {
            return Ok(true);
        }
    }
    let wanted: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    let rows = db
        .execute(&format!("MATCH (n:{SYMBOL}) RETURN n.name"))
        .map_err(error)?
        .into_rows();
    Ok(rows.iter().any(|row| {
        row.first()
            .and_then(Value::as_str)
            .is_some_and(|name| wanted.contains(&name.to_ascii_lowercase()))
    }))
}
