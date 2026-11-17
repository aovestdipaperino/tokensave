//! #940: databases whose `-wal` sidecar disagrees with the database page size
//! are refused before the bundled SQLite, which lacks the fix for
//! CVE-2026-119375, can replay them.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use libsql::Builder;
use tempfile::TempDir;
use tokensave::db::{compatibility, migrations, Database};

const PAGE_SIZE: usize = 4096;
const WAL_HEADER_LEN: usize = 32;
const WAL_FRAME_LEN: usize = 24 + PAGE_SIZE;

fn wal_of(db_path: &Path) -> PathBuf {
    let mut path = OsString::from(db_path.as_os_str());
    path.push("-wal");
    PathBuf::from(path)
}

/// Builds a WAL-mode database whose writes, including page 1, are still in
/// the `-wal` sidecar, and copies both files to `shipped.db` in `dir`, as a
/// `.tokensave/` directory committed to a repository would carry them.
async fn shipped_database(dir: &Path) -> PathBuf {
    let live = dir.join("live.db");
    let db = Builder::new_local(&live)
        .build()
        .await
        .expect("live database opens");
    let conn = db.connect().expect("live database connects");
    conn.execute_batch(
        "PRAGMA page_size = 4096;
         PRAGMA journal_mode = WAL;
         PRAGMA wal_autocheckpoint = 0;
         CREATE TABLE t(x);
         INSERT INTO t VALUES (42);",
    )
    .await
    .expect("live database accepts writes");
    // Read through `File::open`, which shares access with SQLite's open
    // handles on Windows, while the connection keeps the frames in the WAL.
    let db_bytes = fs::read(&live).expect("live database is readable");
    let wal_bytes = fs::read(wal_of(&live)).expect("live WAL is readable");
    drop(conn);
    drop(db);
    assert!(
        wal_bytes.len() >= WAL_HEADER_LEN + WAL_FRAME_LEN,
        "the WAL must hold frames for the test to be meaningful"
    );
    let shipped = dir.join("shipped.db");
    fs::write(&shipped, db_bytes).expect("shipped database is writable");
    fs::write(wal_of(&shipped), wal_bytes).expect("shipped WAL is writable");
    shipped
}

fn rewrite_wal(db_path: &Path, edit: impl FnOnce(&mut Vec<u8>)) {
    let wal = wal_of(db_path);
    let mut bytes = fs::read(&wal).expect("WAL is readable");
    edit(&mut bytes);
    fs::write(&wal, bytes).expect("WAL is writable");
}

async fn assert_refused(db_path: &Path) {
    let db_before = fs::read(db_path).expect("database is readable");
    let wal_before = fs::read(wal_of(db_path)).expect("WAL is readable");

    let error = compatibility::preflight(db_path, migrations::latest_version())
        .await
        .expect_err("preflight refuses the WAL");
    assert!(compatibility::is_refusal(&error), "{error}");
    assert!(error.to_string().contains("CVE-2026-119375"), "{error}");

    let error = Database::open(db_path)
        .await
        .err()
        .expect("open refuses the WAL");
    assert!(compatibility::is_refusal(&error), "{error}");

    // A refusal must leave the files for the user to inspect.
    assert_eq!(fs::read(db_path).expect("database is readable"), db_before);
    assert_eq!(
        fs::read(wal_of(db_path)).expect("WAL is readable"),
        wal_before
    );
}

#[tokio::test]
async fn legitimate_wal_is_accepted_and_replayed() {
    let dir = TempDir::new().expect("temp dir");
    let shipped = shipped_database(dir.path()).await;

    compatibility::preflight(&shipped, migrations::latest_version())
        .await
        .expect("a WAL written by SQLite passes");

    let db = Builder::new_local(&shipped)
        .build()
        .await
        .expect("shipped database opens");
    let conn = db.connect().expect("shipped database connects");
    let mut rows = conn
        .query("SELECT x FROM t", ())
        .await
        .expect("table written only to the WAL is visible");
    let row = rows
        .next()
        .await
        .expect("row is readable")
        .expect("row exists");
    assert_eq!(row.get::<i64>(0).expect("x is an integer"), 42);
}

#[tokio::test]
async fn wal_header_page_size_mismatch_is_refused() {
    let dir = TempDir::new().expect("temp dir");
    let shipped = shipped_database(dir.path()).await;
    rewrite_wal(&shipped, |wal| {
        wal[8..12].copy_from_slice(&512_u32.to_be_bytes())
    });
    assert_refused(&shipped).await;
}

#[tokio::test]
async fn page_one_copy_with_other_page_size_is_refused() {
    let dir = TempDir::new().expect("temp dir");
    let shipped = shipped_database(dir.path()).await;
    rewrite_wal(&shipped, |wal| {
        let mut patched = 0;
        let mut offset = WAL_HEADER_LEN;
        while offset + WAL_FRAME_LEN <= wal.len() {
            if wal[offset..offset + 4] == 1_u32.to_be_bytes() {
                // Page size 512 is stored as bytes 0x02 0x00.
                let page = offset + 24;
                wal[page + 16] = 0x02;
                wal[page + 17] = 0x00;
                patched += 1;
            }
            offset += WAL_FRAME_LEN;
        }
        assert!(patched > 0, "the WAL must hold a copy of page 1");
    });
    assert_refused(&shipped).await;
}

#[tokio::test]
async fn invalid_database_header_with_wal_is_refused() {
    let dir = TempDir::new().expect("temp dir");
    let shipped = shipped_database(dir.path()).await;
    let mut db = fs::read(&shipped).expect("database is readable");
    // Bytes 0x01 0x00 decode to 256, which is not a valid page size.
    db[16] = 0x01;
    db[17] = 0x00;
    fs::write(&shipped, db).expect("database is writable");
    assert_refused(&shipped).await;
}

#[tokio::test]
async fn wal_files_sqlite_ignores_are_accepted() {
    let dir = TempDir::new().expect("temp dir");
    let shipped = shipped_database(dir.path()).await;
    let wal = wal_of(&shipped);
    let original = fs::read(&wal).expect("WAL is readable");

    // A bad magic number makes SQLite ignore the WAL contents.
    let mut bad_magic = original.clone();
    bad_magic[0] = 0;
    bad_magic[8..12].copy_from_slice(&512_u32.to_be_bytes());
    fs::write(&wal, bad_magic).expect("WAL is writable");
    compatibility::check_wal_sidecar(&shipped).expect("bad magic passes");

    // So does a WAL shorter than its header.
    fs::write(&wal, &original[..WAL_HEADER_LEN - 1]).expect("WAL is writable");
    compatibility::check_wal_sidecar(&shipped).expect("short WAL passes");

    // A missing or empty database makes SQLite delete the WAL on open.
    fs::write(&wal, &original).expect("WAL is writable");
    fs::write(&shipped, b"").expect("database is writable");
    compatibility::check_wal_sidecar(&shipped).expect("empty database passes");
    fs::remove_file(&shipped).expect("database is removable");
    compatibility::check_wal_sidecar(&shipped).expect("missing database passes");
}
