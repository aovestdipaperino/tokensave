// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

use crate::context::ranking::{
    apply_connectivity_boost, apply_executable_intent_boost, rerank_candidates,
};
use crate::db::Database;
use crate::errors::Result;
use crate::graph::GraphTraverser;
use crate::text::{is_camel_case, split_compound};
use crate::types::*;

/// Builds AI-ready context by combining search, graph traversal, and source code extraction.
pub struct ContextBuilder<'a> {
    db: &'a Database,
    project_root: &'a Path,
}

impl<'a> ContextBuilder<'a> {
    /// Creates a new `ContextBuilder` backed by the given database and project root.
    pub fn new(db: &'a Database, project_root: &'a Path) -> Self {
        Self { db, project_root }
    }

    /// Builds a complete task context for the given query.
    ///
    /// Pipeline:
    /// 1. Extract symbol names from the query
    /// 2. Search for matching nodes via FTS and exact name lookup
    /// 3. Expand graph around entry points using BFS traversal
    /// 4. Extract code blocks by reading source files
    /// 5. Build and return `TaskContext`
    pub async fn build_context(
        &self,
        query: &str,
        options: &BuildContextOptions,
    ) -> Result<TaskContext> {
        debug_assert!(!query.is_empty(), "build_context called with empty query");
        debug_assert!(options.max_nodes > 0, "max_nodes must be positive");
        // Step 1-3: find relevant subgraph and entry points
        let symbols = extract_symbols_from_query(query);
        let (entry_points, diagnostics) = self.find_entry_points(query, &symbols, options).await?;
        let subgraph = self.expand_subgraph(&entry_points, options).await?;

        // Step 4: extract code blocks from source files
        let code_blocks = if options.include_code {
            // Share one file-content cache across extract + merge so each
            // source file is read at most once for this request.
            let mut file_cache: HashMap<String, Option<String>> = HashMap::new();
            let blocks = self.extract_code_blocks(&entry_points, options, &mut file_cache);
            if options.merge_adjacent {
                self.merge_adjacent_blocks(blocks, &mut file_cache)
            } else {
                blocks
            }
        } else {
            Vec::new()
        };

        // Collect unique related files
        let related_files = Self::collect_related_files(&subgraph);

        // Build summary
        let summary = Self::build_summary(query, &entry_points, &subgraph);

        let seen_node_ids: Vec<String> = entry_points.iter().map(|n| n.id.clone()).collect();

        Ok(TaskContext {
            query: query.to_string(),
            summary,
            subgraph,
            entry_points,
            code_blocks,
            related_files,
            seen_node_ids,
            diagnostics,
        })
    }

    /// Finds the relevant subgraph for a query without extracting code blocks.
    ///
    /// Extracts symbols from the query, searches for matching nodes, and expands
    /// via BFS traversal to the configured depth.
    pub async fn find_relevant_context(
        &self,
        query: &str,
        options: &BuildContextOptions,
    ) -> Result<Subgraph> {
        let symbols = extract_symbols_from_query(query);
        let (entry_points, _) = self.find_entry_points(query, &symbols, options).await?;
        self.expand_subgraph(&entry_points, options).await
    }

    /// Reads the source file and extracts the code for a node.
    ///
    /// Returns `None` if the file cannot be read or the line range is invalid.
    /// The `Result` wrapper is preserved for API stability with the previous
    /// signature; this method does not currently emit `Err`.
    pub fn get_code(&self, node: &Node) -> Result<Option<String>> {
        let mut cache: HashMap<String, Option<String>> = HashMap::new();
        Ok(self.get_code_cached(node, &mut cache))
    }

