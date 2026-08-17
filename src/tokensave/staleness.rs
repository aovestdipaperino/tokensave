//! Staleness detection for incremental sync.
use super::*;

/// Default ceiling on how many files one *automatic* sync will take on.
///
/// A catch-up sync exists to absorb a `git pull`, an IDE save, or a rebase —
/// hundreds of files at the high end. Anything an order of magnitude past
/// that is not a catch-up, it is an index build, and an index build should be
/// something the user asked for. See [`AutoSyncScope`].
pub const DEFAULT_MAX_AUTO_SYNC_FILES: usize = 2_000;

/// What an access-triggered sync should do, given how much work it would be.
///
/// Both automatic entry points — the MCP server's startup catch-up sync and
/// the staleness check at the top of every `tools/call` — fed
/// [`TokenSave::find_stale_files`] straight into a sync with no bound on the
/// result. That is safe only while the index is populated: an empty `files`
/// table makes *every* file on disk stale, so an automatic background task
/// silently became a full initial index. On a 2.5 TiB home directory that
/// reached ~95 GiB of RSS+swap (#396); on a project that had never been
/// `init`ed it burned 24 minutes of CPU (#393).
///
/// Deciding is separated from syncing so the refusal can be logged with a
/// reason and asserted in tests, rather than inferred from wall-clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoSyncScope {
    /// Proceed: these files are stale and within budget.
    Sync(Vec<String>),
    /// The index records no files at all. Building the first index is
    /// `init`'s job — it is explicit, it reports progress, and the user
    /// chose the directory. A background task must not do it by inference.
    Uninitialized,
    /// More files are stale than an automatic sync is willing to take on.
    /// An explicit `tokensave sync` is unbounded and remains the way to do
    /// this deliberately.
    TooManyStale { count: usize, limit: usize },
    /// The working tree has been checked out onto a different branch than
    /// the one this handle resolved. Syncing would write the new branch's
    /// files into the old branch's database. See [`BranchDrift`].
    BranchDrifted(BranchDrift),
}

/// A handle resolved to one branch while the working tree sits on another.
///
/// [`TokenSave::open`] resolves the active branch once, and the MCP server
/// holds that handle for its whole life. A `git checkout` underneath it is
/// invisible: the handle keeps serving — and *writing to* — the branch it
/// started on. The reported consequence is `main`'s index gaining a file that
/// exists only on `feature`, so the index is not merely stale but describes a
/// tree that never existed, with no warning on the tool response or on
/// stderr (#400).
///
/// Only meaningful in multi-branch mode. A project with no per-branch
/// databases has a single index by design, and switching branches there is
/// ordinary — guarding on the branch name alone would break every project
/// that never opted in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchDrift {
    /// The branch whose database this handle reads and writes.
    pub serving: String,
    /// The branch the working tree is actually on now.
    pub working_tree: String,
}

