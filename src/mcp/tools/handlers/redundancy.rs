// Rust guideline compliant 2026-05-25
//! `tokensave_redundancy` — AST-level functional-duplicate detector.
//!
//! Pipeline:
//!
//! 1. Pull all `Function` / `Method` nodes (optionally path-filtered).
//! 2. Group by file. Open each file once, parse with tree-sitter,
//!    locate every target node via its `(start_line, end_line)`, and
//!    compute a [`Fingerprint`](crate::redundancy::Fingerprint). Cache
//!    the result keyed on `(node_id, body source hash)` so we don't pay
//!    re-parse cost on subsequent calls when the file hasn't changed.
//! 3. Bucket the resulting fingerprints by `body_tokens` (±25 % window).
//!    Within each bucket, compare every pair via
//!    [`composite_similarity`](crate::redundancy::composite_similarity).
//! 4. Filter by threshold, sort by score desc, return the top N pairs.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::errors::Result;
use crate::redundancy::{
    composite_similarity, compute_fingerprint, find_node_at_exact_range, jaccard_similarity,
    overlap_kind, parse_file, severity_bucket, tokenize, Fingerprint,
};
use crate::tokensave::TokenSave;
use crate::types::{Node, NodeKind};

use super::super::ToolResult;
use super::{effective_path, truncate_response};

