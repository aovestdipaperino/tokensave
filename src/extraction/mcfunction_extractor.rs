// Rust guideline compliant 2025-10-17
//! Minecraft Java Edition datapack `.mcfunction` extractor (#262).
//!
//! `.mcfunction` files are line-oriented command lists: one command per
//! line, `#` comments, no block structure. The *file itself* is the
//! function — its identity is the datapack resource location
//! `<namespace>:<path>` derived from the
//! `data/<namespace>/function/<path>.mcfunction` layout (the legacy plural
//! `functions` directory used before Minecraft 1.21 is accepted too).
//! Because there is no nesting and no sub-file symbol structure, a
//! hand-rolled line scanner is used instead of a tree-sitter grammar.
//!
//! Emitted per file:
//!   * a `File` root node;
//!   * one `Function` node named `<namespace>:<path>` spanning the file,
//!     with any leading `#` comment block as its docstring;
//!   * `Calls` unresolved refs for `function <id>`,
//!     `execute … run function <id>`, `return run function <id>`, and
//!     `schedule function <id> …` commands (macro lines prefixed with `$`
//!     included). Namespace-less targets are normalized to the implicit
//!     `minecraft:` namespace. Static targets resolve into call edges
//!     against other indexed `.mcfunction` files; macro-generated targets
//!     (`foo:$(name)`) and function tags (`#ns:tag`) are preserved
//!     verbatim and simply stay unresolved — the codebase's standard
//!     representation for dynamic references.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and call references from Minecraft datapack
/// `.mcfunction` files using a lightweight line scanner.
pub struct McFunctionExtractor;

impl McFunctionExtractor {
    /// Extract nodes, edges, and unresolved refs from an `.mcfunction` file.
    pub fn extract_mcfunction(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut unresolved_refs = Vec::new();
        let end_line = source.lines().count().saturating_sub(1) as u32;

        let file_node_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        nodes.push(Self::make_node(
            file_node_id.clone(),
            NodeKind::File,
            file_path.to_string(),
            file_path,
            end_line,
            None,
            None,
            timestamp,
        ));

        let name = Self::resource_location(file_path);
        let fn_node_id = generate_node_id(file_path, &NodeKind::Function, &name, 0);
        nodes.push(Self::make_node(
            fn_node_id.clone(),
            NodeKind::Function,
            name.clone(),
            file_path,
            end_line,
            Some(format!("function {name}")),
            Self::leading_comment_docstring(source),
            timestamp,
        ));
        edges.push(Edge {
            source: file_node_id,
            target: fn_node_id.clone(),
            kind: EdgeKind::Contains,
            line: Some(0),
        });

        for (line_no, line) in source.lines().enumerate() {
            for (target, column) in Self::call_targets(line) {
                unresolved_refs.push(UnresolvedRef {
                    from_node_id: fn_node_id.clone(),
                    reference_name: target,
                    reference_kind: EdgeKind::Calls,
                    line: line_no as u32,
                    column,
                    file_path: file_path.to_string(),
                });
            }
        }

        ExtractionResult {
            nodes,
            edges,
            unresolved_refs,
            errors: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Derive the datapack resource location for a function file.
    ///
    /// `data/<namespace>/function/<path>.mcfunction` (or the legacy plural
    /// `functions` directory) anywhere in the path maps to
    /// `<namespace>:<path>`; nested directories under the function root are
    /// kept as `/`-separated path segments. Files outside a recognizable
    /// datapack layout fall back to their file stem.
    fn resource_location(file_path: &str) -> String {
        let segments: Vec<&str> = file_path.split('/').collect();
        for i in 0..segments.len() {
            if segments[i] == "data"
                && segments.len() > i + 3
                && matches!(segments[i + 2], "function" | "functions")
            {
                let namespace = segments[i + 1];
                let mut rest = segments[i + 3..].join("/");
                if let Some(stripped) = rest.strip_suffix(".mcfunction") {
                    rest = stripped.to_string();
                }
                return format!("{namespace}:{rest}");
            }
        }
        // Not inside a `data/<ns>/function[s]/` tree: use the bare stem.
        let stem = segments.last().copied().unwrap_or(file_path);
        stem.strip_suffix(".mcfunction").unwrap_or(stem).to_string()
    }

    /// Collect the leading block of `#` comment lines as the docstring.
    fn leading_comment_docstring(source: &str) -> Option<String> {
        let doc: Vec<&str> = source
            .lines()
            .take_while(|l| l.trim_start().starts_with('#'))
            .map(|l| l.trim_start().trim_start_matches('#').trim())
            .collect();
        if doc.is_empty() {
            None
        } else {
            Some(doc.join("\n"))
        }
    }

    /// Extract `(target, column)` pairs for every function invocation on a
    /// command line.
    ///
    /// Recognized forms:
    ///   * `function <id> …`
    ///   * `execute … run function <id>` / `return run function <id>`
    ///   * `schedule function <id> <time> …`
    ///
    /// A leading `$` (macro line) on the first token is ignored for command
    /// matching. Comment lines (`#`) and blank lines yield nothing.
    fn call_targets(line: &str) -> Vec<(String, u32)> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Vec::new();
        }

        let tokens = Self::tokenize(line);
        let mut targets = Vec::new();
        for j in 0..tokens.len() {
            let (_, raw) = tokens[j];
            let word = if j == 0 {
                raw.strip_prefix('$').unwrap_or(raw)
            } else {
                raw
            };
            if word != "function" {
                continue;
            }
            let context_ok = j == 0 || {
                let (_, prev_raw) = tokens[j - 1];
                let prev = if j == 1 {
                    prev_raw.strip_prefix('$').unwrap_or(prev_raw)
                } else {
                    prev_raw
                };
                matches!(prev, "run" | "schedule")
            };
            if !context_ok {
                continue;
            }
            if let Some(&(col, target)) = tokens.get(j + 1) {
                targets.push((Self::normalize_target(target), col as u32));
            }
        }
        targets
    }

    /// Split a line into whitespace-separated tokens with byte offsets.
    fn tokenize(line: &str) -> Vec<(usize, &str)> {
        let bytes = line.as_bytes();
        let mut tokens = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            tokens.push((start, &line[start..i]));
        }
        tokens
    }

    /// Normalize a call target to a full resource location.
    ///
    /// Function tags (`#ns:tag`) and macro-generated targets (`foo:$(name)`)
    /// are kept verbatim so they remain unresolved/dynamic references.
    /// Targets without an explicit namespace get the implicit `minecraft:`.
    fn normalize_target(raw: &str) -> String {
        if raw.starts_with('#') || raw.contains("$(") {
            return raw.to_string();
        }
        if raw.contains(':') {
            raw.to_string()
        } else {
            format!("minecraft:{raw}")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_node(
        id: String,
        kind: NodeKind,
        name: String,
        file_path: &str,
        end_line: u32,
        signature: Option<String>,
        docstring: Option<String>,
        timestamp: u64,
    ) -> Node {
        Node {
            id,
            kind,
            name: name.clone(),
            qualified_name: name,
            file_path: file_path.to_string(),
            start_line: 0,
            attrs_start_line: 0,
            end_line,
            start_column: 0,
            end_column: 0,
            signature,
            docstring,
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
            updated_at: timestamp,
            parent_id: None,
        }
    }
}

impl crate::extraction::LanguageExtractor for McFunctionExtractor {
    fn extensions(&self) -> &[&str] {
        &["mcfunction"]
    }

    fn language_name(&self) -> &'static str {
        "MCFunction"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_mcfunction(file_path, source)
    }
}
