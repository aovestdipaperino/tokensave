use codegraph::db::Database;
use codegraph::sync::*;
use codegraph::types::FileRecord;
use tempfile::TempDir;

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

#[test]
fn test_find_stale_files() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();
    db.upsert_file(&FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "old_hash".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 1001,
        node_count: 5,
    })
    .unwrap();

    let current = vec![("src/main.rs".to_string(), "new_hash".to_string())];
    let stale = find_stale_files(&db, &current).unwrap();
    assert_eq!(stale, vec!["src/main.rs"]);
}

#[test]
fn test_find_new_files() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();
    let current = vec!["src/new_file.rs".to_string()];
    let new = find_new_files(&db, &current).unwrap();
    assert_eq!(new, vec!["src/new_file.rs"]);
}

#[test]
fn test_find_removed_files() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();
    db.upsert_file(&FileRecord {
        path: "src/deleted.rs".to_string(),
        content_hash: "hash".to_string(),
        size: 50,
        modified_at: 1000,
        indexed_at: 1001,
        node_count: 2,
    })
    .unwrap();

    let current: Vec<String> = vec![];
    let removed = find_removed_files(&db, &current).unwrap();
    assert_eq!(removed, vec!["src/deleted.rs"]);
}
