use sha2::{Digest, Sha256};

use crate::db::Database;
use crate::errors::Result;

/// Compute SHA-256 content hash of file content.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Find files whose stored content hash differs from the current hash.
pub fn find_stale_files(db: &Database, current_hashes: &[(String, String)]) -> Result<Vec<String>> {
    let mut stale = Vec::new();
    for (path, current_hash) in current_hashes {
        if let Some(file_record) = db.get_file(path)? {
            if file_record.content_hash != *current_hash {
                stale.push(path.clone());
            }
        }
    }
    Ok(stale)
}

/// Find files that exist on disk but not in the database.
pub fn find_new_files(db: &Database, current_files: &[String]) -> Result<Vec<String>> {
    let mut new_files = Vec::new();
    for path in current_files {
        if db.get_file(path)?.is_none() {
            new_files.push(path.clone());
        }
    }
    Ok(new_files)
}

/// Find files that are in the database but no longer exist on disk.
pub fn find_removed_files(db: &Database, current_files: &[String]) -> Result<Vec<String>> {
    let all_db_files = db.get_all_files()?;
    let current_set: std::collections::HashSet<&str> =
        current_files.iter().map(|s| s.as_str()).collect();
    let mut removed = Vec::new();
    for file_record in &all_db_files {
        if !current_set.contains(file_record.path.as_str()) {
            removed.push(file_record.path.clone());
        }
    }
    Ok(removed)
}
