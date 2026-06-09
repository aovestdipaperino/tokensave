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

    let visits = walk_file_history(project_root, file, opts.max_commits)?;
    let commits_walked = visits.len();
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
            // The entity does not exist in this revision → boundary.
            // Flush any pending event before marking introduction.
            if let Some((ev, _)) = pending.take() {
                events.push(ev);
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

    // Oldest-first for callers.
    events.reverse();

    let boundary_reason = if found_introduction {
        BoundaryReason::Introduced
    } else if commits_walked >= opts.max_commits {
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

/// Walk back from HEAD, returning only commits where the named file's blob
/// changed (added, modified, or removed). Stops after `max_commits` total
/// commits *visited* (not yielded). Reverse chronological order.
pub(crate) fn walk_file_history(
    project_root: &std::path::Path,
    file_path: &str,
    max_commits: usize,
) -> Result<Vec<CommitVisit>, String> {
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
        match parent_id {
            Some(pid) => current_id = pid,
            None => break,
        }
    }

    Ok(visits)
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

/// Convenience wrapper: returns the most recent change event, or `None` if
/// the entity was never mutated in tracked history.
pub fn blame(
    project_root: &Path,
    file: &str,
    start_line: u32,
    end_line: u32,
    language_key: &str,
    target_fp: &Fingerprint,
    opts: &BlameOptions,
) -> Result<Option<ChangeEvent>, String> {
    let result = log(
        project_root,
        file,
        start_line,
        end_line,
        language_key,
        target_fp,
        opts,
    )?;
    Ok(result.events.into_iter().next_back())
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
    fn walk_history_yields_commits_in_reverse_chrono_order() {
        use std::process::Command;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        // Bootstrap a tiny repo with three commits touching foo.rs.
        let run = |args: &[&str]| {
            let st = Command::new("git").current_dir(root).args(args).status().unwrap();
            assert!(st.success(), "git {:?} failed", args);
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

        let commits = walk_file_history(root, "foo.rs", 10).expect("walk");
        assert_eq!(commits.len(), 3, "expected 3 commits, got {commits:?}");
        // Reverse chrono: c3 first, c1 last.
        assert!(commits[0].summary.contains("c3"));
        assert!(commits[2].summary.contains("c1"));
    }
}
