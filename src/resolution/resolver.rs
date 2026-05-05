// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};

use rayon::prelude::*;

use crate::db::Database;
use crate::types::*;

/// Infer a coarse language tag from a file path extension.
fn lang_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "go" => "go",
        "py" | "pyi" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "scala" | "sc" => "scala",
        "dart" => "dart",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "sh" | "bash" => "bash",
        "nix" => "nix",
        "zig" => "zig",
        "proto" => "proto",
        _ => "unknown",
    }
}

/// Count shared path segments between two file paths.
fn path_proximity(a: &str, b: &str) -> i64 {
    let seg_a: Vec<&str> = a.split('/').collect();
    let seg_b: Vec<&str> = b.split('/').collect();
    let shared = seg_a
        .iter()
        .zip(seg_b.iter())
        .take_while(|(x, y)| x == y)
        .count();
    // +5 per shared segment, capped at +40
    (shared as i64 * 5).min(40)
}

/// Resolves unresolved references into concrete edges by matching them against
/// known nodes loaded from the database.
///
/// Caches are built once at construction time by loading all nodes from the
/// database and indexing them by `name` and `qualified_name`.
pub struct ReferenceResolver<'a> {
    #[allow(dead_code)]
    db: &'a Database,
    /// Nodes grouped by their short name.
    name_cache: HashMap<String, Vec<Node>>,
    /// Nodes grouped by their qualified name.
    qualified_name_cache: HashMap<String, Vec<Node>>,
    /// Suffix index: maps every `::suffix` of a qualified name to the full
    /// qualified name(s). Enables O(1) suffix lookups instead of scanning
    /// the entire `qualified_name_cache`.
    suffix_cache: HashMap<String, Vec<String>>,
    /// All known symbol names (short + qualified + suffixes) for pre-filtering.
    known_names: HashSet<String>,
}

impl<'a> ReferenceResolver<'a> {
    /// Creates a new resolver, loading all nodes from the database into
    /// in-memory caches.
    ///
    /// # Panics
    ///
    /// This method does not panic. If the database query fails the caches will
    /// simply be empty.
    pub async fn new(db: &'a Database) -> Self {
        let all_nodes = db.get_all_nodes().await.unwrap_or_default();
        Self::from_nodes(db, &all_nodes)
    }

    /// Creates a resolver from pre-loaded nodes, skipping the database roundtrip.
    pub fn from_nodes(db: &'a Database, all_nodes: &[Node]) -> Self {
        let mut name_cache: HashMap<String, Vec<Node>> = HashMap::new();
        let mut qualified_name_cache: HashMap<String, Vec<Node>> = HashMap::new();
        let mut suffix_cache: HashMap<String, Vec<String>> = HashMap::new();

        for node in all_nodes {
            name_cache
                .entry(node.name.clone())
                .or_default()
                .push(node.clone());
            let qn = &node.qualified_name;
            qualified_name_cache
                .entry(qn.clone())
                .or_default()
                .push(node.clone());
            // Build suffix index: for "a::b::c", index "b::c" and "c"
            // (but not the full name — that's in qualified_name_cache already)
            let mut pos = 0;
            while let Some(idx) = qn[pos..].find("::") {
                let suffix = &qn[pos + idx + 2..];
                if !suffix.is_empty() {
                    suffix_cache
                        .entry(suffix.to_string())
                        .or_default()
                        .push(qn.clone());
                }
                pos += idx + 2;
            }
        }

        // Deduplicate suffix entries
        for entries in suffix_cache.values_mut() {
            entries.sort_unstable();
            entries.dedup();
        }

        // Build known_names set for pre-filtering unresolvable refs.
        let mut known_names: HashSet<String> = HashSet::new();
        for key in name_cache.keys() {
            known_names.insert(key.clone());
        }
        for key in qualified_name_cache.keys() {
            known_names.insert(key.clone());
        }
        for key in suffix_cache.keys() {
            known_names.insert(key.clone());
        }

        Self {
            db,
            name_cache,
            qualified_name_cache,
            suffix_cache,
            known_names,
        }
    }

    /// Attempts to resolve a single unresolved reference.
    ///
    /// Resolution strategies are tried in order:
    /// 1. **Qualified name match** -- if the reference contains `::`, try
    ///    matching against qualified names of known nodes (confidence 0.95).
    /// 2. **Exact name match** -- look up the reference name in the name cache.
    ///    A single match yields confidence 0.9; multiple matches are scored via
    ///    `find_best_match` and the winner gets confidence 0.7.
    ///
    /// Returns `None` if no strategy can resolve the reference.
    pub fn resolve_one(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        // Strategy 1: qualified name match
        if uref.reference_name.contains("::") {
            if let Some(resolved) = self.try_qualified_match(uref) {
                return Some(resolved);
            }
            // Fall through to try exact name match with the simple name
            let simple_name = uref
                .reference_name
                .rsplit("::")
                .next()
                .unwrap_or(&uref.reference_name);
            if let Some(resolved) = self.try_exact_name_match_simple(uref, simple_name) {
                return Some(resolved);
            }
            return None;
        }

        // Strategy 2: exact name match
        self.try_exact_name_match(uref)
    }

    /// Returns true if a reference name could plausibly resolve to a known symbol.
    fn is_known_name(&self, name: &str) -> bool {
        self.known_names.contains(name)
    }

