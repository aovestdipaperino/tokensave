//! Full and incremental indexing.
use super::query::resolve_symbol_for_edit;
use super::*;

const RUBY_SINGLETON_KIND_METADATA: &str = "ruby_singleton_method_kind_v1";

fn legacy_ruby_repair_complete(
    repair_required: bool,
    scheduled: &[String],
    extracted: &HashSet<&str>,
) -> bool {
    !repair_required
        || scheduled
            .iter()
            .all(|path| extracted.contains(path.as_str()))
}

/// Extensions that are never source code — binary assets, media, archives,
/// lockfiles, and plain-data formats. These are excluded from the
/// skipped-extension diagnostic (#262, #270) so a verbose sync highlights
/// genuinely unsupported languages instead of `.png` / `.lock` noise.
const NON_SOURCE_EXTS: &[&str] = &[
    // Images / fonts
    "png",
    "jpg",
    "jpeg",
    "gif",
    "bmp",
    "ico",
    "icns",
    "svg",
    "webp",
    "avif",
    "tiff",
    "psd",
    "woff",
    "woff2",
    "ttf",
    "otf",
    "eot", // Audio / video
    "mp3",
    "mp4",
    "m4a",
    "wav",
    "ogg",
    "flac",
    "avi",
    "mov",
    "webm",
    "mkv",
    // Archives / packages
    "zip",
    "gz",
    "tgz",
    "tar",
    "bz2",
    "xz",
    "zst",
    "7z",
    "rar",
    "jar",
    "war",
    "whl",
    "deb",
    "rpm",
    "dmg",
    "pkg",
    "apk",
    "nupkg",
    "crate", // Compiled / binary artifacts
    "exe",
    "dll",
    "so",
    "dylib",
    "a",
    "o",
    "obj",
    "lib",
    "bin",
    "dat",
    "wasm",
    "class",
    "pyc",
    "pyo",
    "pdb",
    "rlib",
    "rmeta",
    "node",
    "onnx",
    "pt",
    "safetensors",
    // Databases / caches / logs / locks
    "db",
    "sqlite",
    "sqlite3",
    "lock",
    "log",
    "tmp",
    "bak",
    "cache",
    "sum",
    // Documents
    "pdf",
    "doc",
    "docx",
    "xls",
    "xlsx",
    "ppt",
    "pptx",
    "odt",
    "rtf",
    // Plain data / config formats (structured data, not code)
    "json",
    "jsonl",
    "yaml",
    "yml",
    "xml",
    "plist",
    "csv",
    "tsv",
    "txt",
    "map",
    "min",
    "properties",
    "env",
    "cfg",
    "ini",
    "conf",
];

/// Cap on how many per-extension lines the verbose skipped-extension
/// summary emits; anything beyond the cap is rolled up into one line.
const MAX_SKIPPED_EXT_LINES: usize = 15;

/// Emit one verbose line per skipped extension, e.g.
/// `.mcfunction: 12 file(s) skipped (no registered extractor)`.
fn report_skipped_extensions<V: Fn(&str)>(skipped: &[(String, usize)], on_verbose: &V) {
    for (ext, count) in skipped.iter().take(MAX_SKIPPED_EXT_LINES) {
        on_verbose(&format!(
            ".{ext}: {count} file(s) skipped (no registered extractor)"
        ));
    }
    let rest = skipped.len().saturating_sub(MAX_SKIPPED_EXT_LINES);
    if rest > 0 {
        on_verbose(&format!(
            "…and {rest} more extension(s) skipped (no registered extractor)"
        ));
    }
}

/// Is `target` the project root itself, or one of the root's ancestors?
///
/// Both arguments must already be canonical. Comparison is component-wise, so
/// a sibling that merely shares the root's string prefix (`/home/user2` next to
/// `/home/user`) is not an ancestor.
fn path_contains_root(target: &Path, canonical_root: &Path) -> bool {
    canonical_root.starts_with(target)
}

