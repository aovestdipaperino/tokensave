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

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use serde_json::{json, Value};

use crate::errors::Result;
use crate::redundancy::{
    composite_similarity, compute_fingerprint, find_node_at_lines, jaccard_similarity,
    overlap_kind, parse_file, severity_bucket, Fingerprint,
};
use crate::tokensave::TokenSave;
use crate::types::{Node, NodeKind};

use super::super::ToolResult;
use super::{effective_path, truncate_response};

const EXACT_SOURCE_BODY_HASH: &str = "exact_source_body_hash";

#[derive(Debug, Serialize)]
pub(super) struct SimplifyDuplication {
    symbol: String,
    id: String,
    qualified_name: String,
    file: String,
    line: u32,
    similar_to: Vec<SimplifyDuplicateMatch>,
}

#[derive(Debug, Serialize)]
struct SimplifyDuplicateMatch {
    name: String,
    id: String,
    qualified_name: String,
    file: String,
    line: u32,
    score: f64,
    evidence_kind: &'static str,
    body_hash: String,
}

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

/// Finds exact copied implementations for changed-file simplify analysis.
///
/// Bare names only bound candidate retrieval. A finding requires receiver-
/// qualified identity, the same language and node kind, a non-empty body, and
/// an exact source-body hash match. Near-duplicate heuristics remain owned by
/// `tokensave_redundancy`.
pub(super) async fn find_target_duplications(
    cg: &TokenSave,
    targets: &[Node],
) -> Result<Vec<SimplifyDuplication>> {
    let registry = crate::extraction::LanguageRegistry::new();
    let project_root = cg.project_root();
    let mut sorted_targets: Vec<Node> = targets
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Function | NodeKind::Method))
        .cloned()
        .collect();
    sorted_targets.sort_by(compare_node_identity);
    sorted_targets.dedup_by(|left, right| left.id == right.id);

    let target_names: HashSet<&str> = sorted_targets
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    let mut candidates_by_name: BTreeMap<String, Vec<Node>> = BTreeMap::new();
    for candidate in cg.get_all_nodes().await? {
        if target_names.contains(candidate.name.as_str())
            && matches!(candidate.kind, NodeKind::Function | NodeKind::Method)
        {
            candidates_by_name
                .entry(candidate.name.clone())
                .or_default()
                .push(candidate);
        }
    }
    for candidates in candidates_by_name.values_mut() {
        candidates.sort_by(compare_node_identity);
    }

    let mut nodes_by_id: BTreeMap<String, Node> = sorted_targets
        .iter()
        .cloned()
        .map(|node| (node.id.clone(), node))
        .collect();
    for candidates in candidates_by_name.values() {
        for candidate in candidates {
            nodes_by_id
                .entry(candidate.id.clone())
                .or_insert_with(|| candidate.clone());
        }
    }
    let fingerprint_nodes: Vec<Node> = nodes_by_id.into_values().collect();
    let fingerprints = ensure_fingerprints(cg, &fingerprint_nodes).await?;
    let non_empty_bodies = non_empty_body_ids(cg, &fingerprint_nodes);

    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
    let mut findings = Vec::new();
    for target in sorted_targets {
        let Some(target_language) = node_language(&registry, project_root, &target) else {
            continue;
        };
        let Some(target_fingerprint) = fingerprints.get(&target.id) else {
            continue;
        };
        if !non_empty_bodies.contains(&target.id) || target_fingerprint.source_hash.is_empty() {
            continue;
        }

        let mut matches = Vec::new();
        if let Some(candidates) = candidates_by_name.get(&target.name) {
            for candidate in candidates {
                if candidate.id == target.id || candidate.kind != target.kind {
                    continue;
                }
                if node_language(&registry, project_root, candidate).as_deref()
                    != Some(target_language.as_str())
                {
                    continue;
                }
                if !non_empty_bodies.contains(&candidate.id) {
                    continue;
                }
                let Some(candidate_fingerprint) = fingerprints.get(&candidate.id) else {
                    continue;
                };
                if candidate_fingerprint.source_hash.is_empty()
                    || candidate_fingerprint.source_hash != target_fingerprint.source_hash
                {
                    continue;
                }

                let pair = canonical_pair(&target.id, &candidate.id);
                if !seen_pairs.insert(pair) {
                    continue;
                }
                matches.push(SimplifyDuplicateMatch {
                    name: candidate.name.clone(),
                    id: candidate.id.clone(),
                    qualified_name: candidate.qualified_name.clone(),
                    file: candidate.file_path.clone(),
                    line: super::display_line(candidate.start_line),
                    score: 1.0,
                    evidence_kind: EXACT_SOURCE_BODY_HASH,
                    body_hash: target_fingerprint.source_hash.clone(),
                });
            }
        }

        if !matches.is_empty() {
            findings.push(SimplifyDuplication {
                symbol: target.name,
                id: target.id,
                qualified_name: target.qualified_name,
                file: target.file_path,
                line: super::display_line(target.start_line),
                similar_to: matches,
            });
        }
    }

    Ok(findings)
}

fn compare_node_identity(left: &Node, right: &Node) -> Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.start_line.cmp(&right.start_line))
        .then_with(|| left.qualified_name.cmp(&right.qualified_name))
        .then_with(|| left.id.cmp(&right.id))
}

fn canonical_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

