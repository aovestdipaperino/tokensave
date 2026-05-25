// Rust guideline compliant 2026-05-25
//! AST-level functional duplicate detection (issue #83).
//!
//! Computes four kinds of fingerprint per function/method body:
//!
//! 1. **AST shape hash** — kind-only pre-order walk of the tree-sitter
//!    subtree, normalised over identifier names. Catches the
//!    `ast_isomorphic` duplicate bucket.
//! 2. **CFG hash** — same walk filtered to control-flow node kinds
//!    (`if`, `for`, `while`, `loop`, `switch`/`match`, `return`/`break`).
//!    Catches reorder-refactor duplicates whose statement order differs.
//! 3. **Call-sequence hash** — ordered list of called identifiers extracted
//!    from call/invocation nodes. Catches "rewrote it from scratch and
//!    didn't notice the helper existed" duplicates.
//! 4. **Token shingles** — set of 32-bit hashes of 5-grams of alphanumeric
//!    tokens within the body. Jaccard similarity over this set catches
//!    the long tail of near-duplicates.
//!
//! These four signals are blended into a composite similarity score and
//! bucketed into `definite` / `likely` / `naming_only` severities.
//!
//! Language-agnostic by design: every signal is derived from raw
//! tree-sitter kind strings, so the same code path works for every
//! grammar the project supports. Two duplicates can only match within the
//! same language (tree-sitter kind names don't align across grammars),
//! which matches user expectations.

use std::collections::HashSet;

use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser, Tree};

/// Length of an n-gram shingle, in tokens.
const SHINGLE_N: usize = 5;

/// Composite-score weights. The weights must sum to 1.0.
const W_AST: f64 = 0.40;
const W_CFG: f64 = 0.25;
const W_CALL_SEQ: f64 = 0.20;
const W_SHINGLE: f64 = 0.15;

/// Per-symbol fingerprint produced by [`compute_fingerprint`].
#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub ast_hash: String,
    pub cfg_hash: String,
    pub call_seq_hash: String,
    /// Sorted, dedup'd set of u32 shingle hashes (rendered as comma-
    /// separated lowercase hex to keep the wire format text-friendly).
    pub shingles: Vec<u32>,
    /// Approximate body size in alphanumeric tokens. Used to bucket
    /// candidates before pairwise comparison so we stay sub-quadratic.
    pub body_tokens: usize,
    /// Hash of the body source. Used to detect when a cached fingerprint
    /// is stale relative to the current file content.
    pub source_hash: String,
}

impl Fingerprint {
    /// Render the shingles vector as a comma-separated lowercase hex
    /// string (suitable for storage in a TEXT column).
    pub fn shingles_to_string(&self) -> String {
        let mut s = String::with_capacity(self.shingles.len() * 9);
        for (i, h) in self.shingles.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            // Use std fmt; not perf-critical, called once per persist.
            s.push_str(&format!("{h:08x}"));
        }
        s
    }

    /// Parse a comma-separated lowercase hex string back into a shingles
    /// vector. Best-effort: unparseable entries are skipped.
    pub fn shingles_from_string(s: &str) -> Vec<u32> {
        if s.is_empty() {
            return Vec::new();
        }
        s.split(',')
            .filter_map(|hex| u32::from_str_radix(hex, 16).ok())
            .collect()
    }
}

/// Compute every fingerprint signal for a single function body.
///
/// `full_source` is the entire file contents (tree-sitter needs context
/// outside the body to parse correctly); `body_node` is the function's
/// AST subtree.
pub fn compute_fingerprint(full_source: &str, body_node: Node<'_>) -> Fingerprint {
    let body_text = body_node
        .utf8_text(full_source.as_bytes())
        .unwrap_or_default();
    let body_tokens = tokenize(body_text);

    Fingerprint {
        ast_hash: hash_kind_walk(body_node, false),
        cfg_hash: hash_kind_walk(body_node, true),
        call_seq_hash: hash_call_sequence(body_node, full_source.as_bytes()),
        shingles: compute_shingles(&body_tokens),
        body_tokens: body_tokens.len(),
        source_hash: short_sha256(body_text),
    }
}

