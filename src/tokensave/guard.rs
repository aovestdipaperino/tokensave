//! Dirty sentinel and cross-process sync lock.
use super::*;

// ---------------------------------------------------------------------------
// Dirty sentinel — detects interrupted sync/index operations
// ---------------------------------------------------------------------------

/// Creates a `.tokensave/dirty` sentinel file before a sync or index begins.
///
/// This file is intentionally NOT cleaned up by a Drop guard — it must be
/// removed explicitly by `clear_dirty_sentinel` after the operation succeeds.
/// If the process is killed (SIGKILL, OOM), the sentinel survives and signals
/// a potential crash on the next open.
pub(crate) fn write_dirty_sentinel(project_root: &Path) {
    let path = get_tokensave_dir(project_root).join("dirty");
    let _ = std::fs::write(
        &path,
        format!(
            "pid={}\ntime={}\nversion={}",
            std::process::id(),
            current_timestamp(),
            env!("CARGO_PKG_VERSION"),
        ),
    );
}

/// Removes the dirty sentinel after a successful sync/index.
pub(crate) fn clear_dirty_sentinel(project_root: &Path) {
    let path = get_tokensave_dir(project_root).join("dirty");
    let _ = std::fs::remove_file(path);
}

/// Returns `true` if the dirty sentinel exists (previous operation was
/// interrupted).
pub(crate) fn has_dirty_sentinel(project_root: &Path) -> bool {
    get_tokensave_dir(project_root).join("dirty").exists()
}

/// Returns `true` if a sync or full reindex currently holds the sync lock
/// (the lockfile exists and the PID recorded inside it is alive).
///
/// Used by read-only paths such as `tokensave_status` to recognise the
/// transient window in which `index_all` has cleared the graph tables but
/// not yet repopulated them, so an empty graph can be reported as "rebuild
/// in progress" instead of being presented as the true index state (#267).
pub(crate) fn sync_in_progress(project_root: &Path) -> bool {
    let lock_path = get_tokensave_dir(project_root).join("sync.lock");
    std::fs::read_to_string(&lock_path)
        .ok()
        .and_then(|contents| contents.trim().parse::<u32>().ok())
        .is_some_and(is_pid_alive)
}

/// Deletes the database and its WAL/SHM sidecars.
pub(crate) fn delete_db_files(db_path: &std::path::Path) {
    let _ = std::fs::remove_file(db_path);
    // WAL and SHM files use the same base name with different extensions
    let mut wal = db_path.to_path_buf();
    wal.set_extension("db-wal");
    let _ = std::fs::remove_file(&wal);
    wal.set_extension("db-shm");
    let _ = std::fs::remove_file(&wal);
}

/// Prints a user-facing warning about database corruption with a request to
/// report the issue.
pub(crate) fn print_corruption_warning() {
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("[tokensave] \x1b[33m⚠ database corruption detected — rebuilding index\x1b[0m");
    eprintln!("[tokensave]");
    eprintln!("[tokensave] This was likely caused by a crash or kill during indexing.");
    eprintln!("[tokensave] Please report this at:");
    eprintln!("[tokensave]   https://github.com/aovestdipaperino/tokensave/issues");
    eprintln!(
        "[tokensave]   Include: tokensave version (v{version}), OS, and what happened before the crash."
    );
    eprintln!("[tokensave]");
}

// ---------------------------------------------------------------------------
// Sync lock — prevents concurrent sync/index operations
// ---------------------------------------------------------------------------

/// RAII guard that holds the sync lockfile open. Removing the lockfile on drop
/// is best-effort; if it fails (e.g. permissions), the stale-PID check on the
/// next attempt will reclaim it.
///
/// Internal: exposed for integration tests; not part of the stable public API.
#[doc(hidden)]
pub struct SyncLockGuard {
    path: PathBuf,
}

