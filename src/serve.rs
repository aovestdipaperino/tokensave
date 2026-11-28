mod constants;

use std::fs::OpenOptions;
use std::path::Path;
use tokensave::auto_init::{self, Decision};
use tokensave::errors::TokenSaveError;
use tokensave::tokensave::TokenSave;

use self::constants::{AUTO_INIT_LOCK_FILENAME, AUTO_INIT_LOCK_POLL, AUTO_INIT_LOCK_TIMEOUT};

/// Opens an existing project, or tells the user to run `tokensave init` first.
pub async fn ensure_initialized(project_path: &Path) -> tokensave::errors::Result<TokenSave> {
    if TokenSave::is_initialized(project_path) {
        return TokenSave::open(project_path).await;
    }
    Err(tokensave::errors::TokenSaveError::Config {
        message: format!(
            "no TokenSave index found at '{}' — run 'tokensave init' first",
            project_path.display()
        ),
    })
}

/// Initializes and indexes the git repository at `cwd` when `auto_init`
/// allows it (#876), then opens it for serving. Concurrent implicit MCP
/// starts wait for the first initialization of the same checkout.
///
/// The index is built before the server starts answering, so the first tool
/// call sees a complete graph. [`tokensave::auto_init::decide`] caps the
/// repository at `max_auto_sync_files`, which bounds that wait. Progress goes
/// nowhere: stdout carries JSON-RPC, and a spinner would only flood the
/// host's stderr log.
pub async fn open_implicit_project(cwd: &Path) -> tokensave::errors::Result<TokenSave> {
    let state = tokensave::config::get_tokensave_dir(cwd);
    let lock_path = state.join(AUTO_INIT_LOCK_FILENAME);
    // Read the decision before testing the lock path. A peer that publishes
    // the database between those checks has already created its lock file.
    let decision = auto_init::decide(cwd);
    let may_initialize = matches!(decision, Decision::Init(_));
    if !may_initialize && !lock_path.try_exists().map_err(auto_init_lock_error)? {
        if let Decision::Skip(skip) = decision {
            if let Some(hint) = skip.hint(cwd) {
                eprintln!("[tokensave] {hint}");
            }
        }
        return ensure_initialized(cwd).await;
    }

    if may_initialize {
        std::fs::create_dir_all(&state).map_err(auto_init_lock_error)?;
    }
    // Keep the inode in place so every opener locks the same file. The OS
    // releases the lock when the handle closes, including on process exit.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(may_initialize)
        .truncate(false)
        .open(&lock_path)
        .map_err(auto_init_lock_error)?;
    let deadline = tokio::time::Instant::now() + AUTO_INIT_LOCK_TIMEOUT;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(AUTO_INIT_LOCK_POLL).await;
            }
            Err(error) => return Err(auto_init_lock_error(error)),
        }
    }
    // Another pane may have initialized the checkout while we waited.
    if let Decision::Init(root) = decision {
        if matches!(
            tokensave::config::discover_project(&root),
            tokensave::config::ProjectDiscovery::Uninitialized { .. }
        ) {
            return initialize_for_serve(&root).await;
        }
    }
    ensure_initialized(cwd).await
}

fn auto_init_lock_error(error: impl std::fmt::Display) -> TokenSaveError {
    TokenSaveError::SyncLock {
        message: format!("could not acquire the MCP initialization lock: {error}"),
    }
}

async fn initialize_for_serve(root: &Path) -> tokensave::errors::Result<TokenSave> {
    eprintln!(
        "[tokensave] no index at '{}'; initializing it (auto_init)",
        root.display()
    );
    let indexed = async {
        let pin = crate::commands::prepare_initial_cpu_usage(root, None)?;
        let cg = TokenSave::init_with_cpu_usage(root, pin).await?;
        // `init` applies the same default when stdin is not a terminal. State
        // relocated out of the tree (#919) needs no exclusion.
        if tokensave::config::project_state_is_in_repo(root)
            && !tokensave::config::is_in_gitignore(root)
        {
            tokensave::config::add_to_git_info_exclude(root);
        }
        let result = cg.index_all().await?;
        crate::global::update_global_db(&cg).await;
        tokensave::errors::Result::Ok(result)
    }
    .await;
    let result = indexed.inspect_err(|e| eprintln!("[tokensave] auto_init failed: {e}"))?;
    eprintln!(
        "[tokensave] auto_init indexed {} files ({} nodes) in {}ms",
        result.file_count, result.node_count, result.duration_ms
    );
    TokenSave::open(root).await
}

/// Fallback for `serve`: when CWD-based discovery fails inside a linked git
/// worktree that sits outside its main checkout, discover the project from
/// the main checkout's equivalent path instead. A worktree nested inside the
/// main checkout already reaches that index by walking up; this gives a
/// sibling worktree the same answer, and the borrowed-worktree notice (#312)
/// then tells the agent which tree it is reading.
pub fn resolve_serve_from_main_worktree() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let counterpart = tokensave::worktree::main_checkout_counterpart(&cwd)?;
    tokensave::config::discover_project_root(&counterpart)
}

