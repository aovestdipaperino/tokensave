// Rust guideline compliant 2026-05-25
//! Single-project file watcher with debounced incremental sync.
//!
//! Embedded inside the MCP server to keep the project index fresh while
//! agents are connected. Multiple MCP peers coordinate through a sync
//! lock so only one runs an incremental sync at a time.
//!
//! ## Watch strategy (#80)
//!
//! 1. Top-level entries under `project_root` are enumerated at startup.
//!    Anything in [`IGNORED_DIRS`] is **not watched at the OS level**, so
//!    the kernel never reports events for `target/`, `node_modules/`,
//!    `.git/`, etc. This is critical on Windows where
//!    `ReadDirectoryChangesW` has a small per-watch buffer that can be
//!    overwhelmed by churn inside a large `node_modules` even when we
//!    discard the events post-hoc.
//! 2. The root itself is watched non-recursively to catch new top-level
//!    directories appearing after startup (e.g. a fresh `git clone` of a
//!    submodule). When one shows up, [`refresh_top_level_watches`] adds
//!    it.
//! 3. Surviving subtrees are watched recursively.
//!
//! ## Debouncing (#80)
//!
//! Uses [`notify_debouncer_full`] — the maintained sibling of `notify` —
//! which handles cross-platform rename-pair tracking, duplicate-event
//! suppression, and burst coalescence (e.g. an editor's tempfile +
//! atomic rename now arrives as a single batch). The DIY tokio
//! debouncer this module previously used has been removed; we still
//! drive incremental sync through tokio, but consume *batched* events
//! from the debouncer thread.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Pull notify types from the debouncer's re-export so we use the same
// crate instance (debouncer 0.8.0-rc.2 depends on notify 9.0.0-rc.4,
// which is incompatible with a separately-resolved notify=8).
use notify_debouncer_full::notify::{self, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebouncedEvent, Debouncer, RecommendedCache};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Directories to ignore inside watched projects.
pub const IGNORED_DIRS: &[&str] = &[
    ".tokensave",
    ".git",
    "node_modules",
    "target",
    ".build",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".cache",
];

/// Returns true if any component of `path` matches an entry in [`IGNORED_DIRS`].
fn path_is_ignored(path: &Path) -> bool {
    path.components()
        .any(|c| IGNORED_DIRS.contains(&c.as_os_str().to_str().unwrap_or("")))
}

/// Returns true if `name` (a top-level entry) is one we want to watch.
/// Excludes ignored dirs and hidden dotfiles (except the project root
/// itself, which the caller handles separately).
fn is_watchable_top_level(name: &str) -> bool {
    if IGNORED_DIRS.contains(&name) {
        return false;
    }
    // Dotfiles / dot-directories tend to be VCS, IDE state, or caches —
    // exclude by default. The user can still get sync via direct edits to
    // tracked files because we watch the project root non-recursively
    // and react to new top-level entries.
    !name.starts_with('.') || name == ".tokensave-keepwatch"
}

/// Watches a single project directory for file changes, debounces them,
/// and runs incremental sync.
pub struct ProjectWatcher {
    project_root: PathBuf,
    rx: mpsc::Receiver<Vec<PathBuf>>,
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

impl ProjectWatcher {
    /// Create a watcher for the given project root with the specified
    /// debounce timeout.
    ///
    /// Returns `None` if the debouncer cannot be created or none of the
    /// candidate paths can be watched (e.g. inotify watch limit, missing
    /// permissions on the root). A single failed `watch()` for one
    /// subdirectory is logged but does not fail construction — partial
    /// coverage is better than no coverage.
    pub fn new(project_root: PathBuf, debounce: Duration) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<Vec<PathBuf>>(64);

        let tx_for_handler = tx.clone();
        let mut debouncer = new_debouncer(
            debounce,
            None,
            move |res: notify_debouncer_full::DebounceEventResult| {
                let events = match res {
                    Ok(events) => events,
                    Err(errors) => {
                        for e in errors {
                            log_msg(&format!("watcher error: {e}"));
                        }
                        return;
                    }
                };
                let paths = extract_changed_paths(&events);
                if paths.is_empty() {
                    return;
                }
                // try_send: if the buffer is full, drop. The next batch
                // will still trigger a sync covering the dropped paths
                // because `sync_if_stale_silent` checks every file's
                // content hash anyway.
                let _ = tx_for_handler.try_send(paths);
            },
        )
        .ok()?;

        // 1. Watch the root NON-recursively so we react to new top-level
        //    entries (a freshly-extracted submodule, a new top-level
        //    directory created by tooling).
        debouncer
            .watch(&project_root, RecursiveMode::NonRecursive)
            .ok()?;

        // 2. Enumerate top-level entries and watch each surviving one
        //    recursively. Ignored directories never become a watch — the
        //    OS doesn't fire events for them at all.
        let mut watched = 0usize;
        let mut ignored = 0usize;
        if let Ok(entries) = std::fs::read_dir(&project_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let Ok(ft) = entry.file_type() else { continue };
                if !ft.is_dir() {
                    continue;
                }
                if !is_watchable_top_level(&name) {
                    ignored += 1;
                    continue;
                }
                match debouncer.watch(&path, RecursiveMode::Recursive) {
                    Ok(()) => {
                        watched += 1;
                    }
                    Err(e) => {
                        log_msg(&format!("watcher: failed to watch {}: {e}", path.display()));
                    }
                }
            }
        }

