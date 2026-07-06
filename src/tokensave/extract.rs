//! Subprocess-isolated extraction and path normalization helpers.
use super::*;

/// Convert any backslash in a *relative* project-root-relative path to a
/// forward slash, matching the canonical form the walker
/// ([`scan_files`](TokenSave::scan_files) → [`accept_file`](TokenSave::accept_file))
/// uses when writing to the DB.
///
/// Applied defensively at sync/staleness entry points so that callers
/// holding OS-native paths (PowerShell-shaped `src\foo.py`, paths echoed
/// back from MCP tool responses on Windows, etc.) hit the same `files`
/// row as the walker would — preventing the duplicate-row corruption
/// from #87 where the same physical file showed up as both `src/foo.py`
/// and `src\foo.py` in the `files` table.
pub(crate) fn normalize_rel_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Normalize a slice of relative paths to canonical (forward-slash)
/// form. Allocates a new `Vec` only when at least one entry needed
/// normalization — common case on Unix is a zero-copy pass-through to
/// the caller's existing `Vec`.
pub(crate) fn normalize_rel_paths(paths: &[String]) -> Vec<String> {
    paths.iter().map(|p| normalize_rel_path(p)).collect()
}

/// Run `extractor.extract()` inside `catch_unwind` so a panic (e.g. from a
/// malformed file or an extractor bug) skips the file instead of aborting sync.
pub(crate) fn safe_extract(
    extractor: &dyn crate::extraction::LanguageExtractor,
    file_path: &str,
    source: &str,
) -> Option<ExtractionResult> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        extractor.extract(file_path, source)
    }))
    .map_err(|_| {
        eprintln!("[tokensave] extraction panicked for {file_path}, skipping");
    })
    .ok()
}

/// Tuple shape produced per file by both extraction paths.
type ExtractTuple = (String, ExtractionResult, String, u64, i64);

/// Extract every file in `files`, isolating each extraction in a subprocess
/// when possible. Subprocess isolation contains C/C++ grammar aborts that
/// `catch_unwind` cannot intercept; it is the primary defense against
/// tree-sitter scanners that call `abort()` (issue #49).
///
/// Falls back to in-process extraction with `safe_extract` if the worker
/// pool cannot start (e.g. when running under `cargo test`, where
/// `current_exe()` points at the test harness rather than the tokensave
/// binary). Either way, returns one tuple per successfully-processed file
/// plus a list of `(path, reason)` pairs for files that timed out or
/// repeatedly crashed during extraction.
pub(crate) fn extract_files_isolated(
    project_root: &Path,
    registry: &crate::extraction::LanguageRegistry,
    files: Vec<String>,
) -> (Vec<ExtractTuple>, Vec<(String, String)>) {
    if should_use_subprocess() {
        let workers = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
        let timeout = std::time::Duration::from_secs(
            crate::user_config::UserConfig::load().extraction_timeout_secs,
        );
        match crate::extraction_worker::WorkerPool::new(workers, project_root.to_path_buf()) {
            Ok(pool) => {
                let outcome = pool.extract_files(files, |_, _, _| {}, timeout);
                return (outcome.results, outcome.skipped);
            }
            Err(e) => eprintln!(
                "[tokensave] could not spawn extraction worker pool ({e}), \
                 falling back to in-process extraction"
            ),
        }
    }
    (
        extract_files_in_process(project_root, registry, &files),
        Vec::new(),
    )
}

pub(crate) fn extract_files_in_process(
    project_root: &Path,
    registry: &crate::extraction::LanguageRegistry,
    files: &[String],
) -> Vec<ExtractTuple> {
    files
        .par_iter()
        .filter_map(|file_path| {
            let abs_path = project_root.join(file_path);
            let source = sync::read_source_file(&abs_path).ok()?;
            let extractor = registry.extractor_for_file(file_path)?;
            let mut result = safe_extract(extractor, file_path, &source)?;
            result.sanitize();
            let hash = sync::content_hash(&source);
            let size = source.len() as u64;
            let mtime = sync::file_stat(&abs_path).map_or_else(current_timestamp, |(m, _)| m);
            Some((file_path.clone(), result, hash, size, mtime))
        })
        .collect()
}

/// Subprocess extraction is the production path. Tests and any environment
/// where `current_exe()` does not point at the real `tokensave` binary
/// transparently fall back to in-process extraction.
pub(crate) fn should_use_subprocess() -> bool {
    if std::env::var_os("TOKENSAVE_DISABLE_SUBPROCESS").is_some() {
        return false;
    }
    let Ok(path) = std::env::current_exe() else {
        return false;
    };
    matches!(path.file_stem().and_then(|s| s.to_str()), Some("tokensave"))
}

#[cfg(test)]
mod path_normalization_tests {
    use super::{normalize_rel_path, normalize_rel_paths};

    #[test]
    fn normalize_rel_path_converts_backslashes() {
        assert_eq!(normalize_rel_path("src\\foo.py"), "src/foo.py");
        assert_eq!(normalize_rel_path("a\\b\\c\\d.rs"), "a/b/c/d.rs");
    }

    #[test]
    fn normalize_rel_path_leaves_forward_slashes_alone() {
        assert_eq!(normalize_rel_path("src/foo.py"), "src/foo.py");
        assert_eq!(normalize_rel_path("a"), "a");
        assert_eq!(normalize_rel_path(""), "");
    }

    #[test]
    fn normalize_rel_paths_processes_a_mixed_slice() {
        let input = vec![
            "src/a.rs".to_string(),
            "src\\b.rs".to_string(),
            "lib\\nested\\c.rs".to_string(),
        ];
        let out = normalize_rel_paths(&input);
        assert_eq!(out, vec!["src/a.rs", "src/b.rs", "lib/nested/c.rs"]);
    }
}
