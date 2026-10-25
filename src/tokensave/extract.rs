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
/// Successful extractions and per-file failures from either execution path.
type ExtractBatch = (Vec<ExtractTuple>, Vec<(String, String)>);

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
    cpu_usage: crate::cpu::CpuUsage,
    memory_usage: crate::memory::MemoryUsage,
) -> Result<ExtractBatch> {
    if files.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // One fresh snapshot per extraction operation, not per file or idle tick.
    crate::cancel::check("extraction")?;
    let available = if memory_usage.workers(cpu_usage.workers()).min(files.len()) > 1 {
        crate::memory::available_memory_bytes()
    } else {
        None
    };
    extract_files_with_headroom(
        project_root,
        registry,
        files,
        cpu_usage,
        memory_usage,
        available,
    )
}

fn extract_files_with_headroom(
    project_root: &Path,
    registry: &crate::extraction::LanguageRegistry,
    files: Vec<String>,
    cpu_usage: crate::cpu::CpuUsage,
    memory_usage: crate::memory::MemoryUsage,
    available: Option<u64>,
) -> Result<ExtractBatch> {
    if files.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let requested = memory_usage.workers(cpu_usage.workers()).min(files.len());
    let parser_limit = crate::memory::workers_for_headroom(requested, available);
    if parser_limit < requested {
        eprintln!("[tokensave] limited RAM headroom: reducing extraction parsers from {requested} to {parser_limit}");
    }
    if should_use_subprocess() {
        let workers = worker_count(files.len(), parser_limit);
        let timeout = std::time::Duration::from_secs(
            crate::user_config::UserConfig::load().extraction_timeout_secs,
        );
        match crate::extraction_worker::WorkerPool::new(workers, project_root.to_path_buf()) {
            Ok(pool) => {
                let outcome = pool.extract_files(files, |_, _, _| {}, timeout);
                return Ok((outcome.results, outcome.skipped));
            }
            Err(e) => eprintln!(
                "[tokensave] could not spawn extraction worker pool ({e}), \
                 falling back to in-process extraction"
            ),
        }
    }
    if parser_limit == requested && memory_usage == crate::memory::MemoryUsage::Performance {
        return cpu_usage.run(|| {
            (
                extract_files_in_process(project_root, registry, &files),
                Vec::new(),
            )
        });
    }
    cpu_usage.run(|| {
        // Each parallel slice obeys both the profile and current headroom ceiling.
        // Reuse the CPU pool without allocating another persistent thread pool.
        let results = files
            .chunks(parser_limit)
            .flat_map(|batch| extract_files_in_process(project_root, registry, batch))
            .collect();
        (results, Vec::new())
    })
}