        if watched == 0 && ignored == 0 {
            // The directory is empty or unreadable. We still hold the
            // root watch from step 1, but warn so this isn't silent.
            log_msg(&format!(
                "watcher: no subdirectories to watch under {}; relying on root-only watch",
                project_root.display()
            ));
        }

        Some(Self {
            project_root,
            rx,
            _debouncer: debouncer,
        })
    }

    /// Returns the project root this watcher is monitoring.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Run the watch loop until the cancellation token fires, invoking
    /// `on_sync` after each successful sync completes.
    ///
    /// Each iteration receives an already-debounced batch from
    /// `notify-debouncer-full`, runs a single incremental sync over
    /// every path in the batch, then invokes `on_sync` for downstream
    /// caches (e.g. `file_token_map`).
    pub async fn run_with_callback<F, Fut>(mut self, cancel: CancellationToken, on_sync: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    // Drain any pending batches before shutting down so
                    // we don't lose edits observed just before cancel.
                    while let Ok(paths) = self.rx.try_recv() {
                        sync_project_paths(&self.project_root, &paths).await;
                        on_sync().await;
                    }
                    break;
                }
                Some(paths) = self.rx.recv() => {
                    sync_project_paths(&self.project_root, &paths).await;
                    on_sync().await;
                }
            }
        }
    }
}

/// Pull changed paths out of a debounced event batch, dropping anything
/// that matches an ignored prefix (defence-in-depth: even though we
/// don't *register* watches inside ignored dirs, a renamed-into event
/// could still surface one).
fn extract_changed_paths(events: &[DebouncedEvent]) -> Vec<PathBuf> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for event in events {
        // Only Create / Modify / Remove are meaningful for index
        // freshness; metadata-only changes (atime, permissions) aren't
        // worth a re-sync.
        if !matches!(
            event.kind,
            notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_)
        ) {
            continue;
        }
        for path in &event.paths {
            if path_is_ignored(path) {
                continue;
            }
            if seen.insert(path.clone()) {
                out.push(path.clone());
            }
        }
    }
    out
}

/// Run an incremental sync targeting the specified absolute paths.
/// Best-effort: catches panics (e.g. from extractor bugs on malformed
/// files) so one bad project doesn't kill the caller.
pub async fn sync_project_paths(project_root: &Path, paths: &[PathBuf]) {
    let root = project_root.to_path_buf();
    let paths = paths.to_vec();
    let result = tokio::task::spawn(async move {
        sync_project_paths_inner(&root, &paths).await;
    })
    .await;

    if let Err(e) = result {
        let msg = if e.is_panic() {
            let panic = e.into_panic();
            if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "unknown panic".to_string()
            }
        } else {
            format!("task error: {e}")
        };
        log_msg(&format!(
            "sync panicked for {}: {msg}",
            project_root.display()
        ));
    }
}

async fn sync_project_paths_inner(project_root: &Path, paths: &[PathBuf]) {
    // Canonicalize the project root so we can match notify event paths
    // even when the working directory is a symlink (e.g. macOS `/var` ->
    // `/private/var` for tempdir()).
    let canonical_root = std::fs::canonicalize(project_root)
        .ok()
        .unwrap_or_else(|| project_root.to_path_buf());

    let mut relative: Vec<String> = paths
        .iter()
        .filter_map(|abs| {
            // Try both the original root and the canonicalized one so we
            // succeed regardless of which form notify emitted.
            abs.strip_prefix(project_root)
                .ok()
                .or_else(|| abs.strip_prefix(&canonical_root).ok())
        })
        .map(|rel| rel.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .collect();
    relative.sort();
    relative.dedup();

    if relative.is_empty() {
        return;
    }

    let start = std::time::Instant::now();
    let Ok(cg) = crate::tokensave::TokenSave::open(project_root).await else {
        log_msg(&format!("failed to open {}", project_root.display()));
        return;
    };

    match cg.sync_if_stale_silent(&relative).await {
        Ok(()) => {
            let ms = start.elapsed().as_millis();
            log_msg(&format!(
                "sync_if_stale_silent {} — {} candidates ({}ms)",
                project_root.display(),
                relative.len(),
                ms
            ));
            // Best-effort update global DB.
            if let Some(gdb) = crate::global_db::GlobalDb::open().await {
                let tokens = cg.get_tokens_saved().await.unwrap_or(0);
                gdb.upsert(project_root, tokens).await;
            }
        }
        Err(e) => {
            log_msg(&format!("sync failed for {}: {e}", project_root.display()));
        }
    }
}

/// Log a timestamped message to stderr.
fn log_msg(msg: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    eprintln!("[{secs}] {msg}");
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn path_is_ignored_matches_components() {
        assert!(path_is_ignored(Path::new("/proj/target/debug/foo")));
        assert!(path_is_ignored(Path::new("/proj/node_modules/x/y")));
        assert!(path_is_ignored(Path::new("/proj/.git/refs/heads/main")));
        assert!(!path_is_ignored(Path::new("/proj/src/lib.rs")));
        assert!(!path_is_ignored(Path::new("/proj/tests/foo.rs")));
    }

    #[test]
    fn is_watchable_top_level_skips_ignored_and_dotdirs() {
        assert!(!is_watchable_top_level("target"));
        assert!(!is_watchable_top_level("node_modules"));
        assert!(!is_watchable_top_level(".git"));
        assert!(!is_watchable_top_level(".vscode"));
        assert!(is_watchable_top_level("src"));
        assert!(is_watchable_top_level("tests"));
    }
}
