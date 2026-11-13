//! Dirty sentinel and cross-process sync lock.
use super::*;

// ---------------------------------------------------------------------------
// Dirty sentinel — detects interrupted sync/index operations
// ---------------------------------------------------------------------------

/// The key of a database line in the sentinel file.
const DIRTY_DATABASE_KEY: &str = "db=";

fn dirty_sentinel_path(project_root: &Path) -> PathBuf {
    get_tokensave_dir(project_root).join("dirty")
}

/// The name that identifies a database in the sentinel file: its path below
/// the project state directory, with `/` separators. This is the `db_file`
/// form of the branch metadata, for example `branches/feature.db`.
pub(crate) fn dirty_database_name(tokensave_dir: &Path, db_path: &Path) -> String {
    let relative = db_path.strip_prefix(tokensave_dir).unwrap_or(db_path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The databases that a sentinel names. Empty for a sentinel in the old
/// format, which names none and applies to each database.
fn dirty_databases(contents: &str) -> Vec<&str> {
    contents
        .lines()
        .filter_map(|line| line.strip_prefix(DIRTY_DATABASE_KEY))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

/// Writes the sentinel with `databases`. Removes it when the list is empty.
fn store_dirty_databases(project_root: &Path, databases: &[&str]) {
    let path = dirty_sentinel_path(project_root);
    if databases.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let mut contents = format!(
        "pid={}\ntime={}\nversion={}",
        std::process::id(),
        current_timestamp(),
        env!("CARGO_PKG_VERSION"),
    );
    for database in databases {
        contents.push('\n');
        contents.push_str(DIRTY_DATABASE_KEY);
        contents.push_str(database);
    }
    let _ = std::fs::write(path, contents);
}

/// Adds `database` to the `.tokensave/dirty` sentinel before a sync or index
/// changes it.
///
/// The sentinel is one file for the project. A project in multi-branch mode
/// has one database for each tracked branch. Thus the sentinel has one `db=`
/// line for each database with an unfinished write (#865). A line whose
/// database no longer exists is dropped here.
///
/// This file is intentionally NOT cleaned up by a Drop guard — it must be
/// removed explicitly by `clear_dirty_sentinel` after the operation succeeds.
/// If the process is killed (SIGKILL, OOM), the sentinel survives and signals
/// a potential crash on the next open.
pub(crate) fn write_dirty_sentinel(project_root: &Path, database: &str) {
    let tokensave_dir = get_tokensave_dir(project_root);
    let path = dirty_sentinel_path(project_root);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let named = dirty_databases(&existing);
    if named.is_empty() && path.exists() {
        // A sentinel in the old format already applies to each database.
        // A name would narrow it to this one.
        return;
    }
    let mut databases: Vec<&str> = named
        .into_iter()
        .filter(|name| *name != database && tokensave_dir.join(name).exists())
        .collect();
    databases.push(database);
    store_dirty_databases(project_root, &databases);
}

/// Removes `database` from the dirty sentinel after a successful sync/index.
/// Removes the file when it names no other database, and when it is in the
/// old format.
pub(crate) fn clear_dirty_sentinel(project_root: &Path, database: &str) {
    let tokensave_dir = get_tokensave_dir(project_root);
    let Ok(existing) = std::fs::read_to_string(dirty_sentinel_path(project_root)) else {
        return;
    };
    let databases: Vec<&str> = dirty_databases(&existing)
        .into_iter()
        .filter(|name| *name != database && tokensave_dir.join(name).exists())
        .collect();
    store_dirty_databases(project_root, &databases);
}

/// Returns `true` if the dirty sentinel says that a previous operation on
/// `database` was interrupted. A sentinel in the old format has no database
/// name and applies to each database.
pub(crate) fn has_dirty_sentinel(project_root: &Path, database: &str) -> bool {
    let path = dirty_sentinel_path(project_root);
    let Ok(contents) = std::fs::read(&path) else {
        // A sentinel that exists and cannot be read applies to each database.
        return path.exists();
    };
    let contents = String::from_utf8_lossy(&contents);
    let databases = dirty_databases(&contents);
    databases.is_empty() || databases.contains(&database)
}

impl TokenSave {
    /// The name of the open database in the dirty sentinel.
    pub(crate) fn dirty_database_name(&self) -> String {
        dirty_database_name(&get_tokensave_dir(&self.project_root), &self.db_path())
    }
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
/// Takes the project's sync lock for a destructive rebuild of `db_path`, or
/// says why the rebuild must not happen.
///
/// A rebuild deletes the database file. Another tokensave process that has
/// it open (a running `serve`, a sync, a hook) would keep reading and writing
/// the unlinked inode while this process builds a new one, so the rebuild is
/// refused while the sync lock is held or a registered server serves this
/// project or database.
pub(crate) fn rebuild_refusal(
    project_root: &std::path::Path,
    db_path: &std::path::Path,
) -> std::result::Result<SyncLockGuard, String> {
    let lock = try_acquire_sync_lock(project_root)
        .map_err(|e| format!("{e}. Stop the other tokensave process and run the command again."))?;
    let canonical = |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.into());
    let db = canonical(db_path);
    let root = canonical(project_root);
    let me = std::process::id();
    let holders: Vec<String> = crate::servers::reap()
        .into_iter()
        .filter(|s| s.pid != me)
        .filter(|s| {
            canonical(std::path::Path::new(&s.db_path)) == db
                || canonical(std::path::Path::new(&s.project_path)) == root
        })
        .map(|s| s.pid.to_string())
        .collect();
    if !holders.is_empty() {
        return Err(format!(
            "a tokensave server (PID {}) is serving this project. Stop it (see `tokensave \
             servers`) and run the command again.",
            holders.join(", ")
        ));
    }
    Ok(lock)
}

pub(crate) fn delete_db_files(db_path: &std::path::Path) {
    let _ = std::fs::remove_file(db_path);
    // WAL and SHM files use the same base name with different extensions
    let mut wal = db_path.to_path_buf();
    wal.set_extension("db-wal");
    let _ = std::fs::remove_file(&wal);
    wal.set_extension("db-shm");
    let _ = std::fs::remove_file(&wal);
}

/// Refuse automatic destructive recovery: graph databases also contain memory.
pub(crate) fn corruption_refusal(db_path: &Path, detail: impl std::fmt::Display) -> TokenSaveError {
    TokenSaveError::Database {
        operation: "database_corruption".into(),
        message: format!(
            "database corruption detected at '{}': {detail}. Database preserved. \
             Stop all tokensave processes, back up the database and its -wal/-shm sidecars \
             together, and run `tokensave verify`. Restore a known-good backup, or move \
             the damaged database and sidecars aside and run `tokensave init` to rebuild \
             source-derived data. A rebuild cannot restore sessions or other user-authored memory.",
            db_path.display()
        ),
    }
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
            return Err(TokenSaveError::BranchLock {
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
            return Err(TokenSaveError::BranchLock {
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
        .map_err(|e| TokenSaveError::BranchLock {
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

impl TokenSave {
    /// Background tasks can retain a handle across a runtime transition. Never
    /// let such a task start writing a different checkout into its old index.
    pub(crate) fn ensure_current_checkout(&self) -> Result<()> {
        if self.tracks_checkout
            && branch_meta::load_branch_meta(&get_tokensave_dir(&self.project_root)).is_some()
            && branch::current_branch(&self.project_root) != self.active_branch
        {
            return Err(TokenSaveError::Config {
                message: "Git branch changed; acquire a fresh branch context before writing".into(),
            });
        }
        Ok(())
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

    /// A project state directory with the named database files in it.
    fn project_with_databases(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let state = get_tokensave_dir(dir.path());
        std::fs::create_dir_all(state.join("branches")).unwrap();
        for name in names {
            std::fs::write(state.join(name), "").unwrap();
        }
        dir
    }

    #[test]
    fn the_sentinel_names_each_database_with_an_unfinished_write() {
        let dir = project_with_databases(&["tokensave.db", "branches/feature.db"]);
        let root = dir.path();
        assert!(!has_dirty_sentinel(root, "tokensave.db"));

        write_dirty_sentinel(root, "branches/feature.db");
        assert!(has_dirty_sentinel(root, "branches/feature.db"));
        assert!(!has_dirty_sentinel(root, "tokensave.db"));

        write_dirty_sentinel(root, "tokensave.db");
        write_dirty_sentinel(root, "tokensave.db");
        assert!(has_dirty_sentinel(root, "branches/feature.db"));
        assert!(has_dirty_sentinel(root, "tokensave.db"));

        clear_dirty_sentinel(root, "branches/feature.db");
        assert!(!has_dirty_sentinel(root, "branches/feature.db"));
        assert!(has_dirty_sentinel(root, "tokensave.db"));

        clear_dirty_sentinel(root, "tokensave.db");
        assert!(!dirty_sentinel_path(root).exists());
    }

    /// A sentinel from an older version names no database. It applies to
    /// each database, and one successful operation removes it, as before.
    #[test]
    fn a_sentinel_in_the_old_format_applies_to_each_database() {
        let dir = project_with_databases(&["tokensave.db", "branches/feature.db"]);
        let root = dir.path();
        std::fs::write(dirty_sentinel_path(root), "pid=1\ntime=0\nversion=7.0.0").unwrap();
        assert!(has_dirty_sentinel(root, "tokensave.db"));
        assert!(has_dirty_sentinel(root, "branches/feature.db"));

        write_dirty_sentinel(root, "tokensave.db");
        assert!(
            has_dirty_sentinel(root, "branches/feature.db"),
            "a write must not narrow a sentinel in the old format"
        );

        clear_dirty_sentinel(root, "tokensave.db");
        assert!(!dirty_sentinel_path(root).exists());
    }

    #[test]
    fn a_line_for_a_removed_database_is_dropped() {
        let dir = project_with_databases(&["tokensave.db", "branches/gone.db"]);
        let root = dir.path();
        write_dirty_sentinel(root, "branches/gone.db");
        std::fs::remove_file(get_tokensave_dir(root).join("branches").join("gone.db")).unwrap();

        write_dirty_sentinel(root, "tokensave.db");
        clear_dirty_sentinel(root, "tokensave.db");
        assert!(!dirty_sentinel_path(root).exists());
    }

    #[test]
    fn the_database_name_is_its_path_below_the_state_directory() {
        let state = Path::new("project").join(".tokensave");
        assert_eq!(
            dirty_database_name(&state, &state.join("tokensave.db")),
            "tokensave.db"
        );
        assert_eq!(
            dirty_database_name(&state, &state.join("branches").join("feature.db")),
            "branches/feature.db"
        );
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