/// A project registered in the global database whose index still exists.
pub struct RegisteredProject {
    /// The path as the global DB recorded it.
    path: std::path::PathBuf,
    /// The path canonicalized once at load, `None` if that failed.
    canonical: Option<std::path::PathBuf>,
}

impl RegisteredProject {
    /// The path as the global DB recorded it.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Loads the registered projects whose index still exists, probing each one's
/// directory exactly once, all of them concurrently.
///
/// `serve` consults this list up to three times before it answers
/// `initialize` when no project resolves (global-DB fallback, MCP roots, the
/// project list a server without a default project names). Each consultation
/// used to probe every registered path again, an `exists` and up to two
/// `canonicalize` calls apiece, one path after another. A path on a slow or
/// unreachable volume (a disconnected network share, a WSL path that boots
/// its VM, a sleeping external drive) costs seconds per probe, so startup
/// outside a project grew with the number of such paths times the probes,
/// while startup inside one touched none of them (#606). Probing once, in
/// parallel, bounds that at the slowest single path.
pub async fn load_registered_projects() -> Vec<RegisteredProject> {
    let Some(gdb) = tokensave::global_db::GlobalDb::open().await else {
        return Vec::new();
    };
    let mut paths = gdb.list_project_paths().await;
    drop(gdb);
    paths.sort();
    paths.dedup();
    let probes: Vec<_> = paths
        .into_iter()
        .map(|path| tokio::task::spawn_blocking(move || probe_registered_project(path)))
        .collect();
    let mut registered = Vec::with_capacity(probes.len());
    for probe in probes {
        if let Ok(Some(project)) = probe.await {
            registered.push(project);
        }
    }
    registered
}

/// Probes one registered path: `None` once its index is gone from disk, or
/// when the path is the home directory and its "index" is a stray database in
/// the global state directory (#1152). Such a registration would otherwise be
/// an ancestor of every working directory and win the global-DB fallback.
fn probe_registered_project(path: String) -> Option<RegisteredProject> {
    let path = std::path::PathBuf::from(path);
    if !tokensave::config::holds_project_index(&path) {
        return None;
    }
    let canonical = path.canonicalize().ok();
    Some(RegisteredProject { path, canonical })
}

/// Fallback for `serve`: when CWD-based discovery fails, check the global DB
/// for registered projects. When multiple projects exist, pick the best match
/// against cwd: prefer a project that is an ancestor of cwd (cwd is inside the
/// project), then a project that is a descendant of cwd (project is under cwd).
/// Among multiple matches, the deepest (most specific) path wins.
pub fn resolve_serve_from_global_db(
    registered: &[RegisteredProject],
) -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    let selected = select_registered_project(registered, &cwd);
    if selected.is_none() && registered.len() > 1 {
        // No cwd-based match — report the ambiguity.
        eprintln!("Multiple tokensave projects found — pass -p <path> to select one:");
        for project in registered {
            eprintln!("  {}", project.path.display());
        }
    }
    selected
}

/// The registered project `serve` should use from `cwd` (already
/// canonicalized), if one is unambiguous. See
/// [`resolve_serve_from_global_db`] for the rule.
fn select_registered_project(
    registered: &[RegisteredProject],
    cwd: &std::path::Path,
) -> Option<std::path::PathBuf> {
    if let [only] = registered {
        return Some(only.path.clone());
    }

    // Priority 1: cwd is inside a project (project is ancestor of cwd).
    // Pick the deepest ancestor (most specific match).
    let ancestor = registered
        .iter()
        .filter_map(|p| {
            let canonical = p.canonical.as_ref()?;
            cwd.starts_with(canonical)
                .then(|| (canonical.components().count(), &p.path))
        })
        .max_by_key(|(depth, _)| *depth);
    if let Some((_, best)) = ancestor {
        return Some(best.clone());
    }

    // Priority 2: a project is under cwd (cwd is ancestor of project).
    // Pick the shallowest descendant (closest child).
    registered
        .iter()
        .filter_map(|p| {
            let canonical = p.canonical.as_ref()?;
            canonical
                .starts_with(cwd)
                .then(|| (canonical.components().count(), &p.path))
        })
        .min_by_key(|(depth, _)| *depth)
        .map(|(_, best)| best.clone())
}

/// Last-resort fallback for `serve`: peek at the first stdin line to read the
/// MCP `initialize` request's `roots` array.  If a root matches a registered
/// project, return its path.  The raw line is stored in `out` so the caller
/// can replay it into the MCP transport (the server still needs to see it).
pub async fn resolve_serve_from_mcp_roots(
    out: &mut Option<String>,
    registered: &[RegisteredProject],
) -> Option<std::path::PathBuf> {
    use tokio::io::AsyncBufReadExt;
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line = String::new();
    // Read the first non-empty line (should be the `initialize` request).
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => return None, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    *out = Some(line.trim().to_string());

    let parsed: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let roots = parsed.pointer("/params/roots").and_then(|v| v.as_array())?;

    // Try each root URI — first match wins.
    for root in roots {
        let uri = root.get("uri").and_then(|v| v.as_str()).unwrap_or_default();
        let Some(root_path) = tokensave::mcp::roots::root_uri_to_path(uri) else {
            tracing::debug!("skipping unsupported root URI: {}", uri);
            continue;
        };
        // Exact match: the root IS a registered project.
        if let Some(hit) = find_registered_project(registered, &root_path) {
            tracing::info!("serving from MCP root: {}", hit.display());
            return Some(hit);
        }
        // Walk up from the root to find the nearest enclosing project.
        if let Some(discovered) = tokensave::config::discover_project_root(&root_path) {
            tracing::info!(
                "serving from discovered project under MCP root: {}",
                discovered.display()
            );
            return Some(discovered);
        }
    }
    None
}

