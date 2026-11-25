//! Named values for the `TokenSave` facade's sync-lock coordination.
use std::time::Duration;

/// How long an automatic refresh waits for a live peer to release the
/// project sync lock before it serves the existing index with a warning
/// (#1123). The same bound applies to branch transitions and same-branch
/// refreshes, so both handle contention the same way.
pub(crate) const SYNC_LOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Interval between attempts while a refresh waits for the sync lock.
pub(crate) const SYNC_LOCK_WAIT_POLL: Duration = Duration::from_millis(50);

/// Attempts to create the lockfile when a peer removes it between our failed
/// create and the PID read. Each such race means the lock was just released.
pub(crate) const SYNC_LOCK_RELEASE_RACE_ATTEMPTS: usize = 3;