    /// Resolves a batch of unresolved references in parallel, returning a
    /// summary of the results.
    ///
    /// Pre-filters references whose name doesn't exist in the graph at all,
    /// turning hopeless lookups into O(1) hash checks.
    pub fn resolve_all(&self, refs: &[UnresolvedRef]) -> ResolutionResult {
        let total = refs.len();

        // Partition into resolvable (name exists in graph) and hopeless.
        let (candidates, hopeless): (Vec<_>, Vec<_>) = refs
            .iter()
            .partition(|uref| self.is_known_name(&uref.reference_name));

        let results: Vec<_> = candidates
            .par_iter()
            .map(|uref| (*uref, self.resolve_one(uref)))
            .collect();

        let mut resolved = Vec::new();
        let mut unresolved: Vec<UnresolvedRef> = hopeless.into_iter().cloned().collect();
        for (uref, res) in results {
            match res {
                Some(r) => resolved.push(r),
                None => unresolved.push(uref.clone()),
            }
        }

        let resolved_count = resolved.len();

        ResolutionResult {
            resolved,
            unresolved,
            total,
            resolved_count,
        }
    }

    /// Converts a slice of resolved references into graph edges.
    pub fn create_edges(&self, resolved: &[ResolvedRef]) -> Vec<Edge> {
        resolved
            .iter()
            .map(|r| Edge {
                source: r.original.from_node_id.clone(),
                target: r.target_node_id.clone(),
                kind: r.original.reference_kind,
                line: Some(r.original.line),
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Strategy 1: try matching the reference name against qualified names.
    fn try_qualified_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        // Direct lookup first
        if let Some(candidates) = self.qualified_name_cache.get(&uref.reference_name) {
            if let Some(node) = candidates.first() {
                return Some(ResolvedRef {
                    original: uref.clone(),
                    target_node_id: node.id.clone(),
                    confidence: 0.95,
                    resolved_by: "qualified-match".to_string(),
                });
            }
        }

        // Suffix match via pre-built suffix index — O(1) lookup instead of
        // scanning the entire qualified_name_cache.
        if let Some(full_names) = self.suffix_cache.get(&uref.reference_name) {
            for full_name in full_names {
                if let Some(candidates) = self.qualified_name_cache.get(full_name) {
                    if let Some(node) = candidates.first() {
                        return Some(ResolvedRef {
                            original: uref.clone(),
                            target_node_id: node.id.clone(),
                            confidence: 0.95,
                            resolved_by: "qualified-match".to_string(),
                        });
                    }
                }
            }
        }

        None
    }

    /// Strategy 2: exact name match using the name cache.
    fn try_exact_name_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        let candidates = self.name_cache.get(&uref.reference_name)?;

        if candidates.len() == 1 {
            let ref_lang = lang_from_path(&uref.file_path);
            let candidate_lang = lang_from_path(&candidates[0].file_path);
            let confidence = if ref_lang != "unknown"
                && candidate_lang != "unknown"
                && ref_lang != candidate_lang
            {
                0.5
            } else {
                0.9
            };
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: candidates[0].id.clone(),
                confidence,
                resolved_by: "exact-match".to_string(),
            });
        }

        // Multiple candidates -- score them and pick the best.
        let best = Self::find_best_match(uref, candidates)?;

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: "exact-match".to_string(),
        })
    }

    fn try_exact_name_match_simple(
        &self,
        uref: &UnresolvedRef,
        simple_name: &str,
    ) -> Option<ResolvedRef> {
        let candidates = self.name_cache.get(simple_name)?;

        if candidates.len() == 1 {
            let ref_lang = lang_from_path(&uref.file_path);
            let candidate_lang = lang_from_path(&candidates[0].file_path);
            let confidence = if ref_lang != "unknown"
                && candidate_lang != "unknown"
                && ref_lang != candidate_lang
            {
                0.5
            } else {
                0.9
            };
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: candidates[0].id.clone(),
                confidence,
                resolved_by: "simple-name-match".to_string(),
            });
        }

        let best = Self::find_best_match(uref, candidates)?;

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: "simple-name-match".to_string(),
        })
    }

    /// Scores candidate nodes for a reference and returns the best match.
    ///
    /// Scoring heuristics:
    /// - Same file as reference: +100
    /// - Directory proximity (shared path segments): +5 per segment, capped at +40
    /// - Same language: +50, cross-language: -80
    /// - Exported / pub visibility: +10
    /// - Callable kind (function/method) when the ref kind is `Calls`: +25
    /// - Line proximity (same file only): +20 - (`line_distance` / 10)
    fn find_best_match(uref: &UnresolvedRef, candidates: &[Node]) -> Option<Node> {
        if candidates.is_empty() {
            return None;
        }

        let ref_lang = lang_from_path(&uref.file_path);
        let mut best_score = i64::MIN;
        let mut best_node: Option<&Node> = None;

        for node in candidates {
            let mut score: i64 = 0;

            // Same file bonus
            if node.file_path == uref.file_path {
                score += 100;

                // Line proximity bonus (same file only)
                let distance = node.start_line.abs_diff(uref.line);
                let proximity = 20_i64.saturating_sub(i64::from(distance) / 10);
                score += proximity.max(0);
            } else {
                // Directory proximity bonus (different files only)
                score += path_proximity(&uref.file_path, &node.file_path);
            }

            // Language matching
            let candidate_lang = lang_from_path(&node.file_path);
            if ref_lang != "unknown" && candidate_lang != "unknown" {
                if ref_lang == candidate_lang {
                    score += 50;
                } else {
                    score -= 80;
                }
            }

            // Exported / pub bonus
            if node.visibility == Visibility::Pub {
                score += 10;
            }

            // Callable kind bonus for Calls references
            if uref.reference_kind == EdgeKind::Calls
                && matches!(
                    node.kind,
                    NodeKind::Function
                        | NodeKind::Method
                        | NodeKind::StructMethod
                        | NodeKind::Constructor
                        | NodeKind::AbstractMethod
                )
            {
                score += 25;
            }

            if score > best_score {
                best_score = score;
                best_node = Some(node);
            }
        }

        best_node.cloned()
    }
}
