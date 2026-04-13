use std::io::Write;
use tokensave::db::Database;
use tokensave::sync::*;
use tokensave::types::FileRecord;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_content_hash_deterministic() {
    let hash1 = content_hash("fn main() {}");
    let hash2 = content_hash("fn main() {}");
    assert_eq!(hash1, hash2);
}

#[test]
fn test_content_hash_different() {
    let hash1 = content_hash("fn main() {}");
    let hash2 = content_hash("fn main() { println!(\"hello\"); }");
    assert_ne!(hash1, hash2);
}

#[tokio::test]
async fn test_find_stale_files() {
    let dir = TempDir::new().unwrap();
    let (db, _) = Database::initialize(&dir.path().join("test.db")).await.unwrap();
    db.upsert_file(&FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "old_hash".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 1001,
        node_count: 5,
    })
    .await
    .unwrap();

    let current = vec![("src/main.rs".to_string(), "new_hash".to_string())];
    let stale = find_stale_files(&db, &current).await.unwrap();
    assert_eq!(stale, vec!["src/main.rs"]);
}

#[tokio::test]
async fn test_find_new_files() {
    let dir = TempDir::new().unwrap();
    let (db, _) = Database::initialize(&dir.path().join("test.db")).await.unwrap();
    let current = vec!["src/new_file.rs".to_string()];
    let new = find_new_files(&db, &current).await.unwrap();
    assert_eq!(new, vec!["src/new_file.rs"]);
}

#[tokio::test]
async fn test_find_removed_files() {
    let dir = TempDir::new().unwrap();
    let (db, _) = Database::initialize(&dir.path().join("test.db")).await.unwrap();
    db.upsert_file(&FileRecord {
        path: "src/deleted.rs".to_string(),
        content_hash: "hash".to_string(),
        size: 50,
        modified_at: 1000,
        indexed_at: 1001,
        node_count: 2,
    })
    .await
    .unwrap();

    let current: Vec<String> = vec![];
    let removed = find_removed_files(&db, &current).await.unwrap();
    assert_eq!(removed, vec!["src/deleted.rs"]);
}

#[test]
fn test_file_stat_returns_mtime_and_size() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"hello world").unwrap();
    f.flush().unwrap();
    let (mtime, size) = file_stat(f.path()).unwrap();
    assert!(mtime > 0, "mtime should be positive");
    assert_eq!(size, 11);
}

#[test]
fn test_file_stat_nonexistent() {
    assert!(file_stat(std::path::Path::new("/nonexistent/file.rs")).is_none());
}

#[test]
fn test_read_source_file_utf8() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"fn main() {}").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "fn main() {}");
}

#[test]
fn test_read_source_file_utf8_bom() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"\xEF\xBB\xBFfn main() {}").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "fn main() {}");
}

#[test]
fn test_read_source_file_utf16_le() {
    let mut f = NamedTempFile::new().unwrap();
    // UTF-16 LE BOM + "hi"
    f.write_all(b"\xFF\xFE\x68\x00\x69\x00").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "hi");
}

#[test]
fn test_read_source_file_utf16_be() {
    let mut f = NamedTempFile::new().unwrap();
    // UTF-16 BE BOM + "hi"
    f.write_all(b"\xFE\xFF\x00\x68\x00\x69").unwrap();
    let content = read_source_file(f.path()).unwrap();
    assert_eq!(content, "hi");
}

#[test]
fn test_read_source_file_invalid_encoding() {
    let mut f = NamedTempFile::new().unwrap();
    // Invalid UTF-8 sequence without any BOM
    f.write_all(b"\x80\x81\x82\x83").unwrap();
    assert!(read_source_file(f.path()).is_err());
}