    /// Same as `get_code` but reads each file at most once per `cache`.
    ///
    /// Used by `extract_code_blocks` and `merge_adjacent_blocks` so a single
    /// `build_context` call doesn't re-read the same source file dozens of
    /// times — the old per-node `fs::read_to_string` was the dominant cost
    /// when many entry points lived in the same file.
    fn get_code_cached(
        &self,
        node: &Node,
        cache: &mut HashMap<String, Option<String>>,
    ) -> Option<String> {
        debug_assert!(
            !node.file_path.is_empty(),
            "get_code called with empty file_path"
        );
        debug_assert!(!node.id.is_empty(), "get_code called with empty node id");
        // Node spans are stored 0-based (tree-sitter rows), so 0 is a valid
        // first-line start — only an inverted span is malformed (#203).
        if node.end_line < node.start_line {
            return None;
        }

        let content = if let Some(slot) = cache.get(&node.file_path) {
            slot.clone()
        } else {
            let file_path = self.project_root.join(&node.file_path);
            // Prevent path traversal: ensure the resolved path stays within
            // the project root. If either side fails to canonicalize (e.g.
            // file missing on disk) fall through to the read attempt so the
            // pre-existing missing-file path still returns `None` naturally.
            let allowed = match (file_path.canonicalize(), self.project_root.canonicalize()) {
                (Ok(canonical), Ok(root)) => canonical.starts_with(&root),
                _ => true,
            };
            let loaded = if allowed {
                fs::read_to_string(&file_path).ok()
            } else {
                None
            };
            cache.insert(node.file_path.clone(), loaded.clone());
            loaded
        };
        let content = content?;

        let lines: Vec<&str> = content.lines().collect();
        // 0-based inclusive span -> slice [start, end] (#203). The previous
        // 1-based interpretation shifted every snippet up a line: it pulled
        // in the line above the symbol and dropped the closing line.
        let start = node.start_line as usize;
        let end = (node.end_line as usize).saturating_add(1);
        if start >= lines.len() {
            return None;
        }
        let end = end.min(lines.len());
        let snippet: String = lines[start..end].join("\n");
        if snippet.is_empty() {
            None
        } else {
            Some(snippet)
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Searches for entry-point nodes matching the query and extracted symbols.
    ///
    /// Pipeline:
    /// 1. FTS search on the full query, each extracted symbol, and
    ///    agent-provided extra keywords (the porter tokenizer handles
    ///    inflected variants at match time).
    /// 2. Exact name supplement — ensures perfect name matches are never buried
    ///    by BM25 noise.
    /// 3. Exact source supplement — qualified expressions such as
    ///    `Type::Variant` seed every matching enclosing symbol.
    /// 4. Re-rank with structural signals (kind, visibility, path).
    /// 5. Connectivity boost (incoming call counts).
    /// 6. Co-occurrence boost for multi-term queries — symbols whose file
    ///    contains multiple search terms rank higher.
    /// 7. Per-file diversity cap — limits how many symbols from a single file
    ///    appear so one large file doesn't dominate the output.
    async fn find_entry_points(
        &self,
        query: &str,
        symbols: &[String],
        options: &BuildContextOptions,
    ) -> Result<(Vec<Node>, RetrievalDiagnostics)> {
        // Base score for an exact name match. Negated-BM25 scores from
        // `search_nodes_bounded` run roughly 5–30 before structural boosts,
        // so this must sit well above that band for a perfect name match to
        // win the MAX merge over FTS hits and rank ahead of them.
        const EXACT_MATCH_SCORE: f64 = 100.0;
        // Floor for the "strong" diagnostics tier: supplements and solidly
        // boosted FTS hits land above this; weak lexical matches fall below.
        const STRONG_MATCH_SCORE: f64 = 10.0;
        debug_assert!(
            !query.is_empty(),
            "find_entry_points called with empty query"
        );
        debug_assert!(options.search_limit > 0, "search_limit must be positive");
        // `excluded` is the hard exclusion set: nodes here never enter
        // `candidates`, regardless of which channel surfaces them.
        let excluded: HashSet<String> = options.exclude_node_ids.clone();
        // `index_of` maps an already-collected node id to its slot in
        // `candidates`, so a node surfaced by multiple channels (FTS + exact
        // name) is merged in place via MAX score rather than first-seen-wins.
        let mut index_of: HashMap<String, usize> = HashMap::new();
        let mut candidates: Vec<SearchResult> = Vec::new();
        // Qualified paths (`foo::bar`) and verbatim multi-word phrases are both
        // exact-copy evidence and share one source scan. The phrases matter for
        // string literals, which live in no symbol name and which FTS ranks
        // badly when they sit inside a large object literal (#362).
        let mut literal_terms = exact_source_terms(query, &options.extra_keywords);
        literal_terms.extend(exact_phrase_terms(query, &options.extra_keywords));
        let exact_source_candidates = self
            .find_exact_source_candidates(&literal_terms, options)
            .await?;
        let exact_source_ids: HashSet<String> = exact_source_candidates
            .iter()
            .map(|candidate| candidate.node.id.clone())
            .collect();
        for candidate in exact_source_candidates {
            index_of.insert(candidate.node.id.clone(), candidates.len());
            candidates.push(candidate);
        }
        let cap = options.max_nodes * 2;

        // Build a deduplicated, ordered list of bounded FTS searches. Passing
        // the whole natural-language sentence to FTS creates a broad OR query;
        // SQLite must BM25-rank every row matching common prose before LIMIT is
        // applied. Search meaningful concepts separately so every ranking is
        // bounded, then merge the candidates below.
        let mut fts_terms: Vec<String> = Vec::new();
        let mut fts_seen: HashSet<String> = HashSet::new();
        let push_term = |t: String, terms: &mut Vec<String>, seen: &mut HashSet<String>| {
            if !t.is_empty() && seen.insert(t.clone()) {
                terms.push(t);
            }
        };
        let body_terms = conceptual_query_terms(query, &options.extra_keywords);
        for term in &body_terms {
            // The symbol FTS index uses `porter unicode61`, so inflected prose
            // ("generates", "ranking") stems to match indexed identifiers —
            // the old query-side plural-strip hack (#264) is no longer needed.
            push_term(term.clone(), &mut fts_terms, &mut fts_seen);
        }
        for s in symbols {
            push_term(s.clone(), &mut fts_terms, &mut fts_seen);
        }
        for k in &options.extra_keywords {
            push_term(k.clone(), &mut fts_terms, &mut fts_seen);
        }

        // Fetch each term's ranked top-K up front, then fill the candidate
        // pool round-robin — one candidate per term per round — instead of
        // exhausting terms in prose order. Early broad terms ("icon",
        // "screen") previously consumed the whole pool cap before later, more
        // discriminating terms ("favicon", "oauth") were ever searched
        // (#264). Per-term work stays bounded: each fetch is a ranked
        // top-`fetch_limit` query.
        // Fetch wider than the BFS-root cap: `search_limit` (default 3) bounds
        // how many roots seed traversal (#120), but 3 FTS rows per term is too
        // few for the right symbol to survive merging — a term's top rows are
        // often file nodes or tests. The root cap below still applies.
        let fetch_limit = (options.search_limit * 3).max(10);
        let mut per_term: Vec<VecDeque<SearchResult>> = Vec::with_capacity(fts_terms.len());
        for term in &fts_terms {
            per_term.push(
                self.db
                    .search_nodes_bounded(term, fetch_limit)
                    .await?
                    .into(),
            );
        }
        // Record per-term hit counts before the fill loop consumes the queues.
        // Counts are capped at `fetch_limit` by the bounded query; zero-hit
        // terms are the signal the caller needs to reformulate.
        let term_hits: Vec<(String, usize)> = fts_terms
            .iter()
            .zip(&per_term)
            .map(|(term, queue)| (term.clone(), queue.len()))
            .collect();
        'fill: loop {
            let mut advanced = false;
            for queue in &mut per_term {
                if candidates.len() >= cap {
                    break 'fill;
                }
                let Some(sr) = queue.pop_front() else {
                    continue;
                };
                advanced = true;
                if !Self::score_passes(sr.score, options.min_score)
                    || excluded.contains(&sr.node.id)
                {
                    continue;
                }
                // Merge by MAX score: a node surfaced by an earlier term keeps
                // the higher of the two BM25 scores instead of being dropped.
                if let Some(&idx) = index_of.get(&sr.node.id) {
                    candidates[idx].score = candidates[idx].score.max(sr.score);
                } else {
                    index_of.insert(sr.node.id.clone(), candidates.len());
                    candidates.push(sr);
                }
            }
            if !advanced {
                break;
            }
        }

        // --- Exact name supplement ---
        // Ensures perfect name matches aren't buried by BM25 noise.
        //
        // Only symbols the user actually wrote qualify: a verbatim query
        // token ("tokensave_context", "UserService") or a synthesized
        // multi-word compound ("PromoBanner" from "promo banner", #202).
        // Single-word segments derived by compound-splitting do NOT — the
        // split of "tokensave_context" yields "tokensave", which
        // case-insensitively exact-matches the repo's `TokenSave` god
        // object, and the API-family supplement then floods the candidate
        // pool with every one of its methods at exact-tier scores.
        let exact_names: Vec<String> = symbols
            .iter()
            .filter(|s| !s.contains("::") && s.len() >= 3)
            .filter(|s| is_authored_symbol(s, query, &options.extra_keywords))
            .cloned()
            .collect();
        // Nodes that arrived through exact-name or exact-source evidence, by
        // id. The diagnostics tier derives `exact` from this provenance, not
        // from the score: multiplicative boosts make the score bands overlap
        // (a well-boosted lexical hit can clear EXACT_MATCH_SCORE, and a
        // down-weighted exact match can fall below it).
        let mut exact_ids: HashSet<String> = exact_source_ids.clone();
        if !exact_names.is_empty() {
            let exact_nodes = self
                .db
                .search_nodes_by_exact_name(&exact_names, options.search_limit)
                .await?;
            for node in exact_nodes {
                if excluded.contains(&node.id) {
                    continue;
                }
                exact_ids.insert(node.id.clone());
                // Exact matches bypass the min_score gate by design. If the node
                // already arrived via FTS (with a low score), upgrade it in place
                // to the exact-match score rather than dropping the duplicate.
                if let Some(&idx) = index_of.get(&node.id) {
                    candidates[idx].score = candidates[idx].score.max(EXACT_MATCH_SCORE);
                } else {
                    index_of.insert(node.id.clone(), candidates.len());
                    candidates.push(SearchResult {
                        node,
                        score: EXACT_MATCH_SCORE,
                    });
                }
            }
        }

        // --- Executable-body supplement ---
        // Symbol FTS intentionally stays compact (name/qname/docs/signature),
        // but conceptual requests often describe behavior that appears only
        // inside a function body: retry policy, cache rebuilds, residual
        // calculations, and similar control-loop details. Search indexed
        // source files for co-occurring query terms and associate each match
        // with the smallest enclosing executable node. This gives context a
        // path to the behavioral owner without promoting local variables to
        // first-class graph nodes.
        if body_terms.len() >= 2 {
            for sr in self
                .find_executable_body_candidates(&body_terms, options)
                .await?
            {
                if excluded.contains(&sr.node.id) {
                    continue;
                }
                if let Some(&idx) = index_of.get(&sr.node.id) {
                    candidates[idx].score = candidates[idx].score.max(sr.score);
                } else {
                    index_of.insert(sr.node.id.clone(), candidates.len());
                    candidates.push(sr);
                }
            }
        }

        // --- Requested API-family supplement ---
        // Conceptual architecture queries often name a type (`Source`,
        // `BoundaryConfig`, `SimulationOutput`) and expect its behavior, not
        // just the declaration. Promote executable children of strongly
        // matched owner types so their methods remain discoverable even when
        // method names use domain-specific vocabulary absent from the query.
        let owner_candidates: Vec<SearchResult> = candidates
            .iter()
            .filter(|candidate| is_api_owner_kind(&candidate.node.kind))
            .cloned()
            .collect();
        let owner_scores: HashMap<String, f64> = owner_candidates
            .iter()
            .map(|owner| (owner.node.id.clone(), owner.score))
            .collect();
        let owner_ids: Vec<String> = owner_scores.keys().cloned().collect();
        for child in self.db.get_children_of_many(&owner_ids).await? {
            if !is_executable_kind(&child.kind) || excluded.contains(&child.id) {
                continue;
            }
            let score = child
                .parent_id
                .as_ref()
                .and_then(|parent| owner_scores.get(parent))
                .copied()
                .unwrap_or_default()
                * 0.5;
            if let Some(&idx) = index_of.get(&child.id) {
                candidates[idx].score = candidates[idx].score.max(score);
            } else {
                index_of.insert(child.id.clone(), candidates.len());
                candidates.push(SearchResult { node: child, score });
            }
        }

        // --- path_prefix filter: restrict entry points to the given subdirectory ---
        if let Some(ref prefix) = options.path_prefix {
            let with_slash = if prefix.ends_with('/') {
                prefix.clone()
            } else {
                format!("{prefix}/")
            };
            candidates.retain(|sr| {
                sr.node.file_path.starts_with(&with_slash) || sr.node.file_path == *prefix
            });
        }

        // --- path_include / path_exclude substring filters (#113) ---
        if !options.path_include.is_empty() || !options.path_exclude.is_empty() {
            candidates.retain(|sr| {
                path_lists_keep(
                    &sr.node.file_path,
                    &options.path_include,
                    &options.path_exclude,
                )
            });
        }

        // --- queryignore filter: drop entry points whose path matches a
        // project-level query-ignore pattern (.tokensave/queryignore) ---
        if !options.query_ignore.is_empty() {
            candidates.retain(|sr| !options.query_ignore.is_ignored(&sr.node.file_path));
        }

        // --- Re-rank with structural signals (kind, visibility, path) ---
        rerank_candidates(&mut candidates);
        apply_executable_intent_boost(&mut candidates, query);

        // --- Connectivity boost (batch edge-count query) ---
        let node_ids: Vec<String> = candidates.iter().map(|c| c.node.id.clone()).collect();
        if let Ok(call_counts) = self.db.batch_incoming_call_counts(&node_ids).await {
            apply_connectivity_boost(&mut candidates, &call_counts);
        }

        // --- Co-occurrence boost for multi-term queries ---
        let query_terms: Vec<String> = query
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|w| w.len() >= 3)
            .collect();
        if query_terms.len() >= 2 {
            apply_cooccurrence_boost(&mut candidates, &query_terms);
        }

        // --- Cap BFS roots (#120) ---
        // Every surviving entry point seeds its own BFS in `expand_subgraph`,
        // which shares a fixed `max_nodes` budget across all roots. With many
        // roots the per-root budget collapses to a shallow fan-out (e.g. 36
        // roots over a 120-node budget gives ~3 nodes each). Capping the roots
        // to `search_limit` keeps each top-ranked root's traversal meaningful.
        // Candidates are score-sorted here (rerank + connectivity + cooccurrence
        // boosts all re-sort). Apply diversity before the final root limit so
        // a large file's executable owner is still available to the cap.
        // --- Per-file diversity cap + final BFS root limit ---
        // Best post-boost score and the id that holds it, captured before
        // `candidates` is consumed by the partition below. The `exact` tier
        // comes from provenance (`exact_ids`), never from the score: boosts
        // make the bands overlap, so a score threshold would mislabel a
        // well-boosted lexical hit as exact and a down-weighted exact match
        // as merely strong. The score decides only strong versus fts-only —
        // boosted negated-BM25 hits land in the ~5–30 band, so a strong
        // match clears STRONG_MATCH_SCORE while weak lexical matches fall
        // below it.
        let best: Option<(&str, f64)> = candidates
            .iter()
            .map(|candidate| (candidate.node.id.as_str(), candidate.score))
            .fold(None, |acc: Option<(&str, f64)>, (id, score)| match acc {
                Some((_, top)) if top >= score => acc,
                _ => Some((id, score)),
            });
        let best_score = best.map(|(_, score)| score);
        let match_quality = best.map(|(id, score)| {
            if exact_ids.contains(id) {
                "exact".to_string()
            } else if score >= STRONG_MATCH_SCORE {
                "strong".to_string()
            } else {
                "fts-only".to_string()
            }
        });
        let diagnostics = RetrievalDiagnostics {
            term_hits,
            best_score,
            match_quality,
        };
        let max_per_file = options.max_per_file.unwrap_or(options.max_nodes);
        // `search_limit` and per-file diversity bound ranked semantic BFS roots
        // (#120), but exact source hits are correctness-sensitive seeds rather
        // than semantic suggestions. Retain all exact enclosing symbols that
        // fit in `max_nodes`, then add up to the usual semantic-root allowance.
        // This prevents a generic candidate from displacing a literal hit even
        // when many hits live in one file.
        let (mut exact_candidates, semantic_candidates): (Vec<_>, Vec<_>) = candidates
            .into_iter()
            .partition(|candidate| exact_source_ids.contains(&candidate.node.id));
        exact_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut entry_points: Vec<Node> = exact_candidates
            .into_iter()
            .take(options.max_nodes)
            .map(|candidate| candidate.node)
            .collect();
        let semantic_slots = options
            .search_limit
            .min(options.max_nodes.saturating_sub(entry_points.len()));
        if semantic_slots > 0 {
            entry_points.extend(apply_per_file_cap(
                semantic_candidates,
                semantic_slots,
                max_per_file,
            ));
        }

        debug_assert!(
            entry_points.len() <= options.max_nodes,
            "entry_points exceeds max_nodes"
        );
        Ok((entry_points, diagnostics))
    }

