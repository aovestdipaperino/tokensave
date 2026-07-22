// Rust guideline compliant 2025-10-17
use std::path::Path;

use sha2::{Digest, Sha256};

/// Read a source file to a UTF-8 string, transparently handling UTF-16 LE/BE
/// (detected via BOM). Invalid non-BOM UTF-8 sequences are replaced with the
/// Unicode replacement character so legacy-encoded source can still be parsed.
/// Returns an IO error only when the file cannot be read or BOM-marked UTF-16
/// cannot be decoded.
pub fn read_source_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;

    // UTF-16 LE BOM: FF FE
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16(&u16s)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
    }

    // UTF-16 BE BOM: FE FF
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16(&u16s)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
    }

    // Strip UTF-8 BOM if present, then validate
    let start = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    match String::from_utf8(bytes[start..].to_vec()) {
        Ok(source) => Ok(source),
        Err(error) => Ok(String::from_utf8_lossy(error.as_bytes()).into_owned()),
    }
}

/// Get filesystem mtime (nanoseconds since epoch) and size for the incremental
/// sync pre-filter.
///
/// Nanosecond — not second — resolution is deliberate (#259). The pre-filter
/// treats a file as unchanged when both its stored mtime and size match, and
/// only then skips re-hashing it. At second resolution, an edit that lands in
/// the *same wall-clock second* as the last index and happens to preserve the
/// byte size (common when a build tool or editor rewrites an HTML file's
/// embedded script structure in place) is indistinguishable from no change, so
/// the stale index is served indefinitely. Nanosecond mtime shrinks that blind
/// window to effectively zero. The returned value feeds `FileRecord::modified_at`,
/// which is compared only against itself across syncs, so the unit change is
/// self-contained (older second-granular records simply trigger one re-hash on
/// the first sync after upgrade, which the content-hash check then resolves).
pub fn file_stat(path: &Path) -> Option<(i64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let nanos = mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos() as i64;
    Some((nanos, meta.len()))
}

/// Compute SHA-256 content hash of file content.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::time::{Duration, UNIX_EPOCH};

    /// Regression for #259: `file_stat` must expose sub-second mtime resolution
    /// so the incremental-sync pre-filter can tell apart two edits that land in
    /// the same wall-clock second while keeping the same byte size. At the old
    /// second granularity both mtimes collapsed to the same value and the second
    /// edit was served stale.
    #[test]
    fn file_stat_distinguishes_same_second_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.html");

        // Two identical-size contents, so `size` cannot disambiguate them.
        let mut f = File::create(&path).unwrap();
        f.write_all(b"<b>AAAA</b>").unwrap();
        f.sync_all().unwrap();

        // Same second, 1 ms apart.
        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        f.set_modified(base).unwrap();
        let (mtime_a, size_a) = file_stat(&path).unwrap();

        let mut f2 = OpenOptions::new().write(true).open(&path).unwrap();
        f2.write_all(b"<b>BBBB</b>").unwrap();
        f2.sync_all().unwrap();
        f2.set_modified(base + Duration::from_millis(1)).unwrap();
        let (mtime_b, size_b) = file_stat(&path).unwrap();

        assert_eq!(size_a, size_b, "sizes are equal by construction");
        assert_ne!(
            mtime_a, mtime_b,
            "sub-second edits within the same second must yield distinct mtimes"
        );
    }
}
