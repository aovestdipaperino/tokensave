use std::collections::HashMap;

use crate::types::TaskContext;

/// Formats a `TaskContext` as a Markdown document suitable for LLM consumption.
///
/// The output includes sections for the query, entry points, related symbols
/// grouped by file, and extracted code blocks.
pub fn format_context_as_markdown(context: &TaskContext) -> String {
    let mut out = String::new();

    out.push_str("## Code Context\n");
    out.push_str(&format!("**Query:** {}\n\n", context.query));

    // Entry Points
    out.push_str("### Entry Points\n");
    if context.entry_points.is_empty() {
        out.push_str("_No entry points found._\n\n");
    } else {
        for node in &context.entry_points {
            out.push_str(&format!(
                "- **{}** ({}) - {}:{}\n",
                node.name,
                node.kind.as_str(),
                node.file_path,
                node.start_line,
            ));
            if let Some(ref sig) = node.signature {
                out.push_str(&format!("  `{}`\n", sig));
            }
        }
        out.push('\n');
    }

    // Related Symbols grouped by file
    out.push_str("### Related Symbols\n");
    if context.subgraph.nodes.is_empty() {
        out.push_str("_No related symbols._\n\n");
    } else {
        // Group nodes by file_path
        let mut by_file: HashMap<&str, Vec<(&str, u32)>> = HashMap::new();
        for node in &context.subgraph.nodes {
            by_file
                .entry(&node.file_path)
                .or_default()
                .push((&node.name, node.start_line));
        }

        let mut files: Vec<&&str> = by_file.keys().collect();
        files.sort();

        for file in files {
            let symbols = by_file.get(*file).unwrap_or(&Vec::new()).clone();
            let formatted: Vec<String> = symbols
                .iter()
                .map(|(name, line)| format!("{}:{}", name, line))
                .collect();
            out.push_str(&format!("- {}: {}\n", file, formatted.join(", ")));
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
                    .map(|n| n.name.clone())
                    .unwrap_or_else(|| node_id.clone())
            } else {
                "unknown".to_string()
            };

            out.push_str(&format!(
                "#### {} ({}:{})\n",
                label, block.file_path, block.start_line,
            ));
            out.push_str("```rust\n");
            out.push_str(&block.content);
            if !block.content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
    }

    out
}

/// Formats a `TaskContext` as pretty-printed JSON.
pub fn format_context_as_json(context: &TaskContext) -> String {
    serde_json::to_string_pretty(context).unwrap_or_default()
}

#[cfg(test)]
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
                end_line: 20,
                start_column: 0,
                end_column: 1,
                signature: Some("pub fn process_data(input: &str) -> Result<()>".to_string()),
                docstring: None,
                visibility: Visibility::Pub,
                is_async: false,
                updated_at: 0,
            }],
            code_blocks: vec![],
            related_files: vec!["src/lib.rs".to_string()],
        };

        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("**process_data**"));
        assert!(md.contains("(function)"));
        assert!(md.contains("src/lib.rs:10"));
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
                end_line: 3,
                start_column: 0,
                end_column: 1,
                signature: None,
                docstring: None,
                visibility: Visibility::Pub,
                is_async: false,
                updated_at: 0,
            }],
            code_blocks: vec![CodeBlock {
                content: "fn my_fn() {\n    println!(\"hello\");\n}".to_string(),
                file_path: "src/main.rs".to_string(),
                start_line: 1,
                end_line: 3,
                node_id: Some("function:abc".to_string()),
            }],
            related_files: vec!["src/main.rs".to_string()],
        };

        let md = format_context_as_markdown(&ctx);
        assert!(md.contains("#### my_fn (src/main.rs:1)"));
        assert!(md.contains("```rust"));
        assert!(md.contains("fn my_fn()"));
    }
}
