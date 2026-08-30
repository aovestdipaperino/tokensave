use std::collections::HashMap;
use std::fmt::Write as _;

use crate::context::ranking::is_test_path;
use crate::types::TaskContext;

/// Longest signature rendered inline for an entry point, in bytes.
pub const MAX_SIGNATURE_LEN: usize = 200;

/// Renders a signature as one bounded line.
///
/// A signature is normally a one-liner, but for a value binding the extractor
/// stores the whole initializer — a localization catalog or a generated lookup
/// table can run to tens of kilobytes. Emitting that verbatim costs more tokens
/// than the file it came from, which is a poor trade in a tool whose purpose is
/// spending fewer of them. The first line identifies the symbol; the code block
/// below it carries content under its own limits.
pub fn compact_signature(sig: &str) -> String {
    let first = sig.lines().next().unwrap_or("").trim_end();
    let truncated_lines = sig.lines().nth(1).is_some();
    match first.char_indices().nth(MAX_SIGNATURE_LEN) {
        Some((idx, _)) => format!("{} …", first[..idx].trim_end()),
        None if truncated_lines => format!("{first} …"),
        None => first.to_string(),
    }
}

/// Markdown fence language for a source file path, derived from its extension.
/// Unknown extensions produce an unlabeled fence.
fn fence_language(file_path: &str) -> &'static str {
    let ext = file_path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "rb" => "ruby",
        "php" => "php",
        "cs" => "csharp",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "swift" => "swift",
        "dart" => "dart",
        "scala" => "scala",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "html" => "html",
        "css" => "css",
        "lua" => "lua",
        "ex" | "exs" => "elixir",
        "hs" => "haskell",
        "zig" => "zig",
        "vue" => "vue",
        "svelte" => "svelte",
        _ => "",
    }
}

