// Rust guideline compliant 2025-10-17
//! Digest and freshness helpers for `tokensave_read` and the edit tools.
//!
//! This module used to back a server-side, cross-session response cache that
//! answered a repeated `tokensave_read` with an "unchanged" stub. The server
//! cannot know what the client still holds (compaction, rewinds, new
//! sessions, subagents), so that decision moved to the client: every body is
//! sent with its digest, and the client passes it back as `if_digest` to get
//! the stub (#650). The `read_cache` table created by the v9 migration is no
//! longer read or written; it stays in the schema so no migration (and no
//! reindex) is needed.

use sha2::{Digest, Sha256};

/// SHA-256 of arbitrary bytes, hex-encoded. Used as the `tokensave_read` body
/// digest a client echoes back as `if_digest`, and by the edit tools.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Reads a file's modification time, normalised to nanoseconds since the
/// UNIX epoch. Reported as `mtime_ns` in `tokensave_read` JSON responses.
pub fn file_mtime_ns(path: &std::path::Path) -> std::io::Result<i64> {
    use std::time::UNIX_EPOCH;
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata.modified()?;
    let dur = mtime
        .duration_since(UNIX_EPOCH)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "mtime before epoch"))?;
    let nanos = i128::from(dur.as_secs()) * 1_000_000_000 + i128::from(dur.subsec_nanos());
    Ok(nanos.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
}