    /// Finds the innermost indexed symbol enclosing each exact qualified
    /// source expression. This intentionally mirrors literal search instead
    /// of relying on symbol FTS: punctuation such as `::` is tokenized away by
    /// semantic indexes, which loses the distinction between `Type::Variant`
    /// and generic mentions of either word.
    async fn find_exact_source_candidates(
        &self,
        terms: &[String],
        options: &BuildContextOptions,
    ) -> Result<Vec<SearchResult>> {
        const EXACT_SOURCE_SCORE: f64 = 1_000.0;
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        // Candidates are keyed by the nodes a file contains, so an artifact
        // could never contribute one — scanning them would only cost I/O (#323).
        let mut files = self.db.get_code_files().await?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut candidates: HashMap<String, SearchResult> = HashMap::new();

        for file in files {
            if !path_lists_keep(&file.path, &options.path_include, &options.path_exclude)
                || options.query_ignore.is_ignored(&file.path)
                || options.path_prefix.as_ref().is_some_and(|prefix| {
                    let with_slash = format!("{}/", prefix.trim_end_matches('/'));
                    file.path != *prefix && !file.path.starts_with(&with_slash)
                })
            {
                continue;
            }

            let path = self.project_root.join(&file.path);
            let Ok(source) = crate::sync::read_source_file(&path) else {
                continue;
            };
            if !terms.iter().any(|term| source.contains(term)) {
                continue;
            }

            let nodes = self.db.get_nodes_by_file(&file.path).await?;
            for (line_index, line) in source.lines().enumerate() {
                let hit_count = terms.iter().filter(|term| line.contains(*term)).count();
                if hit_count == 0 {
                    continue;
                }
                let line0 = line_index as u32;
                let enclosing = nodes
                    .iter()
                    .filter(|node| {
                        node.start_line <= line0
                            && line0 <= node.end_line
                            && !options.exclude_node_ids.contains(&node.id)
                    })
                    .min_by_key(|node| node.end_line.saturating_sub(node.start_line));
                let Some(node) = enclosing else {
                    continue;
                };

                candidates
                    .entry(node.id.clone())
                    .and_modify(|candidate| candidate.score += hit_count as f64)
                    .or_insert_with(|| SearchResult {
                        node: node.clone(),
                        score: EXACT_SOURCE_SCORE + hit_count as f64,
                    });
            }
        }

        let mut candidates: Vec<SearchResult> = candidates.into_values().collect();
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node.file_path.cmp(&b.node.file_path))
                .then_with(|| a.node.start_line.cmp(&b.node.start_line))
        });
        Ok(candidates)
    }

    /// Finds executable symbols whose persistently indexed bodies contain
    /// several conceptual query terms. Source files are read only while
    /// indexing; request-time work is bounded FTS plus one batched node fetch.
    async fn find_executable_body_candidates(
        &self,
        terms: &[String],
        options: &BuildContextOptions,
    ) -> Result<Vec<SearchResult>> {
        let mut results = self
            .db
            .search_executable_bodies(terms, options.max_nodes.saturating_mul(4))
            .await?
            .into_iter()
            .filter(|(node, _)| {
                path_lists_keep(
                    &node.file_path,
                    &options.path_include,
                    &options.path_exclude,
                ) && !options.query_ignore.is_ignored(&node.file_path)
                    && options.path_prefix.as_ref().is_none_or(|prefix| {
                        let with_slash = format!("{}/", prefix.trim_end_matches('/'));
                        node.file_path == *prefix || node.file_path.starts_with(&with_slash)
                    })
            })
            .map(|(node, hits)| {
                let control_flow_boost = f64::from(node.branches + node.loops).min(4.0) * 0.25;
                SearchResult {
                    node,
                    score: 1.5 + hits as f64 + control_flow_boost,
                }
            })
            .collect::<Vec<_>>();
        results.truncate(options.max_nodes * 2);
        Ok(results)
    }

    /// Expands the subgraph around entry points using BFS traversal.
    async fn expand_subgraph(
        &self,
        entry_points: &[Node],
        options: &BuildContextOptions,
    ) -> Result<Subgraph> {
        debug_assert!(
            options.traversal_depth > 0,
            "traversal_depth must be positive"
        );
        debug_assert!(
            options.max_nodes > 0,
            "max_nodes must be positive for expand_subgraph"
        );
        let traverser = GraphTraverser::new(self.db);
        let mut all_nodes: Vec<Node> = Vec::new();
        let mut all_edges: Vec<Edge> = Vec::new();
        let mut all_roots: Vec<String> = Vec::new();
        let mut seen_node_ids: HashSet<String> = HashSet::new();
        let mut seen_edge_keys: HashSet<(String, String, String)> = HashSet::new();

        let traversal_opts = TraversalOptions {
            max_depth: options.traversal_depth as u32,
            edge_kinds: None,
            node_kinds: None,
            direction: TraversalDirection::Both,
            limit: options.max_nodes as u32,
            include_start: true,
        };

        for node in entry_points {
            let sub = traverser.traverse_bfs(&node.id, &traversal_opts).await?;

            for root in sub.roots {
                if !all_roots.contains(&root) {
                    all_roots.push(root);
                }
            }

            for n in sub.nodes {
                if seen_node_ids.insert(n.id.clone()) {
                    all_nodes.push(n);
                }
            }

            for e in sub.edges {
                let key = (
                    e.source.clone(),
                    e.target.clone(),
                    e.kind.as_str().to_string(),
                );
                if seen_edge_keys.insert(key) {
                    all_edges.push(e);
                }
            }

            if all_nodes.len() >= options.max_nodes {
                break;
            }
        }

        // --- Edge recovery after node trimming ---
        // When we truncate nodes, some edges may reference removed nodes.
        // Instead of discarding those edges entirely, we keep edges that
        // connect any two surviving nodes, preserving subgraph connectivity.
        let surviving: HashSet<&str> = if all_nodes.len() > options.max_nodes {
            all_nodes.truncate(options.max_nodes);
            all_nodes.iter().map(|n| n.id.as_str()).collect()
        } else {
            all_nodes.iter().map(|n| n.id.as_str()).collect()
        };
        all_edges.retain(|e| {
            surviving.contains(e.source.as_str()) && surviving.contains(e.target.as_str())
        });

        Ok(Subgraph {
            nodes: all_nodes,
            edges: all_edges,
            roots: all_roots,
        })
    }

    /// Extracts code blocks for the entry-point nodes.
    fn extract_code_blocks(
        &self,
        entry_points: &[Node],
        options: &BuildContextOptions,
        file_cache: &mut HashMap<String, Option<String>>,
    ) -> Vec<CodeBlock> {
        debug_assert!(
            options.max_code_blocks > 0,
            "max_code_blocks must be positive"
        );
        debug_assert!(
            options.max_code_block_size > 0,
            "max_code_block_size must be positive"
        );
        let mut blocks: Vec<CodeBlock> = Vec::new();

        for node in entry_points {
            if blocks.len() >= options.max_code_blocks {
                break;
            }

            if let Some(code) = self.get_code_cached(node, file_cache) {
                let truncated = truncate_code_block(
                    &code,
                    options.max_code_block_size,
                    options.max_code_lines,
                    &node.id,
                );

                blocks.push(CodeBlock {
                    content: truncated,
                    file_path: node.file_path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                    node_id: Some(node.id.clone()),
                });
            }
        }

        blocks
    }

    /// Merges code blocks from the same file that are adjacent or overlapping.
    /// Two blocks are "adjacent" if the gap between them is <= 5 lines.
    fn merge_adjacent_blocks(
        &self,
        blocks: Vec<CodeBlock>,
        file_cache: &mut HashMap<String, Option<String>>,
    ) -> Vec<CodeBlock> {
        if blocks.len() <= 1 {
            return blocks;
        }

        // Group by file_path
        let mut by_file: std::collections::HashMap<String, Vec<CodeBlock>> =
            std::collections::HashMap::new();
        for block in blocks {
            by_file
                .entry(block.file_path.clone())
                .or_default()
                .push(block);
        }

        let mut merged: Vec<CodeBlock> = Vec::new();
        for (_file, mut file_blocks) in by_file {
            file_blocks.sort_by_key(|b| b.start_line);
            let mut current = file_blocks.remove(0);
            for next in file_blocks {
                // Merge if overlapping or gap <= 5 lines
                if next.start_line <= current.end_line + 5 {
                    let new_end = current.end_line.max(next.end_line);
                    // Re-read the merged range from the file
                    let merged_node = Node {
                        id: current.node_id.clone().unwrap_or_default(),
                        kind: NodeKind::Function,
                        name: String::new(),
                        qualified_name: String::new(),
                        file_path: current.file_path.clone(),
                        start_line: current.start_line,
                        attrs_start_line: current.start_line,
                        end_line: new_end,
                        start_column: 0,
                        end_column: 0,
                        signature: None,
                        docstring: None,
                        visibility: Visibility::default(),
                        is_async: false,
                        branches: 0,
                        loops: 0,
                        returns: 0,
                        max_nesting: 0,
                        unsafe_blocks: 0,
                        unchecked_calls: 0,
                        assertions: 0,
                        cognitive_complexity: 0,
                        distinct_operators: 0,
                        distinct_operands: 0,
                        total_operators: 0,
                        total_operands: 0,
                        updated_at: 0,
                        parent_id: None,
                    };
                    if let Some(code) = self.get_code_cached(&merged_node, file_cache) {
                        current.content = code;
                        current.end_line = new_end;
                    } else {
                        // Can't re-read; just concatenate
                        current.content.push_str("\n\n");
                        current.content.push_str(&next.content);
                        current.end_line = new_end;
                    }
                } else {
                    merged.push(current);
                    current = next;
                }
            }
            merged.push(current);
        }
        merged.sort_by(|a, b| (&a.file_path, a.start_line).cmp(&(&b.file_path, b.start_line)));
        merged
    }

    /// Checks whether a search score passes the minimum threshold.
    ///
    /// FTS5 ranks are small negative numbers (closer to zero = better). After
    /// negation the scores are small positive values that may not clear a high
    /// threshold. We accept any result whose score is positive (i.e. the FTS
    /// engine considered it a match) unless the caller explicitly set a
    /// non-default threshold above 0.
    fn score_passes(score: f64, min_score: f64) -> bool {
        score > 0.0 && score >= min_score
    }

    /// Collects unique file paths from all nodes in the subgraph.
    fn collect_related_files(subgraph: &Subgraph) -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut files: Vec<String> = Vec::new();

        for node in &subgraph.nodes {
            if seen.insert(node.file_path.clone()) {
                files.push(node.file_path.clone());
            }
        }

        files
    }

    /// Builds a human-readable summary string.
    fn build_summary(query: &str, entry_points: &[Node], subgraph: &Subgraph) -> String {
        let ep_count = entry_points.len();
        let node_count = subgraph.nodes.len();
        let edge_count = subgraph.edges.len();

        if ep_count == 0 {
            format!("No matching symbols found for \"{query}\"")
        } else {
            format!(
                "Found {ep_count} entry point(s) for \"{query}\" with {node_count} related node(s) and {edge_count} edge(s)"
            )
        }
    }
}