// ---------------------------------------------------------------------------
// Staleness detection
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Check whether the given files need (re-/un-)indexing to bring the DB
    /// into agreement with the filesystem.
    ///
    /// A file is reported stale when any of:
    /// - it is in the DB and has been modified on disk since `indexed_at`,
    /// - it is in the DB but no longer exists on disk (deletion — DB needs cleanup),
    /// - it exists on disk but has no DB record (new file — needs indexing).
    ///
    /// A file that exists in neither the DB nor on disk is out of scope and
    /// is silently dropped.
    pub async fn check_file_staleness(&self, file_paths: &[String]) -> Vec<String> {
        let mut stale = Vec::new();
        for path in file_paths {
            // Match the DB's canonical form (forward slashes). Without this,
            // a caller passing `src\foo.py` on Windows misses the row stored
            // under `src/foo.py` and the file gets treated as "new" — a
            // subsequent sync would insert a *second* row alongside the
            // original, which is #87.
            let normalized = normalize_rel_path(path);
            let abs_path = self.project_root.join(&normalized);
            let file_exists = abs_path.exists();
            match self.db.get_file(&normalized).await {
                Ok(Some(record)) => {
                    if !file_exists {
                        // Indexed but deleted — DB needs cleanup.
                        stale.push(normalized);
                    } else if let Ok(metadata) = std::fs::metadata(&abs_path) {
                        if let Ok(mtime) = metadata.modified() {
                            let mtime_secs = mtime
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            if mtime_secs > record.indexed_at {
                                stale.push(normalized);
                            }
                        }
                    }
                }
                _ => {
                    // Not in the DB. If it exists on disk, it's new and needs indexing.
                    if file_exists {
                        stale.push(normalized);
                    }
                }
            }
        }
        stale
    }

    /// Returns every file whose on-disk mtime is newer than its indexed
    /// timestamp, plus on-disk files the DB doesn't know about yet, plus
    /// DB-known files that no longer exist on disk (so a follow-up sync
    /// can prune them).
    ///
    /// Walks the project tree with the same gitignore-aware logic used by
    /// `sync()`, then compares against a single batched DB read of the
    /// `files` table — no per-file SQL round trips. This is the
    /// notification-free replacement for the `notify`-based watcher
    /// removed in v6.x (see #80): the MCP server calls it on a 30 s
    /// cooldown to keep the index fresh without burning CPU/memory on
    /// kernel event streams.
    pub async fn find_stale_files(&self) -> Vec<String> {
        let on_disk = self.scan_files();
        // DB read failed → be conservative and treat every on-disk file as
        // stale rather than silently dropping the check.
        let Ok(indexed) = self.get_all_files().await else {
            return on_disk;
        };

        let indexed_map: HashMap<&str, i64> = indexed
            .iter()
            .map(|f| (f.path.as_str(), f.indexed_at))
            .collect();
        let on_disk_set: HashSet<&str> = on_disk.iter().map(String::as_str).collect();

        let mut stale: Vec<String> = Vec::new();

        for rel in &on_disk {
            let abs = self.project_root.join(rel);
            let mtime_secs = std::fs::metadata(&abs)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            match indexed_map.get(rel.as_str()) {
                Some(&indexed_at) if mtime_secs <= indexed_at => {}
                _ => stale.push(rel.clone()),
            }
        }

        for indexed_path in indexed_map.keys() {
            if !on_disk_set.contains(*indexed_path) {
                stale.push((*indexed_path).to_string());
            }
        }

        stale.sort();
        stale.dedup();
        stale
    }

    /// Detects a working tree that has moved to a different branch than this
    /// handle resolved, which makes every write cross-branch (#400).
    ///
    /// Returns `None` in single-DB mode (no branch metadata), on a detached
    /// HEAD or outside a git repo (no branch to compare), and — the common
    /// case — when the tree is still on the branch being served.
    ///
    /// Cheap enough for the per-`tools/call` path: one `current_branch`
    /// lookup, which prefers `gix` over spawning git, and one small JSON
    /// read that only happens for projects in multi-branch mode.
    pub fn branch_drift(&self) -> Option<BranchDrift> {
        let meta = branch_meta::load_branch_meta(&get_tokensave_dir(&self.project_root))?;

        let serving = self
            .serving_branch
            .as_ref()
            .or(self.active_branch.as_ref())?;
        let working_tree = branch::current_branch(&self.project_root)?;
        if working_tree == *serving {
            return None;
        }

        // A differing name is not yet a problem: what matters is whether the
        // working tree's branch has a database of its own. `init` writes
        // branch metadata for the default branch alone, so metadata exists
        // even in single-DB mode — an untracked branch there is served by the
        // same top-level `tokensave.db` and syncing it is correct. Mirrors
        // the ownership rule in `branch::track_branch_copy`: the default
        // branch is the top-level DB, tracked branches get their own.
        let working_tree_has_own_db =
            meta.is_tracked(&working_tree) || working_tree == meta.default_branch;
        if !working_tree_has_own_db {
            return None;
        }
        Some(BranchDrift {
            serving: serving.clone(),
            working_tree,
        })
    }

    /// [`Self::find_stale_files`] with the bounds an *automatic* sync must
    /// respect. Callers that sync without the user asking — the MCP server's
    /// startup catch-up and its per-`tools/call` staleness check — must use
    /// this; explicit `tokensave sync` deliberately stays unbounded.
    ///
    /// Three guards, in order (see [`AutoSyncScope`] for why):
    ///
    /// 1. A working tree checked out onto a different branch than this handle
    ///    serves yields [`AutoSyncScope::BranchDrifted`]. Syncing would write
    ///    one branch's files into another's index (#400). Checked first,
    ///    because under drift every other answer is about the wrong database.
    /// 2. An index with no files recorded yields
    ///    [`AutoSyncScope::Uninitialized`]. Every file on disk is stale in
    ///    that state, so proceeding turns a background task into a full
    ///    initial index of whatever directory we happen to be pointed at
    ///    (#396, #393).
    /// 3. A stale set larger than `max_auto_sync_files` yields
    ///    [`AutoSyncScope::TooManyStale`]. The 30 s cooldown bounds how
    ///    *often* a sync runs, never what one costs, and while resolution
    ///    materialises the whole node graph at once (#306) that cost is
    ///    graph-proportional. A limit of `0` disables this third guard.
    pub async fn find_stale_files_bounded(&self) -> AutoSyncScope {
        if let Some(drift) = self.branch_drift() {
            return AutoSyncScope::BranchDrifted(drift);
        }

        // Checked before the walk: when the index is empty there is nothing
        // a catch-up could be catching up *to*, and scanning a 2.5 TiB tree
        // to discover that is itself part of the problem being fixed.
        match self.get_all_files().await {
            Ok(indexed) if indexed.is_empty() => return AutoSyncScope::Uninitialized,
            // A failed read is not evidence the index is empty, so fall
            // through: `find_stale_files` treats that case conservatively
            // and guard 2 still bounds the result.
            _ => {}
        }

        let stale = self.find_stale_files().await;
        let limit = self.max_auto_sync_files;
        if limit > 0 && stale.len() > limit {
            return AutoSyncScope::TooManyStale {
                count: stale.len(),
                limit,
            };
        }
        AutoSyncScope::Sync(stale)
    }

    /// Returns the most recent `indexed_at` timestamp across all indexed files.
    pub async fn last_index_time(&self) -> Result<i64> {
        self.db.last_index_time().await
    }

    /// Returns the timestamp of the most recent successful sync.
    ///
    /// Prefers the `last_sync_at` metadata key, which advances on every sync
    /// invocation regardless of whether any files actually changed. Falls
    /// back to `last_index_time` (the max file `indexed_at`) only if the
    /// metadata key is missing or unreadable — that fallback gives the wrong
    /// answer on quiet repos because `indexed_at` is per-file and only moves
    /// when a file is reindexed, which is exactly the bug #86 was reporting.
    pub async fn last_sync_timestamp(&self) -> i64 {
        if let Ok(Some(raw)) = self.db.get_metadata("last_sync_at").await {
            if let Ok(t) = raw.parse::<i64>() {
                return t;
            }
        }
        self.db.last_index_time().await.unwrap_or(0)
    }

    /// Count git commits newer than the given UNIX timestamp.
    /// Returns 0 if git is unavailable or the directory is not a git repository.
    pub fn git_commits_since(&self, since_timestamp: i64) -> usize {
        let Ok(repo) = gix::open(&self.project_root) else {
            return 0;
        };
        let Ok(head) = repo.head_commit() else {
            return 0;
        };
        let sorting = gix::revision::walk::Sorting::ByCommitTimeCutoff {
            order: gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            seconds: since_timestamp,
        };
        let Ok(walk) = head.ancestors().sorting(sorting).all() else {
            return 0;
        };
        walk.filter_map(std::result::Result::ok).count()
    }
}