/// Finds the registered project that names the same directory as `root_path`.
///
/// Compares canonically as well as literally so that `D:\Dev\app` and
/// `D:/Dev/app` — the same directory spelled two ways — are recognized as one.
/// Canonicalization failure (e.g. the path does not exist) only disables the
/// canonical comparison; the literal one still applies. Returns the registered
/// spelling, not the root URI's, so downstream path comparisons keep working
/// against what the global DB recorded.
fn find_registered_project(
    registered: &[RegisteredProject],
    root_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let canonical_root = root_path.canonicalize().ok();
    registered
        .iter()
        .find(|rp| {
            rp.path == root_path
                || match (rp.canonical.as_ref(), canonical_root.as_ref()) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                }
        })
        .map(|rp| rp.path.clone())
}

#[cfg(test)]
mod tests {
    use super::find_registered_project;
    use std::path::PathBuf;

    #[test]
    fn registered_match_literal_even_when_the_path_does_not_exist() {
        // Canonicalization fails on a nonexistent path; the literal comparison
        // must still hold on its own.
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("missing");
        assert!(!missing.exists());

        let registered = vec![missing.to_string_lossy().into_owned()];
        assert_eq!(
            find_registered_project(&prep(registered), &missing),
            Some(missing)
        );
    }

    #[test]
    fn registered_match_via_canonicalization_returns_the_registered_spelling() {
        // `project/../project` names the same existing directory as `project`
        // without stepping outside the temp dir; only canonicalization can
        // equate the two spellings.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let registered = vec![project.to_string_lossy().into_owned()];
        let alias = project.join("..").join("project");
        assert_ne!(alias.as_path(), project.as_path());
        assert_eq!(
            find_registered_project(&prep(registered), &alias),
            Some(project.clone())
        );
    }

    fn prep(registered: Vec<String>) -> Vec<super::RegisteredProject> {
        registered
            .into_iter()
            .map(|p| {
                let pb = PathBuf::from(p);
                let canonical = pb.canonicalize().ok();
                super::RegisteredProject {
                    path: pb,
                    canonical,
                }
            })
            .collect()
    }

    #[test]
    fn registered_mismatch_between_two_existing_directories() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let registered = vec![a.to_string_lossy().into_owned()];
        assert_eq!(find_registered_project(&prep(registered), &b), None);
    }

    /// A project directory with an index file, as `probe` sees one.
    fn indexed(dir: &std::path::Path) -> String {
        std::fs::create_dir_all(tokensave::config::get_tokensave_dir(dir)).unwrap();
        std::fs::write(
            tokensave::config::get_tokensave_dir(dir).join("tokensave.db"),
            b"",
        )
        .unwrap();
        dir.to_string_lossy().into_owned()
    }

    #[test]
    fn probe_drops_a_registered_project_whose_index_is_gone() {
        let tmp = tempfile::TempDir::new().unwrap();
        let kept = indexed(&tmp.path().join("kept"));
        let gone = tmp.path().join("gone").to_string_lossy().into_owned();
        assert!(super::probe_registered_project(kept).is_some());
        assert!(super::probe_registered_project(gone).is_none());
    }

    #[test]
    fn selection_prefers_the_deepest_project_containing_cwd() {
        let tmp = tempfile::TempDir::new().unwrap();
        let outer = tmp.path().join("outer");
        let inner = outer.join("inner");
        let registered = prep(vec![indexed(&outer), indexed(&inner)]);
        let cwd = inner.join("src");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd = cwd.canonicalize().unwrap();
        assert_eq!(
            super::select_registered_project(&registered, &cwd),
            Some(inner)
        );
    }

    #[test]
    fn selection_falls_back_to_the_shallowest_project_under_cwd() {
        let tmp = tempfile::TempDir::new().unwrap();
        let near = tmp.path().join("near");
        let far = tmp.path().join("a").join("b").join("far");
        let registered = prep(vec![indexed(&far), indexed(&near)]);
        let cwd = tmp.path().canonicalize().unwrap();
        assert_eq!(
            super::select_registered_project(&registered, &cwd),
            Some(near)
        );
    }

    #[test]
    fn selection_is_ambiguous_for_unrelated_projects_and_trivial_for_one() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let outside = outside.canonicalize().unwrap();
        let both = prep(vec![indexed(&a), indexed(&b)]);
        assert_eq!(super::select_registered_project(&both, &outside), None);
        let one = prep(vec![a.to_string_lossy().into_owned()]);
        assert_eq!(super::select_registered_project(&one, &outside), Some(a));
    }
}
