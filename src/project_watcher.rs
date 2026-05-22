// Rust guideline compliant 2025-10-17
//! Single-project file watcher with debounced incremental sync.
//!
//! Embedded inside the MCP server to keep the project index fresh while
//! agents are connected. Multiple MCP peers coordinate through a sync
//! lock so only one runs an incremental sync at a time.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::Instant;
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

/// Watches a single project directory for file changes, debounces them,
/// and runs incremental sync.
pub struct ProjectWatcher {
    project_root: PathBuf,
    debounce: Duration,
    rx: mpsc::Receiver<Vec<PathBuf>>,
    _watcher: RecommendedWatcher,
}

impl ProjectWatcher {
    /// Create a watcher for the given project root with the specified debounce.
    ///
    /// Returns `None` if the notify watcher cannot be created or the directory
    /// cannot be watched.
    pub fn new(project_root: PathBuf, debounce: Duration) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<Vec<PathBuf>>(64);

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
                let Ok(event) = res else { return };
                if !matches!(
                    event.kind,
                    notify::EventKind::Create(_)
                        | notify::EventKind::Modify(_)
                        | notify::EventKind::Remove(_)
                ) {
                    return;
                }
                let kept: Vec<PathBuf> = event
                    .paths
                    .into_iter()
                    .filter(|p| !path_is_ignored(p))
                    .collect();
                if kept.is_empty() {
                    return;
                }
                let _ = tx.try_send(kept);
            })
            .ok()?;

        watcher
            .watch(&project_root, RecursiveMode::Recursive)
            .ok()?;

        Some(Self {
            project_root,
            debounce,
            rx,
            _watcher: watcher,
        })
    }

    /// Returns the project root this watcher is monitoring.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Run the watch loop until the cancellation token fires, invoking
    /// `on_sync` after each successful sync completes.
    ///
    /// Flushes any pending sync before returning so that changes observed
    /// shortly before shutdown are not lost. Used by the embedded MCP
    /// watcher to refresh in-memory caches (e.g. `file_token_map`) after
    /// each background sync.
    pub async fn run_with_callback<F, Fut>(mut self, cancel: CancellationToken, on_sync: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut deadline: Option<Instant> = None;
        let mut pending: HashSet<PathBuf> = HashSet::new();

        loop {
            let sleep_dur = match deadline {
                Some(d) => d.saturating_duration_since(Instant::now()),
                None => Duration::from_hours(1),
            };

            tokio::select! {
                () = cancel.cancelled() => {
                    if deadline.is_some() {
                        let paths: Vec<PathBuf> = pending.drain().collect();
                        sync_project_paths(&self.project_root, &paths).await;
                        on_sync().await;
                    }
                    break;
                }
                Some(paths) = self.rx.recv() => {
                    pending.extend(paths);
                    deadline = Some(Instant::now() + self.debounce);
                }
                () = tokio::time::sleep(sleep_dur), if deadline.is_some() => {
                    deadline = None;
                    let paths: Vec<PathBuf> = pending.drain().collect();
                    sync_project_paths(&self.project_root, &paths).await;
                    on_sync().await;
                }
            }
        }
    }
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
