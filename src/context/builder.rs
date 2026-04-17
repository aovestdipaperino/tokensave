// Rust guideline compliant 2025-10-17
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::context::ranking::{apply_connectivity_boost, rerank_candidates};
use crate::db::Database;
use crate::errors::Result;
use crate::graph::GraphTraverser;
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
        let entry_points = self.find_entry_points(query, &symbols, options).await?;
        let subgraph = self.expand_subgraph(&entry_points, options).await?;

        // Step 4: extract code blocks from source files
        let code_blocks = if options.include_code {
            let blocks = self.extract_code_blocks(&entry_points, options).await?;
            if options.merge_adjacent {
                self.merge_adjacent_blocks(blocks).await
            } else {
                blocks
            }
        } else {
            Vec::new()
        };

        // Collect unique related files
        let related_files = self.collect_related_files(&subgraph);

        // Build summary
        let summary = self.build_summary(query, &entry_points, &subgraph);

        let seen_node_ids: Vec<String> = entry_points.iter().map(|n| n.id.clone()).collect();

        Ok(TaskContext {
            query: query.to_string(),
            summary,
            subgraph,
            entry_points,
            code_blocks,
            related_files,
            seen_node_ids,
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
        let entry_points = self.find_entry_points(query, &symbols, options).await?;
        self.expand_subgraph(&entry_points, options).await
    }

    /// Reads the source file and extracts the code for a node.
    ///
    /// Returns `None` if the file cannot be read or the line range is invalid.
    pub async fn get_code(&self, node: &Node) -> Result<Option<String>> {
        debug_assert!(
            !node.file_path.is_empty(),
            "get_code called with empty file_path"
        );
        debug_assert!(!node.id.is_empty(), "get_code called with empty node id");
        let file_path = self.project_root.join(&node.file_path);
        // Prevent path traversal: ensure the resolved path stays within the project root.
        if let (Ok(canonical), Ok(root)) =
            (file_path.canonicalize(), self.project_root.canonicalize())
        {
            if !canonical.starts_with(&root) {
                return Ok(None);
            }
        }
        let content = match fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let lines: Vec<&str> = content.lines().collect();
        if node.start_line == 0 || node.end_line == 0 {
            return Ok(None);
        }

        let start = (node.start_line as usize).saturating_sub(1);
        let end = node.end_line as usize;

        if start >= lines.len() {
            return Ok(None);
        }

        let end = end.min(lines.len());
        let snippet: String = lines[start..end].join("\n");
        if snippet.is_empty() {
            Ok(None)
        } else {
            Ok(Some(snippet))
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Searches for entry-point nodes matching the query and extracted symbols.
    ///
    /// Pipeline:
    /// 1. FTS search on the full query, each extracted symbol, stem variants,
    ///    and agent-provided extra keywords.
    /// 2. Exact name supplement — ensures perfect name matches are never buried
    ///    by BM25 noise.
    /// 3. Re-rank with structural signals (kind, visibility, path).
    /// 4. Connectivity boost (incoming call counts).
    /// 5. Co-occurrence boost for multi-term queries — symbols whose file
    ///    contains multiple search terms rank higher.
    /// 6. Per-file diversity cap — limits how many symbols from a single file
    ///    appear so one large file doesn't dominate the output.
    async fn find_entry_points(
        &self,
        query: &str,
        symbols: &[String],
        options: &BuildContextOptions,
    ) -> Result<Vec<Node>> {
        debug_assert!(
            !query.is_empty(),
            "find_entry_points called with empty query"
        );
        debug_assert!(options.search_limit > 0, "search_limit must be positive");
        let mut seen_ids: HashSet<String> = options.exclude_node_ids.clone();
        let mut candidates: Vec<SearchResult> = Vec::new();
        let cap = options.max_nodes * 2;

        // --- FTS search: full query ---
        let search_results = self.db.search_nodes(query, options.search_limit).await?;
        for sr in search_results {
            if self.score_passes(sr.score, options.min_score) && seen_ids.insert(sr.node.id.clone())
            {
                candidates.push(sr);
            }
        }

        // --- FTS search: each extracted symbol ---
        for symbol in symbols {
            if candidates.len() >= cap {
                break;
            }
            let results = self.db.search_nodes(symbol, options.search_limit).await?;
            for sr in results {
                if self.score_passes(sr.score, options.min_score)
                    && seen_ids.insert(sr.node.id.clone())
                {
                    candidates.push(sr);
                }
            }
        }

        // --- FTS search: stem variants ---
        let stems = generate_stem_variants(symbols);
        for stem in &stems {
            if candidates.len() >= cap {
                break;
            }
            let results = self.db.search_nodes(stem, options.search_limit).await?;
            for sr in results {
                if self.score_passes(sr.score, options.min_score)
                    && seen_ids.insert(sr.node.id.clone())
                {
                    candidates.push(sr);
                }
            }
        }

        // --- FTS search: agent-provided extra keywords (synonym expansion) ---
        for keyword in &options.extra_keywords {
            if candidates.len() >= cap {
                break;
            }
            let results = self.db.search_nodes(keyword, options.search_limit).await?;
            for sr in results {
                if self.score_passes(sr.score, options.min_score)
                    && seen_ids.insert(sr.node.id.clone())
                {
                    candidates.push(sr);
                }
            }
        }

        // --- Exact name supplement ---
        // Ensures perfect name matches aren't buried by BM25 noise.
        let exact_names: Vec<String> = symbols
            .iter()
            .filter(|s| !s.contains("::") && s.len() >= 3)
            .cloned()
            .collect();
        if !exact_names.is_empty() {
            let exact_nodes = self
                .db
                .search_nodes_by_exact_name(&exact_names, options.search_limit)
                .await?;
            for node in exact_nodes {
                if seen_ids.insert(node.id.clone()) {
                    // Give exact matches a high base score so they compete well.
                    candidates.push(SearchResult { node, score: 20.0 });
                }
            }
        }

        // --- Re-rank with structural signals (kind, visibility, path) ---
        rerank_candidates(&mut candidates);

        // --- Connectivity boost (batch edge-count query) ---
        let node_ids: Vec<String> = candidates.iter().map(|c| c.node.id.clone()).collect();
        if let Ok(call_counts) = self.db.batch_incoming_call_counts(&node_ids).await {
            apply_connectivity_boost(&mut candidates, &call_counts);
        }

        // --- Co-occurrence boost for multi-term queries ---
        let query_terms: Vec<String> = query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() >= 3)
            .collect();
        if query_terms.len() >= 2 {
            apply_cooccurrence_boost(&mut candidates, &query_terms);
        }

        // --- Per-file diversity cap ---
        let max_per_file = options.max_per_file.unwrap_or(options.max_nodes);
        let entry_points = apply_per_file_cap(candidates, options.max_nodes, max_per_file);

        debug_assert!(
            entry_points.len() <= options.max_nodes,
            "entry_points exceeds max_nodes"
        );
        Ok(entry_points)
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
    async fn extract_code_blocks(
        &self,
        entry_points: &[Node],
        options: &BuildContextOptions,
    ) -> Result<Vec<CodeBlock>> {
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

            if let Some(code) = self.get_code(node).await? {
                let truncated = if code.len() > options.max_code_block_size {
                    let mut end = options.max_code_block_size;
                    // Ensure we land on a valid UTF-8 boundary
                    while !code.is_char_boundary(end) && end > 0 {
                        end -= 1;
                    }
                    // Try to truncate at a line boundary
                    if let Some(pos) = code[..end].rfind('\n') {
                        end = pos;
                    }
                    format!("{}...", &code[..end])
                } else {
                    code
                };

                blocks.push(CodeBlock {
                    content: truncated,
                    file_path: node.file_path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                    node_id: Some(node.id.clone()),
                });
            }
        }

        Ok(blocks)
    }

    /// Merges code blocks from the same file that are adjacent or overlapping.
    /// Two blocks are "adjacent" if the gap between them is <= 5 lines.
    async fn merge_adjacent_blocks(&self, blocks: Vec<CodeBlock>) -> Vec<CodeBlock> {
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
                        updated_at: 0,
                    };
                    if let Ok(Some(code)) = self.get_code(&merged_node).await {
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
    fn score_passes(&self, score: f64, min_score: f64) -> bool {
        score > 0.0 && score >= min_score
    }

    /// Collects unique file paths from all nodes in the subgraph.
    fn collect_related_files(&self, subgraph: &Subgraph) -> Vec<String> {
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
    fn build_summary(&self, query: &str, entry_points: &[Node], subgraph: &Subgraph) -> String {
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
/// - snake_case words (e.g. `process_request`, `user_service`)
/// - SCREAMING_SNAKE_CASE words (e.g. `MAX_RETRIES`)
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

    for token in query.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != ':');
        classify_token(clean, &stop_words, &mut symbols, &mut seen);
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

/// Split a compound name into individual words.
///
/// Handles camelCase, PascalCase, and snake_case:
/// - `getUserName` → `["get", "User", "Name"]`
/// - `process_request` → `["process", "request"]`
/// - `MAX_RETRIES` → `["MAX", "RETRIES"]`
fn split_compound(name: &str) -> Vec<&str> {
    if name.contains('_') {
        return name.split('_').filter(|s| !s.is_empty()).collect();
    }

    // camelCase / PascalCase splitting
    let bytes = name.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;

    for i in 1..bytes.len() {
        let cur = bytes[i] as char;
        let prev = bytes[i - 1] as char;

        // Split at lowercase→uppercase boundary (e.g. getUser → get|User)
        let boundary = prev.is_ascii_lowercase() && cur.is_ascii_uppercase();
        // Split at uppercase→uppercase+lowercase (e.g. XMLParser → XML|Parser)
        let acronym_end = i + 1 < bytes.len()
            && prev.is_ascii_uppercase()
            && cur.is_ascii_uppercase()
            && (bytes[i + 1] as char).is_ascii_lowercase();

        if boundary || acronym_end {
            if i > start {
                parts.push(&name[start..i]);
            }
            start = i;
        }
    }
    if start < name.len() {
        parts.push(&name[start..]);
    }
    parts
}

/// Returns `true` if `word` looks like CamelCase.
///
/// The word must contain at least one uppercase letter after the first character
/// and consist only of ASCII alphanumeric characters.
fn is_camel_case(word: &str) -> bool {
    if word.len() < 2 {
        return false;
    }
    // Must be all alphanumeric
    if !word.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    // Must have at least one uppercase letter after the first char
    word[1..].chars().any(|c| c.is_ascii_uppercase())
}

/// Generates suffix-based stem variants for a set of symbols.
///
/// For each symbol, tries common suffixes (e.g. "authenticate" generates
/// "authentication", "authenticator", "authenticated"). Only produces
/// variants that differ from the original and from other symbols.
fn generate_stem_variants(symbols: &[String]) -> Vec<String> {
    /// Common English derivational suffixes, ordered longest-first so that
    /// stripping "ation" is preferred over "ion" when both match.
    const SUFFIX_PAIRS: &[(&str, &[&str])] = &[
        ("tion", &["te", "tor", "t", "ting"]),
        ("sion", &["de", "d", "ding"]),
        ("ment", &["", "ing", "ed"]),
        ("ness", &["", "ly"]),
        ("ing", &["", "e", "ion", "ment"]),
        ("ed", &["", "e", "ing", "ion"]),
        ("er", &["", "e", "ing", "ed"]),
        ("or", &["", "e", "ion"]),
        ("ly", &["", "ness"]),
        ("ize", &["ization", "ized"]),
        ("ise", &["isation", "ised"]),
        ("ate", &["ation", "ator", "ated", "ating"]),
        ("ify", &["ification", "ified"]),
    ];

    let existing: HashSet<String> = symbols.iter().map(|s| s.to_lowercase()).collect();
    let mut variants: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for symbol in symbols {
        let lower = symbol.to_lowercase();
        if lower.len() < 4 {
            continue;
        }

        for &(suffix, replacements) in SUFFIX_PAIRS {
            if let Some(stem) = lower.strip_suffix(suffix) {
                if stem.len() < 2 {
                    continue;
                }
                for &replacement in replacements {
                    let variant = format!("{stem}{replacement}");
                    if variant.len() >= 3
                        && !existing.contains(&variant)
                        && seen.insert(variant.clone())
                    {
                        variants.push(variant);
                    }
                }
                break; // only strip the first matching suffix
            }
        }
    }

    variants
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
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut accepted: Vec<Node> = Vec::new();
    let mut spillover: Vec<Node> = Vec::new();

    for sr in candidates {
        let count = file_counts.entry(sr.node.file_path.clone()).or_insert(0);
        if *count < max_per_file {
            *count += 1;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_snake_case() {
        let symbols = extract_symbols_from_query("fix the process_request function");
        assert!(symbols.contains(&"process_request".to_string()));
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
    fn test_filters_stop_words() {
        let symbols = extract_symbols_from_query("the is in for to a an");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_is_camel_case() {
        assert!(is_camel_case("UserService"));
        assert!(is_camel_case("processRequest"));
        assert!(!is_camel_case("user"));
        assert!(!is_camel_case("U"));
        assert!(!is_camel_case("process_request"));
    }

    // --- stem variant tests ---

    #[test]
    fn test_stem_variants_ate_suffix() {
        let symbols = vec!["authenticate".to_string()];
        let variants = generate_stem_variants(&symbols);
        assert!(variants.contains(&"authentication".to_string()));
        assert!(variants.contains(&"authenticator".to_string()));
    }

    #[test]
    fn test_stem_variants_tion_suffix() {
        let symbols = vec!["authentication".to_string()];
        let variants = generate_stem_variants(&symbols);
        assert!(variants.contains(&"authenticate".to_string()));
    }

    #[test]
    fn test_stem_variants_ing_suffix() {
        let symbols = vec!["parsing".to_string()];
        let variants = generate_stem_variants(&symbols);
        // "parsing" → strip "ing" → stem "pars" → ["pars", "parse", "parsion", "parsment"]
        assert!(variants.contains(&"parse".to_string()));
    }

    #[test]
    fn test_stem_variants_short_words_skipped() {
        let symbols = vec!["ab".to_string()];
        let variants = generate_stem_variants(&symbols);
        assert!(variants.is_empty());
    }

    #[test]
    fn test_stem_variants_no_duplicates_with_existing() {
        let symbols = vec!["authenticate".to_string(), "authentication".to_string()];
        let variants = generate_stem_variants(&symbols);
        // "authentication" is already in symbols, so it shouldn't appear in variants
        assert!(!variants.contains(&"authentication".to_string()));
        // "authenticate" is already in symbols, so it shouldn't appear in variants
        assert!(!variants.contains(&"authenticate".to_string()));
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
                updated_at: 0,
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
        assert!(candidates[0].node.name == "auth_handler");
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
        assert!(result.len() == 4);
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
}