/// Extracts potential symbol names from natural language text.
///
/// Recognizes the following patterns:
/// - CamelCase words (e.g. `UserService`, `processRequest`)
/// - `snake_case` words (e.g. `process_request`, `user_service`)
/// - `SCREAMING_SNAKE_CASE` words (e.g. `MAX_RETRIES`)
/// - Qualified paths with `::` separators (e.g. `crate::types::Node` yields `Node`)
///
/// Common English stop words are filtered out.
pub fn extract_symbols_from_query(query: &str) -> Vec<String> {
    debug_assert!(
        !query.is_empty(),
        "extract_symbols_from_query called with empty query"
    );
    let stop_words: HashSet<&str> = SYMBOL_STOP_WORDS.iter().copied().collect();

    let mut symbols: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut plain_words: Vec<Option<String>> = Vec::new();
    for token in query.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != ':');
        classify_token(clean, &stop_words, &mut symbols, &mut seen);
        // Track plain lowercase words in order for bigram synthesis below.
        let is_plain = clean.len() >= 3
            && clean.chars().all(|c| c.is_ascii_lowercase())
            && !stop_words.contains(clean.to_lowercase().as_str());
        plain_words.push(is_plain.then(|| clean.to_string()));
    }

    // Adjacent plain words often name a CamelCase symbol in spaced form:
    // "promo banner" → `PromoBanner` (#202). Synthesize Pascal/camelCase
    // bigrams so the exact-name supplement can surface those symbols.
    for pair in plain_words.windows(2) {
        if let [Some(a), Some(b)] = pair {
            let cap = |w: &str| {
                let mut c = w.chars();
                c.next().map_or_else(String::new, |f| {
                    f.to_ascii_uppercase().to_string() + c.as_str()
                })
            };
            let pascal = format!("{}{}", cap(a), cap(b));
            if seen.insert(pascal.clone()) {
                symbols.push(pascal);
            }
            let camel = format!("{a}{}", cap(b));
            if seen.insert(camel.clone()) {
                symbols.push(camel);
            }
        }
    }

    symbols
}