/// Formats a `TaskContext` as a Markdown document suitable for LLM consumption.
///
/// The output includes sections for the query, entry points, related symbols
/// grouped by file, and extracted code blocks.
pub fn format_context_as_markdown(context: &TaskContext) -> String {
    debug_assert!(
        !context.query.is_empty(),
        "format_context_as_markdown called with empty query"
    );
    debug_assert!(
        !context.summary.is_empty(),
        "format_context_as_markdown called with empty summary"
    );
    let mut out = String::new();

    out.push_str("## Code Context\n");
    let _ = write!(out, "**Query:** {}\n\n", context.query);

    // Entry Points
    out.push_str("### Entry Points\n");
    if context.entry_points.is_empty() {
        out.push_str("_No entry points found._\n\n");
    } else {
        for node in &context.entry_points {
            let _ = writeln!(
                out,
                "- **{}** ({}) - {}:{}",
                node.name,
                node.kind.as_str(),
                node.file_path,
                node.start_line + 1,
            );
            if let Some(ref sig) = node.signature {
                let _ = writeln!(out, "  `{}`", compact_signature(sig));
            }
            // A docstring's first line often answers the question without a
            // code fetch — cheap to include, expensive to omit.
            if let Some(first) = node
                .docstring
                .as_deref()
                .and_then(|doc| doc.lines().find(|line| !line.trim().is_empty()))
            {
                let _ = writeln!(out, "  {}", first.trim());
            }
        }
        out.push('\n');
    }

    // Related Symbols grouped by file. Test/fixture symbols are collapsed to
    // a one-line count: they dominate BFS expansion by volume but rarely
    // answer a navigation query, so listing each name is noise.
    out.push_str("### Related Symbols\n");
    if context.subgraph.nodes.is_empty() {
        out.push_str("_No related symbols._\n\n");
    } else {
        let mut by_file: HashMap<&str, Vec<(&str, u32)>> = HashMap::new();
        let mut test_symbols = 0usize;
        let mut test_files: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for node in &context.subgraph.nodes {
            if is_test_path(&node.file_path) {
                test_symbols += 1;
                test_files.insert(&node.file_path);
                continue;
            }
            by_file
                .entry(&node.file_path)
                .or_default()
                .push((&node.name, node.start_line + 1));
        }

        let mut files: Vec<&&str> = by_file.keys().collect();
        files.sort();

        for file in files {
            let symbols = by_file.get(*file).unwrap_or(&Vec::new()).clone();
            let formatted: Vec<String> = symbols
                .iter()
                .map(|(name, line)| format!("{name}:{line}"))
                .collect();
            let _ = writeln!(out, "- {}: {}", file, formatted.join(", "));
        }
        if test_symbols > 0 {
            let _ = writeln!(
                out,
                "- test/fixture files: {} symbols across {} files (tokensave_callers on an entry point for details)",
                test_symbols,
                test_files.len()
            );
        }
        out.push('\n');
    }

    // Code blocks
    out.push_str("### Code\n");
    if context.code_blocks.is_empty() {
        out.push_str("_No code blocks extracted._\n");
    } else {
        for block in &context.code_blocks {
            // Determine a label from the node if available
            let label = if let Some(ref node_id) = block.node_id {
                // Try to find a matching entry point name
                context
                    .entry_points
                    .iter()
                    .find(|n| &n.id == node_id)
                    .map_or_else(|| node_id.clone(), |n| n.name.clone())
            } else {
                "unknown".to_string()
            };

            let _ = writeln!(
                out,
                "#### {} ({}:{})",
                label,
                block.file_path,
                block.start_line + 1,
            );
            // Fence language from the block's file extension, not a hardcoded
            // `rust` (#208).
            let _ = writeln!(out, "```{}", fence_language(&block.file_path));
            out.push_str(&block.content);
            if !block.content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
    }

    // Retrieval diagnostics: tells the caller whether to trust this result
    // or reformulate. Zero-hit terms are the reformulation signal; the match
    // tier separates exact/strong hits from lexical-only straws. This is
    // the last formatter section; the context handler's truncation
    // preserves it as a suffix, so a large Code section cannot push it
    // over the response limit. It also fires whenever no entry points
    // were found, even with no term data:
    // a task made of only short or stop-word tokens ("fix bug") extracts no
    // searchable terms at all, and that miss must not stay silent.
    let diag = &context.diagnostics;
    if !diag.term_hits.is_empty() || diag.best_score.is_some() || context.entry_points.is_empty() {
        out.push_str("### Retrieval\n");
        match (diag.match_quality.as_deref(), diag.best_score) {
            (Some(quality), Some(score)) => {
                let score_text = if score < 0.01 {
                    "<0.01".to_string()
                } else {
                    format!("{score:.2}")
                };
                let _ = writeln!(out, "- match: {quality} (best score {score_text})");
            }
            _ if diag.term_hits.is_empty() => {
                out.push_str(
                    "- match: none (no searchable terms extracted — \
                     rephrase with concrete identifiers or add `keywords`)\n",
                );
            }
            _ => {
                out.push_str("- match: none (no candidates)\n");
            }
        }
        if !diag.term_hits.is_empty() {
            // Hit terms listed with counts; zero-hit terms (mostly synthesized
            // bigram/stem variants) collapsed to a capped list so the footer
            // stays token-lean.
            const MAX_MISSES_SHOWN: usize = 6;
            let hits: Vec<String> = diag
                .term_hits
                .iter()
                .filter(|(_, count)| *count > 0)
                .map(|(term, count)| format!("{term}({count})"))
                .collect();
            if !hits.is_empty() {
                let _ = writeln!(out, "- terms: {}", hits.join(", "));
            }
            let misses: Vec<&str> = diag
                .term_hits
                .iter()
                .filter(|(_, count)| *count == 0)
                .map(|(term, _)| term.as_str())
                .collect();
            if !misses.is_empty() {
                let shown = misses.len().min(MAX_MISSES_SHOWN);
                let overflow = misses.len() - shown;
                let mut listed = misses[..shown].join(", ");
                if overflow > 0 {
                    let _ = write!(listed, ", +{overflow} more");
                }
                let _ = writeln!(
                    out,
                    "- ⚠ no hits ({}): {} — add `keywords` synonyms or try tokensave_search",
                    misses.len(),
                    listed
                );
            }
        }
        out.push('\n');
    }

    debug_assert!(
        !out.is_empty(),
        "format_context_as_markdown produced empty output"
    );
    debug_assert!(
        out.contains("## Code Context"),
        "output missing required header"
    );
    out
}

/// Formats a `TaskContext` as pretty-printed JSON.
pub fn format_context_as_json(context: &TaskContext) -> String {
    serde_json::to_string_pretty(context).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_test_context() -> TaskContext {
        TaskContext {
            query: "test query".to_string(),
            summary: "Test summary".to_string(),
            subgraph: Subgraph::default(),
            entry_points: vec![],
            code_blocks: vec![],
            related_files: vec![],
            seen_node_ids: vec![],
            diagnostics: RetrievalDiagnostics::default(),
        }
    }

    #[test]
    fn test_markdown_contains_header() {
        let ctx = make_test_context();
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("## Code Context"));
        assert!(md.contains("test query"));
    }

    #[test]
    fn test_json_roundtrip() {
        let ctx = make_test_context();
        let json = format_context_as_json(&ctx);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["query"], "test query");
    }

    #[test]
    fn test_markdown_with_entry_points() {
        let ctx = TaskContext {
            query: "process".to_string(),
            summary: "Found 1 entry point".to_string(),
            subgraph: Subgraph::default(),
            entry_points: vec![Node {
                id: "function:abc123".to_string(),
                kind: NodeKind::Function,
                name: "process_data".to_string(),
                qualified_name: "src/lib.rs::process_data".to_string(),
                file_path: "src/lib.rs".to_string(),
                start_line: 10,
                attrs_start_line: 10,
                end_line: 20,
                start_column: 0,
                end_column: 1,
                signature: Some("pub fn process_data(input: &str) -> Result<()>".to_string()),
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
            }],
            code_blocks: vec![],
            related_files: vec!["src/lib.rs".to_string()],
            seen_node_ids: vec![],
            diagnostics: RetrievalDiagnostics::default(),
        };

        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("**process_data**"));
        assert!(md.contains("(function)"));
        assert!(md.contains("src/lib.rs:11"));
        assert!(md.contains("`pub fn process_data(input: &str) -> Result<()>`"));
    }

    #[test]
    fn test_markdown_with_code_blocks() {
        let ctx = TaskContext {
            query: "test".to_string(),
            summary: "Summary".to_string(),
            subgraph: Subgraph::default(),
            entry_points: vec![Node {
                id: "function:abc".to_string(),
                kind: NodeKind::Function,
                name: "my_fn".to_string(),
                qualified_name: "my_fn".to_string(),
                file_path: "src/main.rs".to_string(),
                start_line: 1,
                attrs_start_line: 1,
                end_line: 3,
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
            }],
            code_blocks: vec![CodeBlock {
                content: "fn my_fn() {\n    println!(\"hello\");\n}".to_string(),
                file_path: "src/main.rs".to_string(),
                start_line: 1,
                end_line: 3,
                node_id: Some("function:abc".to_string()),
            }],
            related_files: vec!["src/main.rs".to_string()],
            seen_node_ids: vec![],
            diagnostics: RetrievalDiagnostics::default(),
        };

        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("#### my_fn (src/main.rs:2)"));
        assert!(md.contains("```rust"));
        assert!(md.contains("fn my_fn()"));
    }

    fn make_node(name: &str, file_path: &str) -> Node {
        Node {
            id: format!("function:{name}"),
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
        }
    }

    #[test]
    fn test_retrieval_footer_reports_term_hits_and_zero_hit_warning() {
        let mut ctx = make_test_context();
        ctx.diagnostics = RetrievalDiagnostics {
            term_hits: vec![
                ("context".to_string(), 3),
                ("ranking".to_string(), 0),
                ("budget".to_string(), 0),
            ],
            best_score: Some(21.5),
            match_quality: Some("exact".to_string()),
        };
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("### Retrieval"), "{md}");
        assert!(md.contains("- match: exact (best score 21.50)"), "{md}");
        assert!(md.contains("- terms: context(3)"), "{md}");
        assert!(md.contains("no hits (2): ranking, budget"), "{md}");
    }

    #[test]
    fn test_retrieval_footer_caps_zero_hit_term_list() {
        let mut ctx = make_test_context();
        ctx.diagnostics = RetrievalDiagnostics {
            term_hits: (0..10).map(|i| (format!("miss{i}"), 0)).collect(),
            best_score: None,
            match_quality: None,
        };
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("no hits (10):"), "{md}");
        assert!(md.contains("+4 more"), "{md}");
        assert!(!md.contains("miss7"), "{md}");
    }

    #[test]
    fn test_retrieval_footer_tiny_score_shown_as_below_threshold() {
        let mut ctx = make_test_context();
        ctx.diagnostics = RetrievalDiagnostics {
            term_hits: vec![("foo".to_string(), 1)],
            best_score: Some(1e-6),
            match_quality: Some("fts-only".to_string()),
        };
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("- match: fts-only (best score <0.01)"), "{md}");
        assert!(!md.contains("no hits"), "{md}");
    }

    #[test]
    fn test_retrieval_footer_absent_without_diagnostics() {
        // With entry points present and no diagnostics data there is nothing
        // to report, so the footer stays out of the output.
        let mut ctx = make_test_context();
        ctx.entry_points = vec![make_node("found_fn", "src/lib.rs")];
        let md = format_context_as_markdown(&ctx);
        assert!(!md.contains("### Retrieval"), "{md}");
    }

    #[test]
    fn test_retrieval_footer_present_when_no_entry_points_and_no_terms() {
        // A task of only short or stop-word tokens extracts no searchable
        // terms; the resulting miss must still be visible in the footer.
        let ctx = make_test_context();
        assert!(ctx.entry_points.is_empty());
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("### Retrieval"), "{md}");
        assert!(md.contains("no searchable terms extracted"), "{md}");
    }

    #[test]
    fn test_related_symbols_collapse_test_files() {
        let mut ctx = make_test_context();
        ctx.subgraph = Subgraph {
            nodes: vec![
                make_node("real_fn", "src/lib.rs"),
                make_node("test_one", "tests/a_test.rs"),
                make_node("test_two", "tests/b_test.rs"),
            ],
            edges: vec![],
            roots: vec![],
        };
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("- src/lib.rs: real_fn:2"), "{md}");
        assert!(!md.contains("test_one"), "{md}");
        assert!(
            md.contains("- test/fixture files: 2 symbols across 2 files"),
            "{md}"
        );
    }

    #[test]
    fn test_entry_point_docstring_first_line_shown() {
        let mut ctx = make_test_context();
        let mut node = make_node("documented", "src/lib.rs");
        node.docstring = Some("Parses the config file.\n\nLong detail here.".to_string());
        ctx.entry_points = vec![node];
        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("  Parses the config file."), "{md}");
        assert!(!md.contains("Long detail here"), "{md}");
    }

    #[test]
    fn test_fence_language_from_extension() {
        // #208
        assert_eq!(super::fence_language("a/b.tsx"), "tsx");
        assert_eq!(super::fence_language("a/b.ts"), "typescript");
        assert_eq!(super::fence_language("a/b.rs"), "rust");
        assert_eq!(super::fence_language("a/b.unknownext"), "");
    }

    #[test]
    fn compact_signature_leaves_a_normal_signature_alone() {
        let sig = "pub fn process_data(input: &str) -> Result<()>";
        assert_eq!(super::compact_signature(sig), sig);
    }

    #[test]
    fn compact_signature_collapses_a_multiline_initializer() {
        // A localization catalog stored as a const "signature" can run to tens
        // of kilobytes; emitting it verbatim costs more than the source file.
        let sig = "MESSAGES = {\n  \"a\": \"one\",\n  \"b\": \"two\",\n}";
        assert_eq!(super::compact_signature(sig), "MESSAGES = { …");
    }

    #[test]
    fn compact_signature_bounds_a_long_single_line() {
        let sig = format!("const TABLE = [{}]", "0, ".repeat(500));
        let out = super::compact_signature(&sig);
        assert!(
            out.len() <= super::MAX_SIGNATURE_LEN + 8,
            "got {} bytes",
            out.len()
        );
        assert!(out.ends_with('…'));
    }

    #[test]
    fn compact_signature_truncates_on_a_char_boundary() {
        // Multi-byte characters must not be split; slicing mid-char panics.
        let sig = format!("const EMOJI = \"{}\"", "🚀".repeat(400));
        let out = super::compact_signature(&sig);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= super::MAX_SIGNATURE_LEN + 2);
    }
}
