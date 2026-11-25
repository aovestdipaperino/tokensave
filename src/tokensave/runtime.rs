//! Runtime branch selection, independent of checkout hooks or editor integration.
use super::*;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

/// Serializes local requests with branch transitions. Each request keeps its
/// lease until dispatch completes; a replacement is published only after sync.
/// Explicitly selected snapshots do not use this runtime.
#[derive(Default)]
pub struct BranchRuntime {
    current: Mutex<Option<Arc<TokenSave>>>,
    lock_wait: SyncLockWaitPolicy,
}

/// The graph a local request uses, held until dispatch completes.
pub struct BranchLease<'a> {
    current: MutexGuard<'a, Option<Arc<TokenSave>>>,
    /// The requested branch's existing index, served for this request only
    /// because a peer's sync kept the lock past the wait. It is never
    /// published, so the next request retries the transition.
    snapshot: Option<Arc<TokenSave>>,
    warning: Option<String>,
}

impl BranchLease<'_> {
    /// The graph to serve. `None` means that the startup handle remains
    /// current.
    pub fn graph(&self) -> Option<&TokenSave> {
        self.snapshot.as_deref().or(self.current.as_deref())
    }

    /// Whether [`Self::graph`] replaces the startup handle.
    pub fn is_switched(&self) -> bool {
        self.graph().is_some()
    }

    /// Set when the served index could not be refreshed because another
    /// sync held the lock past the wait. Names the branch whose index is
    /// served and how old it is.
    pub fn freshness_warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
}

impl BranchRuntime {
    /// A runtime that waits `lock_wait` for a peer's sync lock.
    #[cfg(test)]
    fn with_lock_wait(lock_wait: SyncLockWaitPolicy) -> Self {
        Self {
            current: Mutex::default(),
            lock_wait,
        }
    }

    /// Resolve the working tree afresh, including linked worktrees whose `.git`
    /// is a file. No metadata means legacy single-database mode. Auto-tracking
    /// still honors the existing configuration and environment override.
    ///
    /// Keep the returned lease alive throughout the operation. Failed
    /// transitions leave the previous handle intact and are retried on the
    /// next request.
    ///
    /// A live sync lock held by another process (for example the
    /// `post-checkout` hook's background sync) is waited out for a bounded
    /// time (#1123). If it is still held, the requested branch's existing
    /// index is served with a freshness warning; another branch's index is
    /// never served in its place.
    pub async fn lease<'a>(&'a self, initial: &TokenSave) -> Result<BranchLease<'a>> {
        let current = self.current.lock().await;
        let mut lease = BranchLease {
            current,
            snapshot: None,
            warning: None,
        };
        let root = initial.project_root();
        if branch_meta::load_branch_meta(&get_tokensave_dir(root)).is_none() {
            return Ok(lease);
        }
        let previous = lease.current.as_deref().unwrap_or(initial);
        let branch = branch::current_branch(root);
        if branch.as_deref() == previous.active_branch() {
            if lease.current.is_some()
                && previous.fallback_warning.is_none()
                && current_timestamp().saturating_sub(previous.last_sync_timestamp().await) >= 30
            {
                if let AutoSyncScope::Sync(stale) = previous.find_stale_files_bounded().await {
                    let outcome = previous
                        .refresh_if_stale_bounded(&stale, self.lock_wait)
                        .await?;
                    lease.warning = stale_snapshot_warning(previous, outcome, self.lock_wait).await;
                }
            }
            return Ok(lease);
        }

        let next = TokenSave::open(root).await?;
        let mut outcome = RefreshOutcome::Refreshed;
        // Never synchronize an ancestor/default fallback with another branch's
        // source. Detached HEAD retains the established default-index policy.
        if next.fallback_warning.is_none() && branch.is_some() {
            match next.find_stale_files_bounded().await {
                AutoSyncScope::Sync(_) => {
                    outcome = next.sync_branch_context_bounded(self.lock_wait).await?;
                }
                scope => {
                    return Err(TokenSaveError::Config {
                        message: format!(
                            "cannot refresh branch context: {scope:?}; run `tokensave sync`"
                        ),
                    })
                }
            }
        }
        // Git clients do not participate in our locks. If another checkout
        // occurred during preparation, do not publish a mismatched context.
        if branch::current_branch(root) != branch || next.active_branch() != branch.as_deref() {
            return Err(TokenSaveError::Config {
                message: "Git branch changed while preparing its index; retry the operation".into(),
            });
        }
        if let Some(warning) = stale_snapshot_warning(&next, outcome, self.lock_wait).await {
            lease.warning = Some(warning);
            lease.snapshot = Some(Arc::new(next));
        } else {
            *lease.current = Some(Arc::new(next));
        }
        Ok(lease)
    }
}