/// Would descending the directory symlink at `path` re-enter the project root?
///
/// True when the link resolves to the root itself or to one of the root's
/// ancestors, which every Wine prefix produces with `dosdevices/z: -> /`. Such
/// a link only re-exposes paths the walk already covers, plus the whole rest of
/// the filesystem, so the walkers prune it instead of following it (#327). A
/// link into a disjoint tree is not affected and is still followed and indexed
/// (#34). Fails open: if either side cannot be canonicalized, the entry keeps
/// its previous treatment.
fn reenters_project_root(path: &Path, canonical_root: Option<&Path>) -> bool {
    let Some(root) = canonical_root else {
        return false;
    };
    path.canonicalize()
        .is_ok_and(|target| path_contains_root(&target, root))
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

/// Whether incremental reference invalidation is active (#484 phase 5).
///
/// A missed invalidation shows up as a silently absent edge, which no user
/// reports as a bug, so there has to be a way back to resolving everything
/// without waiting for a release: `TOKENSAVE_FULL_RESOLVE=1` makes every sync
/// re-attempt the whole reference table, exactly as it did before #484.
/// `tests/incremental_resolution_test.rs` drives both modes over the same edits
/// and asserts they agree, which is also what makes the flag a real fallback
/// rather than a dead branch.
fn incremental_resolution_enabled() -> bool {
    std::env::var_os("TOKENSAVE_FULL_RESOLVE").is_none()
}

/// The name-index fields of freshly extracted nodes, for the touched set (#484).
fn node_touch_records(nodes: &[Node]) -> Vec<TouchedNode> {
    nodes.iter().map(TouchedNode::from).collect()
}

/// What one streamed resolution pass produced.
///
/// `total` counts the references the table holds; `attempted` counts the ones
/// this pass actually resolved, which an incremental pass narrows to those the
/// sync could have changed the answer for (#484). `attempted_refs` identifies
/// those references, and bounds which ambiguity records may be replaced; it is
/// collected only for an incremental pass, since for a full one it would be the
/// whole table — exactly the materialisation #482 removed.
struct StreamedResolution {
    resolved: Vec<ResolvedRef>,
    ambiguous: Vec<AmbiguousCall>,
    total: usize,
    attempted: usize,
    attempted_refs: Vec<AmbiguityRefKey>,
}

impl TokenSave {
    /// Builds `Doc` nodes and `Documents` edges for companion documentation.
    ///
    /// Discovery uses the already-extracted node set as the source of truth for
    /// what is indexed, so a doc can only ever claim files that made it into
    /// the graph. Reading doc content is best-effort: an unreadable doc is
    /// skipped rather than failing the index.
    fn build_companion_docs(
        &self,
        project_root: &Path,
        all_nodes: &[Node],
    ) -> (Vec<Node>, Vec<Edge>) {
        let mut indexed_files: Vec<String> = Vec::new();
        let mut file_node_ids: HashMap<String, String> = HashMap::new();
        for node in all_nodes {
            if node.kind == NodeKind::File {
                file_node_ids.insert(node.file_path.clone(), node.id.clone());
            }
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for node in all_nodes {
            if seen.insert(node.file_path.as_str()) {
                indexed_files.push(node.file_path.clone());
            }
        }
        indexed_files.sort_unstable();

        let markdown_files: Vec<String> = indexed_files
            .iter()
            .filter(|p| {
                let ext = std::path::Path::new(p.as_str())
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                ext == "md" || ext == "markdown"
            })
            .cloned()
            .collect();
        if markdown_files.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let read_doc = |relative: &str| -> Option<String> {
            std::fs::read_to_string(crate::docs::absolute_doc_path(project_root, relative)).ok()
        };
        let docs = crate::docs::discover_docs(
            &markdown_files,
            &indexed_files,
            &self.config.docs_dir,
            read_doc,
        );
        if docs.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let mut summaries: HashMap<String, String> = HashMap::new();
        for doc in &docs {
            if let Some(content) = read_doc(&doc.path) {
                if let Some(summary) = crate::docs::doc_summary(&content) {
                    summaries.insert(doc.path.clone(), summary);
                }
            }
        }
        crate::docs::build_doc_graph(&docs, &file_node_ids, &summaries)
    }

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
        // Fail loudly on a broken project.json (unknown language, bad glob)
        // instead of silently indexing without the manifest (#194).
        self.validate_manifest()?;
        write_dirty_sentinel(&self.project_root);
        let start = Instant::now();

        // 1. Enter bulk-load mode, then clear existing data. Order matters:
        // `begin_bulk_load` can fail while another process holds the table
        // lock, and clearing first would leave the project with an empty index
        // that the MCP server then keeps serving (#320).
        // Before the destructive step, not after: `clear()` empties the index
        // and a shutdown observed a moment later would leave nothing to serve
        // until the next sync. Leaving here costs the caller nothing (#450).
        crate::cancel::check("index")?;
        self.db.begin_bulk_load().await?;
        self.db.clear().await?;

        // 2. Scan for source files
        let phase_start = Instant::now();
        let (files, skipped_extensions) = self.scan_files_diagnostics();
        let total = files.len();
        on_verbose(&format!(
            "scanned {} files in {:.1}s",
            total,
            phase_start.elapsed().as_secs_f64()
        ));
        report_skipped_extensions(&skipped_extensions, &on_verbose);

        // 3. Parallel extraction: read + parse + hash on all cores
        let project_root = self.project_root.clone();
        let registry = &self.registry;

        let phase_start = Instant::now();
        crate::memstats::record("index:extract");
        let (files, artifact_files) = Self::partition_artifacts(files, &self.artifact_extensions());
        let (extractions, _skipped) =
            extract_files_isolated(&project_root, registry, files.clone());
        // Extraction stops early on a shutdown, so a short result here means
        // abandoned work, not an empty project. Committing it would write a
        // partial graph that looks complete.
        crate::cancel::check("index")?;

        // 4. Collect all data
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();
        let mut all_unresolved = Vec::new();
        let mut file_records = Vec::new();
        let mut body_documents = Vec::new();
        let mut total_nodes = 0;

        for (idx, (file_path, result, hash, size, mtime)) in extractions.iter().enumerate() {
            on_file(idx + 1, total, file_path);
            total_nodes += result.nodes.len();
            all_nodes.extend_from_slice(&result.nodes);
            all_edges.extend_from_slice(&result.edges);
            all_unresolved.extend_from_slice(&result.unresolved_refs);
            if let Ok(source) = sync::read_source_file(&project_root.join(file_path)) {
                body_documents.extend(build_executable_body_documents(
                    file_path,
                    &source,
                    &result.nodes,
                ));
            }
            file_records.push(FileRecord {
                path: file_path.clone(),
                content_hash: hash.clone(),
                size: *size,
                modified_at: *mtime,
                indexed_at: current_timestamp(),
                node_count: result.nodes.len() as u32,
                kind: FileKind::Code,
            });
        }

        on_verbose(&format!(
            "extracted {} nodes, {} edges from {} files in {:.1}s",
            total_nodes,
            all_edges.len(),
            extractions.len(),
            phase_start.elapsed().as_secs_f64()
        ));

        // 4b. Companion documentation (#154): map sidecar and docs-directory
        // Markdown onto the source files they describe, as `Doc` nodes with
        // `Documents` edges to the covered `File` nodes. Purely additive — a
        // project with no docs produces nothing here.
        let (doc_nodes, doc_edges) = self.build_companion_docs(&project_root, &all_nodes);
        let doc_count = doc_nodes.len();
        all_nodes.extend(doc_nodes);
        all_edges.extend(doc_edges);
        if doc_count > 0 {
            on_verbose(&format!("discovered {doc_count} companion doc(s)"));
        }

        // Make containment available to resolution before `Contains` edges are
        // denormalized into `nodes.parent_id` during the later DB insert.
        let mut parent_ids: HashMap<&str, &str> = HashMap::new();
        for edge in &all_edges {
            if edge.kind == EdgeKind::Contains {
                parent_ids
                    .entry(edge.target.as_str())
                    .or_insert(edge.source.as_str());
            }
        }
        for node in &mut all_nodes {
            if node.parent_id.is_none() {
                node.parent_id = parent_ids.get(node.id.as_str()).map(|id| (*id).to_string());
            }
        }

        // 5. Resolve references in-memory (parallel) before DB insert
        let phase_start = Instant::now();
        crate::memstats::set_graph_nodes(all_nodes.len() as u64);
        // Last point before anything is written in the full-index path: the
        // inserts all happen after resolution, so leaving here still commits
        // nothing (#450). Resolution itself is a single whole-graph pass with
        // no safe interior seam, hence the check in front of it rather than
        // inside it.
        crate::cancel::check("index")?;
        if !all_unresolved.is_empty() {
            // #253: `from_nodes` borrows from `all_nodes` rather than
            // cloning it into its caches; the remaining peak here is
            // `all_nodes` itself (#306).
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            crate::memstats::record("index:resolve:build_caches");
            let resolution = resolver.resolve_all(&all_unresolved);
            crate::memstats::record("index:resolve:refs");
            // Ties are recorded rather than dropped, so a caller can pick the
            // intended target and `dead_code` can tell "referenced, target
            // unknown" from "uncalled" (#412).
            let ambiguity_files: Vec<String> = self.scan_files();
            let _ = self
                .db
                .replace_ambiguous_calls(&ambiguity_files, &resolution.ambiguous)
                .await;
            all_edges.extend(resolver.create_edges(&resolution.resolved));
            // Propagate call edges across build-config variants (Rust `#[cfg]`
            // twins, Go platform files) so an inactive-platform definition is
            // not seen as dead merely because the call bound to its sibling (#141).
            let variant_edges = crate::resolution::propagate_variant_edges(&all_nodes, &all_edges);
            all_edges.extend(variant_edges);
        }
        crate::memstats::record("index:resolve:done");
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
        // Artifacts contribute a `files` row and nothing else — no nodes, no
        // edges, no body documents (#323).
        file_records.extend(self.artifact_file_records(&artifact_files));
        file_records.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        let total_edges = all_edges.len();

        // 7. Bulk-insert via prepared statements (zero SQL re-parsing)
        let phase_start = Instant::now();
        crate::memstats::record("index:insert");
        self.db.insert_nodes(&all_nodes).await?;
        self.db
            .insert_executable_body_documents(&body_documents)
            .await?;
        self.db.insert_edges(&all_edges).await?;
        self.db.upsert_files(&file_records).await?;

        // Durably record every raw unresolved reference extracted this pass —
        // not just the leftovers `resolve_all` couldn't bind. A full index
        // resolves cross-file refs in-memory in one shot and, until now, never
        // wrote them to the `unresolved_refs` table. That left a later
        // incremental `sync` with no record of e.g. "file A calls a symbol
        // defined in file B": when B is edited, `delete_nodes_by_file` cascades
        // away every edge touching B's (old) node ids — including inbound
        // edges from untouched files like A — and `sync`'s resolution step
        // only replays refs durably stored here, so A's call into B was
        // silently dropped forever (never in the table to retry). Persisting
        // the full set here lets `sync` re-resolve and recreate those edges
        // the next time it runs, keeping `sync` convergent with a full
        // reindex instead of monotonically losing cross-file call edges as
        // files get touched over time.
        if !all_unresolved.is_empty() {
            self.db.insert_unresolved_refs(&all_unresolved).await?;
        }

        // 8. Restore indexes and normal durability
        self.db.end_bulk_load().await?;
        self.db.rebuild_trait_dispatch_callers().await?;
        on_verbose(&format!(
            "wrote to database in {:.1}s",
            phase_start.elapsed().as_secs_f64()
        ));

        let duration_ms = start.elapsed().as_millis() as u64;
        let now_str = current_timestamp().to_string();
        self.db.set_metadata("last_full_sync_at", &now_str).await?;
        self.db.set_metadata("last_sync_at", &now_str).await?;
        self.touch_branch_synced();
        self.db
            .set_metadata("last_sync_duration_ms", &duration_ms.to_string())
            .await?;
        if self.registry.extractor_for_language("ruby").is_some() {
            self.db
                .set_metadata(RUBY_SINGLETON_KIND_METADATA, "1")
                .await?;
        }

        let result = IndexResult {
            file_count: files.len(),
            node_count: total_nodes,
            edge_count: total_edges,
            duration_ms,
            skipped_extensions,
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
        self.record_indexed_version();
        crate::memstats::record("index:done");
        Ok(result)
    }

    /// Records the running version as the one that produced the current index.
    ///
    /// Without this, a project indexed by the CLI keeps `last_indexed_version`
    /// empty, which `bump_kind` classifies as a major bump — so the MCP server
    /// forces a full reindex on the first tool call of every session (#320).
    ///
    /// Best-effort: failing to persist the marker must not fail an index that
    /// otherwise succeeded.
    fn record_indexed_version(&self) {
        let running = env!("CARGO_PKG_VERSION");
        match crate::config::load_config(&self.project_root) {
            Ok(mut config) if config.last_indexed_version != running => {
                config.last_indexed_version = running.to_string();
                if let Err(e) = crate::config::save_config(&self.project_root, &config) {
                    eprintln!("[tokensave] failed to record indexed version: {e}");
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("[tokensave] failed to load config to record indexed version: {e}");
            }
        }
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
    /// Use this from the MCP server's connect-time catch-up and its per-call
    /// staleness check, when another MCP (or any peer process) already holds
    /// the project sync lock. If the peer holds the lock, wait (bounded) for
    /// it to release so the DB is fresh by the time the caller refreshes its
    /// view; if the peer covered our files, return without doing extra work,
    /// otherwise sync ourselves.
    /// How many unresolved references to resolve at a time (#482).
    ///
    /// The whole table used to be materialised: 189,446 records and +74.6 MiB
    /// on this repository, on every sync. 25,000 keeps that page under ~10 MiB
    /// while making the paging overhead — one indexed query per page — a
    /// rounding error against resolving the batch.
    const RESOLVE_BATCH: usize = 25_000;

    /// Resolve every unresolved reference, a page at a time (#482).
    ///
    /// The resolver's name index is built once and stays global; only the
    /// input is paged. That is what makes this safe where chunking the *node*
    /// slice is not — each reference resolves independently, so a page cannot
    /// lose a target the way a chunked index would.
    ///
    /// What still accumulates is small and bounded by the *answers* rather than
    /// the questions: on this repository 28,849 resolved and 11,713 ambiguity
    /// records, against 189,446 inputs. The Go selector suppression runs once
    /// over the accumulated set, since nothing guarantees a selector and its
    /// bare-name sibling land in the same page.
    async fn resolve_all_streamed(
        &self,
        resolver: &ReferenceResolver<'_>,
        touched: Option<&TouchedSet>,
    ) -> Result<StreamedResolution> {
        let mut cursor = 0i64;
        let mut resolved: Vec<ResolvedRef> = Vec::new();
        let mut ambiguous: Vec<AmbiguousCall> = Vec::new();
        let mut total = 0usize;
        let mut attempted = 0usize;
        let mut attempted_refs: Vec<AmbiguityRefKey> = Vec::new();

        loop {
            let page = self
                .db
                .get_unresolved_refs_after(cursor, Self::RESOLVE_BATCH)
                .await?;
            let Some((last_id, _)) = page.last() else {
                break;
            };
            cursor = *last_id;
            let mut refs: Vec<UnresolvedRef> = page.into_iter().map(|(_, r)| r).collect();
            total += refs.len();
            // Incremental invalidation (#484): drop the references this sync
            // provably cannot have changed the answer for. Their edges are
            // already in the table and their ambiguity records already written,
            // so re-deriving them produces byte-identical rows at full cost.
            if let Some(touched) = touched {
                refs.retain(|uref| touched.needs_resolve(&uref.file_path, &uref.reference_name));
            }
            if refs.is_empty() {
                continue;
            }
            attempted += refs.len();
            // Only the references actually re-resolved may have their ambiguity
            // records replaced — see `write_ambiguities`. Not collected for a
            // full pass, where the file-scoped delete already covers every
            // reference and this vector would be the whole table.
            if touched.is_some() {
                attempted_refs.extend(refs.iter().map(AmbiguityRefKey::from));
            }
            let (batch_resolved, batch_ambiguous) = resolver.resolve_batch(&refs);
            resolved.extend(batch_resolved);
            ambiguous.extend(batch_ambiguous);
        }

        resolver.finalize_resolved(&mut resolved);
        Ok(StreamedResolution {
            resolved,
            ambiguous,
            total,
            attempted,
            attempted_refs,
        })
    }

    /// Writes the ambiguity records at the granularity this pass earns
    /// (#484 phase 3).
    ///
    /// A full pass re-resolved every reference in every file, so it clears by
    /// file and rewrites. An incremental pass re-resolved a *subset* of some
    /// files' references, so clearing by file would delete the records of the
    /// references it never looked at — the `area` ambiguity in an untouched
    /// caller that `tests/incremental_resolution_test.rs` watches for. It
    /// clears by reference instead.
    ///
    /// Best effort in both branches, matching the `let _ =` this replaces: a
    /// lost ambiguity record degrades an explanation, and is not worth failing
    /// a sync that has already written its nodes and edges.
    async fn write_ambiguities(&self, resolution: &StreamedResolution, incremental: bool) {
        if incremental {
            let _ = self
                .db
                .replace_ambiguous_calls_for_refs(&resolution.attempted_refs, &resolution.ambiguous)
                .await;
        } else {
            let files = self.scan_files();
            let _ = self
                .db
                .replace_ambiguous_calls(&files, &resolution.ambiguous)
                .await;
        }
    }

    /// Re-propagate build-variant call edges without a whole-graph load (#481).
    ///
    /// The old shape loaded every `annotates` and `calls` edge — and did it
    /// while the resolver's node slice was still alive, so two graph-sized
    /// allocations were resident at once and the sync's peak RSS landed here.
    /// On this repository that was 12.9 MiB and the run's high-water mark, to
    /// emit **zero** edges: the grouping keeps 3 groups out of 19,331 nodes,
    /// and a call has to point into one of them to propagate at all.
    ///
    /// The output set is small by construction, so ask SQL for it. Two bounded
    /// queries — the variant groups, then only the `calls` edges targeting a
    /// member — feed the same emitter the whole-graph path uses, so behaviour
    /// is unchanged. The common case, no multi-member group, returns before
    /// touching the edges table at all.
    ///
    /// Best-effort: a query failure yields no propagated edges rather than
    /// failing the sync, matching the `unwrap_or_default()` this replaces.
    async fn propagate_variant_edges_bounded(&self) -> Vec<Edge> {
        let rust = self.db.variant_group_candidates().await.unwrap_or_default();
        let go = self.db.go_variant_candidates().await.unwrap_or_default();
        let groups = crate::resolution::variant_groups_from_candidates(&rust, &go);
        if groups.is_empty() {
            return Vec::new();
        }
        let members: Vec<String> = groups
            .values()
            .flatten()
            .map(|id| (*id).to_string())
            .collect();
        let edges = self.db.calls_edges_into(&members).await.unwrap_or_default();
        crate::resolution::emit_variant_edges(&groups, &edges)
    }

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
            // caller (e.g. the MCP server's refresh hook) sees the
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
        crate::memstats::record("sync:extract");
        let (sync_extractions, _skipped_extractions) =
            extract_files_isolated(project_root, registry, file_paths.clone());

        // Phase 1: insert all nodes (and metadata) so cross-file edges
        // can reference them. Edges are queued for phase 2 (#58).
        let mut queued_edges: Vec<&Edge> = Vec::new();
        let mut body_documents = Vec::new();
        // Which files and names this sync touched, so resolution can skip the
        // references it provably cannot have changed (#484). The deleted half
        // has to be read before `delete_nodes_by_file` removes the rows.
        let mut touched = TouchedSet::new();
        for (file_path, result, hash, size, mtime) in &sync_extractions {
            touched.touch_file(file_path);
            touched.touch_nodes(&self.db.touched_nodes_by_file(file_path).await?);
            touched.touch_nodes(&node_touch_records(&result.nodes));
            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            if let Ok(source) = sync::read_source_file(&project_root.join(file_path)) {
                body_documents.extend(build_executable_body_documents(
                    file_path,
                    &source,
                    &result.nodes,
                ));
            }
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
                kind: FileKind::Code,
            };
            self.db.upsert_file(&file_record).await?;
        }
        self.db
            .insert_executable_body_documents(&body_documents)
            .await?;

        // Phase 2: insert all queued edges now that every node is present.
        // The conditional INSERT in `insert_edges` silently skips edges
        // whose endpoints are truly missing (e.g. unindexed files).
        if !queued_edges.is_empty() {
            let owned: Vec<Edge> = queued_edges.into_iter().cloned().collect();
            self.db.insert_edges(&owned).await?;
        }

        crate::cancel::check_partial("sync")?;

        // Resolve references for any new/changed unresolved refs
        if !file_paths.is_empty() {
            // #253: `from_nodes` borrows rather than clones. #306: the load
            // drops `docstring` and `signature`, which resolution never
            // reads and which are unbounded TEXT — a const's whole
            // initializer lives in `signature` (43 KB for one node in
            // #362). The remaining peak is the node count itself: the
            // resolver borrows from this slice for its whole life and needs
            // a global name index, so the pass cannot be chunked without a
            // redesign.
            // Samples are taken after the work they name; see the sibling
            // site for why that matters (#409).
            let all_nodes = self
                .db
                .get_all_nodes_for_resolution()
                .await
                .unwrap_or_default();
            crate::memstats::set_graph_nodes(all_nodes.len() as u64);
            crate::memstats::record("sync:resolve:load_nodes");
            let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
            crate::memstats::record("sync:resolve:build_caches");
            // Paged rather than materialised (#482), and narrowed to the
            // references this sync could have changed (#484).
            let incremental = incremental_resolution_enabled();
            let resolution = self
                .resolve_all_streamed(&resolver, incremental.then_some(&touched))
                .await?;
            let resolved_refs = &resolution.resolved;
            crate::memstats::record("sync:resolve:refs");
            if resolution.attempted > 0 {
                // The sync's peak lives between `resolve:done` and `sync:done`,
                // and with no sample in that window it was attributed to
                // whichever sample came next — which is why #409 was argued
                // from `size_of` arithmetic rather than from RSS.
                crate::memstats::record("sync:resolve:refs");
                // See the full-index site: ambiguities are kept, not dropped.
                self.write_ambiguities(&resolution, incremental).await;
                let edges = resolver.create_edges(resolved_refs);
                if !edges.is_empty() {
                    self.db.insert_edges(&edges).await?;
                    // Re-propagate build-variant call edges now that new call
                    // edges exist (#141), from the two bounded queries rather
                    // than the whole graph (#481).
                    let variant_edges = self.propagate_variant_edges_bounded().await;
                    crate::memstats::record("sync:variants");
                    if !variant_edges.is_empty() {
                        self.db.insert_edges(&variant_edges).await?;
                    }
                }
            }
        }

        self.db.rebuild_trait_dispatch_callers().await?;
        self.db
            .set_metadata("last_sync_at", &current_timestamp().to_string())
            .await?;
        self.touch_branch_synced();
        self.db
            .set_metadata(
                "last_sync_duration_ms",
                &start.elapsed().as_millis().to_string(),
            )
            .await?;

        clear_dirty_sentinel(&self.project_root);
        crate::memstats::record("sync:done");
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
        self.validate_manifest()?;
        write_dirty_sentinel(&self.project_root);
        let start = Instant::now();

        crate::cancel::check("sync")?;
        on_progress(0, 0, "scanning files");
        let phase_start = Instant::now();
        let (current_files, skipped_extensions) = self.scan_files_diagnostics();
        on_verbose(&format!(
            "scanned {} files in {:.1}s",
            current_files.len(),
            phase_start.elapsed().as_secs_f64()
        ));
        report_skipped_extensions(&skipped_extensions, &on_verbose);

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

        crate::cancel::check("sync")?;

        // Load all DB file records into a map for O(1) lookups
        let db_files = self.db.get_all_files().await?;
        let db_map: HashMap<String, FileRecord> =
            db_files.into_iter().map(|f| (f.path.clone(), f)).collect();
        let repair_legacy_ruby_singletons = !db_map.is_empty()
            && self
                .db
                .get_metadata(RUBY_SINGLETON_KIND_METADATA)
                .await?
                .is_none();

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
        let legacy_ruby_files: Vec<String> = if repair_legacy_ruby_singletons {
            db_map
                .keys()
                .filter(|path| {
                    current_set.contains(path.as_str())
                        && self
                            .registry
                            .extractor_for_file(path)
                            .is_some_and(|extractor| {
                                extractor.language_name().eq_ignore_ascii_case("ruby")
                            })
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        for path in &legacy_ruby_files {
            if !stale.contains(path) {
                stale.push(path.clone());
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

        // Which files and names this sync touched, so resolution can skip the
        // references it provably cannot have changed (#484). Deletions count as
        // much as insertions: removing a file takes the edges pointing *into*
        // it with them, and only the touched-name set brings those back.
        let mut touched = TouchedSet::new();

        // Remove deleted files
        for path in &removed {
            on_progress(0, 0, &format!("removing {path}"));
            touched.touch_file(path);
            touched.touch_nodes(&self.db.touched_nodes_by_file(path).await?);
            self.db.delete_file(path).await?;
        }

        // Re-index stale and new files — extract in parallel, insert sequentially
        let to_index: Vec<String> = stale.iter().chain(new_files.iter()).cloned().collect();
        // Artifacts take the same add/modify/remove path as source but skip
        // extraction entirely; their row is the whole record (#323).
        let (to_index, changed_artifacts) =
            Self::partition_artifacts(to_index, &self.artifact_extensions());
        for record in self.artifact_file_records(&changed_artifacts) {
            self.db.upsert_file(&record).await?;
        }
        let registry = &self.registry;

        let phase_start = Instant::now();
        let _ = stat_map; // worker re-stats internally
        crate::memstats::record("sync:extract");
        let (sync_extractions, sync_skipped): (Vec<_>, Vec<_>) =
            extract_files_isolated(project_root, registry, to_index.clone());
        let extracted_paths: HashSet<&str> = sync_extractions
            .iter()
            .map(|(path, _, _, _, _)| path.as_str())
            .collect();
        let ruby_repair_complete = legacy_ruby_repair_complete(
            repair_legacy_ruby_singletons,
            &legacy_ruby_files,
            &extracted_paths,
        );
        // Surface extractor timeouts/crashes in `SyncResult.skipped_paths`
        // so the user can see them in `tokensave sync --doctor`.
        skipped.extend(sync_skipped);

        // Extraction is the long phase and stops early on a shutdown, so this
        // is the last point at which nothing has been written yet (#450).
        crate::cancel::check("sync")?;

        // Phase 1: insert all nodes (and metadata) so cross-file edges
        // can reference them. Edges are queued for phase 2 (#58).
        let total = sync_extractions.len();
        let mut total_nodes = 0usize;
        let mut total_edges = 0usize;
        let mut queued_edges: Vec<&Edge> = Vec::new();
        let mut body_documents = Vec::new();
        for (idx, (file_path, result, hash, size, mtime)) in sync_extractions.iter().enumerate() {
            // Past this point rows are being written per file, so an
            // interruption leaves a partially updated index rather than an
            // untouched one — reported as such (#450). Checked per file
            // because on a large tree this loop is minutes of work.
            crate::cancel::check_partial("sync")?;
            on_progress(idx + 1, total, file_path);

            total_nodes += result.nodes.len();
            total_edges += result.edges.len();

            touched.touch_file(file_path);
            touched.touch_nodes(&self.db.touched_nodes_by_file(file_path).await?);
            touched.touch_nodes(&node_touch_records(&result.nodes));
            self.db.delete_nodes_by_file(file_path).await?;
            self.db.insert_nodes(&result.nodes).await?;
            if let Ok(source) = sync::read_source_file(&project_root.join(file_path)) {
                body_documents.extend(build_executable_body_documents(
                    file_path,
                    &source,
                    &result.nodes,
                ));
            }
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
                kind: FileKind::Code,
            };
            self.db.upsert_file(&file_record).await?;
        }
        self.db
            .insert_executable_body_documents(&body_documents)
            .await?;

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
            // Every sample here is taken *after* the work it names. They used
            // to be taken before it, so each one reported the RSS of the
            // previous step under the next step's name — which is how the
            // whole-graph node load came to be blamed for 73 MiB that
            // belonged to the reference load (#409).
            let pending_refs = self.db.count_unresolved_refs().await.unwrap_or(0);
            let mut attempted_refs = 0usize;
            if pending_refs > 0 {
                // #253: `from_nodes` borrows rather than clones. #306: the
                // load drops `docstring` and `signature`, which resolution
                // never reads and which are unbounded TEXT. The remaining
                // peak is the node count itself — see the sibling site in
                // the incremental path for why it cannot be chunked.
                let all_nodes = self
                    .db
                    .get_all_nodes_for_resolution()
                    .await
                    .unwrap_or_default();
                crate::memstats::set_graph_nodes(all_nodes.len() as u64);
                crate::memstats::record("sync:resolve:load_nodes");
                let resolver = ReferenceResolver::from_nodes(&self.db, &all_nodes);
                crate::memstats::record("sync:resolve:build_caches");
                // Paged rather than materialised (#482), and narrowed to the
                // references this sync could have changed (#484).
                let incremental = incremental_resolution_enabled();
                let resolution = self
                    .resolve_all_streamed(&resolver, incremental.then_some(&touched))
                    .await?;
                attempted_refs = resolution.attempted;
                debug_assert!(resolution.attempted <= resolution.total);
                let resolved_refs = &resolution.resolved;
                crate::memstats::record("sync:resolve:refs");
                // The sync's peak lives between `resolve:done` and `sync:done`,
                // and with no sample in that window it was attributed to
                // whichever sample came next — which is why #409 was argued
                // from `size_of` arithmetic rather than from RSS.
                crate::memstats::record("sync:resolve:refs");
                // See the full-index site: ambiguities are kept, not dropped.
                self.write_ambiguities(&resolution, incremental).await;
                let edges = resolver.create_edges(resolved_refs);
                if !edges.is_empty() {
                    self.db.insert_edges(&edges).await?;
                    // Propagate call edges across build-config variants (#141),
                    // from the two bounded queries rather than the whole graph
                    // (#481).
                    let variant_edges = self.propagate_variant_edges_bounded().await;
                    crate::memstats::record("sync:variants");
                    if !variant_edges.is_empty() {
                        self.db.insert_edges(&variant_edges).await?;
                    }
                }
            }
            // Reports what was re-attempted against what the table holds, so
            // the incremental narrowing (#484) is visible rather than implied.
            on_verbose(&format!(
                "resolved {attempted_refs} of {pending_refs} references ({} names touched) in {:.1}s",
                touched.name_count(),
                phase_start.elapsed().as_secs_f64()
            ));
        }

        self.db.rebuild_trait_dispatch_callers().await?;
        let duration_ms = start.elapsed().as_millis() as u64;
        self.db
            .set_metadata("last_sync_at", &current_timestamp().to_string())
            .await?;
        self.touch_branch_synced();
        self.db
            .set_metadata("last_sync_duration_ms", &duration_ms.to_string())
            .await?;
        if self.registry.extractor_for_language("ruby").is_some() && ruby_repair_complete {
            self.db
                .set_metadata(RUBY_SINGLETON_KIND_METADATA, "1")
                .await?;
        }

        clear_dirty_sentinel(&self.project_root);
        crate::memstats::record("sync:done");
        Ok(SyncResult {
            files_added: new_files.len(),
            files_modified: stale.len(),
            files_removed: removed.len(),
            duration_ms,
            added_paths: new_files,
            modified_paths: stale,
            skipped_paths: skipped,
            removed_paths: removed,
            skipped_extensions,
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
    /// Validates `.tokensave/project.json` (when present), surfacing parse
    /// errors, invalid globs, and unknown languages as hard sync errors (#194).
    pub(crate) fn validate_manifest(&self) -> Result<()> {
        crate::project_manifest::load_manifest(&self.project_root, &self.registry).map(|_| ())
    }

    /// Cached `.tokensave/project.json` manifest, if one is configured.
    pub(crate) fn manifest(
        &self,
    ) -> Option<std::sync::Arc<crate::project_manifest::CompiledManifest>> {
        crate::project_manifest::manifest_for(&self.project_root, &self.registry)
    }

    /// Returns a warning message if git-tracked files in hidden directories were skipped by indexing.
    pub fn warn_skipped_hidden_dirs(&self) -> Option<String> {
        detect_skipped_hidden_dirs(
            &self.project_root,
            &self.config,
            self.manifest().as_deref(),
            &self.registry.supported_extensions(),
        )
    }

    /// Advances `last_synced_at` on the branch this handle serves.
    ///
    /// Called wherever `last_sync_at` metadata is written, so the DB-level
    /// timestamp and the per-branch one cannot drift apart. Before #399
    /// `touch_synced` had a single caller in `branch add`, so the field
    /// recorded when a branch entry was *created* and never moved again —
    /// while `tokensave branch list`, `tokensave_branch_list`, and the
    /// `tokensave://branches` resource all render it as live freshness.
    ///
    /// Best-effort: a sync that indexed correctly must not fail because a
    /// metadata file could not be rewritten. Silent on projects with no
    /// branch metadata, and on a branch with no entry of its own.
    fn touch_branch_synced(&self) {
        let Some(branch) = self.serving_branch.as_ref().or(self.active_branch.as_ref()) else {
            return;
        };
        let dir = get_tokensave_dir(&self.project_root);
        let Some(mut meta) = branch_meta::load_branch_meta(&dir) else {
            return;
        };
        meta.touch_synced(branch);
        let _ = branch_meta::save_branch_meta(&dir, &meta);
    }

    pub(crate) fn scan_files(&self) -> Vec<String> {
        self.scan_files_diagnostics().0
    }

    /// Like [`Self::scan_files`], but also reports how many source-like
    /// files were skipped because no registered extractor handles their
    /// extension (#262, #270). Returns `(files, skipped_extensions)` where
    /// `skipped_extensions` is sorted by count (descending), then name.
    ///
    /// Known non-source extensions (images, archives, lockfiles, …) are
    /// excluded from the summary so it highlights genuinely unsupported
    /// languages instead of asset noise; exclude globs, gitignore rules,
    /// and the hidden-directory filter all apply before a file is counted.
    pub(crate) fn scan_files_diagnostics(&self) -> (Vec<String>, Vec<(String, usize)>) {
        debug_assert!(
            self.project_root.is_dir(),
            "scan_files: project_root is not a directory"
        );
        // Artifacts ride the same walk as source rather than getting a second
        // one (#323). Everything that decides whether a path is in the project
        // — exclude globs, gitignore, the symlink-cycle prune, the size limit —
        // lives in that walk, and a parallel implementation would drift from it.
        // Declared before the borrowed list so it outlives the `&str`s taken from it.
        let artifact_exts = self.artifact_extensions();

        let mut supported_exts = self.registry.supported_extensions();
        debug_assert!(
            !supported_exts.is_empty(),
            "scan_files: no supported extensions registered"
        );
        supported_exts.extend(artifact_exts.iter().map(String::as_str));

        let mut skipped_map: HashMap<String, usize> = HashMap::new();
        let mut files = self.scan_project_files(&supported_exts, &mut skipped_map);
        // Manifest external entries (absolute / `~` paths) are additive
        // opt-ins indexed under their resolved absolute path (#194).
        if let Some(manifest) = self.manifest() {
            files.extend(manifest.expand_external_files(self.config.max_file_size));
            files.sort();
            files.dedup();
        }
        let mut skipped: Vec<(String, usize)> = skipped_map.into_iter().collect();
        skipped.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        (files, skipped)
    }

    fn scan_project_files(
        &self,
        supported_exts: &[&str],
        skipped_exts: &mut HashMap<String, usize>,
    ) -> Vec<String> {
        if self.config.git_ignore {
            let files = self.scan_files_with_gitignore(supported_exts, skipped_exts);
            if files.is_empty() {
                // The project directory may be gitignored by a parent repo,
                // causing the ignore-aware walker to skip everything. Fall
                // back to plain walkdir if source files clearly exist.
                let canonical_root = self.project_root.canonicalize().ok();
                let has_source = WalkDir::new(&self.project_root)
                    .follow_links(true)
                    .max_depth(2)
                    .into_iter()
                    .filter_entry(|e| {
                        if e.depth() > 0 && e.path_is_symlink() && e.file_type().is_dir() {
                            return !reenters_project_root(e.path(), canonical_root.as_deref());
                        }
                        true
                    })
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
                    // Don't double-count skips from the aborted first walk.
                    skipped_exts.clear();
                    return self.scan_files_walkdir(supported_exts, skipped_exts);
                }
            }
            files
        } else {
            self.scan_files_walkdir(supported_exts, skipped_exts)
        }
    }

    /// Walk using `walkdir`, skipping hidden directories and `target/`.
    ///
    /// Hidden (dot-prefixed) entries that match a configured `include` glob
    /// are allowed through despite the default filter.
    pub(crate) fn scan_files_walkdir(
        &self,
        supported_exts: &[&str],
        skipped_exts: &mut HashMap<String, usize>,
    ) -> Vec<String> {
        let mut files = Vec::new();
        let root = &self.project_root;
        let config = &self.config;
        let manifest = self.manifest();
        let canonical_root = root.canonicalize().ok();
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                // Checked before every other rule so no `include` glob or
                // manifest entry can re-enable a link that swallows the
                // filesystem (#327).
                if e.path_is_symlink()
                    && e.file_type().is_dir()
                    && reenters_project_root(e.path(), canonical_root.as_deref())
                {
                    return false;
                }
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') || name == "target" {
                    // Allow if the relative path matches an include glob or a
                    // manifest entry (#194).
                    if let Ok(rel) = e.path().strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        return is_included(&rel_str, config)
                            || manifest.as_deref().is_some_and(|m| {
                                m.matches_local_file(&rel_str) || m.local_dir_may_contain(&rel_str)
                            });
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
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts, skipped_exts) {
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
    pub(crate) fn scan_files_with_gitignore(
        &self,
        supported_exts: &[&str],
        skipped_exts: &mut HashMap<String, usize>,
    ) -> Vec<String> {
        let manifest = self.manifest();
        // Manifest entries behave like include globs for hidden-path
        // filtering, so disable the crate's hidden filter when either exists.
        let has_includes = !self.config.include.is_empty() || manifest.is_some();
        let mut files = Vec::new();
        // Prune directories covered by an `exclude` glob *before* descending.
        // The `ignore` crate honors `.gitignore` but not our `config.exclude`,
        // so without this a symlink inside an excluded directory (e.g. a Wine
        // prefix's `dosdevices/z: -> /`) is followed and the whole filesystem
        // gets walked (#170). Mirrors the `is_excluded_dir` prune in
        // `scan_files_walkdir` and applies equally to `--skip-folder`, which
        // feeds the same exclude list.
        let root = self.project_root.clone();
        let config = self.config.clone();
        let canonical_root = self.project_root.canonicalize().ok();
        let walker = ignore::WalkBuilder::new(&self.project_root)
            .follow_links(true)
            .hidden(!has_includes) // disable when we need to check includes
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".gitignore")
            .filter_entry(move |e| {
                // Only prune directories; files are filtered later by accept_file.
                if e.file_type().is_some_and(|ft| ft.is_dir()) {
                    if let Ok(rel) = e.path().strip_prefix(&root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if is_excluded_dir(&rel_str, &config) {
                            return false;
                        }
                    }
                    // A link back onto the root or one of its ancestors would
                    // re-walk the project plus the rest of the filesystem
                    // (#327). The walk root itself is exempt so a project
                    // opened through a symlinked path still scans.
                    if e.depth() > 0
                        && e.path_is_symlink()
                        && reenters_project_root(e.path(), canonical_root.as_deref())
                    {
                        return false;
                    }
                }
                true
            })
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
                        let manifest_allows = manifest.as_deref().is_some_and(|m| {
                            m.matches_local_file(&rel_str) || m.local_dir_may_contain(&rel_str)
                        });
                        if !is_included(&rel_str, &self.config) && !manifest_allows {
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
            if let Some(rel_str) = self.accept_file(entry.path(), supported_exts, skipped_exts) {
                files.push(rel_str);
            }
        }
        files
    }

    /// Checks whether a file should be included: correct extension, not
    /// excluded by config globs, and within the max file size.
    ///
    /// Files rejected because no registered extractor handles their
    /// extension are tallied into `skipped_exts` (source-like extensions
    /// only) so verbose sync can report them (#262, #270).
    pub(crate) fn accept_file(
        &self,
        path: &Path,
        supported_exts: &[&str],
        skipped_exts: &mut HashMap<String, usize>,
    ) -> Option<String> {
        let relative = path.strip_prefix(&self.project_root).ok()?;
        // Normalize to forward slashes so paths are consistent across
        // platforms and between different directory walkers on Windows.
        let rel_str = relative.to_string_lossy().replace('\\', "/");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !supported_exts.contains(&ext) {
            // Extensionless / oddly-named files are still indexable when a
            // manifest entry explicitly lists them (#194).
            let manifest_match = self
                .manifest()
                .is_some_and(|m| m.matches_local_file(&rel_str));
            if !manifest_match {
                if !ext.is_empty() && !is_excluded(&rel_str, &self.config) {
                    let ext_lower = ext.to_ascii_lowercase();
                    if !NON_SOURCE_EXTS.contains(&ext_lower.as_str()) {
                        *skipped_exts.entry(ext_lower).or_insert(0) += 1;
                    }
                }
                return None;
            }
        }
        if is_excluded(&rel_str, &self.config) {
            return None;
        }
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > self.config.max_file_size {
            return None;
        }
        Some(rel_str)
    }
}

/// Detects git-tracked, indexable files that the hidden-directory filter
/// skipped. Returns a formatted warning string if any exist.
///
/// Mirrors the walker's pruning rule exactly (`scan_files_walkdir` /
/// `scan_files_with_gitignore`): a dot-prefixed *directory* blocks descent
/// unless that directory path itself matches an include glob or manifest
/// entry. Matching only the files inside (e.g. `.github/**` without a bare
/// `.github` entry) does not re-enable descent, so this check walks each
/// ancestor prefix the same way the walker does instead of testing the file
/// path — otherwise the warning would go silent in exactly that trap.
/// Every path `git ls-files` reports for the project, as forward-slashed
/// repository-relative strings.
///
/// `None` when the project is not a git repository, git is unavailable, or the
/// command fails — every caller treats that as "cannot tell" and stays silent
/// rather than reporting an empty answer as a complete one.
///
/// Note that these are *index* entries: a path here may not exist on disk
/// (sparse checkout, a staged deletion), so callers that care check.
pub(crate) fn list_git_tracked_files(project_root: &Path) -> Option<Vec<String>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(project_root)
        // -z: NUL-separated output, no C-quoting of non-ASCII paths.
        .args(["ls-files", "-z"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(
        String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

pub fn detect_skipped_hidden_dirs(
    project_root: &Path,
    config: &TokenSaveConfig,
    manifest: Option<&crate::project_manifest::CompiledManifest>,
    supported_exts: &[&str],
) -> Option<String> {
    let tracked = list_git_tracked_files(project_root)?;

    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    // Sibling files share the same verdict, so evaluate the globs once per
    // parent directory instead of once per file. `Some(prefix)` = the hidden
    // ancestor the walker would prune at; `None` = reachable or deliberately
    // excluded.
    let mut dir_cache: HashMap<String, Option<String>> = HashMap::new();

    for rel_path in tracked.iter().map(String::as_str) {
        // Only count files an extractor could actually index; otherwise every
        // repo with a tracked `.github/workflows/*.yml` would warn.
        let ext = Path::new(rel_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !supported_exts.contains(&ext) {
            continue;
        }
        // File-level excludes are a deliberate opt-out; including the dir
        // would not index these files, so warning about them is a false
        // promise.
        if is_excluded(rel_path, config) {
            continue;
        }
        // Root-level dotfiles have no hidden ancestor directory.
        let Some((dirs, _file)) = rel_path.rsplit_once('/') else {
            continue;
        };

        let blocked = dir_cache.entry(dirs.to_string()).or_insert_with(|| {
            // Apply the walker's hidden-directory test at each ancestor prefix.
            let mut end = 0;
            for comp in dirs.split('/') {
                end += comp.len() + usize::from(end > 0);
                if !comp.starts_with('.') {
                    continue;
                }
                let prefix = &dirs[..end];
                // Explicitly excluded dirs are a deliberate opt-out, not a trap.
                if is_excluded_dir(prefix, config) {
                    return None;
                }
                let allowed = is_included(prefix, config)
                    || manifest.is_some_and(|m| {
                        m.matches_local_file(prefix) || m.local_dir_may_contain(prefix)
                    });
                if !allowed {
                    return Some(prefix.to_string());
                }
            }
            None
        });
        if let Some(prefix) = blocked {
            // `git ls-files` lists index entries that may not exist on disk
            // (sparse checkouts, deletions staged but not committed); the
            // walker never would have seen those, so don't warn about them.
            if !project_root.join(rel_path).is_file() {
                continue;
            }
            *dir_counts.entry(prefix.clone()).or_insert(0) += 1;
        }
    }

    if dir_counts.is_empty() {
        return None;
    }

    let total: usize = dir_counts.values().sum();
    let mut dir_vec: Vec<(String, usize)> = dir_counts.into_iter().collect();
    dir_vec.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let dir_summary = dir_vec
        .iter()
        .map(|(dir, count)| format!("{dir}: {count}"))
        .collect::<Vec<_>>()
        .join(", ");

    let top_dir = &dir_vec[0].0;
    let file_word = if total == 1 { "file" } else { "files" };

    Some(format!(
        "\x1b[33mwarning:\x1b[0m skipped {total} tracked {file_word} in hidden directories ({dir_summary}) — add \"{top_dir}\" and \"{top_dir}/**\" to include[] in .tokensave/config.json, then run `tokensave sync -f` to index them (or add \"{top_dir}/**\" to exclude[] to silence this warning)"
    ))
}

impl TokenSave {
    /// Tracked files the index holds no `files` row for, in `git ls-files`
    /// order.
    ///
    /// A file gets a row when a language extractor handles its extension, or
    /// when the extension is listed in `artifact_extensions` (#323) — and a
    /// row is what makes a file reachable by literal search, which reads bytes
    /// and needs no parser. Everything else is tracked by git, invisible to
    /// the index, and therefore silently absent from a literal answer (#442).
    ///
    /// Deliberately *not* filtered by extension: `NON_SOURCE_EXTS` exists to
    /// keep asset noise out of the skipped-*language* diagnostic and contains
    /// `txt`, `xml`, `ini`, `conf` and `csv`, which are exactly the text
    /// formats a literal search might be looking in. Reporting every
    /// unindexed extension and letting the caller decide which are worth
    /// adding is honest; filtering them here would recreate the same silent
    /// gap one level down.
    ///
    /// Config `exclude` globs are applied, since an excluded file is a
    /// deliberate opt-out rather than an omission, and index entries with no
    /// file on disk (sparse checkouts, staged deletions) are dropped: the
    /// walker would never have seen those either.
    ///
    /// `None` when git cannot answer, so a caller can distinguish "nothing is
    /// missing" from "cannot tell".
    pub(crate) fn unindexed_tracked_files(
        &self,
        indexed: &std::collections::HashSet<&str>,
    ) -> Option<Vec<String>> {
        let tracked = list_git_tracked_files(&self.project_root)?;
        Some(
            tracked
                .into_iter()
                .filter(|path| !indexed.contains(path.as_str()))
                .filter(|path| !is_excluded(path, &self.config))
                .filter(|path| self.project_root.join(path).is_file())
                .collect(),
        )
    }

    /// Returns the artifact extensions actually in effect for this project.
    ///
    /// An extension a language extractor already handles is dropped: the symbol
    /// pass owns those files and records them with their symbols, so listing
    /// one here would only race the two passes to write the same row.
    pub(crate) fn artifact_extensions(&self) -> Vec<String> {
        let supported = self.registry.supported_extensions();
        self.config
            .artifact_extensions
            .iter()
            .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
            .filter(|ext| !ext.is_empty() && !supported.contains(&ext.as_str()))
            .collect()
    }

    /// Splits scanned paths into source files and artifacts.
    ///
    /// Artifacts are never handed to the extractor: they have no symbols by
    /// definition, and routing them through extraction would mean teaching both
    /// the in-process and subprocess paths to return an empty result.
    pub(crate) fn partition_artifacts(
        files: Vec<String>,
        artifact_exts: &[String],
    ) -> (Vec<String>, Vec<String>) {
        files.into_iter().partition(|path| {
            !std::path::Path::new(path)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| artifact_exts.contains(&ext.to_ascii_lowercase()))
        })
    }

    /// Builds the `files` row for an artifact, hashing it like any other file.
    ///
    /// The hash and stat are what incremental sync compares against, so an
    /// artifact whose row omitted them would be re-processed on every sync.
    fn artifact_file_record(&self, rel_path: &str) -> Option<FileRecord> {
        let abs_path = self.project_root.join(rel_path);
        let source = sync::read_source_file(&abs_path).ok()?;
        let (modified_at, size) = sync::file_stat(&abs_path)
            .unwrap_or_else(|| (current_timestamp(), source.len() as u64));
        Some(FileRecord {
            path: rel_path.to_string(),
            content_hash: sync::content_hash(&source),
            size,
            modified_at,
            indexed_at: current_timestamp(),
            node_count: 0,
            kind: FileKind::Artifact,
        })
    }

    /// Builds `files` rows for every artifact path, in parallel.
    fn artifact_file_records(&self, paths: &[String]) -> Vec<FileRecord> {
        paths
            .par_iter()
            .filter_map(|path| self.artifact_file_record(path))
            .collect()
    }

    /// Gets the absolute path for a relative path.
    pub(crate) fn absolute_path(&self, relative_path: &str) -> PathBuf {
        self.project_root.join(relative_path)
    }

    /// Resolves an edit tool's `path` (or a symbol's DB-recorded relative
    /// path) to the absolute filesystem location that should actually be
    /// read/written, plus — when that location falls under the *indexed*
    /// project root — the root-relative path used as the DB reindex key.
    ///
    /// Resolution rules (fixes the "wrong-tree write" bug where a caller
    /// working in a git worktree got edits silently redirected into the
    /// primary checkout):
    ///
    /// - An **absolute** `path` is honored verbatim, even when it points
    ///   outside the indexed project root. Passing an absolute path is
    ///   itself the caller's explicit, deliberate statement of where the
    ///   write should land, so no separate opt-out flag is needed. The
    ///   previous behavior rejected out-of-root absolute paths with a "path
    ///   is not within the project" error, which is safe but unhelpful for
    ///   worktree callers; verbatim honoring plus the `resolved_path` echoed
    ///   back in every result is the cheaper, equally-safe guard (caller can
    ///   verify the target instead of being blocked from a legitimate one).
    /// - A **relative** `path` resolves against `root_override` when given
    ///   (a worktree caller's per-call retarget), falling back to the
    ///   indexed project root otherwise — unchanged default behavior.
    /// - The DB reindex key is only populated when the resolved absolute
    ///   path is actually under the indexed project root; writes that land
    ///   elsewhere (verbatim absolute path, or a `root_override` pointing
    ///   outside the index) skip reindexing since the DB has no record of
    ///   that tree.
    pub(crate) fn resolve_edit_target(
        &self,
        path: &str,
        root_override: Option<&str>,
    ) -> (PathBuf, Option<String>) {
        let p = Path::new(path);
        let abs_path = if p.is_absolute() {
            p.to_path_buf()
        } else {
            match root_override {
                Some(root) => Path::new(root).join(p),
                None => self.project_root.join(p),
            }
        };
        let rel_for_index = abs_path
            .strip_prefix(&self.project_root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"));
        (abs_path, rel_for_index)
    }

    /// Re-indexes a single file after an edit.
    pub(crate) async fn reindex_file(&self, file_path: &str) -> Result<()> {
        let abs_path = self.absolute_path(file_path);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read file {file_path}: {e}"),
        })?;

        let Some(extractor) = crate::project_manifest::resolve_extractor_for_source(
            &self.registry,
            &self.project_root,
            file_path,
            &source,
        ) else {
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
        let body_documents = build_executable_body_documents(file_path, &source, &result.nodes);
        self.db
            .insert_executable_body_documents(&body_documents)
            .await?;
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
            kind: FileKind::Code,
        };
        self.db.upsert_file(&file_record).await?;
        self.db.rebuild_trait_dispatch_callers().await?;

        Ok(())
    }

    /// Performs a single string replacement.
    /// Fails if `old_str` is not found or matches more than once.
    ///
    /// `root_override` retargets resolution of a *relative* `path` to a
    /// directory other than the indexed project root (e.g. a git worktree).
    /// An absolute `path` is always honored verbatim regardless of this
    /// parameter. See [`Self::resolve_edit_target`] for full semantics.
    pub async fn str_replace(
        &self,
        path: &str,
        old_str: &str,
        new_str: &str,
        root_override: Option<&str>,
    ) -> Result<EditResult> {
        let (abs_path, rel_path) = self.resolve_edit_target(path, root_override);
        let resolved_path = abs_path.to_string_lossy().to_string();
        let display_path = rel_path.clone().unwrap_or_else(|| resolved_path.clone());

        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {resolved_path}: {e}"),
        })?;

        let matches: Vec<_> = source.match_indices(old_str).collect();
        match matches.len() {
            0 => {
                return Ok(EditResult {
                    success: false,
                    file_path: display_path,
                    resolved_path,
                    matched_str: old_str.to_string(),
                    new_str: new_str.to_string(),
                    message: format!("old_str not found in {path}"),
                })
            }
            1 => {}
            n => {
                return Ok(EditResult {
                    success: false,
                    file_path: display_path,
                    resolved_path,
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
                message: format!("failed to write {resolved_path}: {e}"),
            })?;

        if let Some(rel) = &rel_path {
            self.reindex_file(rel).await?;
        }

        Ok(EditResult {
            success: true,
            file_path: display_path,
            resolved_path,
            matched_str: old_str.to_string(),
            new_str: new_str.to_string(),
            message: "replacement successful".to_string(),
        })
    }

    /// Applies multiple string replacements atomically.
    /// Fails if any `old_str` doesn't match exactly once.
    ///
    /// `root_override` retargets resolution of a *relative* `path` to a
    /// directory other than the indexed project root (e.g. a git worktree).
    /// An absolute `path` is always honored verbatim regardless of this
    /// parameter. See [`Self::resolve_edit_target`] for full semantics.
    pub async fn multi_str_replace(
        &self,
        path: &str,
        replacements: &[(&str, &str)],
        root_override: Option<&str>,
    ) -> Result<MultiEditResult> {
        let (abs_path, rel_path) = self.resolve_edit_target(path, root_override);
        let resolved_path = abs_path.to_string_lossy().to_string();
        let display_path = rel_path.clone().unwrap_or_else(|| resolved_path.clone());

        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {resolved_path}: {e}"),
        })?;

        for (old, _) in replacements {
            let count = source.matches(old).count();
            if count != 1 {
                return Ok(MultiEditResult {
                    success: false,
                    file_path: display_path,
                    resolved_path,
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
                message: format!("failed to write {resolved_path}: {e}"),
            })?;

        if let Some(rel) = &rel_path {
            self.reindex_file(rel).await?;
        }

        Ok(MultiEditResult {
            success: true,
            file_path: display_path,
            resolved_path,
            applied_count: replacements.len(),
            message: format!("applied {} replacements", replacements.len()),
        })
    }

    /// Inserts content before or after a unique anchor.
    /// Anchor can be a string or 1-indexed line number.
    ///
    /// `root_override` retargets resolution of a *relative* `path` to a
    /// directory other than the indexed project root (e.g. a git worktree).
    /// An absolute `path` is always honored verbatim regardless of this
    /// parameter. See [`Self::resolve_edit_target`] for full semantics.
    pub async fn insert_at(
        &self,
        path: &str,
        anchor: &str,
        content: &str,
        before: bool,
        root_override: Option<&str>,
    ) -> Result<InsertResult> {
        let (abs_path, rel_path) = self.resolve_edit_target(path, root_override);
        let resolved_path = abs_path.to_string_lossy().to_string();
        let display_path = rel_path.clone().unwrap_or_else(|| resolved_path.clone());

        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {resolved_path}: {e}"),
        })?;

        let lines: Vec<&str> = source.lines().collect();

        let anchor_line = if anchor.chars().all(|c| c.is_ascii_digit()) {
            let line_num: usize = anchor.parse().map_err(|_| TokenSaveError::Config {
                message: format!("invalid line number: {anchor}"),
            })?;
            if line_num == 0 || line_num > lines.len() {
                return Ok(InsertResult {
                    success: false,
                    file_path: display_path,
                    resolved_path,
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
                    file_path: display_path,
                    resolved_path,
                    anchor_line: 0,
                    content: content.to_string(),
                    before,
                    message: format!("anchor '{anchor}' not found"),
                });
            }
            if matching_lines.len() > 1 {
                return Ok(InsertResult {
                    success: false,
                    file_path: display_path,
                    resolved_path,
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
                message: format!("failed to write {resolved_path}: {e}"),
            })?;

        if let Some(rel) = &rel_path {
            self.reindex_file(rel).await?;
        }

        Ok(InsertResult {
            success: true,
            file_path: display_path,
            resolved_path,
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
    ///
    /// `root_override` retargets where the symbol's (index-relative) file
    /// path is written to — e.g. a git worktree that shares the same
    /// relative layout as the indexed project root but lives at a different
    /// absolute location. See [`Self::resolve_edit_target`] for semantics.
    pub async fn replace_symbol(
        &self,
        symbol: &str,
        new_source: &str,
        root_override: Option<&str>,
    ) -> Result<EditResult> {
        let target = resolve_symbol_for_edit(self, symbol).await?;
        let (abs_path, rel_path) = self.resolve_edit_target(&target.file_path, root_override);
        let resolved_path = abs_path.to_string_lossy().to_string();
        let display_path = rel_path.clone().unwrap_or_else(|| resolved_path.clone());
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {resolved_path}: {e}"),
        })?;
        let lines: Vec<&str> = source.lines().collect();
        let start = target.start_line as usize;
        let end_inclusive = (target.end_line as usize).min(lines.len().saturating_sub(1));
        if start >= lines.len() || start > end_inclusive {
            return Ok(EditResult {
                success: false,
                file_path: display_path,
                resolved_path,
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
                message: format!("failed to write {resolved_path}: {e}"),
            })?;
        if let Some(rel) = &rel_path {
            self.reindex_file(rel).await?;
        }
        Ok(EditResult {
            success: true,
            file_path: display_path,
            resolved_path,
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
    ///
    /// `root_override` retargets where the symbol's (index-relative) file
    /// path is written to. See [`Self::resolve_edit_target`] for semantics.
    pub async fn insert_at_symbol(
        &self,
        symbol: &str,
        content: &str,
        position: &str,
        root_override: Option<&str>,
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
        let (abs_path, rel_path) = self.resolve_edit_target(&target.file_path, root_override);
        let resolved_path = abs_path.to_string_lossy().to_string();
        let display_path = rel_path.clone().unwrap_or_else(|| resolved_path.clone());
        let source = std::fs::read_to_string(&abs_path).map_err(|e| TokenSaveError::Config {
            message: format!("failed to read {resolved_path}: {e}"),
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
                file_path: display_path,
                resolved_path,
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
                message: format!("failed to write {resolved_path}: {e}"),
            })?;
        if let Some(rel) = &rel_path {
            self.reindex_file(rel).await?;
        }
        Ok(InsertResult {
            success: true,
            file_path: display_path,
            resolved_path,
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
    ///
    /// `root_override` retargets resolution of a *relative* `path` to a
    /// directory other than the indexed project root (e.g. a git worktree).
    /// An absolute `path` is always honored verbatim regardless of this
    /// parameter. See [`Self::resolve_edit_target`] for full semantics.
    pub async fn ast_grep_rewrite(
        &self,
        path: &str,
        pattern: &str,
        rewrite: &str,
        root_override: Option<&str>,
    ) -> Result<AstGrepResult> {
        use std::process::Command;

        let (abs_path, rel_path) = self.resolve_edit_target(path, root_override);
        let resolved_path = abs_path.to_string_lossy().to_string();
        let display_path = rel_path.clone().unwrap_or_else(|| resolved_path.clone());

        let check_output = Command::new("ast-grep").args(["--version"]).output();

        if check_output.is_err() {
            if can_use_literal_rewrite_fallback(pattern) {
                let mut source = std::fs::read_to_string(&abs_path).map_err(TokenSaveError::Io)?;
                if !source.contains(pattern) {
                    return Ok(AstGrepResult {
                        success: false,
                        file_path: display_path,
                        resolved_path,
                        pattern: pattern.to_string(),
                        rewrite: rewrite.to_string(),
                        message: "pattern not found (built-in literal fallback)".to_string(),
                    });
                }
                source = source.replace(pattern, rewrite);
                std::fs::write(&abs_path, source).map_err(TokenSaveError::Io)?;
                if let Some(rel) = &rel_path {
                    self.reindex_file(rel).await?;
                }
                return Ok(AstGrepResult {
                    success: true,
                    file_path: display_path,
                    resolved_path,
                    pattern: pattern.to_string(),
                    rewrite: rewrite.to_string(),
                    message: "literal rewrite completed using built-in fallback".to_string(),
                });
            }
            return Ok(AstGrepResult {
                success: false,
                file_path: display_path,
                resolved_path,
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
                     File: {display_path}, pattern: {pattern:?}"
                )
            };
            return Ok(AstGrepResult {
                success: false,
                file_path: display_path,
                resolved_path,
                pattern: pattern.to_string(),
                rewrite: rewrite.to_string(),
                message,
            });
        }

        if let Some(rel) = &rel_path {
            self.reindex_file(rel).await?;
        }

        Ok(AstGrepResult {
            success: true,
            file_path: display_path,
            resolved_path,
            pattern: pattern.to_string(),
            rewrite: rewrite.to_string(),
            message: "ast-grep rewrite completed".to_string(),
        })
    }
}

fn build_executable_body_documents(
    file_path: &str,
    source: &str,
    nodes: &[Node],
) -> Vec<ExecutableBodyDocument> {
    let lines: Vec<&str> = source.lines().collect();
    nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::SingletonMethod
                    | NodeKind::StructMethod
                    | NodeKind::Constructor
                    | NodeKind::AbstractMethod
                    | NodeKind::Procedure
                    | NodeKind::ArrowFunction
            )
        })
        .filter_map(|node| {
            let start = node.start_line as usize;
            let end = (node.end_line as usize).saturating_add(1).min(lines.len());
            (start < end).then(|| ExecutableBodyDocument {
                node_id: node.id.clone(),
                file_path: file_path.to_string(),
                body: lines[start..end].join("\n"),
            })
        })
        .collect()
}

pub(crate) fn can_use_literal_rewrite_fallback(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    !trimmed.is_empty()
        && trimmed == pattern
        && !pattern.contains('$')
        && !pattern.contains('\n')
        && !pattern.contains('\r')
}

#[cfg(test)]
mod ruby_singleton_repair_tests {
    use super::legacy_ruby_repair_complete;
    use std::collections::HashSet;

    #[test]
    fn marker_requires_every_scheduled_legacy_ruby_file() {
        let scheduled = vec!["publisher.rb".to_string(), "report.rb".to_string()];
        let incomplete = HashSet::from(["publisher.rb"]);
        assert!(!legacy_ruby_repair_complete(true, &scheduled, &incomplete));

        let complete = HashSet::from(["publisher.rb", "report.rb"]);
        assert!(legacy_ruby_repair_complete(true, &scheduled, &complete));
        assert!(legacy_ruby_repair_complete(false, &scheduled, &incomplete));
    }
}

/// Unit coverage for the #327 walk-scope predicate. Pure path relations run on
/// every platform (including the Windows verbatim spellings `canonicalize`
/// returns there); the filesystem-resolving wrapper is exercised through real
/// symlinks in `tests/symlink_walk_scope_test.rs`.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod walk_scope_tests {
    use super::{path_contains_root, reenters_project_root};
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn root_itself_contains_root() {
        let root = Path::new("/home/user/proj");
        assert!(path_contains_root(root, root));
    }

    #[test]
    fn ancestor_contains_root() {
        let root = Path::new("/home/user/proj");
        assert!(path_contains_root(Path::new("/"), root));
        assert!(path_contains_root(Path::new("/home"), root));
        assert!(path_contains_root(Path::new("/home/user"), root));
    }

    #[test]
    fn descendant_does_not_contain_root() {
        let root = Path::new("/home/user/proj");
        assert!(!path_contains_root(Path::new("/home/user/proj/src"), root));
    }

    #[test]
    fn disjoint_tree_does_not_contain_root() {
        let root = Path::new("/home/user/proj");
        assert!(!path_contains_root(Path::new("/home/user/other"), root));
        assert!(!path_contains_root(Path::new("/opt/src"), root));
    }

    #[test]
    fn prefix_sharing_sibling_does_not_contain_root() {
        let root = Path::new("/home/user/proj");
        assert!(!path_contains_root(Path::new("/home/user2"), root));
        assert!(!path_contains_root(Path::new("/home/user/proj-old"), root));
    }

    // Only Windows parses the verbatim prefixes into components; on Unix the
    // whole spelling is a single opaque file name.
    #[cfg(windows)]
    #[test]
    fn windows_verbatim_relations_compare_by_component() {
        // `canonicalize` yields `\\?\C:\…` / `\\?\UNC\…` spellings on Windows;
        // both sides go through it, so the comparison stays component-wise.
        let root = Path::new(r"\\?\C:\repo");
        assert!(path_contains_root(Path::new(r"\\?\C:\"), root));
        assert!(path_contains_root(root, root));
        assert!(!path_contains_root(Path::new(r"\\?\C:\repo-old"), root));
        assert!(!path_contains_root(Path::new(r"\\?\C:\repo\src"), root));

        let unc = Path::new(r"\\?\UNC\server\share\repo");
        assert!(path_contains_root(Path::new(r"\\?\UNC\server\share"), unc));
        assert!(!path_contains_root(
            Path::new(r"\\?\UNC\server\share\repo-old"),
            unc
        ));
    }

    #[test]
    fn unknown_root_never_prunes() {
        assert!(!reenters_project_root(Path::new("/"), None));
    }

    #[test]
    fn unresolvable_path_never_prunes() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let missing = root.join("tokensave-327-does-not-exist");
        assert!(!missing.exists(), "the fixture path must not exist");
        assert!(!reenters_project_root(&missing, Some(&root)));
    }
}
