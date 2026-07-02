//! Full and incremental indexing.
use super::query::resolve_symbol_for_edit;
use super::*;

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

impl TokenSave {
    /// Appends runtime skip-folder patterns to the exclude list.
    ///
    /// Each folder name is converted to a `folder/**` glob so that all
    /// files underneath it are excluded during scanning.
    pub fn add_skip_folders(&mut self, folders: &[String]) {
        for folder in folders {
            self.config.exclude.push(format!("{folder}/**"));
        }
    }

    /// Performs a full index: clears existing data, scans all Rust files,
    /// extracts nodes and edges, resolves references, and stores everything
    /// in the database.
    pub async fn index_all(&self) -> Result<IndexResult> {
        self.index_all_with_progress(|_, _, _| {}).await
    }

    /// Like `index_all()`, but calls `on_file(current, total, path)` before
    /// processing each file. Use this to drive a progress spinner with ETA in
    /// the CLI.
    pub async fn index_all_with_progress<F>(&self, on_file: F) -> Result<IndexResult>
    where
        F: Fn(usize, usize, &str),
    {
        self.index_all_with_progress_verbose(on_file, |_| {}).await
    }

    /// Like `index_all_with_progress()`, but also calls `on_verbose` after
    /// each phase completes with a diagnostic summary line.
    pub async fn index_all_with_progress_verbose<F, V>(
        &self,
        on_file: F,
        on_verbose: V,
    ) -> Result<IndexResult>
    where
        F: Fn(usize, usize, &str),
        V: Fn(&str),
    {
        debug_assert!(self.project_root.exists(), "project root does not exist");
        debug_assert!(
            self.project_root.is_dir(),
            "project root is not a directory"
        );
        let _lock = try_acquire_sync_lock(&self.project_root)?;
        write_dirty_sentinel(&self.project_root);
        let start = Instant::now();

        // 1. Clear existing data and enter bulk-load mode
        self.db.clear().await?;
        self.db.begin_bulk_load().await?;

        // 2. Scan for source files
        let phase_start = Instant::now();
        let files = self.scan_files();
        let total = files.len();
        on_verbose(&format!(
            "scanned {} files in {:.1}s",
            total,
            phase_start.elapsed().as_secs_f64()
        ));

        // 3. Parallel extraction: read + parse + hash on all cores
        let project_root = self.project_root.clone();
        let registry = &self.registry;

        let phase_start = Instant::now();
        let (extractions, _skipped) =
            extract_files_isolated(&project_root, registry, files.clone());

        // 4. Collect all data
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();
        let mut all_unresolved = Vec::new();
        let mut file_records = Vec::new();
        let mut total_nodes = 0;

        for (idx, (file_path, result, hash, size, mtime)) in extractions.iter().enumerate() {
            on_file(idx + 1, total, file_path);
            total_nodes += result.nodes.len();
            all_nodes.extend_from_slice(&result.nodes);
            all_edges.extend_from_slice(&result.edges);
            all_unresolved.extend_from_slice(&result.unresolved_refs);
            file_records.push(FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            });
        }

