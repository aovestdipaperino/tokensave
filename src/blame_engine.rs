//! Per-symbol git blame / log engine.
//!
//! Walks a file's commit history via `gix`, fetches the blob at each
//! commit, parses it with `redundancy::parse_file`, and matches a target
//! symbol across commits via `redundancy::Fingerprint` similarity.

use std::path::Path;

use crate::redundancy::Fingerprint;

/// Why the history walk terminated.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryReason {
    /// The earliest commit examined created (or first introduced) the entity.
    Introduced,
    /// The entity moved from a different file at this boundary.
    RenamedFrom,
    /// The walk ran out of parent commits.
    HistoryExhausted,
    /// `max_commits` was reached before history was exhausted.
    MaxCommitsReached,
}

/// A single commit at which the target entity changed structurally.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChangeEvent {
    pub commit: String,
    pub short_sha: String,
    pub author: String,
    pub email: String,
    pub date: String, // RFC3339
    pub summary: String,
    pub file_at_commit: String,
}

/// Tunables passed in by the caller.
#[derive(Debug, Clone)]
pub struct BlameOptions {
    pub max_commits: usize,
    pub similarity_threshold: f64,
    pub max_blob_bytes: u64,
}

impl Default for BlameOptions {
    fn default() -> Self {
        Self {
            max_commits: 500,
            // Identity threshold for matching the target entity across
            // commits. This is NOT the "did the body change" gate — that
            // job is done by ast_hash inequality elsewhere. The threshold
            // exists only to filter out unrelated entities in the same
            // file. Heavy body rewrites drop composite_similarity into
            // the 0.2-0.4 range, so 0.1 is the v1 default to keep
            // tracking through rewrites; we can tighten it later if
            // false-positive tracking becomes an issue.
            similarity_threshold: 0.1,
            max_blob_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Full result returned to the handlers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlameResult {
    pub events: Vec<ChangeEvent>,
    pub boundary_reason: BoundaryReason,
    pub commits_walked: usize,
    pub parse_failures: Vec<ParseFailure>,
    pub skipped_large: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ParseFailure {
    pub commit: String,
    pub error: String,
}

/// Compute the change-event history for an entity.
///
/// `target_fp` is the current working-tree fingerprint of the entity.
/// `start_line` and `end_line` are **0-indexed** (matching `Node` row
/// semantics).
pub fn log(
    project_root: &Path,
    file: &str,
    _start_line: u32,
    _end_line: u32,
    language_key: &str,
    target_fp: &Fingerprint,
    opts: &BlameOptions,
) -> Result<BlameResult, String> {
    use crate::extraction::ts_provider;
    use crate::redundancy::parse_file;

    if !lang_key_is_known(language_key) {
        return Err(format!("unknown language key '{language_key}'"));
    }
    let lang = ts_provider::language(language_key);

    let walk_result = walk_file_history(project_root, file, opts.max_commits)?;
    let commits_walked = walk_result.total_visited;
    let visits = walk_result.visits;
    let hit_max = walk_result.hit_max;
    let mut events: Vec<ChangeEvent> = Vec::new();
    let mut parse_failures: Vec<ParseFailure> = Vec::new();
    let mut skipped_large: Vec<String> = Vec::new();
    // `pending` holds the oldest-so-far commit for the current ast_hash run.
    // We flush it when the hash changes or when we reach a boundary.
    let mut pending: Option<(ChangeEvent, String)> = None;
    let mut found_introduction = false;

    let repo = gix::open(project_root).map_err(|e| format!("gix open: {e}"))?;

    // visits are newest-first. We want to emit one event per distinct
    // ast_hash run, attributed to the OLDEST commit in that run (the
    // commit that first introduced that body). To achieve this we keep a
    // `pending` slot and update it as we walk into older commits with the
    // same hash.
    for visit in &visits {
        if visit.blob_size > opts.max_blob_bytes {
            skipped_large.push(visit.short_sha.clone());
            continue;
        }
        let blob = repo
            .find_object(visit.blob_id)
            .map_err(|e| format!("cannot read blob {}: {e}", visit.short_sha))?;
        let source = if let Ok(s) = std::str::from_utf8(&blob.data) {
            s.to_string()
        } else {
            parse_failures.push(ParseFailure {
                commit: visit.short_sha.clone(),
                error: "blob is not valid UTF-8".to_string(),
            });
            continue;
        };
        let Some(tree) = parse_file(&source, &lang) else {
            parse_failures.push(ParseFailure {
                commit: visit.short_sha.clone(),
                error: "tree-sitter parse failed".to_string(),
            });
            continue;
        };
        // For identity-tracking across commits we use a very permissive
        // lower bound (0.1) rather than the clone-detection threshold.
        // The `ast_hash` comparison below is the real change-detection gate.
        let Some(matched) = best_match_in_tree(&source, &tree, target_fp, opts.similarity_threshold)
        else {
            // Entity vanished — probe other files touched in the next-older
            // commit (the boundary commit) for a structural match.
            // Flush pending before building combined to avoid losing the last
            // post-rename event collected so far.
            if let Some((ev, _)) = pending.take() {
                events.push(ev);
            }
            if let Some(renamed) = probe_rename_at_boundary(
                &repo,
                project_root,
                visit,
                target_fp,
                opts,
            )? {
                // Switch the walk to follow the prior file. We rerun `log` on
                // that file path starting from the boundary commit's parent.
                let prior = log_from_commit(
                    project_root,
                    &renamed.prior_file,
                    visit.commit_id,
                    language_key,
                    target_fp,
                    opts,
                )?;
                // Pre-rename events first (oldest-first), then the post-rename
                // events we already collected (already oldest-first after the
                // reverse below).
                let mut combined = prior.events;
                events.reverse();
                combined.extend(events);
                return Ok(BlameResult {
                    events: combined,
                    boundary_reason: BoundaryReason::RenamedFrom,
                    commits_walked: commits_walked + prior.commits_walked,
                    parse_failures: {
                        let mut p = prior.parse_failures;
                        p.extend(parse_failures);
                        p
                    },
                    skipped_large: {
                        let mut s = prior.skipped_large;
                        s.extend(skipped_large);
                        s
                    },
                });
            }
            found_introduction = true;
            break;
        };

        let ev = ChangeEvent {
            commit: visit.commit_id.to_string(),
            short_sha: visit.short_sha.clone(),
            author: visit.author.clone(),
            email: visit.email.clone(),
            date: visit.date_rfc3339.clone(),
            summary: visit.summary.clone(),
            file_at_commit: file.to_string(),
        };

        match pending.take() {
            None => {
                // First match: start a new pending run.
                pending = Some((ev, matched.ast_hash));
            }
            Some((prev_ev, prev_hash)) => {
                if prev_hash == matched.ast_hash {
                    // Same hash as the previous (newer) commit — update the
                    // pending event to point to this older commit (we want
                    // to attribute the run to its oldest commit).
                    pending = Some((ev, matched.ast_hash));
                } else {
                    // Hash changed: flush the previous run's event and start
                    // a new pending run for this commit's hash.
                    events.push(prev_ev);
                    pending = Some((ev, matched.ast_hash));
                }
            }
        }
    }

    // Flush the final pending event (history exhausted or max reached).
    if let Some((ev, _)) = pending.take() {
        events.push(ev);
    }

    // Post-loop rename probe: if the walk ended naturally (not due to a
    // mid-blob "entity not found") and the entity was still present in the
    // oldest-visited commit, check whether that commit introduced the file
    // (parent doesn't have it). If so, the file might have been renamed from
    // another file — probe the boundary commit's parent for a structural match.
    if !found_introduction && !hit_max {
        if let Some(oldest_visit) = visits.last() {
            if let Some(renamed) = probe_rename_at_boundary(
                &repo,
                project_root,
                oldest_visit,
                target_fp,
                opts,
            )? {
                let prior = log_from_commit(
                    project_root,
                    &renamed.prior_file,
                    oldest_visit.commit_id,
                    language_key,
                    target_fp,
                    opts,
                )?;
                // Pre-rename events first (oldest-first), then the post-rename
                // events we already collected. events is still newest-first here
                // so reverse it first.
                let mut combined = prior.events;
                events.reverse();
                combined.extend(events);
                return Ok(BlameResult {
                    events: combined,
                    boundary_reason: BoundaryReason::RenamedFrom,
                    commits_walked: commits_walked + prior.commits_walked,
                    parse_failures: {
                        let mut p = prior.parse_failures;
                        p.extend(parse_failures);
                        p
                    },
                    skipped_large: {
                        let mut s = prior.skipped_large;
                        s.extend(skipped_large);
                        s
                    },
                });
            }
        }
    }

    // Oldest-first for callers.
    events.reverse();

    let boundary_reason = if found_introduction {
        BoundaryReason::Introduced
    } else if hit_max {
        BoundaryReason::MaxCommitsReached
    } else if !events.is_empty() {
        // We exhausted history (no more parent commits) and the entity was
        // still present in the oldest commit → that commit introduced it.
        BoundaryReason::Introduced
    } else {
        BoundaryReason::HistoryExhausted
    };

    Ok(BlameResult {
        events,
        boundary_reason,
        commits_walked,
        parse_failures,
        skipped_large,
    })
}

/// Walk every node in `tree`, fingerprint each function-like body, and
/// return the best-matching fingerprint (above `threshold`) against `target`.
fn best_match_in_tree(
    source: &str,
    tree: &tree_sitter::Tree,
    target: &Fingerprint,
    threshold: f64,
) -> Option<Fingerprint> {
    use crate::redundancy::composite_similarity;

    let mut best: Option<(f64, Fingerprint)> = None;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if is_entity_node(n.kind()) {
            let fp = crate::redundancy::compute_fingerprint(source, n);
            let score = composite_similarity(target, &fp);
            if score >= threshold {
                let better = best.as_ref().is_none_or(|(s, _)| score > *s);
                if better {
                    best = Some((score, fp));
                }
            }
        }
        let mut cursor = n.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    best.map(|(_, fp)| fp)
}

struct RenameMatch {
    prior_file: String,
}

/// At a "vanish" boundary, walk to the next-older commit and probe every
/// file present at that older commit for a structural match.
///
/// # Recursion safety
/// `log_from_commit` calls `log` again with a different file. If the entity
/// also disappears from the prior file in its own history walk, the probe will
/// recurse. In v1 this is bounded by `max_commits` and the structure of git
/// history (each rename traverses a different commit set), so no infinite-loop
/// guard is needed for realistic fixtures. Pathological cases (e.g. repeated
/// cross-file bouncing) can exhaust the stack in theory; a depth counter can
/// be added to `BlameOptions` if this becomes an issue.
fn probe_rename_at_boundary(
    repo: &gix::Repository,
    project_root: &std::path::Path,
    boundary_visit: &CommitVisit,
    target_fp: &Fingerprint,
    opts: &BlameOptions,
) -> Result<Option<RenameMatch>, String> {
    use crate::extraction::ts_provider;
    use crate::redundancy::parse_file;

    // Resolve the parent of the boundary commit.
    let commit = repo
        .find_object(boundary_visit.commit_id)
        .map_err(|e| format!("cannot find boundary commit: {e}"))?
        .try_into_commit()
        .map_err(|e| format!("not a commit: {e}"))?;
    let Some(parent_id) = commit.parent_ids().next() else {
        return Ok(None);
    };
    let parent = repo
        .find_object(parent_id.detach())
        .map_err(|e| format!("cannot find parent: {e}"))?
        .try_into_commit()
        .map_err(|e| format!("not a parent commit: {e}"))?;

    // Files changed between parent and boundary tell us the candidate set.
    let parent_tree = parent.tree().map_err(|e| format!("parent tree: {e}"))?;
    let boundary_tree = commit.tree().map_err(|e| format!("boundary tree: {e}"))?;

    let mut candidates: Vec<String> = Vec::new();
    parent_tree
        .changes()
        .map_err(|e| format!("diff init: {e}"))?
        .for_each_to_obtain_tree(&boundary_tree, |change| {
            use gix::object::tree::diff::Change;
            match &change {
                Change::Deletion { location, entry_mode, .. }
                | Change::Modification { location, entry_mode, .. }
                | Change::Addition { location, entry_mode, .. } => {
                    if !entry_mode.is_tree() {
                        candidates.push(location.to_string());
                    }
                }
                Change::Rewrite {
                    source_location,
                    location,
                    source_entry_mode,
                    entry_mode,
                    ..
                } => {
                    if !source_entry_mode.is_tree() {
                        candidates.push(source_location.to_string());
                    }
                    if !entry_mode.is_tree() {
                        candidates.push(location.to_string());
                    }
                }
            }
            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
        })
        .map_err(|e| format!("tree diff: {e}"))?;

    let mut best: Option<(f64, String)> = None;
    for cand in candidates {
        let Some(lang_key) = ts_lang_key_from_path(&cand) else {
            continue;
        };
        let Some(entry) = parent_tree
            .lookup_entry_by_path(std::path::Path::new(&cand))
            .map_err(|e| format!("lookup_entry_by_path: {e}"))?
        else {
            continue;
        };
        let blob = repo
            .find_object(entry.object_id())
            .map_err(|e| format!("blob lookup: {e}"))?;
        if blob.data.len() as u64 > opts.max_blob_bytes {
            continue;
        }
        let Ok(source) = std::str::from_utf8(&blob.data) else {
            continue;
        };
        let lang = ts_provider::language(lang_key);
        let Some(tree) = parse_file(source, &lang) else {
            continue;
        };
        if let Some(fp) =
            best_match_in_tree(source, &tree, target_fp, opts.similarity_threshold)
        {
            let score = crate::redundancy::composite_similarity(target_fp, &fp);
            let better = best.as_ref().is_none_or(|(s, _)| score > *s);
            if better {
                best = Some((score, cand));
            }
        }
    }

    // `project_root` is reserved for a future on-disk optimisation.
    let _ = project_root;

    Ok(best.map(|(_, prior_file)| RenameMatch { prior_file }))
}

/// Recursive helper: re-run the engine on a different file. The walker
/// begins at HEAD and naturally includes pre-rename commits because the
/// pre-rename file existed there. `boundary_commit` is currently unused
/// — reserved for a future "start from this commit" optimisation that
/// avoids re-walking the post-rename history.
fn log_from_commit(
    project_root: &std::path::Path,
    file: &str,
    boundary_commit: gix::ObjectId,
    language_key: &str,
    target_fp: &Fingerprint,
    opts: &BlameOptions,
) -> Result<BlameResult, String> {
    let _ = boundary_commit;
    log(project_root, file, 0, 0, language_key, target_fp, opts)
}

/// True for tree-sitter node kinds that bound an identifiable code entity
/// (function, method, class, struct, etc.) across the supported languages.
/// This list is intentionally permissive — extra kinds just add work to
/// the fingerprint loop; missing kinds cause false "not found" boundaries.
fn is_entity_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_declaration"
            | "method_definition"
            | "function"
            | "method"
            | "impl_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "class_declaration"
            | "class_definition"
            | "interface_declaration"
            | "type_alias_declaration"
    )
}

/// One commit that touched the watched file.
#[derive(Debug, Clone)]
pub(crate) struct CommitVisit {
    pub commit_id: gix::ObjectId,
    pub short_sha: String,
    pub author: String,
    pub email: String,
    pub date_rfc3339: String,
    pub summary: String,
    pub blob_id: gix::ObjectId,
    pub blob_size: u64,
}

/// Result of walking a file's commit history.
pub(crate) struct WalkResult {
    /// Commits where the file's blob actually changed (yielded entries).
    pub visits: Vec<CommitVisit>,
    /// Total number of commits inspected (includes commits where the blob
    /// was unchanged, i.e. `total_visited >= visits.len()`).
    pub total_visited: usize,
    /// `true` when the walk stopped because `max_commits` was reached rather
    /// than because history was exhausted.
    pub hit_max: bool,
}

/// Walk back from HEAD, returning only commits where the named file's blob
/// changed (added, modified, or removed). Stops after `max_commits` total
/// commits *visited* (not yielded). Reverse chronological order.
///
/// **First-parent only**: the walker follows only the first parent of each
/// commit (equivalent to `git log --first-parent`). Commits reachable only
/// via merge-branch sides are skipped. This keeps the walk fast and
/// deterministic on mainline histories but means changes introduced
/// exclusively on a feature branch before merge are not surfaced.
pub(crate) fn walk_file_history(
    project_root: &std::path::Path,
    file_path: &str,
    max_commits: usize,
) -> Result<WalkResult, String> {
    let repo = gix::open(project_root).map_err(|e| format!("failed to open git repo: {e}"))?;
    let head = repo
        .head()
        .map_err(|e| format!("cannot read HEAD: {e}"))?
        .into_peeled_id()
        .map_err(|e| format!("cannot peel HEAD: {e}"))?;

    let mut visits = Vec::new();
    let mut last_blob_id: Option<gix::ObjectId> = None;
    let mut visited = 0_usize;
    let mut current_id = head.detach();
    let mut exhausted = false;

    while visited < max_commits {
        let commit = repo
            .find_object(current_id)
            .map_err(|e| format!("cannot find commit object: {e}"))?
            .try_into_commit()
            .map_err(|e| format!("not a commit: {e}"))?;

        let tree = commit
            .tree()
            .map_err(|e| format!("cannot read tree for commit {current_id}: {e}"))?;
        let entry = tree
            .lookup_entry_by_path(std::path::Path::new(file_path))
            .map_err(|e| format!("lookup_entry_by_path failed: {e}"))?;

        // If the file existed in this commit AND its blob differs from the
        // newer-side blob we last recorded, yield this commit.
        if let Some(entry) = entry {
            let blob_id = entry.object_id();
            let differs = last_blob_id != Some(blob_id);
            if differs {
                let blob = repo
                    .find_object(blob_id)
                    .map_err(|e| format!("cannot find blob: {e}"))?;
                let blob_size = blob.data.len() as u64;
                let (author, email, date, summary) = commit_metadata(&commit)?;
                visits.push(CommitVisit {
                    commit_id: current_id,
                    short_sha: format!("{current_id:.7}"),
                    author,
                    email,
                    date_rfc3339: date,
                    summary,
                    blob_id,
                    blob_size,
                });
                last_blob_id = Some(blob_id);
            }
        }

        visited += 1;
        let parent_id = commit.parent_ids().next().map(gix::Id::detach);
        if let Some(pid) = parent_id {
            current_id = pid;
        } else {
            exhausted = true;
            break;
        }
    }

    // The loop exits either by exhausting parents (exhausted=true) or by the
    // `visited < max_commits` guard. Only the latter means we truly hit the cap.
    // When the repo has exactly `max_commits` commits the loop exits naturally
    // via the None branch, so `visited == max_commits` alone is not sufficient.
    let hit_max = !exhausted && visited == max_commits;

    Ok(WalkResult { visits, total_visited: visited, hit_max })
}

fn commit_metadata(
    commit: &gix::Commit<'_>,
) -> Result<(String, String, String, String), String> {
    let author_sig = commit
        .author()
        .map_err(|e| format!("cannot decode author: {e}"))?;
    let author = author_sig.name.to_string();
    let email = author_sig.email.to_string();
    let secs = author_sig.seconds();
    let date = format_rfc3339(secs);
    let message = commit
        .message_raw()
        .map_err(|e| format!("cannot read commit message: {e}"))?;
    let summary = std::str::from_utf8(message.as_ref())
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    Ok((author, email, date, summary))
}

fn format_rfc3339(unix_secs: i64) -> String {
    // gix doesn't ship a time formatter; format manually as UTC.
    let (yr, mo, dy, hr, mn, sc) = ymd_hms_from_unix(unix_secs);
    format!("{yr:04}-{mo:02}-{dy:02}T{hr:02}:{mn:02}:{sc:02}Z")
}

#[allow(clippy::many_single_char_names)] // algorithm variables match the reference
fn ymd_hms_from_unix(mut ts: i64) -> (i32, u32, u32, u32, u32, u32) {
    // Howard Hinnant's `civil_from_days`, simplified for UTC.
    let day_sec = ts.rem_euclid(86_400);
    ts = ts.div_euclid(86_400);
    let hour = (day_sec / 3600) as u32;
    let min = ((day_sec % 3600) / 60) as u32;
    let sec = (day_sec % 60) as u32;

    let z = ts + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (y + i64::from(month <= 2)) as i32;
    (year, month, day, hour, min, sec)
}

/// Map a project-relative file path to the `ts_provider` language key.
///
/// Returns `None` if the extension isn't recognised by any tree-sitter
/// grammar bundled with `tokensave-large-treesitters`. Keys must match
/// those accepted by `crate::extraction::ts_provider::language`.
pub fn ts_lang_key_from_path(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "rs" => "rust",
        "go" => "go",
        "py" | "pyi" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "cs" => "c_sharp",
        "rb" => "ruby",
        "php" => "php",
        "scala" | "sc" => "scala",
        "dart" => "dart",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "sh" | "bash" => "bash",
        "nix" => "nix",
        "zig" => "zig",
        "proto" => "protobuf",
        _ => return None,
    })
}

