// Rust guideline compliant 2025-10-17
//! Single-project file watcher with debounced incremental sync.
//!
//! Extracted from the daemon's per-project logic so it can be reused
//! from both [`crate::daemon`] (multi-project) and the MCP server
//! (single-project, when the daemon isn't running).

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// Directories to ignore inside watched projects.
pub const IGNORED_DIRS: &[&str] = &[
    ".tokensave", ".git", "node_modules", "target", ".build",
    "__pycache__", ".next", "dist", "build", ".cache",
];

/// Watches a single project directory for file changes, debounces them,
/// and runs incremental sync.
pub struct ProjectWatcher {
    project_root: PathBuf,
    debounce: Duration,
    rx: mpsc::Receiver<()>,
    _watcher: RecommendedWatcher,
}

impl ProjectWatcher {
    /// Create a watcher for the given project root with the specified debounce.
    ///
    /// Returns `None` if the notify watcher cannot be created or the directory
    /// cannot be watched.
    pub fn new(project_root: PathBuf, debounce: Duration) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<()>(64);

        let mut watcher = notify::recommended_watcher(
            move |res: std::result::Result<Event, notify::Error>| {
                let Ok(event) = res else { return };
                if !matches!(
                    event.kind,
                    notify::EventKind::Create(_)
                        | notify::EventKind::Modify(_)
                        | notify::EventKind::Remove(_)
                ) {
                    return;
                }
                let dominated_by_ignored = event.paths.iter().all(|p| {
                    p.components().any(|c| {
                        IGNORED_DIRS.contains(&c.as_os_str().to_str().unwrap_or(""))
                    })
                });
                if dominated_by_ignored {
                    return;
                }
                let _ = tx.try_send(());
            },
        )
        .ok()?;

        watcher.watch(&project_root, RecursiveMode::Recursive).ok()?;

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

    /// Run the watch loop until the cancellation token fires.
    ///
    /// Flushes any pending sync before returning so that changes observed
    /// shortly before shutdown are not lost.
    pub async fn run(mut self, cancel: CancellationToken) {
        let mut deadline: Option<Instant> = None;

        loop {
            let sleep_dur = match deadline {
                Some(d) => d.saturating_duration_since(Instant::now()),
                None => Duration::from_secs(3600),
            };

            tokio::select! {
                _ = cancel.cancelled() => {
                    if deadline.is_some() {
                        sync_project(&self.project_root).await;
                    }
                    break;
                }
                Some(()) = self.rx.recv() => {
                    deadline = Some(Instant::now() + self.debounce);
                }
                _ = tokio::time::sleep(sleep_dur), if deadline.is_some() => {
                    deadline = None;
                    sync_project(&self.project_root).await;
                }
            }
        }
    }
}

/// Run an incremental sync on a single project. Best-effort.
///
/// Catches panics (e.g. from extractor bugs on malformed files) so one
/// bad project doesn't kill the caller.
pub async fn sync_project(project_root: &Path) {
    let root = project_root.to_path_buf();
    let result = tokio::task::spawn(async move {
        sync_project_inner(&root).await;
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

async fn sync_project_inner(project_root: &Path) {
    let start = std::time::Instant::now();
    let Ok(cg) = crate::tokensave::TokenSave::open(project_root).await else {
        log_msg(&format!("failed to open {}", project_root.display()));
        return;
    };
    match cg.sync().await {
        Ok(result) => {
            let ms = start.elapsed().as_millis();
            if result.files_added > 0 || result.files_modified > 0 || result.files_removed > 0 {
                log_msg(&format!(
                    "synced {} — {} added, {} modified, {} removed ({}ms)",
                    project_root.display(),
                    result.files_added,
                    result.files_modified,
                    result.files_removed,
                    ms
                ));
            }
            // Best-effort update global DB
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