        on_verbose(&format!(
            "extracted {} nodes, {} edges from {} files in {:.1}s",
            total_nodes,
            all_edges.len(),
            extractions.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // 5. Resolve references in-memory (parallel) before DB insert
        let phase_start = Instant::now();
        if !all_unresolved.is_empty() {
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            let resolution = resolver.resolve_all(&all_unresolved);
            all_edges.extend(resolver.create_edges(&resolution.resolved));
            // Propagate call edges across build-config variants (Rust `#[cfg]`
            // twins, Go platform files) so an inactive-platform definition is
            // not seen as dead merely because the call bound to its sibling (#141).
            let variant_edges = crate::resolution::propagate_variant_edges(&all_nodes, &all_edges);
            all_edges.extend(variant_edges);
        }
        on_verbose(&format!(
            "resolved {} references in {:.1}s",
            all_unresolved.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // 6. Sort by PK order + dedup edges
        all_nodes.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        all_edges.sort_unstable_by(|a, b| {
            (&a.source, &a.target, a.kind.as_str(), &a.line).cmp(&(
                &b.source,
                &b.target,
                b.kind.as_str(),
                &b.line,
            ))
        });
        all_edges.dedup_by(|a, b| {
            a.source == b.source && a.target == b.target && a.kind == b.kind && a.line == b.line
        });
        file_records.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        let total_edges = all_edges.len();

        // 7. Bulk-insert via prepared statements (zero SQL re-parsing)
        let phase_start = Instant::now();
        self.db.insert_nodes(&all_nodes).await?;
        self.db.insert_edges(&all_edges).await?;
        self.db.upsert_files(&file_records).await?;

        // 8. Restore indexes and normal durability
        self.db.end_bulk_load().await?;
        on_verbose(&format!(
            "wrote to database in {:.1}s",
            phase_start.elapsed().as_secs_f64()
        ));

        let duration_ms = start.elapsed().as_millis() as u64;
        let now_str = current_timestamp().to_string();
        self.db.set_metadata("last_full_sync_at", &now_str).await?;
        self.db.set_metadata("last_sync_at", &now_str).await?;
        self.db
            .set_metadata("last_sync_duration_ms", &duration_ms.to_string())
            .await?;

        let result = IndexResult {
            file_count: files.len(),
            node_count: total_nodes,
            edge_count: total_edges,
            duration_ms,
        };
        debug_assert!(
            result.node_count >= result.file_count || result.file_count == 0,
            "fewer nodes than files is unexpected"
        );
        debug_assert!(
            result.duration_ms > 0 || result.file_count == 0,
            "non-empty index completed in zero milliseconds"
        );
        clear_dirty_sentinel(&self.project_root);
        Ok(result)
    }

    /// Performs an incremental sync: detects changed, new, and removed files
    /// and re-indexes only those that need updating.
    pub async fn sync(&self) -> Result<SyncResult> {
        self.sync_with_progress(|_, _, _| {}).await
    }

    /// Like `sync()`, but calls `on_progress` for spinner updates.
    /// Equivalent to `sync_with_progress_verbose(on_progress, |_| {})`.
    pub async fn sync_with_progress<F>(&self, on_progress: F) -> Result<SyncResult>
    where
        F: Fn(usize, usize, &str),
    {
        self.sync_with_progress_verbose(on_progress, |_| {}).await
    }

    /// Sync only the specified files if they are stale, then recheck.
    ///
    /// Returns `Ok(false)` if all files are now in sync after the call.
    /// Returns `Ok(true)` if files are still stale after sync (either sync
    /// didn't update these specific files, or sync failed to acquire lock).
    /// Returns `Err` on sync failure.
    pub async fn sync_if_stale(&self, stale_files: &[String]) -> Result<bool> {
        if stale_files.is_empty() {
            return Ok(false);
        }
        // Normalize once at the entry; downstream helpers can rely on
        // forward-slash form matching the walker's canonical path
        // (defends against #87 — Windows duplicate-row corruption).
        let stale_files = normalize_rel_paths(stale_files);

        let still_stale_before = self.check_file_staleness(&stale_files).await;
        if still_stale_before.is_empty() {
            return Ok(false);
        }

        let Ok(lock) = try_acquire_sync_lock(&self.project_root) else {
            return Ok(true);
        };

        let result = self.sync_single_files(&stale_files).await;
        drop(lock);

        match result {
            Ok(()) => {
                let still_stale_after = self.check_file_staleness(&stale_files).await;
                Ok(!still_stale_after.is_empty())
            }
            Err(_) => Ok(true),
        }
    }

    /// Like `sync_if_stale` but treats lock contention as success.
    ///
    /// Use this from the embedded MCP watcher when another MCP (or any peer
    /// process) already holds the project sync lock. If the peer holds the
    /// lock, wait (bounded) for it to release so the DB is fresh by the time
    /// the caller refreshes its view; if the peer covered our files, return
    /// without doing extra work, otherwise sync ourselves.
    pub async fn sync_if_stale_silent(&self, stale_files: &[String]) -> Result<()> {
        if stale_files.is_empty() {
            return Ok(());
        }
        // Normalize once at the entry — see `sync_if_stale` and #87.
        let stale_files = normalize_rel_paths(stale_files);

        let still_stale_before = self.check_file_staleness(&stale_files).await;
        if still_stale_before.is_empty() {
            return Ok(());
        }

        let lock = if let Ok(lock) = try_acquire_sync_lock(&self.project_root) {
            lock
        } else {
            // Peer is syncing. Wait for them to release the lock so the
            // caller (e.g. the embedded watcher's refresh hook) sees the
            // post-sync DB state — returning early here leaves the caller
            // refreshing against pre-sync data and silently dropping the
            // update on the floor.
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if Instant::now() >= deadline {
                    // Peer is stuck or crashed — best-effort, give up.
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
                if let Ok(lock) = try_acquire_sync_lock(&self.project_root) {
                    // Peer released. If they covered our files, the DB is
                    // fresh and we're done; otherwise sync ourselves.
                    let still_stale = self.check_file_staleness(&stale_files).await;
                    if still_stale.is_empty() {
                        drop(lock);
                        return Ok(());
                    }
                    break lock;
                }
            }
        };

        let _ = self.sync_single_files(&stale_files).await;
        drop(lock);
        Ok(())
    }

    /// Index/reexamine the given file paths, updating their graph nodes and edges.
    /// This is a focused, single-shot operation used by `sync_if_stale`.
    pub(crate) async fn sync_single_files(&self, file_paths: &[String]) -> Result<()> {
        use crate::sync as sync_mod;

        let start = Instant::now();
        let project_root = &self.project_root;
        let registry = &self.registry;

        // Defence-in-depth: even though the public `sync_if_stale[_silent]`
        // entry points already normalize, this is the single chokepoint
        // where paths get written to the DB — so we normalize again here
        // in case a future internal caller skips the wrappers. The DB's
        // canonical form is forward-slash (#87).
        let file_paths = normalize_rel_paths(file_paths);

        // Files deleted from disk produce no extraction, so the replace-on-
        // reindex path below would never drop their rows — prune them here,
        // mirroring the removal branch of the full sync (#108).
        let mut existing: Vec<String> = Vec::with_capacity(file_paths.len());
        for path in file_paths {
            if project_root.join(&path).exists() {
                existing.push(path);
            } else {
                self.db.delete_file(&path).await?;
            }
        }
        let file_paths = existing;

        // Read and hash the files
        let mut hash_map: HashMap<String, String> = HashMap::new();
        let mut stat_map: HashMap<String, (i64, u64)> = HashMap::new();

        for path in &file_paths {
            let abs_path = project_root.join(path);
            if let Some((mtime, size)) = sync_mod::file_stat(&abs_path) {
                stat_map.insert(path.clone(), (mtime, size));
            }
            if let Ok(source) = sync_mod::read_source_file(&abs_path) {
                let hash = sync_mod::content_hash(&source);
                hash_map.insert(path.clone(), hash);
            }
        }

        // Extract graph data from the files in parallel (subprocess-isolated)
        let _ = stat_map; // worker re-stats internally; map kept for potential future use
        let (sync_extractions, _skipped_extractions) =
            extract_files_isolated(project_root, registry, file_paths.clone());

        // Phase 1: insert all nodes (and metadata) so cross-file edges
        // can reference them. Edges are queued for phase 2 (#58).
        let mut queued_edges: Vec<&Edge> = Vec::new();
        for (file_path, result, hash, size, mtime) in &sync_extractions {
            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            queued_edges.extend(&result.edges);
            if !result.unresolved_refs.is_empty() {
                self.db
                    .insert_unresolved_refs(&result.unresolved_refs)
                    .await?;
            }

            let file_record = FileRecord {
                path: (*file_path).clone(),
                content_hash: (*hash).clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record).await?;
        }

        // Phase 2: insert all queued edges now that every node is present.
        // The conditional INSERT in `insert_edges` silently skips edges
        // whose endpoints are truly missing (e.g. unindexed files).
        if !queued_edges.is_empty() {
            let owned: Vec<Edge> = queued_edges.into_iter().cloned().collect();
            self.db.insert_edges(&owned).await?;
        }

        // Resolve references for any new/changed unresolved refs
        if !file_paths.is_empty() {
            let all_nodes = self.db.get_all_nodes().await.unwrap_or_default();
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            let unresolved = self.db.get_unresolved_refs().await?;
            if !unresolved.is_empty() {
                let resolution = resolver.resolve_all(&unresolved);
                let edges = resolver.create_edges(&resolution.resolved);
                if !edges.is_empty() {
                    self.db.insert_edges(&edges).await?;
                    // Re-propagate build-variant call edges over the full graph
                    // now that new call edges exist (#141).
                    let all_db_edges = self.db.get_all_edges().await.unwrap_or_default();
                    let variant_edges =
                        crate::resolution::propagate_variant_edges(&all_nodes, &all_db_edges);
                    if !variant_edges.is_empty() {
                        self.db.insert_edges(&variant_edges).await?;
                    }
                }
            }
        }

        self.db
            .set_metadata("last_sync_at", &current_timestamp().to_string())
            .await?;
        self.db
            .set_metadata(
                "last_sync_duration_ms",
                &start.elapsed().as_millis().to_string(),
            )
            .await?;

        clear_dirty_sentinel(&self.project_root);
        Ok(())
    }

    /// Like `sync()`, but calls `on_progress` with a description and the
    /// current step for each phase of work, and `on_verbose` after each phase
    /// completes with a diagnostic summary line (count + timing).
    ///
    /// The progress callback receives `(current_file_index, total_files, message)`
    /// where `current_file_index` and `total_files` are zero during non-file phases
    /// (scanning, hashing, detecting, resolving) and populated during the
    /// per-file syncing phase.
    pub async fn sync_with_progress_verbose<F, V>(
        &self,
        on_progress: F,
        on_verbose: V,
    ) -> Result<SyncResult>
    where
        F: Fn(usize, usize, &str),
        V: Fn(&str),
    {
        debug_assert!(
            self.project_root.exists(),
            "sync: project root does not exist"
        );
        debug_assert!(
            self.project_root.is_dir(),
            "sync: project root is not a directory"
        );
        let _lock = try_acquire_sync_lock(&self.project_root)?;
        write_dirty_sentinel(&self.project_root);
        let start = Instant::now();

        on_progress(0, 0, "scanning files");
        let phase_start = Instant::now();
        let current_files = self.scan_files();
        on_verbose(&format!(
            "scanned {} files in {:.1}s",
            current_files.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // Stat all files in parallel to get (mtime, size) — ~11ms for 20k files
        on_progress(0, 0, "checking file timestamps");
        let phase_start = Instant::now();
        let project_root = &self.project_root;
        let file_stats: Vec<(String, i64, u64)> = current_files
            .par_iter()
            .filter_map(|path| {
                let abs_path = project_root.join(path);
                let (mtime, size) = sync::file_stat(&abs_path)?;
                Some((path.clone(), mtime, size))
            })
            .collect();
        on_verbose(&format!(
            "stat-checked {} files in {:.1}s",
            file_stats.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // Load all DB file records into a map for O(1) lookups
        let db_files = self.db.get_all_files().await?;
        let db_map: HashMap<String, FileRecord> =
            db_files.into_iter().map(|f| (f.path.clone(), f)).collect();

        // Partition files by comparing (mtime, size) against stored values
        let mut new_files: Vec<String> = Vec::new();
        let mut stat_changed: Vec<String> = Vec::new();
        let mut current_set: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(file_stats.len());
        let mut stat_map: HashMap<String, (i64, u64)> = HashMap::with_capacity(file_stats.len());

        for (path, mtime, size) in &file_stats {
            current_set.insert(path.as_str());
            stat_map.insert(path.clone(), (*mtime, *size));
            match db_map.get(path) {
                None => new_files.push(path.clone()),
                Some(record) => {
                    if record.modified_at != *mtime || record.size != *size {
                        stat_changed.push(path.clone());
                    }
                }
            }
        }

        // Detect removed files from the same DB map
        let removed: Vec<String> = db_map
            .keys()
            .filter(|path| !current_set.contains(path.as_str()))
            .cloned()
            .collect();

        on_verbose(&format!(
            "changes: {} new, {} stat-changed, {} removed, {} unchanged",
            new_files.len(),
            stat_changed.len(),
            removed.len(),
            file_stats.len() - new_files.len() - stat_changed.len()
        ));

        // Read + hash only files with changed stats or new files
        on_progress(0, 0, "hashing changed files");
        let phase_start = Instant::now();
        let needs_read: Vec<&String> = new_files.iter().chain(stat_changed.iter()).collect();
        let hash_results: Vec<_> = needs_read
            .par_iter()
            .map(|path| {
                let abs_path = project_root.join(path.as_str());
                match sync::read_source_file(&abs_path) {
                    Ok(source) => Ok(((*path).clone(), sync::content_hash(&source))),
                    Err(e) => Err(((*path).clone(), e.to_string())),
                }
            })
            .collect();

        let mut skipped: Vec<(String, String)> = Vec::new();
        let mut hash_map: HashMap<String, String> = HashMap::new();
        for result in hash_results {
            match result {
                Ok((path, hash)) => {
                    hash_map.insert(path, hash);
                }
                Err((path, reason)) => {
                    skipped.push((path, reason));
                }
            }
        }
        on_verbose(&format!(
            "hashed {} files in {:.1}s ({} read errors)",
            hash_map.len(),
            phase_start.elapsed().as_secs_f64(),
            skipped.len()
        ));

        // Among stat_changed files, find those with actually different content
        on_progress(0, 0, "detecting changes");
        let mut stale: Vec<String> = Vec::new();
        let mut mtime_only_changed: Vec<String> = Vec::new();
        for path in &stat_changed {
            if let Some(new_hash) = hash_map.get(path) {
                if let Some(record) = db_map.get(path) {
                    if record.content_hash == *new_hash {
                        // mtime changed but content identical (e.g. touch) —
                        // update stored mtime so we skip it next time
                        mtime_only_changed.push(path.clone());
                    } else {
                        stale.push(path.clone());
                    }
                }
            }
        }
        on_verbose(&format!(
            "content check: {} modified, {} mtime-only",
            stale.len(),
            mtime_only_changed.len()
        ));

        // Update mtime for false-positive files so future syncs skip them
        for path in &mtime_only_changed {
            if let (Some(record), Some(&(mtime, size))) = (db_map.get(path), stat_map.get(path)) {
                let updated = FileRecord {
                    modified_at: mtime,
                    size,
                    ..record.clone()
                };
                self.db.upsert_file(&updated).await?;
            }
        }

        // Remove deleted files
        for path in &removed {
            on_progress(0, 0, &format!("removing {path}"));
            self.db.delete_file(path).await?;
        }

        // Re-index stale and new files — extract in parallel, insert sequentially
        let to_index: Vec<String> = stale.iter().chain(new_files.iter()).cloned().collect();
        let registry = &self.registry;

        let phase_start = Instant::now();
        let _ = stat_map; // worker re-stats internally
        let (sync_extractions, sync_skipped): (Vec<_>, Vec<_>) =
            extract_files_isolated(project_root, registry, to_index.clone());
        // Surface extractor timeouts/crashes in `SyncResult.skipped_paths`
        // so the user can see them in `tokensave sync --doctor`.
        skipped.extend(sync_skipped);

        // Phase 1: insert all nodes (and metadata) so cross-file edges
        // can reference them. Edges are queued for phase 2 (#58).
        let total = sync_extractions.len();
        let mut total_nodes = 0usize;
        let mut total_edges = 0usize;
        let mut queued_edges: Vec<&Edge> = Vec::new();
        for (idx, (file_path, result, hash, size, mtime)) in sync_extractions.iter().enumerate() {
            on_progress(idx + 1, total, file_path);

            total_nodes += result.nodes.len();
            total_edges += result.edges.len();

            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            queued_edges.extend(&result.edges);
            if !result.unresolved_refs.is_empty() {
                self.db
                    .insert_unresolved_refs(&result.unresolved_refs)
                    .await?;
            }

            let file_record = FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
            };
            self.db.upsert_file(&file_record).await?;
        }

        // Phase 2: insert all queued edges now that every node is present.
        if !queued_edges.is_empty() {
            let owned: Vec<Edge> = queued_edges.into_iter().cloned().collect();
            self.db.insert_edges(&owned).await?;
        }

        if !to_index.is_empty() {
            on_verbose(&format!(
                "indexed {} files ({} nodes, {} edges) in {:.1}s",
                to_index.len(),
                total_nodes,
                total_edges,
                phase_start.elapsed().as_secs_f64()
            ));
        }

        // Resolve references (call edges, uses, etc.) across all files.
        // This must run after all files are indexed so cross-file references
        // can find their targets.
        if !to_index.is_empty() {
            on_progress(0, 0, "resolving references");
            let phase_start = Instant::now();
            let unresolved = self.db.get_unresolved_refs().await?;
            if !unresolved.is_empty() {
                let all_nodes = self.db.get_all_nodes().await.unwrap_or_default();
                let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
                let resolution = resolver.resolve_all(&unresolved);
                let edges = resolver.create_edges(&resolution.resolved);
                if !edges.is_empty() {
                    self.db.insert_edges(&edges).await?;
                    // Propagate call edges across build-config variants (#141).
                    let all_db_edges = self.db.get_all_edges().await.unwrap_or_default();
                    let variant_edges =
                        crate::resolution::propagate_variant_edges(&all_nodes, &all_db_edges);
                    if !variant_edges.is_empty() {
                        self.db.insert_edges(&variant_edges).await?;
                    }
                }
            }
            on_verbose(&format!(
                "resolved {} references in {:.1}s",
                unresolved.len(),
                phase_start.elapsed().as_secs_f64()
            ));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        self.db
            .set_metadata("last_sync_at", &current_timestamp().to_string())
            .await?;
        self.db
            .set_metadata("last_sync_duration_ms", &duration_ms.to_string())
            .await?;

        clear_dirty_sentinel(&self.project_root);
        Ok(SyncResult {
            files_added: new_files.len(),
            files_modified: stale.len(),
            files_removed: removed.len(),
            duration_ms,
            added_paths: new_files,
            modified_paths: stale,
            skipped_paths: skipped,
            removed_paths: removed,
        })
    }

    /// Scans the project root for source files in all supported languages,
    /// respecting the configured exclude patterns and max file size.
    ///
    /// When `git_ignore` is enabled in the config, `.gitignore` rules are
    /// applied via the `ignore` crate. Otherwise, hidden directories and
    /// `target/` are skipped with a simple name-based filter.
    ///
    /// Supported extensions are derived from the `LanguageRegistry` so that
    /// adding a new extractor automatically picks up its files.
    pub(crate) fn scan_files(&self) -> Vec<String> {
        debug_assert!(
            self.project_root.is_dir(),
            "scan_files: project_root is not a directory"
        );
        let supported_exts = self.registry.supported_extensions();
        debug_assert!(
            !supported_exts.is_empty(),
            "scan_files: no supported extensions registered"
        );

        if self.config.git_ignore {
            let files = self.scan_files_with_gitignore(&supported_exts);
            if files.is_empty() {
                // The project directory may be gitignored by a parent repo,
                // causing the ignore-aware walker to skip everything. Fall
                // back to plain walkdir if source files clearly exist.
                let has_source = WalkDir::new(&self.project_root)
                    .follow_links(true)
                    .max_depth(2)
                    .into_iter()
                    .filter_map(std::result::Result::ok)
                    .any(|e| {
                        e.file_type().is_file()
                            && e.path()
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| supported_exts.contains(&ext))
                    });
                if has_source {
                    eprintln!("warning: gitignore-aware scan found no files; falling back to plain walk (project may be gitignored by parent repo)");
                    return self.scan_files_walkdir(&supported_exts);
                }
            }
            files
        } else {
            self.scan_files_walkdir(&supported_exts)
        }
    }

    /// Walk using `walkdir`, skipping hidden directories and `target/`.
    ///
    /// Hidden (dot-prefixed) entries that match a configured `include` glob
    /// are allowed through despite the default filter.
    pub(crate) fn scan_files_walkdir(&self, supported_exts: &[&str]) -> Vec<String> {
        let mut files = Vec::new();
        let root = &self.project_root;
        let config = &self.config;
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || name == "target" {
                    // Allow if the relative path matches an include glob.
                    if let Ok(rel) = e.path().strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        return is_included(&rel_str, config);
                    }
                    return false;
                }
                // Prune directories covered by an exclude glob before descending.
                // This prevents entering large trees (e.g. node_modules) and
                // avoids following symlinks that cycle back into source directories.
                if e.file_type().is_dir() {
                    if let Ok(rel) = e.path().strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if is_excluded_dir(&rel_str, config) {
                            return false;
                        }
                    }
                }
                true
            })
        {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts) {
                files.push(rel_str);
            }
        }
        files
    }

    /// Walk using the `ignore` crate, which respects `.gitignore` rules,
    /// `.git/info/exclude`, and the user's global gitignore.
    ///
    /// `git_ignore(true)` alone only reads nested `.gitignore` files when a
    /// `.git` directory is reachable from the walk root (it relies on git repo
    /// discovery). `add_custom_ignore_filename(".gitignore")` makes the crate
    /// additionally treat every `.gitignore` it encounters as a standalone
    /// ignore file, ensuring nested rules are applied even outside a git repo.
    ///
    /// When `include` globs are configured, the crate's built-in hidden filter
    /// is disabled and hidden entries are filtered manually so that included
    /// dot-paths can pass through.
    pub(crate) fn scan_files_with_gitignore(&self, supported_exts: &[&str]) -> Vec<String> {
        let has_includes = !self.config.include.is_empty();
        let mut files = Vec::new();
        let walker = ignore::WalkBuilder::new(&self.project_root)
            .follow_links(true)
            .hidden(!has_includes) // disable when we need to check includes
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".gitignore")
            .build();

        for entry in walker {
            let Ok(entry) = entry else { continue };
            let Some(ft) = entry.file_type() else {
                continue;
            };

            // When we disabled the crate's hidden filter, manually skip hidden
            // entries that don't match an include glob.
            if has_includes && entry.depth() > 0 {
                let name = entry.file_name().to_string_lossy();
                if name.starts_with('.') {
                    if let Ok(rel) = entry.path().strip_prefix(&self.project_root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if !is_included(&rel_str, &self.config) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
            }

            if !ft.is_file() {
                continue;
            }
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts) {
                files.push(rel_str);
            }
        }
        files
    }

    /// Checks whether a file should be included: correct extension, not
    /// excluded by config globs, and within the max file size.
    pub(crate) fn accept_file(&self, path: &Path, supported_exts: &[&str]) -> Option<String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !supported_exts.contains(&ext) {
            return None;
        }
        let relative = path.strip_prefix(&self.project_root).ok()?;
        // Normalize to forward slashes so paths are consistent across
        // platforms and between different directory walkers on Windows.
        let rel_str = relative.to_string_lossy().replace('\\', "/");
        if is_excluded(&rel_str, &self.config) {
            return None;
        }
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > self.config.max_file_size {
            return None;
        }
        Some(rel_str)
    }