pub(crate) fn extract_files_in_process(
    project_root: &Path,
    registry: &crate::extraction::LanguageRegistry,
    files: &[String],
) -> Vec<ExtractTuple> {
    files
        .par_iter()
        .filter_map(|file_path| {
            // Cheapest place to observe a shutdown: the parallel extraction
            // pass is the long phase of a large sync, and a per-file atomic
            // load is nothing against reading and parsing a source file
            // (#450). Files already extracted are simply dropped by the
            // caller's cancellation check — the sync leaves through `Err`
            // with the dirty sentinel still in place.
            if crate::cancel::is_requested() {
                return None;
            }
            let abs_path = project_root.join(file_path);
            let source = sync::read_source_file(&abs_path).ok()?;
            let extractor = crate::project_manifest::resolve_extractor_for_source(
                registry,
                project_root,
                file_path,
                &source,
            )?;
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
/// Number of subprocess extraction workers to spawn.
///
/// CPU and memory profiles set the ceiling. `TOKENSAVE_WORKERS=N` can further
/// reduce subprocess concurrency (#183); pending files cap the pool size.
fn worker_count(file_count: usize, parser_limit: usize) -> usize {
    clamp_workers(
        std::env::var("TOKENSAVE_WORKERS").ok().as_deref(),
        parser_limit,
        file_count,
    )
}

/// Pure resolution of the worker count from a raw `TOKENSAVE_WORKERS` value.
///
/// `None` (unset) uses available cores, capped by pending files. A positive
/// integer further limits concurrency. Invalid values use the same default.
fn clamp_workers(raw: Option<&str>, cores: usize, file_count: usize) -> usize {
    let cores = cores.min(file_count);
    match raw {
        None => cores,
        Some(v) => match v.trim().parse::<usize>() {
            Ok(n) if n >= 1 => n.min(cores),
            _ => {
                eprintln!(
                    "[tokensave] ignoring invalid TOKENSAVE_WORKERS={v:?} \
                     (expected a positive integer); using {cores} workers"
                );
                cores
            }
        },
    }
}

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
mod worker_count_tests {
    use super::clamp_workers;

    #[test]
    fn empty_batch_returns_no_extractions_or_skips() -> crate::errors::Result<()> {
        let registry = crate::extraction::LanguageRegistry::new();
        let (results, skipped) = super::extract_files_isolated(
            std::path::Path::new("."),
            &registry,
            Vec::new(),
            crate::cpu::CpuUsage::Low,
            crate::memory::MemoryUsage::Low,
        )?;
        assert!(results.is_empty());
        assert!(skipped.is_empty());
        Ok(())
    }

    #[test]
    fn constrained_headroom_preserves_extraction_results(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::{cpu::CpuUsage, memory::MemoryUsage};
        let root = tempfile::TempDir::new()?;
        let files: Vec<String> = (0..7).map(|i| format!("m{i}.rs")).collect();
        for (i, path) in files.iter().enumerate() {
            std::fs::write(
                root.path().join(path),
                format!("fn helper{i}() {{}}\nfn caller{i}() {{ helper{i}(); }}\n"),
            )?;
        }
        let registry = crate::extraction::LanguageRegistry::new();
        let mut baseline = None;
        for profile in [
            MemoryUsage::Low,
            MemoryUsage::Balanced,
            MemoryUsage::Performance,
        ] {
            for available in [None, Some(0), Some(512 * 1024 * 1024), Some(u64::MAX)] {
                let (results, skipped) = super::extract_files_with_headroom(
                    root.path(),
                    &registry,
                    files.clone(),
                    CpuUsage::Performance,
                    profile,
                    available,
                )?;
                assert!(skipped.is_empty());
                assert_eq!(results.len(), files.len());
                let rows = results
                    .into_iter()
                    .map(|(path, mut result, hash, size, _mtime)| -> std::result::Result<_, serde_json::Error> {
                        assert!(!result.nodes.is_empty());
                        assert!(!result.unresolved_refs.is_empty());
                        result.duration_ms = 0;
                        for node in &mut result.nodes {
                            node.updated_at = 0;
                        }
                        Ok((path, serde_json::to_value(result)?, hash, size))
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if let Some(expected) = &baseline {
                    assert_eq!(&rows, expected);
                } else {
                    baseline = Some(rows);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn small_batches_never_spawn_idle_workers() {
        for raw in [None, Some("100"), Some("0"), Some("garbage"), Some("2")] {
            assert_eq!(clamp_workers(raw, 64, 0), 0);
            assert_eq!(clamp_workers(raw, 64, 1), 1);
            assert_eq!(clamp_workers(raw, 64, 2), 2);
        }
        assert_eq!(clamp_workers(Some("1"), 64, 2), 1);
        assert_eq!(clamp_workers(None, 4, 100), 4);
    }

    #[test]
    fn unset_uses_all_cores() {
        assert_eq!(clamp_workers(None, 8, 100), 8);
    }

    #[test]
    fn valid_value_is_honored() {
        assert_eq!(clamp_workers(Some("3"), 8, 100), 3);
        assert_eq!(clamp_workers(Some("  2 "), 8, 100), 2);
    }

    #[test]
    fn value_is_clamped_to_cores() {
        assert_eq!(clamp_workers(Some("100"), 8, 100), 8);
    }

    #[test]
    fn zero_and_garbage_fall_back_to_cores() {
        assert_eq!(clamp_workers(Some("0"), 8, 100), 8);
        assert_eq!(clamp_workers(Some("-1"), 8, 100), 8);
        assert_eq!(clamp_workers(Some("abc"), 8, 100), 8);
        assert_eq!(clamp_workers(Some(""), 8, 100), 8);
    }
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
