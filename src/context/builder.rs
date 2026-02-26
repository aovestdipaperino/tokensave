use std::collections::HashSet;
use std::fs;
use std::path::Path;

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
    pub fn build_context(&self, query: &str, options: &BuildContextOptions) -> Result<TaskContext> {
        // Step 1-3: find relevant subgraph and entry points
        let symbols = extract_symbols_from_query(query);
        let entry_points = self.find_entry_points(query, &symbols, options)?;
        let subgraph = self.expand_subgraph(&entry_points, options)?;

        // Step 4: extract code blocks from source files
        let code_blocks = if options.include_code {
            self.extract_code_blocks(&entry_points, options)?
        } else {
            Vec::new()
        };

        // Collect unique related files
        let related_files = self.collect_related_files(&subgraph);

        // Build summary
        let summary = self.build_summary(query, &entry_points, &subgraph);

        Ok(TaskContext {
            query: query.to_string(),
            summary,
            subgraph,
            entry_points,
            code_blocks,
            related_files,
        })
    }

    /// Finds the relevant subgraph for a query without extracting code blocks.
    ///
    /// Extracts symbols from the query, searches for matching nodes, and expands
    /// via BFS traversal to the configured depth.
    pub fn find_relevant_context(
        &self,
        query: &str,
        options: &BuildContextOptions,
    ) -> Result<Subgraph> {
        let symbols = extract_symbols_from_query(query);
        let entry_points = self.find_entry_points(query, &symbols, options)?;
        self.expand_subgraph(&entry_points, options)
    }

    /// Reads the source file and extracts the code for a node.
    ///
    /// Returns `None` if the file cannot be read or the line range is invalid.
    pub fn get_code(&self, node: &Node) -> Result<Option<String>> {
        let file_path = self.project_root.join(&node.file_path);
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
    /// The search results from the database are already ranked by relevance and
    /// limited. We apply `min_score` only when it is positive, allowing the
    /// caller to disable filtering with `min_score = 0.0`.
    fn find_entry_points(
        &self,
        query: &str,
        symbols: &[String],
        options: &BuildContextOptions,
    ) -> Result<Vec<Node>> {
        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut entry_points: Vec<Node> = Vec::new();

        // Search using the full query
        let search_results = self.db.search_nodes(query, options.search_limit)?;
        for sr in &search_results {
            if self.score_passes(sr.score, options.min_score) && seen_ids.insert(sr.node.id.clone())
            {
                entry_points.push(sr.node.clone());
            }
        }

        // Search for each extracted symbol individually
        for symbol in symbols {
            if entry_points.len() >= options.max_nodes {
                break;
            }
            let results = self.db.search_nodes(symbol, options.search_limit)?;
            for sr in results {
                if self.score_passes(sr.score, options.min_score)
                    && seen_ids.insert(sr.node.id.clone())
                {
                    entry_points.push(sr.node.clone());
                }
            }
        }

        // Cap at max_nodes
        entry_points.truncate(options.max_nodes);
        Ok(entry_points)
    }

    /// Expands the subgraph around entry points using BFS traversal.
    fn expand_subgraph(
        &self,
        entry_points: &[Node],
        options: &BuildContextOptions,
    ) -> Result<Subgraph> {
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
            let sub = traverser.traverse_bfs(&node.id, &traversal_opts)?;

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

        all_nodes.truncate(options.max_nodes);

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
    ) -> Result<Vec<CodeBlock>> {
        let mut blocks: Vec<CodeBlock> = Vec::new();

        for node in entry_points {
            if blocks.len() >= options.max_code_blocks {
                break;
            }

            if let Some(code) = self.get_code(node)? {
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
    let stop_words: HashSet<&str> = [
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
    ]
    .into_iter()
    .collect();

    let mut symbols: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // First, handle qualified paths like crate::types::Node
    // Split on whitespace, find tokens containing ::
    let tokens: Vec<&str> = query.split_whitespace().collect();

    for token in &tokens {
        // Strip leading/trailing punctuation
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != ':');

        if clean.contains("::") {
            // Take the last segment of the qualified path
            if let Some(last) = clean.rsplit("::").next() {
                if !last.is_empty()
                    && !stop_words.contains(last.to_lowercase().as_str())
                    && seen.insert(last.to_string())
                {
                    symbols.push(last.to_string());
                }
            }
            // Also add the full path if it looks like a symbol
            let full = clean.to_string();
            if seen.insert(full.clone()) {
                symbols.push(full);
            }
            continue;
        }

        // Clean token of surrounding punctuation
        let word = clean;
        if word.is_empty() {
            continue;
        }

        // Check for snake_case or SCREAMING_SNAKE (contains underscore)
        if word.contains('_') {
            if !stop_words.contains(word.to_lowercase().as_str()) && seen.insert(word.to_string()) {
                symbols.push(word.to_string());
            }
            continue;
        }

        // Check for CamelCase: at least one uppercase letter after position 0
        if is_camel_case(word) {
            if !stop_words.contains(word.to_lowercase().as_str()) && seen.insert(word.to_string()) {
                symbols.push(word.to_string());
            }
            continue;
        }

        // Skip plain lowercase words that are stop words
        // (non-stop single words that are all lowercase are not typically symbols)
    }

    symbols
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

#[cfg(test)]
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
}