fn node_language(
    registry: &crate::extraction::LanguageRegistry,
    project_root: &std::path::Path,
    node: &Node,
) -> Option<String> {
    let extractor =
        crate::project_manifest::resolve_extractor(registry, project_root, &node.file_path)?;
    Some(extractor.language_name().to_string())
}

fn non_empty_body_ids(cg: &TokenSave, candidates: &[Node]) -> HashSet<String> {
    let registry = crate::extraction::LanguageRegistry::new();
    let project_root = cg.project_root();
    let mut by_file: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
    for node in candidates {
        by_file.entry(&node.file_path).or_default().push(node);
    }

    let mut non_empty = HashSet::new();
    for (file_path, nodes) in by_file {
        let Some(extractor) =
            crate::project_manifest::resolve_extractor(&registry, project_root, file_path)
        else {
            continue;
        };
        let Some(language_key) = extractor_to_language_key(extractor.language_name()) else {
            continue;
        };
        let Ok(source) = std::fs::read_to_string(project_root.join(file_path)) else {
            continue;
        };
        let language = crate::extraction::ts_provider::language(language_key);
        let Some(tree) = parse_file(&source, &language) else {
            continue;
        };

        for node in nodes {
            let Some(syntax_node) = find_node_at_lines(&tree, node.start_line, node.end_line)
            else {
                continue;
            };
            if has_non_empty_body(syntax_node, &source) {
                non_empty.insert(node.id.clone());
            }
        }
    }
    non_empty
}

fn has_non_empty_body(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let Some(body) = node.child_by_field_name("body") else {
        return false;
    };
    if body.kind().contains("block") || body.kind() == "compound_statement" {
        return body.named_child_count() > 0;
    }
    !body
        .utf8_text(source.as_bytes())
        .unwrap_or_default()
        .trim()
        .is_empty()
}

// ---------------------------------------------------------------------------
// 1. Candidate selection
// ---------------------------------------------------------------------------

async fn collect_candidates(
    cg: &TokenSave,
    path_prefix: Option<&str>,
    min_lines: u32,
) -> Result<Vec<Node>> {
    let all = cg.get_all_nodes().await?;
    Ok(all
        .into_iter()
        .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Method))
        .filter(|n| n.end_line.saturating_sub(n.start_line) + 1 >= min_lines)
        .filter(|n| {
            path_prefix.is_none_or(|pfx| {
                let prefix = if pfx.ends_with('/') {
                    pfx.to_string()
                } else {
                    format!("{pfx}/")
                };
                n.file_path.starts_with(&prefix) || n.file_path == pfx
            })
        })
        .collect())
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
        // Figure out which tree-sitter language this file maps to.
        let Some(extractor) =
            crate::project_manifest::resolve_extractor(&registry, &project_root, &file_path)
        else {
            continue;
        };
        let lang_key = extractor_to_language_key(extractor.language_name());
        let Some(lang_key) = lang_key else {
            continue;
        };

        // Read the file contents. Silently skip on read failure (the file
        // may have been deleted between sync and this call).
        let abs = project_root.join(&file_path);
        let Ok(source) = std::fs::read_to_string(&abs) else {
            continue;
        };

        // Cheap path: every cached fingerprint whose source_hash matches
        // the current body content is reusable without re-parsing.
        let mut needs_parse = false;
        let mut cached: HashMap<&str, Fingerprint> = HashMap::new();
        for node in &file_nodes {
            let body = body_slice(&source, node.start_line, node.end_line);
            let expected_hash = quick_body_hash(body);
            match cg.db().get_fingerprint(&node.id).await? {
                Some(stored) if stored.source_hash == expected_hash => {
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
            // Node.start_line / end_line are stored as raw tree-sitter
            // row indices (0-based) — see info::extract_lines docs.
            let Some(ts_node) = find_node_at_lines(&tree, node.start_line, node.end_line) else {
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

/// Extract the inclusive 0-indexed line range from `source` as a borrowed
/// slice. Node `start_line` / `end_line` are stored as raw tree-sitter
/// row indices (see `info::extract_lines`).
fn body_slice(source: &str, start_line: u32, end_line: u32) -> &str {
    let start = start_line as usize;
    let end = (end_line as usize).saturating_add(1);
    let mut offset = 0usize;
    let mut start_byte: Option<usize> = None;
    let mut end_byte: usize = source.len();
    for (i, line) in source.split_inclusive('\n').enumerate() {
        if i == start {
            start_byte = Some(offset);
        }
        if i + 1 == end {
            end_byte = offset + line.len();
            break;
        }
        offset += line.len();
    }
    let Some(s) = start_byte else { return "" };
    if end_byte <= s || end_byte > source.len() {
        return "";
    }
    &source[s..end_byte]
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
mod tests {
    use super::body_slice;

    #[test]
    fn body_slice_extracts_single_line_zero_indexed() {
        let src = "alpha\nbeta\ngamma\n";
        // row 1 (0-indexed) == "beta"
        assert_eq!(body_slice(src, 1, 1), "beta\n");
    }

    #[test]
    fn body_slice_extracts_multi_line_inclusive() {
        let src = "alpha\nbeta\ngamma\ndelta\n";
        // rows 1..=2 (0-indexed) == "beta", "gamma"
        assert_eq!(body_slice(src, 1, 2), "beta\ngamma\n");
    }

    #[test]
    fn body_slice_handles_out_of_bounds() {
        let src = "alpha\nbeta\n";
        assert_eq!(body_slice(src, 5, 9), "");
    }
}
