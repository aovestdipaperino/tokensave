//! File-level copies of a Grafeo index.

use std::path::Path;

use crate::errors::Result;

/// Copies the index at `parent_db` to `new_db_path` for a newly tracked
/// branch.
///
/// Grafeo locks a file it has open, so the parent can only be copied while
/// no process holds it. The copy goes through a temporary file renamed over
/// the destination, so a reader never sees a partial copy. The parent's
/// write-ahead-log directory is not copied; it is empty once the holder has
/// closed the database.
pub async fn copy_database(parent_db: &Path, new_db_path: &Path) -> Result<()> {
    let file_name = new_db_path.file_name().map_or_else(
        || "branch.db".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let tmp = new_db_path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    std::fs::copy(parent_db, &tmp)?;
    if let Err(e) = std::fs::rename(&tmp, new_db_path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}
