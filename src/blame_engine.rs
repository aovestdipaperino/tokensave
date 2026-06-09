//! Per-symbol git blame / log engine.
//!
//! Walks a file's commit history via `gix`, fetches the blob at each
//! commit, parses it with `redundancy::parse_file`, and matches a target
//! symbol across commits via `redundancy::Fingerprint` similarity.

use std::path::Path;

use crate::redundancy::Fingerprint;

/// Why the history walk terminated.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryReason {
    /// The earliest commit examined created (or first introduced) the entity.
    Introduced,
    /// The entity moved from a different file at this boundary.
    RenamedFrom,
    /// The walk ran out of parent commits.
    HistoryExhausted,
    /// `max_commits` was reached before history was exhausted.
    MaxCommitsReached,
}

/// A single commit at which the target entity changed structurally.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChangeEvent {
    pub commit: String,
    pub short_sha: String,
    pub author: String,
    pub email: String,
    pub date: String, // RFC3339
    pub summary: String,
    pub file_at_commit: String,
}

/// Tunables passed in by the caller.
#[derive(Debug, Clone)]
pub struct BlameOptions {
    pub max_commits: usize,
    pub similarity_threshold: f64,
    pub max_blob_bytes: u64,
}

impl Default for BlameOptions {
    fn default() -> Self {
        Self {
            max_commits: 500,
            similarity_threshold: 0.85,
            max_blob_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Full result returned to the handlers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlameResult {
    pub events: Vec<ChangeEvent>,
    pub boundary_reason: BoundaryReason,
    pub commits_walked: usize,
    pub parse_failures: Vec<ParseFailure>,
    pub skipped_large: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ParseFailure {
    pub commit: String,
    pub error: String,
}

/// Compute the change-event history for an entity.
///
/// `target_fp` is the current working-tree fingerprint of the entity.
/// `start_line` and `end_line` are **0-indexed** (matching `Node` row
/// semantics).
pub fn log(
    _project_root: &Path,
    _file: &str,
    _start_line: u32,
    _end_line: u32,
    _language_key: &str,
    _target_fp: &Fingerprint,
    _opts: &BlameOptions,
) -> Result<BlameResult, String> {
    Err("not yet implemented".to_string())
}

/// Convenience wrapper: returns the most recent change event, or `None` if
/// the entity was never mutated in tracked history.
pub fn blame(
    project_root: &Path,
    file: &str,
    start_line: u32,
    end_line: u32,
    language_key: &str,
    target_fp: &Fingerprint,
    opts: &BlameOptions,
) -> Result<Option<ChangeEvent>, String> {
    let result = log(
        project_root,
        file,
        start_line,
        end_line,
        language_key,
        target_fp,
        opts,
    )?;
    Ok(result.events.into_iter().next_back())
}

/// Map a project-relative file path to the `ts_provider` language key.
///
/// Returns `None` if the extension isn't recognised by any tree-sitter
/// grammar bundled with `tokensave-large-treesitters`. Keys must match
/// those accepted by `crate::extraction::ts_provider::language`.
pub fn ts_lang_key_from_path(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "rs" => "rust",
        "go" => "go",
        "py" | "pyi" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "cs" => "c_sharp",
        "rb" => "ruby",
        "php" => "php",
        "scala" | "sc" => "scala",
        "dart" => "dart",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "sh" | "bash" => "bash",
        "nix" => "nix",
        "zig" => "zig",
        "proto" => "protobuf",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_key_recognises_common_extensions() {
        assert_eq!(ts_lang_key_from_path("src/foo.rs"), Some("rust"));
        assert_eq!(ts_lang_key_from_path("a.tsx"), Some("tsx"));
        assert_eq!(ts_lang_key_from_path("a.ts"), Some("typescript"));
        assert_eq!(ts_lang_key_from_path("a.proto"), Some("protobuf"));
        assert_eq!(ts_lang_key_from_path("a.cs"), Some("c_sharp"));
        assert_eq!(ts_lang_key_from_path("README.md"), None);
    }

    #[test]
    fn default_options_match_spec() {
        let opts = BlameOptions::default();
        assert_eq!(opts.max_commits, 500);
        assert!((opts.similarity_threshold - 0.85).abs() < f64::EPSILON);
        assert_eq!(opts.max_blob_bytes, 2 * 1024 * 1024);
    }
}