    /// Resolves a path to a relative path string.
    /// If the path is already relative, returns it as-is.
    /// If absolute, strips the `project_root` prefix.
    pub(crate) fn resolve_path(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        if path.is_absolute() {
            let relative = path.strip_prefix(&self.project_root).ok()?;
            Some(relative.to_string_lossy().replace('\\', "/"))
        } else {
            Some(path.to_string_lossy().replace('\\', "/"))
        }
    }

    /// Gets the absolute path for a relative path.
    pub(crate) fn absolute_path(&self, relative_path: &str) -> PathBuf {
        self.project_root.join(relative_path)
    }

    /// Re-indexes a single file after an edit.
    pub(crate) async fn reindex_file(&self, file_path: &str) -> Result<()> {
        let abs_path = self.absolute_path(file_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read file {file_path}: {e}"),
        })?;

        let Some(extractor) = self.registry.extractor_for_file(file_path) else {
            return Ok(());
        };

        let mut result =
            safe_extract(extractor, file_path, &source).ok_or_else(|| TokenSaveError::Config {
                message: format!("extraction panicked for {file_path}"),
            })?;
        result.sanitize();

        let hash = sync::content_hash(&source);
        let size = source.len() as u64;
        let mtime = sync::file_stat(&abs_path).map_or_else(current_timestamp, |(m, _)| m);