/// Parse a source file with the given tree-sitter language and return the
/// `Tree`. Returns `None` when parsing fails (malformed input, missing
/// grammar). Builds a fresh `Parser` per call — the call site for
/// fingerprint computation invokes this once per file, not per node.
pub fn parse_file(source: &str, language: tree_sitter::Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

/// Locate a child node within `tree` that overlaps the given 0-indexed
/// line range. Used to map a `Node` row (with its `start_line` /
/// `end_line`) back to a tree-sitter node after re-parsing.
pub fn find_node_at_lines<'tree>(
    tree: &'tree Tree,
    start_line_zero_indexed: u32,
    end_line_zero_indexed: u32,
) -> Option<Node<'tree>> {
    let root = tree.root_node();
    let mut best: Option<Node<'tree>> = None;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let ns = node.start_position().row as u32;
        let ne = node.end_position().row as u32;
        if ns <= start_line_zero_indexed && ne >= end_line_zero_indexed {
            // Prefer the deepest enclosing match (most specific).
            if let Some(b) = best {
                let b_span = b.end_position().row - b.start_position().row;
                let n_span = ne - ns;
                if n_span < u32::try_from(b_span).unwrap_or(u32::MAX) {
                    best = Some(node);
                }
            } else {
                best = Some(node);
            }
            // Continue descending only into matching children.
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    stack.push(cursor.node());
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Tokenisation
// ---------------------------------------------------------------------------

/// Split body text into alphanumeric runs (a–z, A–Z, 0–9, underscore).
/// Whitespace and punctuation are skipped. Numbers are kept as their
/// literal text so `1` and `2` are different tokens (helps shingles).
fn tokenize(body: &str) -> Vec<&str> {
    let bytes = body.as_bytes();
    let mut tokens: Vec<&str> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_alphanumeric() || b == b'_' {
            let start = i;
            while i < bytes.len() {
                let bb = bytes[i];
                if bb.is_ascii_alphanumeric() || bb == b'_' {
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(&body[start..i]);
        } else {
            i += 1;
        }
    }
    tokens
}

// ---------------------------------------------------------------------------
// AST / CFG fingerprints
// ---------------------------------------------------------------------------

/// Pre-order kind walk. If `control_flow_only`, emit only the kinds whose
/// names look like control-flow constructs.
fn hash_kind_walk(root: Node<'_>, control_flow_only: bool) -> String {
    let mut hasher = Sha256::new();
    let mut stack: Vec<(Node<'_>, u32)> = vec![(root, 0)];
    while let Some((node, depth)) = stack.pop() {
        let kind = node.kind();
        let emit = if control_flow_only {
            is_control_flow_kind(kind)
        } else {
            true
        };
        if emit {
            // Encode depth so structural reshapes don't collide. Using a
            // separator byte (0x1f, unit separator) keeps the
            // serialisation unambiguous.
            hasher.update(kind.as_bytes());
            hasher.update([0x1f]);
            hasher.update(depth.to_le_bytes());
            hasher.update([0x1e]);
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let mut children: Vec<Node<'_>> = Vec::new();
            loop {
                children.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            // Reverse-push so pop yields left-to-right order.
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }
    short_hex(hasher.finalize().as_slice())
}

/// Heuristic: a tree-sitter kind name represents control flow if it
/// contains any of the marker substrings below. Language-agnostic — all
/// supported grammars use these strings consistently.
fn is_control_flow_kind(kind: &str) -> bool {
    const MARKERS: [&str; 12] = [
        "if", "for", "while", "loop", "switch", "case", "match", "return", "break", "continue",
        "try", "catch",
    ];
    MARKERS.iter().any(|m| kind.contains(m))
}

// ---------------------------------------------------------------------------
// Call-sequence fingerprint
// ---------------------------------------------------------------------------

/// Pre-order walk, collecting the leftmost identifier of every
/// call/invocation/macro node, in source order, then hashing them.
fn hash_call_sequence(root: Node<'_>, source: &[u8]) -> String {
    let mut calls: Vec<String> = Vec::new();
    let mut stack: Vec<Node<'_>> = vec![root];
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        if is_call_kind(kind) {
            if let Some(name) = leftmost_callable_name(node, source) {
                calls.push(name);
            }
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let mut children: Vec<Node<'_>> = Vec::new();
            loop {
                children.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
    }

    let mut hasher = Sha256::new();
    for name in &calls {
        hasher.update(name.as_bytes());
        hasher.update([0x1f]);
    }
    short_hex(hasher.finalize().as_slice())
}

fn is_call_kind(kind: &str) -> bool {
    const MARKERS: [&str; 4] = ["call", "invocation", "macro", "apply"];
    MARKERS.iter().any(|m| kind.contains(m))
}

/// Return the leftmost identifier-like child of a call node, treating
/// `field_expression` / `member_expression` as a chain (returns the
/// rightmost field of the leftmost chain — i.e. the called method).
fn leftmost_callable_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    if !cursor.goto_first_child() {
        return None;
    }
    loop {
        let child = cursor.node();
        let kind = child.kind();
        if kind == "identifier"
            || kind == "field_identifier"
            || kind == "property_identifier"
            || kind == "scoped_identifier"
        {
            return child.utf8_text(source).ok().map(str::to_string);
        }
        if kind.contains("field_expression")
            || kind.contains("member_expression")
            || kind.contains("scoped")
        {
            let mut inner = child.walk();
            if inner.goto_first_child() {
                let mut last_id: Option<String> = None;
                loop {
                    let ic = inner.node();
                    let ik = ic.kind();
                    if ik.contains("identifier") {
                        if let Ok(t) = ic.utf8_text(source) {
                            last_id = Some(t.to_string());
                        }
                    }
                    if !inner.goto_next_sibling() {
                        break;
                    }
                }
                if last_id.is_some() {
                    return last_id;
                }
            }
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shingles + Jaccard
// ---------------------------------------------------------------------------

/// Build a sorted, deduplicated vector of u32 shingle hashes over the
/// token stream. `n` is the n-gram length (`SHINGLE_N`).
fn compute_shingles(tokens: &[&str]) -> Vec<u32> {
    if tokens.len() < SHINGLE_N {
        return Vec::new();
    }
    let mut set: HashSet<u32> = HashSet::new();
    for window in tokens.windows(SHINGLE_N) {
        let mut hasher = Sha256::new();
        for tok in window {
            hasher.update(tok.as_bytes());
            hasher.update([0x1f]);
        }
        let digest = hasher.finalize();
        // Fold the digest into a u32 by xoring 32-bit chunks.
        let mut acc: u32 = 0;
        for chunk in digest.chunks(4) {
            let mut b = [0u8; 4];
            for (i, v) in chunk.iter().enumerate() {
                b[i] = *v;
            }
            acc ^= u32::from_le_bytes(b);
        }
        set.insert(acc);
    }
    let mut out: Vec<u32> = set.into_iter().collect();
    out.sort_unstable();
    out
}

/// Jaccard similarity over two sorted/dedup'd shingle sets. Returns 1.0
/// for two empty sets (vacuous match — they're both "no content").
pub fn jaccard_similarity(a: &[u32], b: &[u32]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    // Two pointer merge over sorted sequences.
    let (mut i, mut j) = (0usize, 0usize);
    let mut inter = 0usize;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                inter += 1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    let union = a.len() + b.len() - inter;
    if union == 0 {
        return 1.0;
    }
    inter as f64 / union as f64
}

// ---------------------------------------------------------------------------
// Composite similarity + severity
// ---------------------------------------------------------------------------

/// Blend the four signals into a single \[0,1\] similarity score.
pub fn composite_similarity(a: &Fingerprint, b: &Fingerprint) -> f64 {
    let ast = if a.ast_hash == b.ast_hash { 1.0 } else { 0.0 };
    let cfg = if a.cfg_hash == b.cfg_hash { 1.0 } else { 0.0 };
    let call = if a.call_seq_hash == b.call_seq_hash {
        1.0
    } else {
        0.0
    };
    let shingle = jaccard_similarity(&a.shingles, &b.shingles);
    W_AST * ast + W_CFG * cfg + W_CALL_SEQ * call + W_SHINGLE * shingle
}

/// Determine the "kind" of overlap two functions share. Returned alongside
/// the composite score so callers can filter (e.g. drop `naming` matches).
pub fn overlap_kind(a: &Fingerprint, b: &Fingerprint) -> &'static str {
    if a.ast_hash == b.ast_hash {
        "ast_isomorphic"
    } else if a.cfg_hash == b.cfg_hash {
        "control_flow"
    } else if a.call_seq_hash == b.call_seq_hash {
        "algorithmic"
    } else if jaccard_similarity(&a.shingles, &b.shingles) >= 0.5 {
        "token_overlap"
    } else {
        "naming"
    }
}

/// Severity bucket for a `(score, overlap_kind)` pair.
///
/// `definite` requires AST isomorphism — anything less can still be a
/// false positive. `likely` covers control-flow or algorithmic matches
/// with high shingle overlap. `naming_only` is the long tail.
pub fn severity_bucket(score: f64, kind: &str) -> &'static str {
    if kind == "ast_isomorphic" && score >= 0.80 {
        "definite"
    } else if kind == "naming" {
        "naming_only"
    } else if score >= 0.55 {
        "likely"
    } else {
        "naming_only"
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn short_hex(bytes: &[u8]) -> String {
    // 16 hex chars = 64 bits of entropy — enough to make a collision
    // between two functions in the same repo astronomically unlikely.
    let mut s = String::with_capacity(16);
    for b in bytes.iter().take(8) {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn short_sha256(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    short_hex(h.finalize().as_slice())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Helper that parses a Rust snippet and returns the first function body.
    fn fingerprint_for_rust_fn(snippet: &str) -> Fingerprint {
        let lang = crate::extraction::ts_provider::language("rust");
        let tree = parse_file(snippet, lang).expect("parse failed");
        let root = tree.root_node();
        let fn_node = find_first_kind(root, "function_item").expect("no function in snippet");
        compute_fingerprint(snippet, fn_node)
    }

    fn find_first_kind<'t>(root: Node<'t>, target: &str) -> Option<Node<'t>> {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n.kind() == target {
                return Some(n);
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
        None
    }

    #[test]
    fn identical_functions_have_identical_ast_hash() {
        let a = fingerprint_for_rust_fn(
            "fn a(x: i32) -> i32 { if x > 0 { x + 1 } else { x - 1 } }",
        );
        let b = fingerprint_for_rust_fn(
            "fn b(y: i32) -> i32 { if y > 0 { y + 1 } else { y - 1 } }",
        );
        assert_eq!(a.ast_hash, b.ast_hash, "renamed identifiers must not change AST hash");
        // AST + CFG + call-seq all match; shingles diverge because token
        // names changed. Score lower-bound: 0.40+0.25+0.20 = 0.85.
        let score = composite_similarity(&a, &b);
        assert!(score >= 0.85, "expected >= 0.85, got {score}");
        assert_eq!(overlap_kind(&a, &b), "ast_isomorphic");
        assert_eq!(severity_bucket(score, "ast_isomorphic"), "definite");
    }

    #[test]
    fn different_structure_produces_different_ast_hash() {
        let a = fingerprint_for_rust_fn("fn a(x: i32) -> i32 { x + 1 }");
        let b = fingerprint_for_rust_fn(
            "fn b(x: i32) -> i32 { if x > 0 { x + 1 } else { x - 1 } }",
        );
        assert_ne!(a.ast_hash, b.ast_hash);
        assert_ne!(a.cfg_hash, b.cfg_hash);
    }

    #[test]
    fn cfg_hash_matches_under_renaming_and_inline_changes() {
        // Two functions with identical control flow but different operations.
        let a = fingerprint_for_rust_fn(
            "fn a(x: i32) -> i32 { if x > 0 { return 1; } else { return 2; } }",
        );
        let b = fingerprint_for_rust_fn(
            "fn b(x: i32) -> i32 { if x > 0 { return 99; } else { return 100; } }",
        );
        assert_eq!(a.cfg_hash, b.cfg_hash);
    }

    #[test]
    fn jaccard_self_similarity_is_one() {
        let a = fingerprint_for_rust_fn(
            "fn a() { let x = 1; let y = 2; let z = x + y; println!(\"{}\", z); }",
        );
        assert!((jaccard_similarity(&a.shingles, &a.shingles) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_disjoint_is_zero() {
        let a = fingerprint_for_rust_fn("fn a() { let aaaa = 1; let bbbb = 2; let cccc = 3; let dddd = 4; let eeee = 5; }");
        let b = fingerprint_for_rust_fn("fn b() { let zzzz = 9; let yyyy = 8; let xxxx = 7; let wwww = 6; let vvvv = 5; }");
        let j = jaccard_similarity(&a.shingles, &b.shingles);
        // Some token overlap (e.g. `let`), but should be very low.
        assert!(j < 0.4, "expected low Jaccard, got {j}");
    }

    #[test]
    fn shingles_roundtrip_through_string_format() {
        let original: Vec<u32> = vec![1, 2, 0xdead_beef, 0xffff_ffff];
        let fp = Fingerprint {
            ast_hash: "x".into(),
            cfg_hash: "x".into(),
            call_seq_hash: "x".into(),
            shingles: original.clone(),
            body_tokens: 0,
            source_hash: "x".into(),
        };
        let s = fp.shingles_to_string();
        let parsed = Fingerprint::shingles_from_string(&s);
        assert_eq!(parsed, original);
    }

    #[test]
    fn call_sequence_captures_order() {
        let a = fingerprint_for_rust_fn(
            "fn a() { foo(); bar(); baz(); }",
        );
        let b = fingerprint_for_rust_fn(
            "fn b() { foo(); bar(); baz(); }",
        );
        let c = fingerprint_for_rust_fn(
            "fn c() { baz(); bar(); foo(); }",
        );
        assert_eq!(a.call_seq_hash, b.call_seq_hash);
        assert_ne!(a.call_seq_hash, c.call_seq_hash);
    }

    #[test]
    fn severity_naming_only_for_low_score() {
        assert_eq!(severity_bucket(0.10, "naming"), "naming_only");
        assert_eq!(severity_bucket(0.30, "token_overlap"), "naming_only");
        assert_eq!(severity_bucket(0.60, "control_flow"), "likely");
    }
}
