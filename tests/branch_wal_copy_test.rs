//! Regression tests for #292: tracking a branch copies the parent DB while a
//! long-lived server may still hold committed rows in the parent's
//! uncheckpointed `-wal` sidecar. A bare `.db` copy silently drops those rows
//! (queries against checkpointed tables keep working while e.g. `files` comes
//! up empty); `copy_branch_db` must carry them over.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use tokensave::branch::copy_branch_db;

fn wal_path(db: &Path) -> std::path::PathBuf {
    let mut os = db.as_os_str().to_os_string();
    os.push("-wal");
    std::path::PathBuf::from(os)
}

/// Opens `db`, commits 100 rows into a `files` table in WAL mode, and returns
/// the live connection (kept open so SQLite does not checkpoint on close).
async fn seed_wal_only_rows(db: &Path) -> libsql::Connection {
    let lib_db = libsql::Builder::new_local(db).build().await.unwrap();
    let conn = lib_db.connect().unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .await
        .unwrap();
    conn.execute_batch("CREATE TABLE files (path TEXT);")
        .await
        .unwrap();
    for i in 0..100 {
        conn.execute(
            "INSERT INTO files (path) VALUES (?1)",
            [format!("src/file{i}.rs")],
        )
        .await
        .unwrap();
    }
    assert!(
        std::fs::metadata(wal_path(db)).unwrap().len() > 0,
        "test setup must leave a non-empty WAL"
    );
    conn
}

async fn count_files(db: &Path) -> i64 {
    let lib_db = libsql::Builder::new_local(db).build().await.unwrap();
    let conn = lib_db.connect().unwrap();
    let mut rows = conn.query("SELECT COUNT(*) FROM files", ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    row.get(0).unwrap()
}

#[tokio::test]
async fn copy_branch_db_preserves_rows_still_in_wal() {
    let dir = tempfile::TempDir::new().unwrap();
    let parent = dir.path().join("parent.db");
    let _writer = seed_wal_only_rows(&parent).await;

    let copy = dir.path().join("copy.db");
    copy_branch_db(&parent, &copy).await.unwrap();

    assert_eq!(count_files(&copy).await, 100);
}

#[tokio::test]
async fn copy_branch_db_preserves_wal_rows_under_a_concurrent_reader() {
    let dir = tempfile::TempDir::new().unwrap();
    let parent = dir.path().join("parent.db");
    let writer = seed_wal_only_rows(&parent).await;

    // An open read transaction pins the WAL, so a checkpoint cannot fully
    // fold it into the main file. The snapshot layer (`VACUUM INTO`) must
    // carry the rows over regardless.
    let tx = writer
        .transaction_with_behavior(libsql::TransactionBehavior::Deferred)
        .await
        .unwrap();
    let mut rows = tx.query("SELECT COUNT(*) FROM files", ()).await.unwrap();
    let _pin = rows.next().await.unwrap().unwrap();

    let copy = dir.path().join("copy.db");
    copy_branch_db(&parent, &copy).await.unwrap();
    drop(rows);
    drop(tx);

    // The snapshot path writes a self-contained DB; a `-wal` sidecar next to
    // the copy would mean the racy checkpoint-and-copy fallback ran instead.
    assert!(
        !wal_path(&copy).exists(),
        "expected the VACUUM INTO snapshot path, not the file-copy fallback"
    );
    assert_eq!(count_files(&copy).await, 100);
}