/// Stop words filtered out during symbol extraction from natural language.
const SYMBOL_STOP_WORDS: &[&str] = &[
    "the",
    "is",
    "in",
    "for",
    "to",
    "a",
    "an",
    "of",
    "and",
    "or",
    "not",
    "this",
    "that",
    "it",
    "with",
    "on",
    "at",
    "by",
    "from",
    "as",
    "be",
    "was",
    "are",
    "been",
    "being",
    "have",
    "has",
    "had",
    "do",
    "does",
    "did",
    "will",
    "would",
    "could",
    "should",
    "may",
    "might",
    "can",
    "shall",
    "how",
    "what",
    "where",
    "when",
    "who",
    "which",
    "why",
    "if",
    "then",
    "else",
    "but",
    "so",
    "up",
    "out",
    "no",
    "yes",
    "all",
    "any",
    "each",
    "every",
    "fix",
    "look",
    "update",
    "add",
    "remove",
    "delete",
    "change",
    "check",
    "find",
    "get",
    "set",
    "use",
    "make",
    "call",
    "function",
    "method",
    "singleton_method",
    "class",
    "struct",
    "type",
    "module",
    "file",
    "handler",
    "implement",
    "create",
    "about",
    // Code-specific noise words (ported from codegraph)
    "interface",
    "trait",
    "enum",
    "variable",
    "import",
    "export",
    "return",
    "error",
    "test",
    "spec",
    "helper",
    "util",
    "config",
    "service",
    "model",
    "view",
    "controller",
    "code",
    "new",
    "init",
    "default",
    "value",
    "data",
    "result",
];

/// Classify a single cleaned token and push any symbols it yields.
fn classify_token(
    clean: &str,
    stop_words: &HashSet<&str>,
    symbols: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if clean.is_empty() {
        return;
    }

    if clean.contains("::") {
        // Qualified path: extract last segment and full path
        if let Some(last) = clean.rsplit("::").next() {
            if !last.is_empty()
                && !stop_words.contains(last.to_lowercase().as_str())
                && seen.insert(last.to_string())
            {
                symbols.push(last.to_string());
            }
        }
        let full = clean.to_string();
        if seen.insert(full.clone()) {
            symbols.push(full);
        }
        return;
    }

    // snake_case or SCREAMING_SNAKE
    if clean.contains('_') {
        if !stop_words.contains(clean.to_lowercase().as_str()) && seen.insert(clean.to_string()) {
            symbols.push(clean.to_string());
        }
        // Also emit individual segments for FTS matching.
        for part in split_compound(clean) {
            if part.len() >= 3
                && !stop_words.contains(part.to_lowercase().as_str())
                && seen.insert(part.to_string())
            {
                symbols.push(part.to_string());
            }
        }
        return;
    }

    // CamelCase
    if is_camel_case(clean) {
        if !stop_words.contains(clean.to_lowercase().as_str()) && seen.insert(clean.to_string()) {
            symbols.push(clean.to_string());
        }
        // Also emit individual segments for FTS matching.
        for part in split_compound(clean) {
            if part.len() >= 3
                && !stop_words.contains(part.to_lowercase().as_str())
                && seen.insert(part.to_string())
            {
                symbols.push(part.to_string());
            }
        }
    }
}

