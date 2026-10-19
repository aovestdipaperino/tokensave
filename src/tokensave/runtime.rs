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
}

impl BranchRuntime {
    /// Resolve the working tree afresh, including linked worktrees whose `.git`
    /// is a file. No metadata means legacy single-database mode. Auto-tracking
    /// still honors the existing configuration and environment override.
    ///
    /// Keep the returned guard alive throughout the operation. `None` means
    /// that the startup handle remains current. Failed transitions leave the
    /// previous handle intact and are retried on the next request.
    pub async fn lease<'a>(
        &'a self,
        initial: &TokenSave,
    ) -> Result<MutexGuard<'a, Option<Arc<TokenSave>>>> {
        let mut current = self.current.lock().await;
        let root = initial.project_root();
        if branch_meta::load_branch_meta(&get_tokensave_dir(root)).is_none() {
            return Ok(current);
        }
        let previous = current.as_deref().unwrap_or(initial);
        let branch = branch::current_branch(root);
        if branch.as_deref() == previous.active_branch() {
            if current.is_some()
                && previous.fallback_warning.is_none()
                && current_timestamp().saturating_sub(previous.last_sync_timestamp().await) >= 30
            {
                if let AutoSyncScope::Sync(stale) = previous.find_stale_files_bounded().await {
                    previous.sync_if_stale_silent(&stale).await?;
                }
            }
            return Ok(current);
        }

        let next = TokenSave::open(root).await?;
        // Never synchronize an ancestor/default fallback with another branch's
        // source. Detached HEAD retains the established default-index policy.
        if next.fallback_warning.is_none() && branch.is_some() {
            match next.find_stale_files_bounded().await {
                AutoSyncScope::Sync(_) => {
                    next.sync_branch_context().await?;
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
        *current = Some(Arc::new(next));
        Ok(current)
    }
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
        let next = lease.as_ref().unwrap();
        assert_eq!(next.active_branch(), Some("feature"));
        assert!(next.db.get_file("feature.rs").await.unwrap().is_some());
        assert!(next.db.get_file("base.rs").await.unwrap().is_none());
        assert!(initial.db.get_file("feature.rs").await.unwrap().is_none());
        // Reusing the same context does not copy or reopen its database.
        let saved = Arc::clone(next);
        drop(lease);
        let lease = runtime.lease(&initial).await.unwrap();
        assert!(Arc::ptr_eq(&saved, lease.as_ref().unwrap()));
        drop(lease);
        git(tmp.path(), &["checkout", "main"]);
        let lease = runtime.lease(&initial).await.unwrap();
        let main = lease.as_ref().unwrap();
        assert_eq!(main.active_branch(), Some("main"));
        assert!(main.db.get_file("base.rs").await.unwrap().is_some());
        assert!(main.db.get_file("feature.rs").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn leases_serialize_concurrent_transitions_and_retry_after_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::default();
        let held = runtime.lease(&initial).await.unwrap();
        git(tmp.path(), &["checkout", "-b", "feature"]);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), runtime.lease(&initial))
                .await
                .is_err()
        );
        drop(held);
        let sync_lock = try_acquire_sync_lock(tmp.path()).unwrap();
        assert!(runtime.lease(&initial).await.is_err());
        drop(sync_lock);
        let (a, b) = tokio::join!(
            async {
                runtime
                    .lease(&initial)
                    .await
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .active_branch()
                    .map(str::to_owned)
            },
            async {
                runtime
                    .lease(&initial)
                    .await
                    .unwrap()
                    .as_ref()
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
        assert_eq!(lease.as_ref().unwrap().active_branch(), Some("linked-next"));
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
        let fallback = lease.as_ref().unwrap();
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
        assert_eq!(
            lease.as_ref().unwrap().active_branch(),
            Some("large-change")
        );
    }

    #[tokio::test]
    async fn legacy_and_detached_contexts_keep_existing_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let initial = project(tmp.path()).await;
        let runtime = BranchRuntime::default();
        git(tmp.path(), &["checkout", "--detach"]);
        let lease = runtime.lease(&initial).await.unwrap();
        assert_eq!(lease.as_ref().unwrap().active_branch(), None);
        assert!(lease.as_ref().unwrap().fallback_warning.is_some());
        drop(lease);
        std::fs::remove_file(get_tokensave_dir(tmp.path()).join("branch-meta.json")).unwrap();
        let legacy = TokenSave::open(tmp.path()).await.unwrap();
        git(tmp.path(), &["checkout", "-b", "legacy"]);
        let runtime = BranchRuntime::default();
        assert!(runtime.lease(&legacy).await.unwrap().is_none());
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