/// Parse `source` with the given language key, find the node enclosing the
/// 0-indexed line range, and compute its `Fingerprint`.
///
/// Returns `None` if the language key is unknown to `ts_provider`, parsing
/// fails, or no node matches the line range.
pub fn compute_target_fingerprint(
    source: &str,
    language_key: &str,
    start_line: u32,
    end_line: u32,
) -> Option<Fingerprint> {
    use crate::extraction::ts_provider;
    use crate::redundancy::{compute_fingerprint, find_node_at_lines, parse_file};

    // `ts_provider::language` panics on unknown keys, so guard first.
    if !lang_key_is_known(language_key) {
        return None;
    }
    let lang = ts_provider::language(language_key);
    let tree = parse_file(source, &lang)?;
    let node = find_node_at_lines(&tree, start_line, end_line)?;
    Some(compute_fingerprint(source, node))
}

/// Returns true when `key` is registered in `ts_provider::LANGUAGES`.
///
/// `ts_provider::language` panics on unknown keys; this helper lets the
/// engine reject them gracefully.
fn lang_key_is_known(key: &str) -> bool {
    matches!(
        key,
        "bash" | "batch" | "c" | "c_sharp" | "clojure" | "cobol" | "cpp" | "dart"
            | "dockerfile" | "elixir" | "erlang" | "fortran" | "fsharp" | "glsl"
            | "go" | "gwbasic" | "haskell" | "java" | "javascript" | "julia"
            | "kotlin" | "lean" | "lua" | "msbasic2" | "nix" | "objc" | "ocaml"
            | "pascal" | "perl" | "php" | "powershell" | "protobuf" | "python"
            | "qbasic" | "quint" | "r" | "ruby" | "rust" | "scala" | "sql"
            | "swift" | "toml" | "tsx" | "typescript" | "vbnet" | "zig"
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn lang_key_recognises_common_extensions() {
        assert_eq!(ts_lang_key_from_path("src/foo.rs"), Some("rust"));
        assert_eq!(ts_lang_key_from_path("a.tsx"), Some("tsx"));
        assert_eq!(ts_lang_key_from_path("a.ts"), Some("typescript"));
        assert_eq!(ts_lang_key_from_path("a.proto"), Some("protobuf"));
        assert_eq!(ts_lang_key_from_path("a.cs"), Some("c_sharp"));
        assert_eq!(ts_lang_key_from_path("README.md"), None);
    }

    #[test]
    fn default_options_match_spec() {
        let opts = BlameOptions::default();
        assert_eq!(opts.max_commits, 500);
        assert!((opts.similarity_threshold - 0.1).abs() < f64::EPSILON);
        assert_eq!(opts.max_blob_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn compute_target_fingerprint_from_rust_source() {
        let source = "pub fn add(a: i32, b: i32) -> i32 { a + b }\n";
        let fp = compute_target_fingerprint(source, "rust", 0, 0)
            .expect("rust language must be available");
        // Body has tokens; fingerprint non-empty
        assert!(fp.body_tokens > 0);
        assert!(!fp.ast_hash.is_empty());
    }

    #[test]
    fn compute_target_fingerprint_returns_none_for_unknown_lang() {
        let source = "anything";
        assert!(compute_target_fingerprint(source, "no_such_lang_key", 0, 0).is_none());
    }

    #[test]
    fn log_returns_events_when_function_body_changes() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            let st = Command::new("git").current_dir(root).args(args).status().unwrap();
            assert!(st.success());
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // c1: original body
        std::fs::write(root.join("foo.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        git(&["add", "foo.rs"]);
        git(&["commit", "-q", "-m", "c1: initial"]);

        // c2: change comment only — should NOT yield an event (fingerprint unchanged)
        std::fs::write(
            root.join("foo.rs"),
            "// trivial helper\npub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .unwrap();
        git(&["commit", "-q", "-am", "c2: comment"]);

        // c3: real mutation
        std::fs::write(
            root.join("foo.rs"),
            "// trivial helper\npub fn add(a: i32, b: i32) -> i32 { let s = a + b; s }\n",
        )
        .unwrap();
        git(&["commit", "-q", "-am", "c3: rebody"]);

        let source = std::fs::read_to_string(root.join("foo.rs")).unwrap();
        // c3 leaves the `pub fn add` body starting on line 2 (0-indexed: 1).
        let fp = compute_target_fingerprint(&source, "rust", 1, 1).expect("fp");
        let result = log(root, "foo.rs", 1, 1, "rust", &fp, &BlameOptions::default()).expect("log");

        // Expect two distinct mutation events (c1 introduction + c3 rebody).
        // c2 (comment-only) should be filtered because the fingerprint matches c3.
        assert_eq!(result.events.len(), 2, "got: {:#?}", result.events);
        // Oldest first
        assert!(result.events[0].summary.contains("c1"));
        assert!(result.events[1].summary.contains("c3"));
        assert_eq!(result.boundary_reason, BoundaryReason::Introduced);
    }

    #[test]
    fn log_follows_function_renamed_across_files() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            let st = Command::new("git").current_dir(root).args(args).status().unwrap();
            assert!(st.success());
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // c1: function in a.rs
        std::fs::write(root.join("a.rs"), "pub fn helper() -> i32 { 42 }\n").unwrap();
        git(&["add", "a.rs"]);
        git(&["commit", "-q", "-m", "c1: born in a.rs"]);

        // c2: function moves to b.rs (a.rs deleted, b.rs created)
        std::fs::remove_file(root.join("a.rs")).unwrap();
        std::fs::write(root.join("b.rs"), "pub fn helper() -> i32 { 42 }\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "c2: moved to b.rs"]);

        // c3: change body in b.rs
        std::fs::write(root.join("b.rs"), "pub fn helper() -> i32 { 43 }\n").unwrap();
        git(&["commit", "-q", "-am", "c3: edit in b.rs"]);

        let source = std::fs::read_to_string(root.join("b.rs")).unwrap();
        let fp = compute_target_fingerprint(&source, "rust", 0, 0).expect("fp");
        let result = log(root, "b.rs", 0, 0, "rust", &fp, &BlameOptions::default()).expect("log");

        // Should have at least one event from each side of the rename, and the
        // boundary should be RenamedFrom (with the prior file recorded).
        assert!(result.events.iter().any(|e| e.file_at_commit == "a.rs"),
            "expected an event from a.rs in {:#?}", result.events);
        assert_eq!(result.boundary_reason, BoundaryReason::RenamedFrom);
    }

    #[test]
    fn max_commits_boundary_classifies_correctly() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            Command::new("git").current_dir(root).args(args).status().unwrap();
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // 6 commits, all changing foo.rs
        for i in 0..6 {
            std::fs::write(
                root.join("foo.rs"),
                format!("pub fn add() -> i32 {{ {i} }}\n"),
            )
            .unwrap();
            if i == 0 {
                git(&["add", "foo.rs"]);
            }
            git(&["commit", "-q", "-am", &format!("c{i}")]);
        }

        let opts = BlameOptions {
            max_commits: 3,
            ..BlameOptions::default()
        };
        let source = std::fs::read_to_string(root.join("foo.rs")).unwrap();
        let fp = compute_target_fingerprint(&source, "rust", 0, 0).expect("fp");
        let result = log(root, "foo.rs", 0, 0, "rust", &fp, &opts).expect("log");
        assert_eq!(result.commits_walked, 3);
        assert_eq!(result.boundary_reason, BoundaryReason::MaxCommitsReached);
    }

    #[test]
    fn blob_size_cap_records_skipped() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let git = |args: &[&str]| {
            Command::new("git").current_dir(root).args(args).status().unwrap();
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "T"]);
        git(&["config", "commit.gpgsign", "false"]);

        // Write a large file (3 MB of comments + a tiny function)
        let huge = format!(
            "// {}\npub fn tiny() -> i32 {{ 1 }}\n",
            "x".repeat(3 * 1024 * 1024)
        );
        std::fs::write(root.join("foo.rs"), &huge).unwrap();
        git(&["add", "foo.rs"]);
        git(&["commit", "-q", "-m", "huge"]);

        let opts = BlameOptions {
            max_blob_bytes: 1024, // tiny cap to force skip
            ..BlameOptions::default()
        };
        let small_src = "pub fn tiny() -> i32 { 1 }\n";
        let fp = compute_target_fingerprint(small_src, "rust", 0, 0).expect("fp");
        let result = log(root, "foo.rs", 0, 0, "rust", &fp, &opts).expect("log");
        assert_eq!(result.skipped_large.len(), 1);
    }

    #[test]
    fn walk_history_yields_commits_in_reverse_chrono_order() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        // Bootstrap a tiny repo with three commits touching foo.rs.
        let run = |args: &[&str]| {
            let st = Command::new("git").current_dir(root).args(args).status().unwrap();
            assert!(st.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@t"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "commit.gpgsign", "false"]);

        std::fs::write(root.join("foo.rs"), "fn a() {}\n").unwrap();
        run(&["add", "foo.rs"]);
        run(&["commit", "-q", "-m", "c1"]);

        std::fs::write(root.join("foo.rs"), "fn a() { let _ = 1; }\n").unwrap();
        run(&["commit", "-q", "-am", "c2"]);

        std::fs::write(root.join("foo.rs"), "fn a() { let _ = 2; }\n").unwrap();
        run(&["commit", "-q", "-am", "c3"]);

        let walk = walk_file_history(root, "foo.rs", 10).expect("walk");
        let commits = &walk.visits;
        assert_eq!(commits.len(), 3, "expected 3 commits, got {commits:?}");
        // Reverse chrono: c3 first, c1 last.
        assert!(commits[0].summary.contains("c3"));
        assert!(commits[2].summary.contains("c1"));
    }
}
