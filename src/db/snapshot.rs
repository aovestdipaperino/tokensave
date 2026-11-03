//! Consistent file-level copies of a libSQL index (#292).
//!
//! Tracking a new branch starts from a copy of its parent's index. How to
//! take that copy safely depends on the storage engine's files, so it lives
//! with the backend.

use std::path::{Path, PathBuf};

use crate::errors::Result;

/// Returns the path of a database's `-wal` sidecar file.
fn wal_sidecar(db_path: &Path) -> PathBuf {
    let mut os = db_path.as_os_str().to_os_string();
    os.push("-wal");
    PathBuf::from(os)
}

/// File size in bytes, or 0 when the file does not exist.
fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |m| m.len())
}

/// Snapshots `parent_db` into `new_db_path` via `VACUUM INTO`, which writes a
/// consistent copy under the connection's read snapshot even while another
/// connection is committing. Fails if the parent is not a readable `SQLite`
/// file; `new_db_path` must not exist.
async fn vacuum_into(parent_db: &Path, new_db_path: &Path) -> std::result::Result<(), String> {
    let db = libsql::Builder::new_local(parent_db)
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let conn = db.connect().map_err(|e| e.to_string())?;
    // A lossy conversion would make VACUUM INTO "succeed" at a mangled
    // replacement-character path; bail to the file-copy fallback instead,
    // which handles non-UTF-8 paths losslessly.
    let dest = new_db_path
        .to_str()
        .ok_or_else(|| "non-UTF-8 destination path".to_string())?
        .to_owned();
    conn.execute("VACUUM INTO ?1", [dest])
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Copies `parent_db` to `new_db_path` without dropping data that sits in the
/// parent's uncheckpointed WAL (#292).
///
/// A long-lived MCP server keeps the parent DB open in WAL mode, so committed
/// rows can live only in the `-wal` sidecar for a long time. A bare
/// `fs::copy` of the `.db` then produces a truncated snapshot — nodes that
/// happen to be checkpointed still answer queries while e.g. the `files`
/// table comes up empty. Two layers close that hole:
///
/// 1. `VACUUM INTO` writes a transactionally consistent snapshot of the
///    parent (main file + WAL contents), correct even while a concurrent
///    connection is writing. It targets a unique temp file that is renamed
///    over the destination — the same atomic-publish idiom as
///    `save_branch_meta` — so two trackers racing on the same branch each
///    produce a complete DB and the loser atomically replaces the winner's,
///    and a stale destination left by an interrupted earlier attempt is
///    replaced rather than diverting to the fallback below.
/// 2. If the snapshot fails (the parent is not a readable `SQLite` file, or
///    an exotic locking failure), fall back to a best-effort
///    `wal_checkpoint(TRUNCATE)` followed by a plain file copy, bringing the
///    `-wal` sidecar along if the checkpoint could not empty it so `SQLite`
///    recovers it on first open of the new DB. This path is not
///    race-free against an active writer — the snapshot layer above is.
pub async fn copy_database(parent_db: &Path, new_db_path: &Path) -> Result<()> {
    if let Err(error) =
        super::compatibility::preflight(parent_db, super::migrations::latest_version()).await
    {
        if super::compatibility::is_refusal(&error) {
            return Err(error);
        }
    }
    let file_name = new_db_path.file_name().map_or_else(
        || "branch.db".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp =
        new_db_path.with_file_name(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));
    if vacuum_into(parent_db, &tmp).await.is_ok() {
        if let Err(e) = std::fs::rename(&tmp, new_db_path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        return Ok(());
    }
    let _ = std::fs::remove_file(&tmp);

    let parent_wal = wal_sidecar(parent_db);
    if file_len(&parent_wal) > 0 {
        // Errors are deliberately ignored: the non-empty-WAL check below
        // catches every failure mode uniformly by falling back to a sidecar
        // copy.
        if let Ok(db) = libsql::Builder::new_local(parent_db).build().await {
            if let Ok(conn) = db.connect() {
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").await;
            }
        }
    }
    std::fs::copy(parent_db, new_db_path)?;
    if file_len(&parent_wal) > 0 {
        std::fs::copy(&parent_wal, wal_sidecar(new_db_path))?;
    }
    Ok(())
}
