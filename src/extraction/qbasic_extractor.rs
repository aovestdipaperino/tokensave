/// Tree-sitter based QBasic source code extractor.
///
/// Parses QBasic source files and emits nodes and edges for the code graph.
/// QBasic has proper structured programming constructs: SUB/END SUB,
/// FUNCTION/END FUNCTION, TYPE/END TYPE, SELECT CASE, DO/LOOP. This
/// extractor maps SUB and FUNCTION definitions to Function nodes, TYPE
/// definitions to Struct nodes with Field children, CONST statements to
/// Const nodes, DIM SHARED to Field nodes, CALL sites to unresolved refs,
/// and apostrophe comments to docstrings.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, QBASIC_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from QBasic source files using tree-sitter.
pub struct QBasicExtractor;

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
}

impl QBasicExtractor {
    /// Extract code graph nodes and edges from a QBasic source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the QBasic source code to parse.
    pub fn extract_qbasic(file_path: &str, source: &str) -> ExtractionResult {
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
        state
            .node_stack
            .push((file_path.to_string(), file_node_id));

        let root = tree.root_node();

        // Walk the top-level children of the program.
        let mut cursor = root.walk();
        // Collect preceding comments for docstrings.
        let mut pending_comment: Option<String> = None;
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                match node.kind() {
                    "line" => {
                        // A line can contain: apostrophe_comment, const_statement,
                        // dim_statement, call_statement, declare_statement, let_statement, etc.
                        if let Some(comment) = Self::extract_line_comment(&state, node) {
                            // Accumulate comments as potential docstrings.
                            pending_comment = Some(comment);
                        } else {
                            Self::visit_line(&mut state, node, &pending_comment);
                            pending_comment = None;
                        }
                    }
                    "type_definition" => {
                        Self::visit_type_definition(&mut state, node, &pending_comment);
                        pending_comment = None;
                    }
                    "sub_definition" => {
                        Self::visit_sub_definition(&mut state, node, &pending_comment);
                        pending_comment = None;
                    }
                    "function_definition" => {
                        Self::visit_function_definition(&mut state, node, &pending_comment);
                        pending_comment = None;
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

        state.node_stack.pop();
        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_qbasic::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load QBasic grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Extract a comment from a line node, if the line is purely a comment.
    /// Returns the comment text (with leading ' stripped), or None if not a comment line.
    fn extract_line_comment(state: &ExtractionState, line: TsNode<'_>) -> Option<String> {
        let stmt_list = Self::find_child_by_kind(line, "statement_list")?;
        let stmt = Self::find_child_by_kind(stmt_list, "statement")?;
        let comment = Self::find_child_by_kind(stmt, "apostrophe_comment")?;
        let text = state.node_text(comment);
        // Strip leading ' and whitespace.
        let stripped = text.trim_start_matches('\'').trim().to_string();
        Some(stripped)
    }

    /// Visit a top-level `line` node, extracting CONST, DIM SHARED, CALL, etc.
    fn visit_line(state: &mut ExtractionState, line: TsNode<'_>, pending_comment: &Option<String>) {
        let stmt_list = match Self::find_child_by_kind(line, "statement_list") {
            Some(sl) => sl,
            None => return,
        };
        let stmt = match Self::find_child_by_kind(stmt_list, "statement") {
            Some(s) => s,
            None => return,
        };

        // Get the first named child to determine the statement kind.
        let mut stmt_cursor = stmt.walk();
        if !stmt_cursor.goto_first_child() {
            return;
        }
        let child = stmt_cursor.node();
        let kind = child.kind();

        match kind {
            "const_statement" => {
                Self::visit_const_statement(state, line, child, pending_comment);
            }
            "dim_statement" => {
                Self::visit_dim_statement(state, line, child, pending_comment);
            }
            "call_statement" => {
                Self::extract_call_from_call_statement(state, child);
            }
            "declare_statement" => {
                // Skip forward declarations.
            }
            "let_statement" => {
                // Top-level assignments — extract calls from within if any.
            }
            _ => {}
        }
    }

    /// Visit a CONST statement and emit a Const node.
    fn visit_const_statement(
        state: &mut ExtractionState,
        line: TsNode<'_>,
        const_stmt: TsNode<'_>,
        pending_comment: &Option<String>,
    ) {
        // Find the identifier child of const_statement.
        let name = match Self::find_child_by_kind(const_stmt, "identifier") {
            Some(id_node) => state.node_text(id_node),
            None => return,
        };

        let start_line = line.start_position().row as u32;
        let end_line = line.end_position().row as u32;
        let start_column = line.start_position().column as u32;
        let end_column = line.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
        let text = state.node_text(line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring: pending_comment.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Visit a DIM SHARED statement and emit a Field node.
    fn visit_dim_statement(
        state: &mut ExtractionState,
        line: TsNode<'_>,
        dim_stmt: TsNode<'_>,
        pending_comment: &Option<String>,
    ) {
        // Check if this is DIM SHARED by looking at the text.
        let text = state.node_text(line);
        if !text.contains("SHARED") {
            return; // Only extract DIM SHARED at top level
        }

        // Find the dim_variable child, then get its identifier.
        let dim_var = match Self::find_child_by_kind(dim_stmt, "dim_variable") {
            Some(dv) => dv,
            None => return,
        };
        let name = match Self::find_child_by_kind(dim_var, "identifier") {
            Some(id_node) => state.node_text(id_node),
            None => return,
        };

        let start_line = line.start_position().row as u32;
        let end_line = line.end_position().row as u32;
        let start_column = line.start_position().column as u32;
        let end_column = line.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
            docstring: pending_comment.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Visit a TYPE...END TYPE definition and emit a Struct node with Field children.
    fn visit_type_definition(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        pending_comment: &Option<String>,
    ) {
        // type_definition has name: (identifier) and type_member children.
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name = state.node_text(name_node);

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let struct_id =
            generate_node_id(&state.file_path, &NodeKind::Struct, &name, start_line);
        let text = state.node_text(node);
        let signature = text.lines().next().unwrap_or("").trim().to_string();

        let graph_node = Node {
            id: struct_id.clone(),
            kind: NodeKind::Struct,
            name: name.clone(),
            qualified_name: qualified_name.clone(),
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature),
            docstring: pending_comment.clone(),
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

        // Contains edge from parent (file).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: struct_id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract type_member children as Field nodes.
        state.node_stack.push((name, struct_id.clone()));
        let mut child_cursor = node.walk();
        if child_cursor.goto_first_child() {
            loop {
                let child = child_cursor.node();
                if child.kind() == "type_member" {
                    Self::visit_type_member(state, child);
                }
                if !child_cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_stack.pop();
    }

    /// Visit a type_member inside a TYPE block and emit a Field node.
    fn visit_type_member(state: &mut ExtractionState, member: TsNode<'_>) {
        let name = match Self::find_child_by_kind(member, "identifier") {
            Some(id_node) => state.node_text(id_node),
            None => return,
        };

        let start_line = member.start_position().row as u32;
        let end_line = member.end_position().row as u32;
        let start_column = member.start_position().column as u32;
        let end_column = member.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);
        let text = state.node_text(member);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Visit a SUB...END SUB definition and emit a Function node.
    fn visit_sub_definition(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        pending_comment: &Option<String>,
    ) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name = state.node_text(name_node);

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let fn_id =
            generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        // Build signature from the first line of text.
        let text = state.node_text(node);
        let signature = text.lines().next().unwrap_or("").trim().to_string();

        // Count complexity using the generic counter.
        let metrics = if node.child_count() > 0 {
            count_complexity(node, &QBASIC_COMPLEXITY, &state.source)
        } else {
            Default::default()
        };

        let graph_node = Node {
            id: fn_id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature),
            docstring: pending_comment.clone(),
            visibility: Visibility::Pub,
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: metrics.unsafe_blocks,
            unchecked_calls: metrics.unchecked_calls,
            assertions: metrics.assertions,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: fn_id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from within the SUB body.
        state.node_stack.push((name, fn_id.clone()));
        Self::walk_for_calls(state, node);
        state.node_stack.pop();
    }

    /// Visit a FUNCTION...END FUNCTION definition and emit a Function node.
    fn visit_function_definition(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        pending_comment: &Option<String>,
    ) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name = state.node_text(name_node);

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let fn_id =
            generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let text = state.node_text(node);
        let signature = text.lines().next().unwrap_or("").trim().to_string();

        let metrics = if node.child_count() > 0 {
            count_complexity(node, &QBASIC_COMPLEXITY, &state.source)
        } else {
            Default::default()
        };

        let graph_node = Node {
            id: fn_id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature),
            docstring: pending_comment.clone(),
            visibility: Visibility::Pub,
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: metrics.unsafe_blocks,
            unchecked_calls: metrics.unchecked_calls,
            assertions: metrics.assertions,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: fn_id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from within the FUNCTION body.
        state.node_stack.push((name, fn_id.clone()));
        Self::walk_for_calls(state, node);
        state.node_stack.pop();
    }

    /// Extract a call reference from a call_statement node.
    fn extract_call_from_call_statement(state: &mut ExtractionState, call_stmt: TsNode<'_>) {
        let target_name = match Self::find_child_by_kind(call_stmt, "identifier") {
            Some(id_node) => state.node_text(id_node),
            None => return,
        };

        let from_node_id = state
            .node_stack
            .last()
            .map(|(_, id)| id.clone())
            .unwrap_or_default();

        state.unresolved_refs.push(UnresolvedRef {
            from_node_id,
            reference_name: target_name,
            reference_kind: EdgeKind::Calls,
            line: call_stmt.start_position().row as u32,
            column: call_stmt.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
    }

    /// Recursively walk AST nodes looking for call_statement and function_call nodes.
    fn walk_for_calls(state: &mut ExtractionState, node: TsNode<'_>) {
        let kind = node.kind();
        if kind == "call_statement" {
            Self::extract_call_from_call_statement(state, node);
        } else if kind == "function_call" {
            // Built-in function calls like STR$() — extract if they have an identifier.
            // We skip built-in functions as they aren't user-defined.
        }

        // Recurse into children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::walk_for_calls(state, child);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
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

impl crate::extraction::LanguageExtractor for QBasicExtractor {
    fn extensions(&self) -> &[&str] {
        &["qb"]
    }

    fn language_name(&self) -> &str {
        "QBasic"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_qbasic(file_path, source)
    }
}
