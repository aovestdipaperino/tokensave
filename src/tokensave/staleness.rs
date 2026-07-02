//! Staleness detection for incremental sync.
use super::*;

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