        self.db.delete_nodes_by_file(file_path).await?;
        self.db.insert_nodes(&result.nodes).await?;
        self.db.insert_edges(&result.edges).await?;
        if !result.unresolved_refs.is_empty() {
            self.db
                .insert_unresolved_refs(&result.unresolved_refs)
                .await?;
        }

        let file_record = FileRecord {
            path: file_path.to_string(),
            content_hash: hash,
            size,
            modified_at: mtime,
            indexed_at: current_timestamp(),
            node_count: result.nodes.len() as u32,
        };
        self.db.upsert_file(&file_record).await?;

        Ok(())
    }

    /// Performs a single string replacement.
    /// Fails if `old_str` is not found or matches more than once.
    pub async fn str_replace(
        &self,
        path: &str,
        old_str: &str,
        new_str: &str,
    ) -> Result<EditResult> {
        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {path}: {e}"),
        })?;

        let matches: Vec<_> = source.match_indices(old_str).collect();
        match matches.len() {
            0 => {
                return Ok(EditResult {
                    success: false,
                    file_path: rel_path.clone(),
                    matched_str: old_str.to_string(),
                    new_str: new_str.to_string(),
                    message: format!("old_str not found in {path}"),
                })
            }
            1 => {}
            n => {
                return Ok(EditResult {
                    success: false,
                    file_path: rel_path.clone(),
                    matched_str: old_str.to_string(),
                    new_str: new_str.to_string(),
                    message: format!("old_str matches {n} times, must match exactly once"),
                })
            }
        }

        let modified = source.replacen(old_str, new_str, 1);

        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {path}: {e}"),
            })?;

        self.reindex_file(&rel_path).await?;

        Ok(EditResult {
            success: true,
            file_path: rel_path,
            matched_str: old_str.to_string(),
            new_str: new_str.to_string(),
            message: "replacement successful".to_string(),
        })
    }

    /// Applies multiple string replacements atomically.
    /// Fails if any `old_str` doesn't match exactly once.
    pub async fn multi_str_replace(
        &self,
        path: &str,
        replacements: &[(&str, &str)],
    ) -> Result<MultiEditResult> {
        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {path}: {e}"),
        })?;

        for (old, _) in replacements {
            let count = source.matches(old).count();
            if count != 1 {
                return Ok(MultiEditResult {
                    success: false,
                    file_path: rel_path.clone(),
                    applied_count: 0,
                    message: format!(
                        "replacement '{}' matches {} times, must match exactly once",
                        crate::text::utf8_prefix_at_or_before(old, 20),
                        count
                    ),
                });
            }
        }

        let mut modified = source;
        for (old, new) in replacements {
            modified = modified.replacen(old, new, 1);
        }

        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {path}: {e}"),
            })?;

        self.reindex_file(&rel_path).await?;

        Ok(MultiEditResult {
            success: true,
            file_path: rel_path,
            applied_count: replacements.len(),
            message: format!("applied {} replacements", replacements.len()),
        })
    }

    /// Inserts content before or after a unique anchor.
    /// Anchor can be a string or 1-indexed line number.
    pub async fn insert_at(
        &self,
        path: &str,
        anchor: &str,
        content: &str,
        before: bool,
    ) -> Result<InsertResult> {
        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {path}: {e}"),
        })?;

        let lines: Vec<&str> = source.lines().collect();

        let anchor_line = if anchor.chars().all(|c| c.is_ascii_digit()) {
            let line_num: usize = anchor.parse().map_err(|_| TokenSaveError::Config {
                message: format!("invalid line number: {anchor}"),
            })?;
            if line_num == 0 || line_num > lines.len() {
                return Ok(InsertResult {
                    success: false,
                    file_path: rel_path.clone(),
                    anchor_line: line_num as u32,
                    content: content.to_string(),
                    before,
                    message: format!(
                        "line number {line_num} out of range (file has {} lines)",
                        lines.len()
                    ),
                });
            }
            line_num - 1
        } else {
            let anchor_prefix = crate::text::utf8_prefix_at_or_before(anchor, 100);
            let matching_lines: Vec<usize> = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains(anchor_prefix))
                .map(|(i, _)| i)
                .collect();

            if matching_lines.is_empty() {
                return Ok(InsertResult {
                    success: false,
                    file_path: rel_path.clone(),
                    anchor_line: 0,
                    content: content.to_string(),
                    before,
                    message: format!("anchor '{anchor}' not found"),
                });
            }
            if matching_lines.len() > 1 {
                return Ok(InsertResult {
                    success: false,
                    file_path: rel_path.clone(),
                    anchor_line: matching_lines.len() as u32,
                    content: content.to_string(),
                    before,
                    message: format!(
                        "anchor '{anchor}' matches {} lines, must match exactly one",
                        matching_lines.len()
                    ),
                });
            }
            matching_lines[0]
        };

        let insert_idx = if before { anchor_line } else { anchor_line + 1 };
        let mut new_lines: Vec<&str> = lines[..insert_idx].to_vec();
        new_lines.push(content);
        new_lines.extend_from_slice(&lines[insert_idx..]);
        let mut modified = new_lines.join("\n");
        if source.ends_with('\n') {
            modified.push('\n');
        }

        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {path}: {e}"),
            })?;

        self.reindex_file(&rel_path).await?;

        Ok(InsertResult {
            success: true,
            file_path: rel_path,
            anchor_line: (anchor_line + 1) as u32,
            content: content.to_string(),
            before,
            message: format!("inserted at line {}", anchor_line + 1),
        })
    }

    /// Replaces the full source of a named symbol (function, method, struct,
    /// etc.) with `new_source`. Resolves the symbol via exact qualified-name
    /// match — if the name is ambiguous, callable definitions win; if still
    /// ambiguous after that filter, the edit is refused so we don't clobber
    /// the wrong site.
    pub async fn replace_symbol(&self, symbol: &str, new_source: &str) -> Result<EditResult> {
        let target = resolve_symbol_for_edit(self, symbol).await?;
        let rel_path = target.file_path.clone();
        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {rel_path}: {e}"),
        })?;
        let lines: Vec<&str> = source.lines().collect();
        let start = target.start_line as usize;
        let end_inclusive = (target.end_line as usize).min(lines.len().saturating_sub(1));
        if start >= lines.len() || start > end_inclusive {
            return Ok(EditResult {
                success: false,
                file_path: rel_path,
                matched_str: symbol.to_string(),
                new_str: String::new(),
                message: format!(
                    "symbol range [{}..={}] out of bounds for {}-line file",
                    target.start_line,
                    target.end_line,
                    lines.len()
                ),
            });
        }
        let trailing_newline = source.ends_with('\n');
        let mut rebuilt: Vec<String> = Vec::with_capacity(lines.len());
        rebuilt.extend(lines[..start].iter().map(|s| (*s).to_string()));
        rebuilt.push(new_source.trim_end_matches('\n').to_string());
        rebuilt.extend(lines[end_inclusive + 1..].iter().map(|s| (*s).to_string()));
        let mut modified = rebuilt.join("\n");
        if trailing_newline {
            modified.push('\n');
        }
        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {rel_path}: {e}"),
            })?;
        self.reindex_file(&rel_path).await?;
        Ok(EditResult {
            success: true,
            file_path: rel_path,
            matched_str: format!("{} ({})", target.name, target.kind.as_str()),
            new_str: new_source.to_string(),
            message: format!(
                "replaced {}:{}-{}",
                target.file_path,
                target.start_line + 1,
                target.end_line + 1
            ),
        })
    }

    /// Inserts `content` immediately before or after a named symbol. `position`
    /// is one of `"before"` or `"after"`. Uses the same resolution logic as
    /// `replace_symbol`.
    pub async fn insert_at_symbol(
        &self,
        symbol: &str,
        content: &str,
        position: &str,
    ) -> Result<InsertResult> {
        let before = match position {
            "before" => true,
            "after" => false,
            other => {
                return Err(TokenSaveError::Config {
                    message: format!("position must be \"before\" or \"after\", got {other:?}"),
                });
            }
        };
        let target = resolve_symbol_for_edit(self, symbol).await?;
        let rel_path = target.file_path.clone();
        let abs_path = self.absolute_path(&rel_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {rel_path}: {e}"),
        })?;
        let lines: Vec<&str> = source.lines().collect();
        let anchor_line = if before {
            target.start_line as usize
        } else {
            (target.end_line as usize).saturating_add(1)
        };
        if anchor_line > lines.len() {
            return Ok(InsertResult {
                success: false,
                file_path: rel_path,
                anchor_line: anchor_line as u32,
                content: content.to_string(),
                before,
                message: format!("anchor line {anchor_line} past EOF ({})", lines.len()),
            });
        }
        let trailing_newline = source.ends_with('\n');
        let mut rebuilt: Vec<String> = Vec::with_capacity(lines.len() + 1);
        rebuilt.extend(lines[..anchor_line].iter().map(|s| (*s).to_string()));
        rebuilt.push(content.trim_end_matches('\n').to_string());
        rebuilt.extend(lines[anchor_line..].iter().map(|s| (*s).to_string()));
        let mut modified = rebuilt.join("\n");
        if trailing_newline {
            modified.push('\n');
        }
        tokio::fs::write(&abs_path, &modified)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to write {rel_path}: {e}"),
            })?;
        self.reindex_file(&rel_path).await?;
        Ok(InsertResult {
            success: true,
            file_path: rel_path,
            anchor_line: (anchor_line + 1) as u32,
            content: content.to_string(),
            before,
            message: format!(
                "inserted {} {} ({}) at line {}",
                position,
                target.name,
                target.kind.as_str(),
                anchor_line + 1
            ),
        })
    }

    /// Performs structural rewrite using ast-grep CLI.
    pub async fn ast_grep_rewrite(
        &self,
        path: &str,
        pattern: &str,
        rewrite: &str,
    ) -> Result<AstGrepResult> {
        use std::process::Command;

        let rel_path = self
            .resolve_path(path)
            .ok_or_else(|| TokenSaveError::Config {
                message: "path is not within the project".to_string(),
            })?;

        let abs_path = self.absolute_path(&rel_path);

        let check_output = Command::new("ast-grep").args(["--version"]).output();

        if check_output.is_err() {
            if can_use_literal_rewrite_fallback(pattern) {
                let mut source = std::fs::read_to_string(&abs_path).map_err(TokenSaveError::Io)?;
                if !source.contains(pattern) {
                    return Ok(AstGrepResult {
                        success: false,
                        file_path: rel_path.clone(),
                        pattern: pattern.to_string(),
                        rewrite: rewrite.to_string(),
                        message: "pattern not found (built-in literal fallback)".to_string(),
                    });
                }
                source = source.replace(pattern, rewrite);
                std::fs::write(&abs_path, source).map_err(TokenSaveError::Io)?;
                self.reindex_file(&rel_path).await?;
                return Ok(AstGrepResult {
                    success: true,
                    file_path: rel_path,
                    pattern: pattern.to_string(),
                    rewrite: rewrite.to_string(),
                    message: "literal rewrite completed using built-in fallback".to_string(),
                });
            }
            return Ok(AstGrepResult {
                success: false,
                file_path: rel_path.clone(),
                pattern: pattern.to_string(),
                rewrite: rewrite.to_string(),
                message: "ast-grep is not installed and this pattern needs SGPattern matching. Simple literal rewrites are handled by the built-in fallback.".to_string(),
            });
        }

        let output = Command::new("ast-grep")
            .args([
                "run",
                "-p",
                pattern,
                "-r",
                rewrite,
                "-U",
                abs_path.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|e| TokenSaveError::Config {
                message: format!("failed to run ast-grep: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr_trim = stderr.trim();
            let stdout_trim = stdout.trim();
            let exit = output
                .status
                .code()
                .map_or_else(|| "killed by signal".to_string(), |c| c.to_string());
            let message = if !stderr_trim.is_empty() {
                format!("ast-grep failed (exit {exit}): {stderr_trim}")
            } else if !stdout_trim.is_empty() {
                format!("ast-grep failed (exit {exit}). stdout: {stdout_trim}")
            } else {
                format!(
                    "ast-grep failed (exit {exit}) with no output. Likely causes: \
                     pattern matched 0 nodes, language not inferred from file extension \
                     (e.g. .txt has no parser), or invalid pattern syntax. \
                     File: {rel_path}, pattern: {pattern:?}"
                )
            };
            return Ok(AstGrepResult {
                success: false,
                file_path: rel_path.clone(),
                pattern: pattern.to_string(),
                rewrite: rewrite.to_string(),
                message,
            });
        }

        self.reindex_file(&rel_path).await?;

        Ok(AstGrepResult {
            success: true,
            file_path: rel_path,
            pattern: pattern.to_string(),
            rewrite: rewrite.to_string(),
            message: "ast-grep rewrite completed".to_string(),
        })
    }
}

pub(crate) fn can_use_literal_rewrite_fallback(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    !trimmed.is_empty()
        && trimmed == pattern
        && !pattern.contains('$')
        && !pattern.contains('\n')
        && !pattern.contains('\r')
}