/// `tokensave_redundancy` handler.
pub(super) async fn handle_redundancy(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path_prefix = effective_path(&args, scope_prefix);
    let min_lines = args
        .get("min_lines")
        .and_then(Value::as_u64)
        .map_or(8u32, |v| u32::try_from(v).unwrap_or(8));
    let max_pairs = args
        .get("max_pairs")
        .and_then(Value::as_u64)
        .map_or(20usize, |v| usize::try_from(v.min(500)).unwrap_or(20));
    let threshold = args
        .get("similarity_threshold")
        .and_then(Value::as_f64)
        .unwrap_or(0.6)
        .clamp(0.0, 1.0);
    let include_naming = args
        .get("include_naming_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // 1. Collect candidate function nodes.
    let nodes = collect_candidates(cg, path_prefix, min_lines).await?;
    let total_candidates = nodes.len();

    // 2. Ensure each has a fresh fingerprint (cache by source hash).
    let fingerprints = ensure_fingerprints(cg, &nodes).await?;
    let scanned = fingerprints.len();

    // 3. Bucket by token count to keep pairwise comparison sub-quadratic.
    let pairs = find_redundant_pairs(&nodes, &fingerprints, threshold, include_naming, max_pairs);

    let pair_count = pairs.len();
    let output = json!({
        "candidates": total_candidates,
        "scanned": scanned,
        "skipped_for_size": total_candidates.saturating_sub(scanned),
        "pair_count": pair_count,
        "pairs": pairs,
        "ranked_by": "similarity desc",
        "scope": path_prefix.unwrap_or("(whole project)"),
        "thresholds": {
            "min_lines": min_lines,
            "similarity_threshold": threshold,
            "include_naming_only": include_naming,
        },
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

// ---------------------------------------------------------------------------
// 1. Candidate selection
// ---------------------------------------------------------------------------

async fn collect_candidates(
    cg: &TokenSave,
    path_prefix: Option<&str>,
    min_lines: u32,
) -> Result<Vec<Node>> {
    // Filtered in SQL rather than by loading every node and discarding most
    // of them (#410). All three predicates are the caller's own.
    // Template carries a C++ `template_declaration`'s whole body, function or class, and without it a
    // header-only template layer has no candidate at all and the scan answers `scanned: 0`, which
    // reads exactly like a clean bill of health rather than "nothing was looked at".
    let mut filter = crate::db::NodeFilter::new()
        .kinds(&[
            NodeKind::Function,
            NodeKind::Method,
            NodeKind::SingletonMethod,
            NodeKind::Template,
        ])
        .min_lines(min_lines);
    if let Some(prefix) = path_prefix {
        filter = filter.path_prefix(prefix);
    }
    cg.db().get_nodes_filtered(&filter).await
}

// ---------------------------------------------------------------------------
// 2. Fingerprint computation + caching
// ---------------------------------------------------------------------------

/// Returns a map from `node_id` to its fingerprint. Reuses any cached row
/// whose stored `source_hash` matches the live file content for that
/// node's body; otherwise re-parses the file once, computes fingerprints
/// for all candidate nodes in that file, and persists them.
async fn ensure_fingerprints(
    cg: &TokenSave,
    candidates: &[Node],
) -> Result<HashMap<String, Fingerprint>> {
    let registry = crate::extraction::LanguageRegistry::new();
    let project_root = cg.project_root().to_path_buf();

    // Group candidates by file so we parse each file at most once.
    let mut by_file: HashMap<String, Vec<&Node>> = HashMap::new();
    for n in candidates {
        by_file.entry(n.file_path.clone()).or_default().push(n);
    }

    let mut out: HashMap<String, Fingerprint> = HashMap::new();

    for (file_path, file_nodes) in by_file {
        // Deleted between sync and this call -> skip. Read first: the source, not the extension,
        // names a `.h`'s language.
        let abs = project_root.join(&file_path);
        let Ok(source) = std::fs::read_to_string(&abs) else {
            continue;
        };

        // Figure out which tree-sitter language this file maps to.
        let Some(extractor) = crate::project_manifest::resolve_extractor_for_source(
            &registry,
            &project_root,
            &file_path,
            &source,
        ) else {
            continue;
        };
        let lang_key = extractor_to_language_key(extractor.language_name());
        let Some(lang_key) = lang_key else {
            continue;
        };
        // Same bytes the extractor parsed, or a stored coordinate lands in a different tree.
        let source = crate::extraction::c_api_macro::source_for_parse(lang_key, &source);

        // Cheap path: every cached fingerprint whose source_hash matches
        // the current body content is reusable without re-parsing.
        let mut needs_parse = false;
        let mut cached: HashMap<&str, Fingerprint> = HashMap::new();
        for node in &file_nodes {
            // Exact byte-range extraction using stored (line, column).
            // Matches ts_node.utf8_text() byte-for-byte so the hash
            // aligns with compute_fingerprint for every node shape:
            // indented, mid-line, CRLF, comment after }, etc.
            let body = body_bytes(
                &source,
                node.start_line,
                node.start_column,
                node.end_line,
                node.end_column,
            );
            let expected_hash = quick_body_hash(body);
            match cg.db().get_fingerprint(&node.id).await? {
                Some(stored) if stored.source_hash == expected_hash => {
                    // Sanity check: reject poisoned rows whose stored
                    // `body_tokens` count does not match the exact body.
                    // The writer computes `body_tokens` via
                    // `tokenize(body_text).len()` on the same exact byte
                    // range, so any mismatch means the row is stale (#380).
                    let live_body_tokens = tokenize(body).len();
                    let poisoned = stored.body_tokens as usize != live_body_tokens;
                    if poisoned {
                        needs_parse = true;
                    } else {
                        cached.insert(
                            node.id.as_str(),
                            Fingerprint {
                                ast_hash: stored.ast_hash,
                                cfg_hash: stored.cfg_hash,
                                call_seq_hash: stored.call_seq_hash,
                                shingles: stored.shingles,
                                body_tokens: stored.body_tokens as usize,
                                source_hash: stored.source_hash,
                            },
                        );
                    }
                }
                _ => {
                    needs_parse = true;
                }
            }
        }

        // Insert cached hits.
        for (id, fp) in cached {
            out.insert(id.to_string(), fp);
        }
        if !needs_parse {
            continue;
        }

        // At least one node in this file needs a fresh fingerprint —
        // parse once and compute for every miss.
        let language = crate::extraction::ts_provider::language(lang_key);
        let Some(tree) = parse_file(&source, &language) else {
            continue;
        };

        for node in &file_nodes {
            if out.contains_key(&node.id) {
                continue;
            }
            // Use all four coordinates (line, column) to locate the
            // exact tree-sitter node — line-only lookup would pick the
            // wrong node when two functions share a line (#380).
            let Some(ts_node) = find_node_at_exact_range(
                &tree,
                node.start_line,
                node.start_column,
                node.end_line,
                node.end_column,
            ) else {
                continue;
            };
            let fp = compute_fingerprint(&source, ts_node);
            // Persist for next time. Errors are logged but not fatal —
            // the redundancy query still returns results.
            if let Err(e) = cg.db().upsert_fingerprint(&node.id, &fp).await {
                eprintln!("[tokensave] redundancy: upsert_fingerprint failed: {e}");
            }
            out.insert(node.id.clone(), fp);
        }
    }

    Ok(out)
}

/// Map `extractor.language_name()` (e.g. "Rust", "TypeScript") to the
/// language key used by `ts_provider::language`. Returns `None` for
/// extractors whose grammar isn't wired up here (extending the map
/// extends fingerprinting to that language).
fn extractor_to_language_key(name: &str) -> Option<&'static str> {
    Some(match name {
        "Rust" => "rust",
        "Go" => "go",
        "Java" => "java",
        "Scala" => "scala",
        "TypeScript" => "typescript",
        "TSX" => "tsx",
        "Python" => "python",
        "C" => "c",
        "C++" => "cpp",
        "C#" => "c_sharp",
        "Kotlin" => "kotlin",
        "Swift" => "swift",
        "JavaScript" => "javascript",
        "Ruby" => "ruby",
        "PHP" => "php",
        "Lua" => "lua",
        "Zig" => "zig",
        "Bash" => "bash",
        "Dart" => "dart",
        "Haskell" => "haskell",
        "OCaml" => "ocaml",
        "Elixir" => "elixir",
        "Erlang" => "erlang",
        "Clojure" => "clojure",
        "F#" => "fsharp",
        "Perl" => "perl",
        "R" => "r",
        "Julia" => "julia",
        "Nix" => "nix",
        _ => return None,
    })
}

/// Convert a 0-indexed (line, column) pair to a byte offset in `source`.
/// Tree-sitter columns are byte offsets within the line, so this function
/// counts bytes, not characters. Returns `source.len()` when the position
/// is past the end of the file.
fn line_column_to_byte(source: &str, line: u32, column: u32) -> usize {
    let target_line = line as usize;
    let target_col = column as usize;
    let mut offset = 0;
    for (i, line_text) in source.split_inclusive('\n').enumerate() {
        if i == target_line {
            let col = target_col.min(line_text.len());
            return offset + col;
        }
        offset += line_text.len();
    }
    // Past end of source — clamp to source length.
    offset
}

/// Extract the exact byte range of a node from `source` using its stored
/// (line, column) positions. Returns the same text that `ts_node.utf8_text()`
/// would produce for the corresponding tree-sitter node, which is what
/// `compute_fingerprint` hashes. This alignment is what makes the fingerprint
/// cache actually hit (issue #380).
fn body_bytes(
    source: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> &str {
    let start = line_column_to_byte(source, start_line, start_column);
    let end = line_column_to_byte(source, end_line, end_column);
    if end <= start || end > source.len() {
        return "";
    }
    &source[start..end]
}

/// Cheap body hash used for cache invalidation. Matches the format used
/// by `compute_fingerprint` (first 8 bytes of SHA-256, hex-encoded).
fn quick_body_hash(body: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    let d = h.finalize();
    let mut s = String::with_capacity(16);
    for b in d.iter().take(8) {
        let _ = write!(s, "{b:02x}");
    }
    s
}

// ---------------------------------------------------------------------------
// 3. Pairwise comparison + ranking
// ---------------------------------------------------------------------------

fn find_redundant_pairs(
    nodes: &[Node],
    fingerprints: &HashMap<String, Fingerprint>,
    threshold: f64,
    include_naming: bool,
    max_pairs: usize,
) -> Vec<Value> {
    // Pair each node with its fingerprint (skip nodes that failed to
    // produce one).
    let scope: Vec<(&Node, &Fingerprint)> = nodes
        .iter()
        .filter_map(|n| fingerprints.get(&n.id).map(|fp| (n, fp)))
        .collect();

    // Sort by body_tokens so the size-window check is a linear scan.
    let mut sorted = scope;
    sorted.sort_by_key(|(_, fp)| fp.body_tokens);

    let mut found: Vec<(f64, &str, &Node, &Node, &Fingerprint, &Fingerprint)> = Vec::new();
    for (i, (node_a, fp_a)) in sorted.iter().enumerate() {
        let lo = (fp_a.body_tokens as f64 * 0.75).floor() as usize;
        let hi = (fp_a.body_tokens as f64 * 1.25).ceil() as usize;
        for (node_b, fp_b) in sorted.iter().skip(i + 1) {
            if fp_b.body_tokens > hi {
                break; // sorted, no need to scan further
            }
            if fp_b.body_tokens < lo {
                continue;
            }
            let score = composite_similarity(fp_a, fp_b);
            if score < threshold {
                continue;
            }
            let kind = overlap_kind(fp_a, fp_b);
            if !include_naming && kind == "naming" {
                continue;
            }
            found.push((score, kind, node_a, node_b, fp_a, fp_b));
        }
    }

    found.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(max_pairs);

    found
        .into_iter()
        .map(|(score, kind, na, nb, fp_a, fp_b)| {
            let shingle_jaccard = jaccard_similarity(&fp_a.shingles, &fp_b.shingles);
            let severity = severity_bucket(score, kind);
            json!({
                "similarity": (score * 10000.0).round() / 10000.0,
                "severity": severity,
                "overlap_kind": kind,
                "a": {
                    "file": na.file_path,
                    "line": super::display_line(na.start_line),
                    "name": na.name,
                    "id": na.id,
                },
                "b": {
                    "file": nb.file_path,
                    "line": super::display_line(nb.start_line),
                    "name": nb.name,
                    "id": nb.id,
                },
                "signals": {
                    "ast_match": fp_a.ast_hash == fp_b.ast_hash,
                    "cfg_match": fp_a.cfg_hash == fp_b.cfg_hash,
                    "call_seq_match": fp_a.call_seq_hash == fp_b.call_seq_hash,
                    "shingle_jaccard": (shingle_jaccard * 10000.0).round() / 10000.0,
                    "body_tokens": [fp_a.body_tokens, fp_b.body_tokens],
                },
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::body_bytes;
    use super::find_redundant_pairs;
    use super::quick_body_hash;

    // -- exact byte-range extraction (issue #380) --

    #[test]
    fn body_bytes_exact_range() {
        // A C-style function: the exact byte range from (0,0) to (2,1)
        // matches what tree-sitter's utf8_text would return.
        let src = "int add(int a, int b) {\n    return a + b;\n}\n";
        // function body: start (0,0), end right after } at (2,1)
        let body = body_bytes(src, 0, 0, 2, 1);
        assert_eq!(body, "int add(int a, int b) {\n    return a + b;\n}");
        assert!(
            !body.ends_with('\n'),
            "exact byte range must NOT include trailing newline (matches utf8_text)"
        );
    }

    #[test]
    fn body_bytes_comment_after_close_brace() {
        // When a comment trails the closing brace, body_bytes stops at the
        // } column exactly — unlike line-based slicing which would include
        // the comment in the hash and never hit the cache.
        let src = "fn foo() {} // trailing\n";
        // function_item spans from (0,0) to (0,11) — column 11 is right
        // after }, before the comment.
        let body = body_bytes(src, 0, 0, 0, 11);
        assert_eq!(body, "fn foo() {}");
        assert!(
            !body.contains("//"),
            "comment after }} must be excluded from exact byte range"
        );
    }

    #[test]
    fn body_bytes_indented_function() {
        // Indentation: start_column > 0. Line-based slicing would include
        // leading whitespace in the hash.
        let src = "mod m;\n  fn bar() {}\n";
        // "  fn bar() {}\n": ' '=col0, ' '=col1, f=col2, …, }=col12, \n=col13
        // function_item starts at (1,2), ends right after } at (1,13).
        let body = body_bytes(src, 1, 2, 1, 13);
        assert_eq!(body, "fn bar() {}");
    }

    #[test]
    fn body_bytes_mid_line_node() {
        // Node that starts and ends mid-line — line-based slicing can't
        // express this at all.
        let src = "class C { int get() { return 42; } }\n";
        // "class C { int get() { return 42; } }\n":
        //   { at col20 (the inner block), } at col33, so end_column=34.
        // The inner block "{ return 42; }" spans columns 20..34.
        let body = body_bytes(src, 0, 20, 0, 34);
        assert_eq!(body, "{ return 42; }");
    }

    #[test]
    fn body_bytes_hash_is_deterministic() {
        let src = "fn validate(x: i32) -> bool {\n    x > 0\n}\n";
        let body = body_bytes(src, 0, 0, 2, 1);
        let h = quick_body_hash(body);
        assert!(!h.is_empty());
        assert_eq!(h, quick_body_hash(body), "same input → same hash");
    }

    #[test]
    fn body_bytes_crlf_line_endings() {
        // CRLF: split_inclusive('\n') splits on \n, leaving \r inside the
        // line text. Column counting still works because tree-sitter columns
        // are byte offsets within the line (including \r bytes).
        let src = "fn foo() {\r\n    bar()\r\n}\r\n";
        // function body (0,0)..(2,1): column 1 on line 2 points to \r,
        // so the range ends right after }.
        let body = body_bytes(src, 0, 0, 2, 1);
        assert_eq!(body, "fn foo() {\r\n    bar()\r\n}");
        assert!(!body.ends_with('\n'), "body must not include trailing \\n");
        assert!(!body.ends_with('\r'), "body must not include trailing \\r");
    }

    #[test]
    fn body_bytes_handles_out_of_bounds() {
        let src = "alpha\nbeta\n";
        // Past end of file → empty.
        assert_eq!(body_bytes(src, 5, 0, 9, 0), "");
    }

    // ---------------------------------------------------------------
    // End-to-end regression tests for fingerprint caching (issue #380)
    //
    // These tests exercise ensure_fingerprints end-to-end:
    //  1. Store & read node_fingerprints from the database.
    //  2. Prove second invocation is a cache hit.
    //  3. Plant a poisoned row and prove recovery.
    //  4. Prove phantom 1.0-similarity pairs disappear.
    // ---------------------------------------------------------------

    use super::ensure_fingerprints;
    use crate::redundancy::{composite_similarity, tokenize, Fingerprint};
    use crate::tokensave::TokenSave;
    use crate::types::{Node, NodeKind, Visibility};

    /// Build a minimal `Node` for a function at the given position.
    fn function_node(
        id: &str,
        name: &str,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        start_column: u32,
        end_column: u32,
    ) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Function,
            name: name.to_string(),
            qualified_name: name.to_string(),
            file_path: file_path.to_string(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: None,
            docstring: None,
            visibility: Visibility::Private,
            is_async: false,
            branches: 1,
            loops: 0,
            returns: 1,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 2,
            distinct_operands: 2,
            total_operators: 2,
            total_operands: 2,
            updated_at: 1,
            parent_id: None,
        }
    }

    /// A Rust source file with two distinct functions.
    ///
    /// Positions (0-indexed line, column):
    ///   add  — start (0,0), end (2,1)
    ///   sub  — start (4,0), end (6,1)
    const TWO_FNS: &str =
        "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n";

    /// A Rust source file where the function body is clearly smaller than the
    /// whole file — needed for the poisoned-row test.
    ///
    ///   small  — start (0,0), end (2,1)
    const SMALL_FN_WITH_EXTRAS: &str =
        "fn small() -> i32 {\n    1\n}\n\n// trailing comment\ntype Alias = u32;\n";

    /// 1. Call `ensure_fingerprints` → rows appear in `node_fingerprints` table.
    #[tokio::test]
    async fn fingerprints_stored_and_read_from_db() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), TWO_FNS).unwrap();

        let cg = TokenSave::init(tmp.path()).await.unwrap();

        let nodes = vec![
            function_node("node:add", "add", "lib.rs", 0, 2, 0, 1),
            function_node("node:sub", "sub", "lib.rs", 4, 6, 0, 1),
        ];

        // Insert nodes first — node_fingerprints has a FK to nodes.
        for n in &nodes {
            cg.db().insert_node(n).await.unwrap();
        }

        let fingerprints = ensure_fingerprints(&cg, &nodes).await.unwrap();

        // Both nodes produced fingerprints.
        assert_eq!(fingerprints.len(), 2);
        assert!(fingerprints.contains_key("node:add"));
        assert!(fingerprints.contains_key("node:sub"));

        // Fingerprints are stored in the DB.
        let stored_add = cg
            .db()
            .get_fingerprint("node:add")
            .await
            .unwrap()
            .expect("add fingerprint must be in db");
        let stored_sub = cg
            .db()
            .get_fingerprint("node:sub")
            .await
            .unwrap()
            .expect("sub fingerprint must be in db");

        // Hashes are non-empty.
        assert!(!stored_add.ast_hash.is_empty());
        assert!(!stored_add.source_hash.is_empty());
        assert!(!stored_add.cfg_hash.is_empty());
        assert!(!stored_sub.ast_hash.is_empty());

        // Stored fingerprints match the returned ones.
        assert_eq!(stored_add.ast_hash, fingerprints["node:add"].ast_hash);
        assert_eq!(stored_sub.ast_hash, fingerprints["node:sub"].ast_hash);
        assert_eq!(stored_add.source_hash, fingerprints["node:add"].source_hash);
    }

    /// 2. Second invocation with unchanged source is a pure cache hit.
    ///
    /// We prove this by planting a fake `ast_hash` after the first call.
    /// If the second call re-parses, it would overwrite the fake hash with
    /// the real one. If it's a cache hit, the fake hash survives.
    #[tokio::test]
    async fn second_invocation_is_cache_hit() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), TWO_FNS).unwrap();

        let cg = TokenSave::init(tmp.path()).await.unwrap();

        let nodes = vec![function_node("node:add", "add", "lib.rs", 0, 2, 0, 1)];

        // Insert node first — node_fingerprints has a FK to nodes.
        cg.db().insert_node(&nodes[0]).await.unwrap();

        // First call — parse and store real fingerprint.
        let fp1 = ensure_fingerprints(&cg, &nodes).await.unwrap();
        let real_hash = fp1["node:add"].ast_hash.clone();
        assert!(!real_hash.is_empty());

        // Plant a fake fingerprint with the SAME source_hash so the cache
        // check sees a match, but a distinctive ast_hash so we can tell
        // whether the second call read from cache.
        let stored = cg.db().get_fingerprint("node:add").await.unwrap().unwrap();
        let correct_source_hash = stored.source_hash.clone();

        let fake_fp = Fingerprint {
            ast_hash: "cached-hit-0000000000".to_string(),
            cfg_hash: stored.cfg_hash.clone(),
            call_seq_hash: stored.call_seq_hash.clone(),
            shingles: vec![99, 88, 77],
            body_tokens: stored.body_tokens as usize,
            source_hash: correct_source_hash,
        };
        cg.db()
            .upsert_fingerprint("node:add", &fake_fp)
            .await
            .unwrap();

        // Second call — must return the FAKE ast_hash, proving cache hit.
        let fp2 = ensure_fingerprints(&cg, &nodes).await.unwrap();
        assert_eq!(
            fp2["node:add"].ast_hash, "cached-hit-0000000000",
            "second call must return cached (fake) hash, not recompute"
        );
        // Shingles also come from cache.
        assert_eq!(fp2["node:add"].shingles, vec![99, 88, 77]);
    }

    /// 3. Poisoned row (`body_tokens` >= `whole_file_tokens` but actual body is
    ///    smaller) triggers recompute instead of using the stale cache.
    #[tokio::test]
    async fn poisoned_row_triggers_recompute() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), SMALL_FN_WITH_EXTRAS).unwrap();

        let cg = TokenSave::init(tmp.path()).await.unwrap();

        let nodes = vec![function_node("node:small", "small", "lib.rs", 0, 2, 0, 1)];

        // Insert node first — node_fingerprints has a FK to nodes.
        cg.db().insert_node(&nodes[0]).await.unwrap();

        // First call — compute and store real fingerprint.
        let fp1 = ensure_fingerprints(&cg, &nodes).await.unwrap();
        let real_body_tokens = fp1["node:small"].body_tokens;
        let real_ast_hash = fp1["node:small"].ast_hash.clone();

        // The whole file has more tokens than just the function body.
        let source = std::fs::read_to_string(tmp.path().join("lib.rs")).unwrap();
        let whole_tokens = tokenize(&source).len();
        assert!(
            real_body_tokens < whole_tokens,
            "precondition: small() body is smaller than whole file ({real_body_tokens} < {whole_tokens})"
        );

        // Plant a poisoned row: body_tokens equals whole_file_tokens but
        // the actual body token count is smaller. The sanity check in
        // ensure_fingerprints must detect this.
        let stored = cg
            .db()
            .get_fingerprint("node:small")
            .await
            .unwrap()
            .unwrap();
        let correct_source_hash = stored.source_hash.clone();

        let poison_fp = Fingerprint {
            ast_hash: "poison-ast-0000000000".to_string(),
            cfg_hash: stored.cfg_hash.clone(),
            call_seq_hash: stored.call_seq_hash.clone(),
            shingles: vec![],
            body_tokens: whole_tokens, // ← claims to cover the whole file
            source_hash: correct_source_hash, // ← matches, passes initial cache check
        };
        cg.db()
            .upsert_fingerprint("node:small", &poison_fp)
            .await
            .unwrap();

        // Second call — must detect poison and recompute, NOT return the
        // poisoned row's fake ast_hash.
        let fp2 = ensure_fingerprints(&cg, &nodes).await.unwrap();
        assert_ne!(
            fp2["node:small"].ast_hash, "poison-ast-0000000000",
            "poisoned row must be rejected; ast_hash must be recomputed"
        );
        // After recompute, body_tokens should be the real count (smaller
        // than whole file tokens).
        assert_eq!(
            fp2["node:small"].body_tokens, real_body_tokens,
            "recomputed body_tokens must match real function body size"
        );
        assert_eq!(
            fp2["node:small"].ast_hash, real_ast_hash,
            "recomputed ast_hash must match original"
        );
    }

    /// 4. Real phantom-pair regression test: plant two fingerprints with
    ///    identical non-source signals but poisoned `body_tokens`, prove
    ///    they'd produce similarity 1.0, then run `ensure_fingerprints` +
    ///    `find_redundant_pairs` and verify the phantom disappears.
    ///
    ///    This test guards the token-count validation required once exact-range
    ///    cache hits are enabled. Without that validation, cached rows with
    ///    matching source hashes but poisoned fingerprint signals would survive
    ///    and produce a phantom 1.0-similarity pair.
    #[tokio::test]
    #[allow(clippy::float_cmp)]
    async fn phantom_pair_disappears_after_recompute() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), TWO_FNS).unwrap();

        let cg = TokenSave::init(tmp.path()).await.unwrap();

        let nodes = vec![
            function_node("node:add", "add", "lib.rs", 0, 2, 0, 1),
            function_node("node:sub", "sub", "lib.rs", 4, 6, 0, 1),
        ];

        // Insert nodes first — node_fingerprints has a FK to nodes.
        for n in &nodes {
            cg.db().insert_node(n).await.unwrap();
        }

        // Compute correct source hashes for each exact node body.
        let source = std::fs::read_to_string(tmp.path().join("lib.rs")).unwrap();
        let add_body = body_bytes(&source, 0, 0, 2, 1);
        let sub_body = body_bytes(&source, 4, 0, 6, 1);
        let add_source_hash = quick_body_hash(add_body);
        let sub_source_hash = quick_body_hash(sub_body);
        assert_ne!(
            add_source_hash, sub_source_hash,
            "different bodies must have different source hashes"
        );

        // Plant two fingerprints where every non-source signal is identical
        // and `body_tokens` is poisoned (0 — real bodies have > 0 tokens).
        // The correct `source_hash` lets them pass the initial cache check.
        let base_poison = Fingerprint {
            ast_hash: "identical-ast-deadbeef".to_string(),
            cfg_hash: "identical-cfg-cafebabe".to_string(),
            call_seq_hash: "identical-call-12345678".to_string(),
            shingles: vec![1, 2, 3, 4, 5],
            body_tokens: 0,             // ← poisoned: real bodies have > 0 tokens
            source_hash: String::new(), // overwritten per node below
        };

        let poison_add = Fingerprint {
            source_hash: add_source_hash.clone(),
            ..base_poison.clone()
        };
        let poison_sub = Fingerprint {
            source_hash: sub_source_hash.clone(),
            ..base_poison
        };
        cg.db()
            .upsert_fingerprint("node:add", &poison_add)
            .await
            .unwrap();
        cg.db()
            .upsert_fingerprint("node:sub", &poison_sub)
            .await
            .unwrap();

        // Prove the planted fingerprints would produce similarity 1.0 —
        // this is the bug: different functions reported as identical.
        assert_eq!(
            composite_similarity(&poison_add, &poison_sub),
            1.0,
            "planted fingerprints with identical non-source signals produce similarity 1.0"
        );

        // Run ensure_fingerprints — must detect poison and recompute both.
        let fingerprints = ensure_fingerprints(&cg, &nodes).await.unwrap();

        let add_fp = &fingerprints["node:add"];
        let sub_fp = &fingerprints["node:sub"];

        // After recompute, ast_hash must NOT be the poisoned value.
        assert_ne!(
            add_fp.ast_hash, "identical-ast-deadbeef",
            "add fingerprint must be recomputed (poison rejected)"
        );
        assert_ne!(
            sub_fp.ast_hash, "identical-ast-deadbeef",
            "sub fingerprint must be recomputed (poison rejected)"
        );

        // Run find_redundant_pairs through the production path — the
        // phantom 1.0 pair must not appear.
        let pairs = find_redundant_pairs(&nodes, &fingerprints, 0.95, true, 10);
        let has_phantom = pairs.iter().any(|p| {
            let score = p["similarity"].as_f64().unwrap_or(0.0);
            score >= 1.0
        });
        assert!(
            !has_phantom,
            "phantom 1.0-similarity pair must disappear after poison recovery.\npairs: {pairs:?}"
        );

        // Sanity: two genuinely different functions have similarity < 1.0.
        let real_score = composite_similarity(add_fp, sub_fp);
        assert!(
            real_score < 1.0,
            "recomputed fingerprints for different functions must have similarity < 1.0, got {real_score}"
        );
    }

    /// 5. Exact-range lookup: two functions on the same line must map to
    ///    the correct tree-sitter node. Line-only lookup (`find_node_at_lines`)
    ///    would pick the wrong node because both share the same row.
    ///
    ///    Positions for `"fn a() {} fn b() {}\n"` (0-indexed line, column):
    ///      `fn a() {}` — `function_item` start (0,0), end (0,9)
    ///      `fn b() {}` — `function_item` start (0,10), end (0,19)
    #[tokio::test]
    async fn same_line_functions_get_correct_exact_range() {
        // Two different Rust functions on the same source line.
        let src = "fn a() {} fn b() {}\n";

        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), src).unwrap();

        let cg = TokenSave::init(tmp.path()).await.unwrap();

        let nodes = vec![
            function_node("node:a", "a", "lib.rs", 0, 0, 0, 9),
            function_node("node:b", "b", "lib.rs", 0, 0, 10, 19),
        ];

        for n in &nodes {
            cg.db().insert_node(n).await.unwrap();
        }

        // First call — parse and store fingerprints for both nodes.
        let fp1 = ensure_fingerprints(&cg, &nodes).await.unwrap();
        assert_eq!(fp1.len(), 2);

        let a_fp = &fp1["node:a"];
        let b_fp = &fp1["node:b"];

        // Each function's source_hash must match its exact body bytes.
        let body_a = body_bytes(src, 0, 0, 0, 9); // "fn a() {}"
        let body_b = body_bytes(src, 0, 10, 0, 19); // "fn b() {}"
        assert_eq!(body_a, "fn a() {}");
        assert_eq!(body_b, "fn b() {}");
        assert_eq!(a_fp.source_hash, quick_body_hash(body_a));
        assert_eq!(b_fp.source_hash, quick_body_hash(body_b));

        // The two functions have different bodies → different source_hash.
        assert_ne!(a_fp.source_hash, b_fp.source_hash);
        // ast_hash may be identical — both are empty functions with the same
        // structure; the exact-range test only cares that each node maps to
        // the correct byte range, which source_hash already proves.

        // Plant fake fingerprints with matching source_hashes to prove
        // the second invocation is a pure cache hit (no re-parse).
        let fake_a = Fingerprint {
            ast_hash: "cache-hit-a-deadbeef".to_string(),
            cfg_hash: a_fp.cfg_hash.clone(),
            call_seq_hash: a_fp.call_seq_hash.clone(),
            shingles: a_fp.shingles.clone(),
            body_tokens: a_fp.body_tokens,
            source_hash: a_fp.source_hash.clone(),
        };
        let fake_b = Fingerprint {
            ast_hash: "cache-hit-b-cafebabe".to_string(),
            cfg_hash: b_fp.cfg_hash.clone(),
            call_seq_hash: b_fp.call_seq_hash.clone(),
            shingles: b_fp.shingles.clone(),
            body_tokens: b_fp.body_tokens,
            source_hash: b_fp.source_hash.clone(),
        };
        cg.db().upsert_fingerprint("node:a", &fake_a).await.unwrap();
        cg.db().upsert_fingerprint("node:b", &fake_b).await.unwrap();

        // Second call — must return the fake ast_hashes (cache hit).
        let fp2 = ensure_fingerprints(&cg, &nodes).await.unwrap();
        assert_eq!(
            fp2["node:a"].ast_hash, "cache-hit-a-deadbeef",
            "second invocation for node:a must be a cache hit"
        );
        assert_eq!(
            fp2["node:b"].ast_hash, "cache-hit-b-cafebabe",
            "second invocation for node:b must be a cache hit"
        );
    }

    /// 6. Single-function file without trailing newline: `fn f() {}` produces
    ///    `source_file` and `function_item` with identical (0,0)..(0,9) spans.
    ///    `ensure_fingerprints` must select the deepest match (`function_item`),
    ///    not the root. Verify by comparing structural fingerprints against a
    ///    direct `compute_fingerprint` call on the `function_item` child.
    #[tokio::test]
    async fn exact_range_lookup_selects_function_not_root_for_no_trailing_newline() {
        // No trailing newline — source_file and function_item share (0,0)..(0,9).
        let src = "fn f() {}";

        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("lib.rs"), src).unwrap();

        let cg = TokenSave::init(tmp.path()).await.unwrap();

        let nodes = vec![function_node("node:f", "f", "lib.rs", 0, 0, 0, 9)];
        cg.db().insert_node(&nodes[0]).await.unwrap();

        // Run through the production path.
        let fingerprints = ensure_fingerprints(&cg, &nodes).await.unwrap();
        let fp = &fingerprints["node:f"];

        // Compute the expected fingerprint from the function_item child
        // directly — this is independent ground truth, not routing through
        // the same helper that the production path uses.
        let lang = crate::extraction::ts_provider::language("rust");
        let tree = crate::redundancy::parse_file(src, &lang).unwrap();
        let root = tree.root_node();
        let fn_node = root.named_child(0).expect("function_item child");
        assert_eq!(fn_node.kind(), "function_item");
        let expected_fp = crate::redundancy::compute_fingerprint(src, fn_node);

        // Structural fingerprints must match — proving ensure_fingerprints
        // used function_item, not source_file.
        assert_eq!(
            fp.ast_hash, expected_fp.ast_hash,
            "ast_hash must come from function_item, not source_file"
        );
        assert_eq!(
            fp.cfg_hash, expected_fp.cfg_hash,
            "cfg_hash must come from function_item, not source_file"
        );
        assert_eq!(
            fp.call_seq_hash, expected_fp.call_seq_hash,
            "call_seq_hash must come from function_item, not source_file"
        );
    }
}
