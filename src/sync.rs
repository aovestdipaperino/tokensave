// Rust guideline compliant 2025-10-17
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::db::Database;
use crate::errors::Result;

/// Read a source file to a UTF-8 string, transparently handling UTF-16 LE/BE
/// (detected via BOM). Returns an IO error only when the file genuinely cannot
/// be read or decoded.
pub fn read_source_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;

    // UTF-16 LE BOM: FF FE
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16(&u16s).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        });
    }

    // UTF-16 BE BOM: FE FF
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16(&u16s).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        });
    }

    // Strip UTF-8 BOM if present, then validate
    let start = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    String::from_utf8(bytes[start..].to_vec()).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })
}

/// Get filesystem mtime (seconds since epoch) and size for pre-filter.
pub fn file_stat(path: &Path) -> Option<(i64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let secs = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((secs, meta.len()))
}

/// Compute SHA-256 content hash of file content.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Find files whose stored content hash differs from the current hash.
pub async fn find_stale_files(db: &Database, current_hashes: &[(String, String)]) -> Result<Vec<String>> {
    let mut stale = Vec::new();
    for (path, current_hash) in current_hashes {
        if let Some(file_record) = db.get_file(path).await? {
            if file_record.content_hash != *current_hash {
                stale.push(path.clone());
            }
        }
    }
    Ok(stale)
}

/// Find files that exist on disk but not in the database.
pub async fn find_new_files(db: &Database, current_files: &[String]) -> Result<Vec<String>> {
    let mut new_files = Vec::new();
    for path in current_files {
        if db.get_file(path).await?.is_none() {
            new_files.push(path.clone());
        }
    }
    Ok(new_files)
}

/// Find files that are in the database but no longer exist on disk.
pub async fn find_removed_files(db: &Database, current_files: &[String]) -> Result<Vec<String>> {
    let all_db_files = db.get_all_files().await?;
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