impl Drop for SyncLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Try to acquire the sync lock for `project_root`.
///
/// Creates `.tokensave/sync.lock` containing the current PID. If the file
/// already exists and the PID inside is still alive, returns a `SyncLock`
/// error. Stale lockfiles (dead PID or unreadable content) are reclaimed
/// automatically.
///
/// Internal: exposed for integration tests; not part of the stable public API.
#[doc(hidden)]
pub fn try_acquire_sync_lock(project_root: &Path) -> Result<SyncLockGuard> {
    use std::io::Write;
    let lock_path = get_tokensave_dir(project_root).join("sync.lock");
    let pid = std::process::id();

    // Fast path: try atomic create.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut f) => {
            let _ = write!(f, "{pid}");
            return Ok(SyncLockGuard { path: lock_path });
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Fall through to stale-check below.
        }
        Err(e) => {
            return Err(TokenSaveError::SyncLock {
                message: format!("could not create lockfile: {e}"),
            });
        }
    }

    // Lockfile exists — check if the owning process is still alive.
    let contents = std::fs::read_to_string(&lock_path).unwrap_or_default();
    if let Ok(existing_pid) = contents.trim().parse::<u32>() {
        if is_pid_alive(existing_pid) {
            return Err(TokenSaveError::SyncLock {
                message: format!(
                    "another sync is already in progress (PID {existing_pid}). \
                     If this is stale, remove {}",
                    lock_path.display()
                ),
            });
        }
    }

    // Stale lock — reclaim it.
    let _ = std::fs::remove_file(&lock_path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|e| TokenSaveError::SyncLock {
            message: format!("could not reclaim lockfile: {e}"),
        })?;
    let _ = write!(f, "{pid}");
    Ok(SyncLockGuard { path: lock_path })
}

const BRANCH_OPERATION_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const BRANCH_OPERATION_LOCK_POLL: Duration = Duration::from_millis(50);

/// RAII guard for the branch-copy and metadata-update portion of branch add.
#[derive(Debug)]
#[doc(hidden)]
pub struct BranchOperationLock {
    path: PathBuf,
}

impl Drop for BranchOperationLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Acquire the project-wide branch-operation lock, waiting for another
/// branch add or auto-track operation to finish.
///
/// This lock is separate from `sync.lock`: branch operations coordinate their
/// copy and metadata steps, while sync contention remains an error from the
/// existing sync lock.
#[doc(hidden)]
pub async fn acquire_branch_operation_lock(tokensave_dir: &Path) -> Result<BranchOperationLock> {
    acquire_branch_operation_lock_with_timeout(tokensave_dir, BRANCH_OPERATION_LOCK_TIMEOUT).await
}

async fn acquire_branch_operation_lock_with_timeout(
    tokensave_dir: &Path,
    timeout: Duration,
) -> Result<BranchOperationLock> {
    let started = std::time::Instant::now();
    loop {
        if let Some(lock) = try_acquire_branch_operation_lock(tokensave_dir)? {
            return Ok(lock);
        }
        if started.elapsed() >= timeout {
            return Err(TokenSaveError::SyncLock {
                message: format!(
                    "branch add lock timed out after {} seconds",
                    timeout.as_secs()
                ),
            });
        }
        tokio::time::sleep(BRANCH_OPERATION_LOCK_POLL).await;
    }
}

fn try_acquire_branch_operation_lock(tokensave_dir: &Path) -> Result<Option<BranchOperationLock>> {
    use std::io::Write;
    let lock_path = tokensave_dir.join("branch-add.lock");
    let pid = std::process::id();

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut f) => {
            let _ = write!(f, "{pid}");
            return Ok(Some(BranchOperationLock { path: lock_path }));
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(TokenSaveError::SyncLock {
                message: format!("could not create branch add lockfile: {e}"),
            });
        }
    }

    let contents = std::fs::read_to_string(&lock_path).unwrap_or_default();
    if let Ok(existing_pid) = contents.trim().parse::<u32>() {
        if is_pid_alive(existing_pid) {
            return Ok(None);
        }
    }

    let _ = std::fs::remove_file(&lock_path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|e| TokenSaveError::SyncLock {
            message: format!("could not reclaim branch add lockfile: {e}"),
        })?;
    let _ = write!(f, "{pid}");
    Ok(Some(BranchOperationLock { path: lock_path }))
}

/// Returns `true` if a process with the given PID is currently running.
pub(crate) fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn branch_operation_lock_waits_for_the_same_branch_operation() {
        let dir = tempfile::tempdir().unwrap();
        let first = acquire_branch_operation_lock(dir.path()).await.unwrap();
        let release = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(first);
        });

        let second = acquire_branch_operation_lock(dir.path()).await.unwrap();
        release.await.unwrap();
        drop(second);
    }

    #[tokio::test]
    async fn branch_operation_lock_times_out_with_an_explicit_error() {
        let dir = tempfile::tempdir().unwrap();
        let _first = acquire_branch_operation_lock(dir.path()).await.unwrap();

        let error =
            acquire_branch_operation_lock_with_timeout(dir.path(), Duration::from_millis(20))
                .await
                .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("branch add lock"),
            "expected branch-add lock context: {message}"
        );
        assert!(
            message.contains("timed out"),
            "expected bounded timeout: {message}"
        );
    }
}
