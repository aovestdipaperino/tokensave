/// Tree-sitter based COBOL source code extractor.
///
/// Parses COBOL source files and emits nodes and edges for the code graph.
/// COBOL programs have a fixed structure: IDENTIFICATION, ENVIRONMENT, DATA,
/// and PROCEDURE divisions. Paragraphs in the PROCEDURE DIVISION act as
/// subroutines, and PERFORM statements are the primary call mechanism.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from COBOL source files using tree-sitter.
pub struct CobolExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    /// Stack of (name, node_id) for building qualified names and parent edges.
    node_stack: Vec<(String, String)>,
    file_path: String,
    source: Vec<u8>,
    timestamp: u64,
}

impl ExtractionState {
    fn new(file_path: &str, source: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            node_stack: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            timestamp,
        }
    }

    /// Returns the current qualified name prefix from the node stack.
    fn qualified_prefix(&self) -> String {
        let mut parts = vec![self.file_path.clone()];
        for (name, _) in &self.node_stack {
            parts.push(name.clone());
        }
        parts.join("::")
    }

    /// Returns the current parent node ID, or None if at file root level.
    fn parent_node_id(&self) -> Option<&str> {
        self.node_stack.last().map(|(_, id)| id.as_str())
    }

    /// Gets the text of a tree-sitter node from the source.
    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }

    /// Extracts the full source line at a given byte offset.
    ///
    /// COBOL comment nodes in tree-sitter-cobol have zero-width byte ranges,
    /// so we must extract the entire line to recover the comment text.
    fn full_line_at(&self, byte_offset: usize) -> String {
        let line_start = self.source[..byte_offset]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let line_end = self.source[byte_offset..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| byte_offset + p)
            .unwrap_or(self.source.len());
        String::from_utf8_lossy(&self.source[line_start..line_end]).to_string()
    }
}