/// The warning for an index that a refresh could not update, or `None` when
/// the refresh completed.
async fn stale_snapshot_warning(
    graph: &TokenSave,
    outcome: RefreshOutcome,
    lock_wait: SyncLockWaitPolicy,
) -> Option<String> {
    let RefreshOutcome::LockTimedOut { holder_pid } = outcome else {
        return None;
    };
    let branch = graph
        .serving_branch()
        .or(graph.active_branch())
        .unwrap_or("the default index");
    let holder = holder_pid.map_or_else(
        || "another process".to_owned(),
        |pid| format!("another process (PID {pid})"),
    );
    let last_sync = graph.last_sync_timestamp().await;
    let age = if last_sync > 0 {
        format!(
            "last synced {}s ago",
            current_timestamp().saturating_sub(last_sync)
        )
    } else {
        "never synced".to_owned()
    };
    Some(format!(
        "serving the existing index of branch '{branch}' ({age}) without refreshing it: \
         {holder} still held the sync lock after {}s. Results may not reflect the working \
         tree; a newly tracked branch may still hold its parent's index. Retry shortly for \
         a refreshed index.",
        lock_wait.timeout.as_secs()
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    async fn project(root: &Path) -> TokenSave {
        git(root, &["init", "-b", "main"]);
        std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
        std::fs::write(root.join("base.rs"), "fn base() {}\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "base"]);
        let cg = TokenSave::init(root).await.unwrap();
        cg.sync().await.unwrap();
        let mut config = load_config(root).unwrap();
        config.auto_track = true;
        save_config(root, &config).unwrap();
        TokenSave::open(root).await.unwrap()
    }

    /// Short wait bounds so lock-timeout tests stay fast.
    const SHORT_WAIT: SyncLockWaitPolicy = SyncLockWaitPolicy {
        timeout: Duration::from_millis(200),
        poll: Duration::from_millis(10),
    };

    /// Wait bounds long enough that a test's peer always finishes first.
    const LONG_WAIT: SyncLockWaitPolicy = SyncLockWaitPolicy {
        timeout: Duration::from_secs(30),
        poll: Duration::from_millis(10),
    };

    /// Switch to a new branch `feature` that adds `feature.rs`.
    fn switch_to_feature(root: &Path) {
        git(root, &["checkout", "-b", "feature"]);
        std::fs::write(
            root.join("feature.rs"),
            "fn feature() {}
",
        )
        .unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "feature"]);
    }

    #[tokio::test]
    async fn transition_waits_for_live_lock_released_before_deadline() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::with_lock_wait(LONG_WAIT);
        switch_to_feature(tmp.path());
        // Simulates the post-checkout hook's background sync.
        let sync_lock = try_acquire_sync_lock(tmp.path()).unwrap();
        let (released_tx, released_rx) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            released_rx.await.unwrap();
            drop(sync_lock);
        });
        let lease = runtime.lease(&initial);
        tokio::pin!(lease);
        // The lease is blocked on the live lock instead of failing.
        assert!(tokio::time::timeout(Duration::from_millis(100), &mut lease)
            .await
            .is_err());
        released_tx.send(()).unwrap();
        let lease = lease.await.unwrap();
        peer.await.unwrap();
        let graph = lease.graph().unwrap();
        assert_eq!(graph.active_branch(), Some("feature"));
        assert!(graph.db.get_file("feature.rs").await.unwrap().is_some());
        assert!(lease.freshness_warning().is_none());
        assert!(lease.current.is_some(), "a refreshed context is published");
    }

    #[tokio::test]
    async fn transition_timeout_serves_requested_branch_with_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::with_lock_wait(SHORT_WAIT);
        switch_to_feature(tmp.path());
        let sync_lock = try_acquire_sync_lock(tmp.path()).unwrap();
        let lease = runtime.lease(&initial).await.unwrap();
        let graph = lease.graph().unwrap();
        // Never another branch's index: the snapshot is `feature`'s own,
        // not yet refreshed from the working tree.
        assert_eq!(graph.active_branch(), Some("feature"));
        assert_eq!(graph.serving_branch(), Some("feature"));
        assert!(graph.db.get_file("feature.rs").await.unwrap().is_none());
        let warning = lease.freshness_warning().unwrap();
        assert!(warning.contains("branch 'feature'"), "{warning}");
        assert!(
            warning.contains(&format!("PID {}", std::process::id())),
            "{warning}"
        );
        drop(lease);
        // The live lock was never removed.
        assert!(get_tokensave_dir(tmp.path()).join("sync.lock").exists());
        drop(sync_lock);
        let lease = runtime.lease(&initial).await.unwrap();
        assert!(lease.freshness_warning().is_none());
        let graph = lease.graph().unwrap();
        assert!(graph.db.get_file("feature.rs").await.unwrap().is_some());
        assert!(initial.db.get_file("feature.rs").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn transition_reuses_a_peer_sync_completed_during_the_wait() {
        let tmp = tempfile::tempdir().unwrap();
        let _initial = project(tmp.path()).await;
        switch_to_feature(tmp.path());
        let peer = TokenSave::open(tmp.path()).await.unwrap();
        let next = TokenSave::open(tmp.path()).await.unwrap();
        assert_eq!(next.active_branch(), Some("feature"));
        let sync_lock = try_acquire_sync_lock(tmp.path()).unwrap();
        let peer = tokio::spawn(async move {
            // `last_sync_at` has one-second resolution; make the peer's sync
            // observable.
            tokio::time::sleep(Duration::from_millis(1_100)).await;
            peer.sync_with_lock(sync_lock, |_, _, _| {}, |_| {}, true)
                .await
                .unwrap();
        });
        let outcome = next.sync_branch_context_bounded(LONG_WAIT).await.unwrap();
        peer.await.unwrap();
        assert_eq!(outcome, RefreshOutcome::PeerRefreshed);
        assert!(next.db.get_file("feature.rs").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn dispatch_refresh_waits_for_lock_and_times_out_consistently() {
        let tmp = tempfile::tempdir().unwrap();
        let cg = project(tmp.path()).await;
        cg.db
            .set_metadata("language_dispatch_revision", "outdated")
            .await
            .unwrap();
        assert!(cg.dispatch_changed().await.unwrap());
        let sync_lock = try_acquire_sync_lock(tmp.path()).unwrap();
        let outcome = cg.refresh_if_stale_bounded(&[], SHORT_WAIT).await.unwrap();
        assert_eq!(
            outcome,
            RefreshOutcome::LockTimedOut {
                holder_pid: Some(std::process::id())
            }
        );
        // The public silent variant no longer fails on a live lock.
        drop(sync_lock);
        let outcome = cg.refresh_if_stale_bounded(&[], SHORT_WAIT).await.unwrap();
        assert_eq!(outcome, RefreshOutcome::Refreshed);
        assert!(!cg.dispatch_changed().await.unwrap());
    }

    #[tokio::test]
    async fn bounded_lock_wait_reports_genuine_lock_errors_at_once() {
        let tmp = tempfile::tempdir().unwrap();
        let _cg = project(tmp.path()).await;
        let lock_path = get_tokensave_dir(tmp.path()).join("sync.lock");
        std::fs::write(&lock_path, "not-a-pid").unwrap();
        let started = Instant::now();
        let result = acquire_sync_lock_bounded(tmp.path(), LONG_WAIT).await;
        assert!(matches!(result, Err(TokenSaveError::SyncLock { .. })));
        assert!(started.elapsed() < LONG_WAIT.timeout);
        assert!(lock_path.exists(), "a malformed lock is never broken");
        // An empty lockfile is a writer between create and PID write: wait.
        std::fs::write(&lock_path, "").unwrap();
        let result = acquire_sync_lock_bounded(tmp.path(), SHORT_WAIT).await;
        assert!(matches!(
            result,
            Ok(SyncLockWait::TimedOut { holder_pid: None })
        ));
    }

    #[tokio::test]
    async fn round_trip_tracks_and_syncs_without_polluting_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::default();
        git(tmp.path(), &["checkout", "-b", "feature"]);
        std::fs::remove_file(tmp.path().join("base.rs")).unwrap();
        std::fs::write(tmp.path().join("feature.rs"), "fn feature() {}\n").unwrap();
        git(tmp.path(), &["add", "."]);
        git(tmp.path(), &["commit", "-m", "feature"]);
        let lease = runtime.lease(&initial).await.unwrap();
        let next = lease.graph().unwrap();
        assert_eq!(next.active_branch(), Some("feature"));
        assert!(next.db.get_file("feature.rs").await.unwrap().is_some());
        assert!(next.db.get_file("base.rs").await.unwrap().is_none());
        assert!(initial.db.get_file("feature.rs").await.unwrap().is_none());
        // Reusing the same context does not copy or reopen its database.
        let saved = Arc::clone(lease.current.as_ref().unwrap());
        drop(lease);
        let lease = runtime.lease(&initial).await.unwrap();
        assert!(Arc::ptr_eq(&saved, lease.current.as_ref().unwrap()));
        drop(lease);
        git(tmp.path(), &["checkout", "main"]);
        let lease = runtime.lease(&initial).await.unwrap();
        let main = lease.graph().unwrap();
        assert_eq!(main.active_branch(), Some("main"));
        assert!(main.db.get_file("base.rs").await.unwrap().is_some());
        assert!(main.db.get_file("feature.rs").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn leases_serialize_concurrent_transitions_and_retry_after_lock_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::with_lock_wait(SHORT_WAIT);
        let held = runtime.lease(&initial).await.unwrap();
        git(tmp.path(), &["checkout", "-b", "feature"]);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), runtime.lease(&initial))
                .await
                .is_err()
        );
        drop(held);
        // A transition whose sync timed out is not published, so the next
        // request retries it.
        let sync_lock = try_acquire_sync_lock(tmp.path()).unwrap();
        let lease = runtime.lease(&initial).await.unwrap();
        assert!(lease.freshness_warning().is_some());
        assert!(
            lease.current.is_none(),
            "a timed-out snapshot is not published"
        );
        drop(lease);
        drop(sync_lock);
        let (a, b) = tokio::join!(
            async {
                runtime
                    .lease(&initial)
                    .await
                    .unwrap()
                    .graph()
                    .unwrap()
                    .active_branch()
                    .map(str::to_owned)
            },
            async {
                runtime
                    .lease(&initial)
                    .await
                    .unwrap()
                    .graph()
                    .unwrap()
                    .active_branch()
                    .map(str::to_owned)
            },
        );
        assert_eq!(a.as_deref(), Some("feature"));
        assert_eq!(a, b);
        assert!(
            initial.sync().await.is_err(),
            "old background handles must not write"
        );
    }

    #[tokio::test]
    async fn worktree_switches_use_worktree_local_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir(&main).unwrap();
        let original = project(&main).await;
        let wt = tmp.path().join("linked");
        git(
            &main,
            &["worktree", "add", "-b", "linked", wt.to_str().unwrap()],
        );
        let linked = TokenSave::init(&wt).await.unwrap();
        linked.sync().await.unwrap();
        let mut config = load_config(&wt).unwrap();
        config.auto_track = true;
        save_config(&wt, &config).unwrap();
        let linked = TokenSave::open(&wt).await.unwrap();
        git(&wt, &["checkout", "-b", "linked-next"]);
        let runtime = BranchRuntime::default();
        let lease = runtime.lease(&linked).await.unwrap();
        assert_eq!(lease.graph().unwrap().active_branch(), Some("linked-next"));
        assert_eq!(original.active_branch(), Some("main"));
        assert!(!branch_meta::load_branch_meta(&get_tokensave_dir(&main))
            .unwrap()
            .is_tracked("linked-next"));
    }

    #[tokio::test]
    async fn auto_tracking_disabled_keeps_ancestor_fallback_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let mut config = load_config(tmp.path()).unwrap();
        config.auto_track = false;
        save_config(tmp.path(), &config).unwrap();
        git(tmp.path(), &["checkout", "-b", "untracked"]);
        std::fs::write(tmp.path().join("new.rs"), "fn new() {}\n").unwrap();
        let runtime = BranchRuntime::default();
        let lease = runtime.lease(&initial).await.unwrap();
        let fallback = lease.graph().unwrap();
        assert_eq!(fallback.active_branch(), Some("untracked"));
        assert!(fallback.fallback_warning.is_some());
        assert!(fallback.db.get_file("new.rs").await.unwrap().is_none());
        assert!(
            !branch_meta::load_branch_meta(&get_tokensave_dir(tmp.path()))
                .unwrap()
                .is_tracked("untracked")
        );
    }

    #[tokio::test]
    async fn sync_bounds_refuse_transition_then_allow_retry_after_explicit_sync() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let mut config = load_config(tmp.path()).unwrap();
        config.max_auto_sync_files = 1;
        save_config(tmp.path(), &config).unwrap();
        git(tmp.path(), &["checkout", "-b", "large-change"]);
        for file in ["one.rs", "two.rs"] {
            std::fs::write(tmp.path().join(file), "fn added() {}\n").unwrap();
        }
        let runtime = BranchRuntime::default();
        assert!(runtime.lease(&initial).await.is_err());
        let explicit = TokenSave::open(tmp.path()).await.unwrap();
        explicit.sync().await.unwrap();
        let lease = runtime.lease(&initial).await.unwrap();
        assert_eq!(lease.graph().unwrap().active_branch(), Some("large-change"));
    }

    #[tokio::test]
    async fn legacy_and_detached_contexts_keep_existing_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::default();
        git(tmp.path(), &["checkout", "--detach"]);
        let lease = runtime.lease(&initial).await.unwrap();
        assert_eq!(lease.graph().unwrap().active_branch(), None);
        assert!(lease.graph().unwrap().fallback_warning.is_some());
        drop(lease);
        std::fs::remove_file(get_tokensave_dir(tmp.path()).join("branch-meta.json")).unwrap();
        let legacy = TokenSave::open(tmp.path()).await.unwrap();
        git(tmp.path(), &["checkout", "-b", "legacy"]);
        let runtime = BranchRuntime::default();
        assert!(!runtime.lease(&legacy).await.unwrap().is_switched());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod content_tests {
    use super::*;

    #[tokio::test]
    async fn transition_detects_same_size_same_timestamp_content_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("base.rs");
        std::fs::write(&source, "fn before() {}\n").unwrap();
        let initial = TokenSave::init(tmp.path()).await.unwrap();
        initial.sync().await.unwrap();
        let timestamp = std::fs::metadata(&source).unwrap().modified().unwrap();
        std::fs::write(&source, "fn after_() {}\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(timestamp))
            .unwrap();
        let result = initial.sync_branch_context().await.unwrap();
        assert_eq!(result.files_modified, 1);
        let result = initial.sync_branch_context().await.unwrap();
        assert_eq!(
            result.files_modified, 0,
            "identical content must not be re-extracted"
        );
    }
}
