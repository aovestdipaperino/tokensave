use std::collections::HashMap;

use crate::db::Database;
use crate::types::*;

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
}

impl<'a> ReferenceResolver<'a> {
    /// Creates a new resolver, loading all nodes from the database into
    /// in-memory caches.
    ///
    /// # Panics
    ///
    /// This method does not panic. If the database query fails the caches will
    /// simply be empty.
    pub fn new(db: &'a Database) -> Self {
        let all_nodes = db.get_all_nodes().unwrap_or_default();

        let mut name_cache: HashMap<String, Vec<Node>> = HashMap::new();
        let mut qualified_name_cache: HashMap<String, Vec<Node>> = HashMap::new();

        for node in all_nodes {
            name_cache
                .entry(node.name.clone())
                .or_default()
                .push(node.clone());
            qualified_name_cache
                .entry(node.qualified_name.clone())
                .or_default()
                .push(node);
        }

        Self {
            db,
            name_cache,
            qualified_name_cache,
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
        }

        // Strategy 2: exact name match
        self.try_exact_name_match(uref)
    }

    /// Resolves a batch of unresolved references, returning a summary of the
    /// results.
    pub fn resolve_all(&self, refs: &[UnresolvedRef]) -> ResolutionResult {
        let total = refs.len();
        let mut resolved = Vec::new();
        let mut unresolved = Vec::new();

        for uref in refs {
            match self.resolve_one(uref) {
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
                kind: r.original.reference_kind.clone(),
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

        // Suffix match: check if any qualified name ends with the reference
        // name (e.g. reference "types::Node" matches
        // "crate::types::Node").
        for (qname, candidates) in &self.qualified_name_cache {
            if qname.ends_with(&uref.reference_name) {
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

        None
    }

    /// Strategy 2: exact name match using the name cache.
    fn try_exact_name_match(&self, uref: &UnresolvedRef) -> Option<ResolvedRef> {
        let candidates = self.name_cache.get(&uref.reference_name)?;

        if candidates.len() == 1 {
            return Some(ResolvedRef {
                original: uref.clone(),
                target_node_id: candidates[0].id.clone(),
                confidence: 0.9,
                resolved_by: "exact-match".to_string(),
            });
        }

        // Multiple candidates -- score them and pick the best.
        let best = self.find_best_match(uref, candidates)?;

        Some(ResolvedRef {
            original: uref.clone(),
            target_node_id: best.id.clone(),
            confidence: 0.7,
            resolved_by: "exact-match".to_string(),
        })
    }

    /// Scores candidate nodes for a reference and returns the best match.
    ///
    /// Scoring heuristics:
    /// - Same file as reference: +100
    /// - Exported / pub visibility: +10
    /// - Callable kind (function/method) when the ref kind is `Calls`: +25
    /// - Line proximity (same file only): +20 - (line_distance / 10)
    fn find_best_match(&self, uref: &UnresolvedRef, candidates: &[Node]) -> Option<Node> {
        if candidates.is_empty() {
            return None;
        }

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
