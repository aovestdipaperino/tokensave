//! `sync --force` never deletes a database another connection holds.
//!
//! A separate test binary because it sets `TOKENSAVE_BUSY_TIMEOUT_MS`, which
//! is process-global: with the default two-minute timeout, five open attempts
//! would wait ten minutes on the held lock.

use libsql::Builder;
use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

/// An indexed project whose database is at v17 with the legacy column, so the
/// next open has a migration to run.
async fn project_needing_migration() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn f() {}\npub fn g() { f() }\n",
    )
    .expect("write source");
    let cg = TokenSave::init(dir.path()).await.expect("init");
    cg.index_all().await.expect("index");
    drop(cg);
    let db_path = dir.path().join(".tokensave").join("tokensave.db");
    let db = Builder::new_local(&db_path).build().await.expect("db");
    let conn = db.connect().expect("conn");
    conn.execute_batch(
        "ALTER TABLE edges DROP COLUMN resolved_by;
         ALTER TABLE edges ADD COLUMN resolved_by TEXT NOT NULL DEFAULT 'direct';
         CREATE INDEX idx_edges_resolved_by ON edges(resolved_by);
         PRAGMA user_version = 17;",
    )
    .await
    .expect("simulate a v17 database");
    (dir, db_path)
}

#[cfg(unix)]
fn inode(path: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).expect("metadata").ino()
}

#[tokio::test]
async fn a_forced_open_does_not_delete_a_locked_database() {
    std::env::set_var("TOKENSAVE_BUSY_TIMEOUT_MS", "100");
    let (dir, db_path) = project_needing_migration().await;
    #[cfg(unix)]
    let before = inode(&db_path);

    // A second connection holds the write lock, as a running server would.
    let holder_db = Builder::new_local(&db_path).build().await.expect("db");
    let holder = holder_db.connect().expect("conn");
    holder
        .execute("BEGIN IMMEDIATE", ())
        .await
        .expect("hold the write lock");

    let err = match TokenSave::open_rebuilding_failed_migration(dir.path()).await {
        Ok(_) => panic!("the open must fail while the lock is held"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("locked") || err.contains("busy"), "{err}");
    assert!(!err.contains("rebuild the index"), "{err}");

    #[cfg(unix)]
    assert_eq!(inode(&db_path), before, "the database file was replaced");
    // The holder still sees the original, unmigrated database.
    let mut rows = holder
        .query("PRAGMA user_version", ())
        .await
        .expect("version");
    let version: i64 = rows
        .next()
        .await
        .expect("row")
        .expect("one row")
        .get(0)
        .expect("value");
    assert_eq!(version, 17);
    holder.execute("ROLLBACK", ()).await.expect("release");

    // With the lock released, the same open migrates in place.
    let cg = TokenSave::open_rebuilding_failed_migration(dir.path())
        .await
        .expect("open after release");
    assert!(!cg.get_nodes_by_name("f").await.expect("query").is_empty());
    #[cfg(unix)]
    assert_eq!(
        inode(&db_path),
        before,
        "a migration must not replace the file"
    );
}