/// Returns `true` when `symbol` plausibly came from the user's own words:
/// a multi-word compound (`PromoBanner`, `process_request`), a verbatim
/// whitespace-delimited query token (modulo surrounding punctuation), or an
/// explicitly supplied extra keyword. Single-word segments produced by
/// compound-splitting fail all three tests and are rejected.
fn is_authored_symbol(symbol: &str, query: &str, extra_keywords: &[String]) -> bool {
    split_compound(symbol).len() >= 2
        || query
            .split_whitespace()
            .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_') == symbol)
        || extra_keywords.iter().any(|k| k == symbol)
}

/// Extracts exact, namespace-qualified source tokens from the task and extra
/// keywords. Surrounding prose punctuation and Markdown backticks are removed,
/// but the original case is retained for case-sensitive literal matching.
/// Shortest phrase accepted as exact-copy evidence. Two short words ("is not")
/// would match half a codebase and turn the source scan into noise.
const MIN_EXACT_PHRASE_LEN: usize = 8;

/// Cap on phrase terms, so a keyword list can't make the source scan unbounded.
const MAX_EXACT_PHRASES: usize = 8;

/// Extracts multi-word phrases to be matched verbatim against file source.
///
/// A quoted phrase is the most specific evidence a caller can give — UI copy, a
/// log line, an error message — and it is precisely what identifier extraction
/// cannot represent, since it survives in the source as a string literal and
/// nowhere in any symbol name. Left to FTS alone such a phrase loses: it may hit
/// exactly one node in the codebase, but if that node is a large object literal
/// (a localization catalog, say) BM25's length normalization scores the unique
/// hit below hundreds of generic short matches, and the highest-signal evidence
/// ranks last (#362).
///
/// Phrases come from two places: any caller-supplied keyword containing inner
/// whitespace, and any double-quoted span in the task text. Single words are
/// deliberately excluded — they are already handled by FTS, and scanning source
/// for one common word would match everywhere.
fn exact_phrase_terms(query: &str, extra_keywords: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut phrases = Vec::new();

    let mut push = |candidate: &str, phrases: &mut Vec<String>| {
        let phrase = candidate.trim();
        if phrase.len() >= MIN_EXACT_PHRASE_LEN
            && phrase.split_whitespace().count() >= 2
            && seen.insert(phrase.to_string())
        {
            phrases.push(phrase.to_string());
        }
    };

    for keyword in extra_keywords {
        push(keyword, &mut phrases);
    }
    // Double-quoted spans in the task text: `"..."` pairs, in order.
    let mut rest = query;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        push(&after[..close], &mut phrases);
        rest = &after[close + 1..];
    }

    phrases.truncate(MAX_EXACT_PHRASES);
    phrases
}

fn exact_source_terms(query: &str, extra_keywords: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    std::iter::once(query)
        .chain(extra_keywords.iter().map(String::as_str))
        .flat_map(str::split_whitespace)
        .filter_map(|word| {
            let token = word
                .trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':' || c == '#'));
            let mut segments = token.split("::");
            let first = segments.next()?;
            let rest: Vec<&str> = segments.collect();
            let valid_segment = |segment: &str| {
                !segment.is_empty()
                    && segment
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '#')
            };
            (!rest.is_empty()
                && valid_segment(first)
                && rest.iter().all(|segment| valid_segment(segment))
                && seen.insert(token.to_string()))
            .then(|| token.to_string())
        })
        .collect()
}

/// Extracts lower-case concept words suitable for body co-occurrence search.
/// This intentionally keeps identifier splitting for a separate stage: here
/// ordinary prose terms are enough to discover behavior described in a task.
fn conceptual_query_terms(query: &str, extra_keywords: &[String]) -> Vec<String> {
    const STOP: &[&str] = &[
        "about", "after", "before", "code", "find", "from", "function", "harden", "into", "locate",
        "near", "request", "that", "their", "then", "this", "with",
    ];
    let mut seen = HashSet::new();
    query
        .split_whitespace()
        .chain(extra_keywords.iter().flat_map(|s| s.split_whitespace()))
        .filter_map(|word| {
            let normalized: String = word
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .flat_map(char::to_lowercase)
                .collect();
            (normalized.len() >= 4
                && !STOP.contains(&normalized.as_str())
                && seen.insert(normalized.clone()))
            .then_some(normalized)
        })
        .collect()
}

fn is_executable_kind(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function
            | NodeKind::Method
            | NodeKind::SingletonMethod
            | NodeKind::StructMethod
            | NodeKind::Constructor
            | NodeKind::AbstractMethod
            | NodeKind::Procedure
            | NodeKind::ArrowFunction
    )
}

fn is_api_owner_kind(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Struct
            | NodeKind::Class
            | NodeKind::Enum
            | NodeKind::Trait
            | NodeKind::Interface
            | NodeKind::InterfaceType
            | NodeKind::Impl
            | NodeKind::DataClass
            | NodeKind::Record
    )
}