impl CobolExtractor {
    /// Extract code graph nodes and edges from a COBOL source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the COBOL source code to parse.
    pub fn extract_cobol(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return Self::build_result(state, start);
            }
        };

        // Create the File root node.
        let file_node = Node {
            id: generate_node_id(file_path, &NodeKind::File, file_path, 0),
            kind: NodeKind::File,
            name: file_path.to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            start_line: 0,
            end_line: source.lines().count().saturating_sub(1) as u32,
            start_column: 0,
            end_column: 0,
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
            updated_at: state.timestamp,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state.node_stack.push((file_path.to_string(), file_node_id));

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("cobol");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load COBOL grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit all children of a node.
    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::visit_node(state, child);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit a single AST node, dispatching on its type.
    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        if node.kind() == "program_definition" {
            Self::visit_program_definition(state, node);
        }
    }

    /// Visit a program_definition node, which contains all four divisions.
    fn visit_program_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "identification_division" => Self::visit_identification_division(state, child),
                    "data_division" => Self::visit_data_division(state, child),
                    "procedure_division" => Self::visit_procedure_division(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the PROGRAM-ID as a Module node.
    fn visit_identification_division(state: &mut ExtractionState, node: TsNode<'_>) {
        let program_name_node = Self::find_child_by_kind(node, "program_name");
        if let Some(pn) = program_name_node {
            let name = state.node_text(pn);
            let start_line = node.start_position().row as u32;
            let end_line = node.end_position().row as u32;
            let start_column = node.start_position().column as u32;
            let end_column = node.end_position().column as u32;
            let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
            let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

            let text = state.node_text(node);
            let signature = text
                .lines()
                .find(|l| l.to_uppercase().contains("PROGRAM-ID"))
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty());

            let graph_node = Node {
                id: id.clone(),
                kind: NodeKind::Module,
                name: name.clone(),
                qualified_name,
                file_path: state.file_path.clone(),
                start_line,
                end_line,
                start_column,
                end_column,
                signature,
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
                updated_at: state.timestamp,
            };
            state.nodes.push(graph_node);

            // Contains edge from file.
            if let Some(parent_id) = state.parent_node_id() {
                state.edges.push(Edge {
                    source: parent_id.to_string(),
                    target: id.clone(),
                    kind: EdgeKind::Contains,
                    line: Some(start_line),
                });
            }

            // Push module onto stack for qualified names.
            state.node_stack.push((name, id));
        }
    }

    /// Visit the DATA DIVISION and extract data items.
    fn visit_data_division(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "working_storage_section" {
                    Self::visit_working_storage(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit WORKING-STORAGE SECTION and extract 01-level data items.
    fn visit_working_storage(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            // Track preceding comments for docstrings.
            let mut pending_comment: Option<String> = None;
            loop {
                let child = cursor.node();
                match child.kind() {
                    "comment" => {
                        let line = state.full_line_at(child.start_byte());
                        let trimmed = line.trim();
                        // Strip the leading "* " from COBOL comments
                        let comment_text = if let Some(rest) = trimmed.strip_prefix('*') {
                            rest.trim().to_string()
                        } else {
                            trimmed.to_string()
                        };
                        pending_comment = Some(comment_text);
                    }
                    "data_description" => {
                        Self::visit_data_description(state, child, pending_comment.take());
                    }
                    _ => {
                        pending_comment = None;
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single data description (01 level) as a Field node.
    ///
    /// Data items with a VALUE clause are emitted as Const nodes; others as Field.
    fn visit_data_description(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        docstring: Option<String>,
    ) {
        // Only extract 01-level items.
        let level_node = Self::find_child_by_kind(node, "level_number");
        if let Some(ln) = level_node {
            let level_text = state.node_text(ln);
            if level_text.trim() != "01" {
                return;
            }
        } else {
            return;
        }

        let name_node = Self::find_child_by_kind(node, "entry_name");
        let name = if let Some(n) = name_node {
            state.node_text(n)
        } else {
            return;
        };

        let has_value = Self::find_child_by_kind(node, "value_clause").is_some();
        let kind = if has_value {
            NodeKind::Const
        } else {
            NodeKind::Field
        };

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (module or file).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Visit the PROCEDURE DIVISION and extract paragraphs as Functions.
    ///
    /// In COBOL, the PROCEDURE DIVISION is a flat sequence of paragraph headers
    /// followed by statements. We group statements between paragraph headers
    /// into logical "function" bodies.
    fn visit_procedure_division(state: &mut ExtractionState, node: TsNode<'_>) {
        // Collect all children for multi-pass grouping.
        let child_count = node.child_count();
        let mut children: Vec<TsNode<'_>> = Vec::new();
        let mut i: usize = 0;
        while i < child_count {
            if let Some(child) = node.child(i as u32) {
                children.push(child);
            }
            i += 1;
        }

        // Group: find paragraph_header nodes and associate them with their
        // following statements (up to the next paragraph_header or end).
        let mut idx = 0;
        while idx < children.len() {
            let child = children[idx];
            if child.kind() == "paragraph_header" {
                // Gather the docstring from preceding comment(s).
                let docstring = Self::gather_preceding_comments(state, &children, idx);

                // Determine the range of statements belonging to this paragraph.
                let para_start = idx;
                let mut para_end = idx + 1;
                while para_end < children.len()
                    && children[para_end].kind() != "paragraph_header"
                    && children[para_end].kind() != "comment"
                    || (para_end < children.len()
                        && children[para_end].kind() == "comment"
                        && para_end + 1 < children.len()
                        && children[para_end + 1].kind() != "paragraph_header")
                {
                    para_end += 1;
                }
                // Check if the next item after comments is a paragraph_header.
                // If so, the comments belong to the next paragraph, not this one.
                if para_end < children.len() && children[para_end].kind() == "comment" {
                    // Don't include trailing comments; they belong to the next paragraph.
                } else {
                    // Include up to para_end.
                }

                Self::visit_paragraph(state, &children, para_start, para_end, docstring);
                idx = para_end;
            } else {
                idx += 1;
            }
        }
    }

    /// Gather preceding comment lines before a paragraph_header.
    fn gather_preceding_comments(
        state: &ExtractionState,
        children: &[TsNode<'_>],
        header_idx: usize,
    ) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut i = header_idx;
        while i > 0 {
            i -= 1;
            if children[i].kind() == "comment" {
                let line = state.full_line_at(children[i].start_byte());
                let trimmed = line.trim();
                let comment_text = if let Some(rest) = trimmed.strip_prefix('*') {
                    rest.trim().to_string()
                } else {
                    trimmed.to_string()
                };
                comments.push(comment_text);
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Extract a paragraph (header + body statements) as a Function node.
    fn visit_paragraph(
        state: &mut ExtractionState,
        children: &[TsNode<'_>],
        start_idx: usize,
        end_idx: usize,
        docstring: Option<String>,
    ) {
        let header = children[start_idx];
        let header_text = state.node_text(header);
        // Strip trailing period from paragraph name: "MAIN-PROGRAM." -> "MAIN-PROGRAM"
        let name = header_text.trim().trim_end_matches('.').to_string();

        let start_line = header.start_position().row as u32;
        // End line is the last statement in this paragraph.
        let last_child = if end_idx > start_idx + 1 {
            children[end_idx - 1]
        } else {
            header
        };
        let end_line = last_child.end_position().row as u32;
        let start_column = header.start_position().column as u32;
        let end_column = last_child.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        // Count complexity by walking the body statements.
        let mut branches: u32 = 0;
        let mut loops: u32 = 0;
        let mut returns: u32 = 0;
        for child in &children[(start_idx + 1)..end_idx] {
            match child.kind() {
                "if_header" => branches += 1,
                "perform_statement_loop" => loops += 1,
                "stop_statement" | "goback_statement" => returns += 1,
                _ => {}
            }
        }

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(header_text.trim().to_string()),
            docstring,
            visibility: Visibility::Pub,
            is_async: false,
            branches,
            loops,
            returns,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from body statements.
        for child in &children[(start_idx + 1)..end_idx] {
            Self::extract_call_sites_from_node(state, *child, &id);
        }
    }

    /// Recursively extract PERFORM and CALL references from a node.
    fn extract_call_sites_from_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        match node.kind() {
            "perform_statement_call_proc" => {
                // PERFORM paragraph-name: field "procedure" -> perform_procedure -> label -> qualified_word -> WORD
                if let Some(proc_node) = node.child_by_field_name("procedure") {
                    let callee = Self::extract_label_name(state, proc_node);
                    if let Some(name) = callee {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: name,
                            reference_kind: EdgeKind::Calls,
                            line: node.start_position().row as u32,
                            column: node.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                }
            }
            "perform_statement_loop" => {
                // A PERFORM VARYING ... END-PERFORM block may contain nested statements.
                // Recurse into children to find nested PERFORM calls.
                Self::recurse_call_sites(state, node, fn_node_id);
            }
            "call_statement" => {
                // CALL "program-name": extract the string literal.
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "string" {
                            let text = state.node_text(child);
                            let name = text.trim_matches('"').trim_matches('\'').to_string();
                            if !name.is_empty() {
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: fn_node_id.to_string(),
                                    reference_name: name,
                                    reference_kind: EdgeKind::Calls,
                                    line: node.start_position().row as u32,
                                    column: node.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                            }
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
            _ => {
                // Recurse into other statement types for nested calls.
                Self::recurse_call_sites(state, node, fn_node_id);
            }
        }
    }

    /// Recurse into children of a node to find call sites.
    fn recurse_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::extract_call_sites_from_node(state, cursor.node(), fn_node_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the label name from a perform_procedure node.
    fn extract_label_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // perform_procedure -> label -> qualified_word -> WORD
        let label = Self::find_child_by_kind(node, "label")?;
        let qw = Self::find_child_by_kind(label, "qualified_word")?;
        let word = Self::find_child_by_kind(qw, "WORD")?;
        Some(state.node_text(word))
    }

    /// Find the first child of a node with a given kind.
    fn find_child_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == kind {
                    return Some(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Build the final ExtractionResult from the accumulated state.
    fn build_result(state: ExtractionState, start: Instant) -> ExtractionResult {
        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: state.unresolved_refs,
            errors: state.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl crate::extraction::LanguageExtractor for CobolExtractor {
    fn extensions(&self) -> &[&str] {
        &["cob", "cbl", "cpy"]
    }

    fn language_name(&self) -> &str {
        "COBOL"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_cobol(file_path, source)
    }
}