/// Boosts candidates whose file contains multiple query terms.
///
/// For each candidate, counts how many of the query terms appear (case-
/// insensitive) in the candidate's `name`, `qualified_name`, or `file_path`.
/// Candidates matching 2+ terms get a multiplicative boost.
fn apply_cooccurrence_boost(candidates: &mut [SearchResult], query_terms: &[String]) {
    for candidate in candidates.iter_mut() {
        let haystack = format!(
            "{} {} {}",
            candidate.node.name.to_lowercase(),
            candidate.node.qualified_name.to_lowercase(),
            candidate.node.file_path.to_lowercase(),
        );
        let hits: usize = query_terms
            .iter()
            .filter(|term| haystack.contains(term.as_str()))
            .count();
        if hits >= 2 {
            // Boost proportional to coverage: 2 terms → 1.3×, 3 → 1.6×, etc.
            candidate.score *= 1.0 + (hits as f64 - 1.0) * 0.3;
        }
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Returns `true` if `path` survives the include/exclude substring filters.
///
/// Mirrors the MCP handler helper `filter_by_path_lists`: backslashes are
/// normalized to `/`, exclude takes precedence over include, and an empty
/// include list means "no positive constraint". Case-sensitive substring match.
fn path_lists_keep(path: &str, include: &[String], exclude: &[String]) -> bool {
    let normalized = path.replace('\\', "/");
    if exclude.iter().any(|sub| normalized.contains(sub.as_str())) {
        return false;
    }
    if !include.is_empty() {
        return include.iter().any(|sub| normalized.contains(sub.as_str()));
    }
    true
}

/// Applies a per-file cap to search results, keeping the top `max_total`
/// results but allowing at most `max_per_file` from any single file.
///
/// Results must already be sorted by score (descending). Excess results from
/// over-represented files are moved to a spillover list and appended at the
/// end if there's room.
fn apply_per_file_cap(
    candidates: Vec<SearchResult>,
    max_total: usize,
    max_per_file: usize,
) -> Vec<Node> {
    let files_with_owner: HashSet<String> = candidates
        .iter()
        .filter(|candidate| is_executable_kind(&candidate.node.kind))
        .map(|candidate| candidate.node.file_path.clone())
        .collect();
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut owner_accepted: HashSet<String> = HashSet::new();
    let mut accepted: Vec<Node> = Vec::new();
    let mut spillover: Vec<Node> = Vec::new();

    for sr in candidates {
        let file_path = sr.node.file_path.clone();
        let is_owner = is_executable_kind(&sr.node.kind);
        let count = file_counts.entry(file_path.clone()).or_insert(0);
        let reserve_owner_slot = !is_owner
            && files_with_owner.contains(&file_path)
            && !owner_accepted.contains(&file_path)
            && count.saturating_add(1) >= max_per_file;
        if *count < max_per_file && !reserve_owner_slot {
            *count += 1;
            if is_owner {
                owner_accepted.insert(file_path);
            }
            accepted.push(sr.node);
        } else {
            spillover.push(sr.node);
        }
        if accepted.len() >= max_total {
            break;
        }
    }

    // Fill remaining slots from spillover
    for node in spillover {
        if accepted.len() >= max_total {
            break;
        }
        accepted.push(node);
    }

    accepted
}

/// Truncates a code snippet to `max_lines` lines and `max_size` bytes,
/// whichever bites first, preferring a line boundary.
///
/// A truncated snippet closes with a `tokensave_body` handle rather than a bare
/// ellipsis, so the caller can fetch the remainder in one follow-up call instead
/// of re-deriving which symbol the fragment came from. Snippets under both
/// limits are returned unchanged.
fn truncate_code_block(
    code: &str,
    max_size: usize,
    max_lines: Option<usize>,
    node_id: &str,
) -> String {
    // The line cap goes first: it is the caller's explicit budget, and applying
    // it up front means the byte cap only ever trims what the line cap kept.
    let capped = match max_lines {
        Some(limit) => line_prefix(code, limit),
        None => code,
    };
    if capped.len() <= max_size {
        return if capped.len() == code.len() {
            code.to_string()
        } else {
            with_body_handle(capped, node_id)
        };
    }
    let prefix = crate::text::utf8_prefix_at_or_before(capped, max_size);
    let end = prefix.rfind('\n').unwrap_or(prefix.len());
    with_body_handle(&prefix[..end], node_id)
}

/// Returns the prefix of `code` spanning at most `max_lines` lines.
///
/// A cap of zero is meaningless — an empty snippet is strictly worse than none
/// at all — so it is treated as "no cap", matching the handler, which clamps
/// the parameter to at least one line.
fn line_prefix(code: &str, max_lines: usize) -> &str {
    if max_lines == 0 {
        return code;
    }
    match code.match_indices('\n').nth(max_lines - 1) {
        // Only the snippet's own trailing newline follows the cut, so the
        // snippet already fits — returning a shortened slice here would make
        // the caller mark an untruncated block as truncated.
        Some((idx, _)) if idx + 1 == code.len() => code,
        Some((idx, _)) => &code[..idx],
        None => code,
    }
}

/// Appends the follow-up handle that points at the full body.
fn with_body_handle(body: &str, node_id: &str) -> String {
    format!("{body}\n... [truncated — full body: tokensave_body node_id={node_id}]")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn test_spaced_words_generate_camel_bigrams() {
        // #202: "promo banner" should surface the CamelCase symbol PromoBanner.
        let symbols = extract_symbols_from_query("redesign the promo banner layout");
        assert!(symbols.contains(&"PromoBanner".to_string()), "{symbols:?}");
        assert!(symbols.contains(&"promoBanner".to_string()), "{symbols:?}");
    }

    #[test]
    fn test_extract_snake_case() {
        let symbols = extract_symbols_from_query("fix the process_request function");
        assert!(symbols.contains(&"process_request".to_string()));
    }

    // --- exact-name provenance ---

    #[test]
    fn test_authored_symbol_multiword_compound_passes() {
        // Compounds can only come from the query or synthesis — always eligible.
        assert!(is_authored_symbol("PromoBanner", "anything", &[]));
        assert!(is_authored_symbol("process_request", "anything", &[]));
    }

    #[test]
    fn test_authored_symbol_verbatim_token_passes() {
        let q = "how does tokensave_context build its result?";
        assert!(is_authored_symbol("tokensave_context", q, &[]));
    }

    #[test]
    fn test_authored_symbol_trims_punctuation() {
        assert!(is_authored_symbol(
            "tokensave_context",
            "call tokensave_context.",
            &[]
        ));
    }

    #[test]
    fn test_authored_symbol_derived_segment_rejected() {
        // "tokensave" is a split segment of "tokensave_context", not a word
        // the user wrote — it must not qualify for exact-name matching.
        let q = "how does tokensave_context build its result?";
        assert!(!is_authored_symbol("tokensave", q, &[]));
    }

    #[test]
    fn test_authored_symbol_extra_keyword_passes() {
        assert!(is_authored_symbol(
            "ranker",
            "unrelated query",
            &["ranker".to_string()]
        ));
    }

    #[test]
    fn test_extract_camel_case() {
        let symbols = extract_symbols_from_query("update UserService handler");
        assert!(symbols.contains(&"UserService".to_string()));
    }

    #[test]
    fn test_extract_screaming_snake() {
        let symbols = extract_symbols_from_query("increase MAX_RETRIES limit");
        assert!(symbols.contains(&"MAX_RETRIES".to_string()));
    }

    #[test]
    fn test_extract_qualified_path() {
        let symbols = extract_symbols_from_query("look at crate::types::Node");
        assert!(symbols.iter().any(|s| s.contains("Node")));
    }

    #[test]
    fn test_exact_source_terms_preserve_qualified_expressions() {
        let terms = exact_source_terms(
            "Find every path for `BasisOrder::Linear`, not generic BasisOrder.",
            &[
                "BasisOrder::Linear".to_string(),
                "crate::types::Node".to_string(),
            ],
        );
        assert_eq!(
            terms,
            vec![
                "BasisOrder::Linear".to_string(),
                "crate::types::Node".to_string()
            ]
        );
    }

    #[test]
    fn exact_phrase_terms_takes_multiword_keywords_verbatim() {
        // The whole point: a quoted phrase survives in source as a string
        // literal and appears in no symbol name, so it must be matched as-is.
        let terms = exact_phrase_terms(
            "Diagnose the dashboard",
            &["Waiting for status".to_string()],
        );
        assert_eq!(terms, vec!["Waiting for status".to_string()]);
    }

    #[test]
    fn exact_phrase_terms_rejects_single_words_and_short_phrases() {
        // A single common word would match everywhere; FTS already covers it.
        let terms = exact_phrase_terms(
            "no quotes here",
            &["dashboard".to_string(), "status".to_string()],
        );
        assert!(
            terms.is_empty(),
            "single words must not trigger a source scan"
        );

        // Two very short words are no more specific than one.
        let terms = exact_phrase_terms("x", &["is не".to_string(), "a b".to_string()]);
        assert!(
            terms.is_empty(),
            "sub-minimum phrases must be rejected: {terms:?}"
        );
    }

    #[test]
    fn exact_phrase_terms_extracts_quoted_spans_from_the_task() {
        let terms = exact_phrase_terms(
            r#"Why does it show "Waiting for status" instead of "Ready to go" now?"#,
            &[],
        );
        assert_eq!(
            terms,
            vec!["Waiting for status".to_string(), "Ready to go".to_string()]
        );
    }

    #[test]
    fn exact_phrase_terms_ignores_an_unclosed_quote() {
        let terms = exact_phrase_terms(r#"look for "Waiting for status"#, &[]);
        assert!(
            terms.is_empty(),
            "an unterminated quote must not scan: {terms:?}"
        );
    }

    #[test]
    fn exact_phrase_terms_dedupes_and_caps() {
        // Same phrase from both the keyword list and the task text counts once.
        let terms = exact_phrase_terms(
            r#"see "Waiting for status" please"#,
            &["Waiting for status".to_string()],
        );
        assert_eq!(terms.len(), 1);

        // The source scan reads every file, so the term list must stay bounded.
        let many: Vec<String> = (0..40).map(|i| format!("phrase number {i}")).collect();
        assert_eq!(exact_phrase_terms("x", &many).len(), MAX_EXACT_PHRASES);
    }

    #[test]
    fn test_filters_stop_words() {
        let symbols = extract_symbols_from_query("the is in for to a an");
        assert!(symbols.is_empty());
    }

    // --- co-occurrence boost tests ---

    fn make_search_result(name: &str, file_path: &str, score: f64) -> SearchResult {
        SearchResult {
            node: Node {
                id: format!("test:{name}"),
                kind: NodeKind::Function,
                name: name.to_string(),
                qualified_name: format!("{file_path}::{name}"),
                file_path: file_path.to_string(),
                start_line: 1,
                attrs_start_line: 1,
                end_line: 5,
                start_column: 0,
                end_column: 1,
                signature: None,
                docstring: None,
                visibility: Visibility::Pub,
                is_async: false,
                branches: 0,
                loops: 0,
                returns: 0,
                max_nesting: 0,
                unsafe_blocks: 0,
                unchecked_calls: 0,
                assertions: 0,
                cognitive_complexity: 0,
                distinct_operators: 0,
                distinct_operands: 0,
                total_operators: 0,
                total_operands: 0,
                updated_at: 0,
                parent_id: None,
            },
            score,
        }
    }

    #[test]
    fn test_cooccurrence_boost_multi_term() {
        let mut candidates = vec![
            make_search_result("auth_handler", "src/auth.rs", 10.0),
            make_search_result("user_list", "src/user.rs", 10.0),
        ];
        let terms = vec!["auth".to_string(), "handler".to_string()];
        apply_cooccurrence_boost(&mut candidates, &terms);
        // auth_handler matches both terms, user_list matches neither
        assert_eq!(candidates[0].node.name, "auth_handler");
        assert!(candidates[0].score > candidates[1].score);
    }

    #[test]
    fn test_cooccurrence_no_boost_single_term() {
        let mut candidates = vec![make_search_result("auth", "src/auth.rs", 10.0)];
        let terms = vec!["auth".to_string(), "handler".to_string()];
        apply_cooccurrence_boost(&mut candidates, &terms);
        // Only 1 term matches — no boost
        assert_eq!(candidates[0].score, 10.0);
    }

    // --- per-file diversity cap tests ---

    #[test]
    fn test_per_file_cap_limits_single_file() {
        let candidates = vec![
            make_search_result("fn1", "src/big.rs", 10.0),
            make_search_result("fn2", "src/big.rs", 9.0),
            make_search_result("fn3", "src/big.rs", 8.0),
            make_search_result("fn4", "src/other.rs", 7.0),
        ];
        let result = apply_per_file_cap(candidates, 10, 2);
        // Only 2 from big.rs, then other.rs, then spillover
        let big_count = result
            .iter()
            .filter(|n| n.file_path == "src/big.rs")
            .count();
        assert!(big_count <= 3); // 2 accepted + possibly 1 spillover
        assert_eq!(result.len(), 4);
        // First 2 slots for big.rs, 3rd for other.rs
        assert_eq!(result[0].name, "fn1");
        assert_eq!(result[1].name, "fn2");
        assert_eq!(result[2].name, "fn4");
        assert_eq!(result[3].name, "fn3"); // spillover
    }

    #[test]
    fn test_per_file_cap_respects_max_total() {
        let candidates = vec![
            make_search_result("fn1", "src/a.rs", 10.0),
            make_search_result("fn2", "src/b.rs", 9.0),
            make_search_result("fn3", "src/c.rs", 8.0),
        ];
        let result = apply_per_file_cap(candidates, 2, 5);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_per_file_cap_reserves_executable_owner() {
        let mut import = make_search_result("solver_import", "src/big.rs", 10.0);
        import.node.kind = NodeKind::Use;
        let mut config = make_search_result("SolverConfig", "src/big.rs", 9.0);
        config.node.kind = NodeKind::Struct;
        let owner = make_search_result("run_solver", "src/big.rs", 8.0);
        let other = make_search_result("helper", "src/other.rs", 7.0);

        let result = apply_per_file_cap(vec![import, config, owner, other], 3, 2);
        let names: Vec<_> = result.iter().map(|node| node.name.as_str()).collect();
        assert!(names.contains(&"run_solver"), "selected roots: {names:?}");
        assert!(names.contains(&"helper"), "selected roots: {names:?}");
    }

    #[test]
    fn test_truncated_code_block_points_at_tokensave_body() {
        let code = "fn wide() {\n".to_string() + &"    line();\n".repeat(50);
        let out = truncate_code_block(&code, 60, None, "abc123");
        assert!(
            out.contains("tokensave_body node_id=abc123"),
            "truncated block must carry a follow-up handle: {out}"
        );
        assert!(out.starts_with("fn wide() {"), "{out}");
        // Truncation still lands on a line boundary, not mid-line.
        let body = out.split("\n... [truncated").next().unwrap();
        assert!(!body.ends_with("    line"), "cut mid-line: {body:?}");
    }

    #[test]
    fn test_short_code_block_is_returned_verbatim() {
        let code = "fn small() {}\n";
        assert_eq!(truncate_code_block(code, 1500, None, "abc123"), code);
        // Exactly at the limit is not truncation.
        assert_eq!(truncate_code_block(code, code.len(), None, "abc123"), code);
    }

    #[test]
    fn test_truncate_code_block_does_not_split_multibyte_char() {
        // A multi-byte char straddling the cutoff must not panic or corrupt.
        let code = "fn f() {\n    // ✅✅✅✅✅✅✅✅✅✅\n    body();\n}\n";
        let out = truncate_code_block(code, 20, None, "n1");
        assert!(out.contains("tokensave_body node_id=n1"), "{out}");
    }

    #[test]
    fn test_max_code_lines_caps_the_snippet() {
        let code = "fn wide() {\n".to_string() + &"    line();\n".repeat(50);
        let out = truncate_code_block(&code, usize::MAX, Some(3), "abc123");
        let body = out.split("\n... [truncated").next().unwrap();
        assert_eq!(body.lines().count(), 3, "{out}");
        assert!(out.contains("tokensave_body node_id=abc123"), "{out}");
    }

    #[test]
    fn test_max_code_lines_leaves_shorter_snippets_verbatim() {
        let code = "fn small() {\n    body();\n}\n";
        // A cap above the snippet's length must not add a truncation marker.
        assert_eq!(
            truncate_code_block(code, usize::MAX, Some(20), "n1"),
            code,
            "a snippet under the cap must come back untouched"
        );
    }

    #[test]
    fn test_byte_limit_still_applies_under_a_line_cap() {
        // The line cap keeps 10 lines, but the byte budget only fits ~2 — the
        // tighter of the two has to win.
        let code = "fn wide() {\n".to_string() + &"    line();\n".repeat(50);
        let out = truncate_code_block(&code, 30, Some(10), "n1");
        let body = out.split("\n... [truncated").next().unwrap();
        assert!(body.len() <= 30, "byte cap ignored: {body:?}");
        assert!(!body.ends_with("    line"), "cut mid-line: {body:?}");
    }

    #[test]
    fn line_prefix_boundaries() {
        let code = "a\nb\nc\n";
        assert_eq!(line_prefix(code, 1), "a");
        assert_eq!(line_prefix(code, 2), "a\nb");
        // A cap at or above the line count returns everything, trailing \n included.
        assert_eq!(line_prefix(code, 3), code);
        assert_eq!(line_prefix(code, 99), code);
        // Zero means "no cap" — see the doc comment.
        assert_eq!(line_prefix(code, 0), code);
        // A snippet with no trailing newline is not truncated by an exact cap.
        assert_eq!(line_prefix("a\nb", 2), "a\nb");
    }
}
