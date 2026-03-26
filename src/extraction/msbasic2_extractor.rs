/// Tree-sitter based MS BASIC 2.0 source code extractor.
///
/// Parses MS BASIC 2.0 (Commodore 64 era) source files and emits nodes and
/// edges for the code graph. MS BASIC 2.0 is line-number based with no
/// structural subroutine concept. This extractor synthesizes Function nodes
/// from REM-labelled sections that end with RETURN, and extracts LET
/// assignments as constants, GOSUB/GOTO as call references, and REM
/// lines as docstrings.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from MS BASIC 2.0 source files using tree-sitter.
pub struct MsBasic2Extractor;

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

/// Represents a collected line from the BASIC program for subroutine synthesis.
struct BasicLine<'a> {
    /// The `line` AST node.
    node: TsNode<'a>,
    /// The line number (e.g. 10, 20, 100).
    line_number: u32,
    /// The kind of the first statement on this line.
    statement_kind: String,
    /// The text of the REM comment, if this line is a REM.
    comment_text: Option<String>,
}

impl MsBasic2Extractor {
    /// Extract code graph nodes and edges from an MS BASIC 2.0 source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the BASIC source code to parse.
    pub fn extract_msbasic2(file_path: &str, source: &str) -> ExtractionResult {
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

        // Collect all lines from the AST.
        let root = tree.root_node();
        let lines = Self::collect_lines(&state, root);

        // First pass: extract top-level LET constants (before the first subroutine).
        Self::extract_top_level_lets(&mut state, &lines);

        // Second pass: synthesize subroutine Function nodes from REM ... RETURN blocks.
        Self::extract_subroutines(&mut state, &lines);

        // Third pass: extract GOSUB/GOTO references at the file level (those not in subroutines).
        Self::extract_top_level_calls(&mut state, &lines);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_msbasic2::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load MS BASIC 2.0 grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Collect all lines from the program into a structured list.
    fn collect_lines<'a>(state: &ExtractionState, root: TsNode<'a>) -> Vec<BasicLine<'a>> {
        let mut lines = Vec::new();
        let mut cursor = root.walk();
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                if node.kind() == "line" {
                    if let Some(basic_line) = Self::parse_line(state, node) {
                        lines.push(basic_line);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        lines
    }

    /// Parse a single `line` node into a BasicLine struct.
    fn parse_line<'a>(state: &ExtractionState, node: TsNode<'a>) -> Option<BasicLine<'a>> {
        let line_number_node = Self::find_child_by_kind(node, "line_number")?;
        let line_number_text = state.node_text(line_number_node);
        let line_number: u32 = line_number_text.trim().parse().unwrap_or(0);

        // Navigate: line -> statement_list -> statement -> specific_kind
        let statement_list = Self::find_child_by_kind(node, "statement_list")?;
        let statement = Self::find_child_by_kind(statement_list, "statement")?;

        // Get the first named child of statement (the actual statement type).
        let mut stmt_cursor = statement.walk();
        let mut statement_kind = String::new();
        let mut comment_text = None;
        if stmt_cursor.goto_first_child() {
            let child = stmt_cursor.node();
            statement_kind = child.kind().to_string();
            if child.kind() == "comment" {
                let text = state.node_text(child);
                // Strip "REM " prefix.
                let stripped = if text.len() > 4 {
                    text[4..].trim().to_string()
                } else {
                    text.trim_start_matches("REM").trim().to_string()
                };
                comment_text = Some(stripped);
            }
        }

        Some(BasicLine {
            node,
            line_number,
            statement_kind,
            comment_text,
        })
    }

    /// Extract LET statements that are outside subroutines as top-level constants.
    ///
    /// In MS BASIC 2.0, lines like `30 LET MR = 3` serve as variable initialization.
    /// We treat only top-level LET assignments (those not inside subroutines) as constants.
    fn extract_top_level_lets(state: &mut ExtractionState, lines: &[BasicLine<'_>]) {
        let subroutine_ranges = Self::find_subroutine_ranges(lines);
        for (idx, basic_line) in lines.iter().enumerate() {
            // Skip lines that are inside subroutines.
            if subroutine_ranges
                .iter()
                .any(|(start, end)| idx >= *start && idx < *end)
            {
                continue;
            }
            if basic_line.statement_kind == "let_statement" {
                Self::visit_let_statement(state, basic_line);
            }
        }
    }

    /// Extract a LET statement as a Const node.
    fn visit_let_statement(state: &mut ExtractionState, basic_line: &BasicLine<'_>) {
        let statement_list = match Self::find_child_by_kind(basic_line.node, "statement_list") {
            Some(sl) => sl,
            None => return,
        };
        let statement = match Self::find_child_by_kind(statement_list, "statement") {
            Some(s) => s,
            None => return,
        };
        let let_stmt = match Self::find_child_by_kind(statement, "let_statement") {
            Some(ls) => ls,
            None => return,
        };

        // Extract the variable name from: let_statement -> variable -> identifier
        let var_node = match Self::find_child_by_kind(let_stmt, "variable") {
            Some(v) => v,
            None => return,
        };
        let id_node = match Self::find_child_by_kind(var_node, "identifier") {
            Some(i) => i,
            None => return,
        };
        let name = state.node_text(id_node);

        let start_line = basic_line.node.start_position().row as u32;
        let end_line = basic_line.node.end_position().row as u32;
        let start_column = basic_line.node.start_position().column as u32;
        let end_column = basic_line.node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
        let text = state.node_text(basic_line.node);

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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Synthesize subroutine Function nodes from REM-labelled sections that end with RETURN.
    ///
    /// MS BASIC 2.0 has no formal subroutine syntax. We detect patterns like:
    /// ```basic
    /// 100 REM LOG A MESSAGE
    /// 110 REM PARAMS: ...
    /// 200 PRINT ...
    /// 210 RETURN
    /// ```
    /// A REM line (or consecutive REM lines) followed by code ending with RETURN
    /// is treated as a subroutine. The REM text becomes the docstring, and a
    /// function name is derived from the first REM text.
    fn extract_subroutines(state: &mut ExtractionState, lines: &[BasicLine<'_>]) {
        let mut i = 0;
        while i < lines.len() {
            // Look for a REM line that starts a potential subroutine.
            if lines[i].statement_kind == "comment" {
                // Gather consecutive REM lines.
                let rem_start = i;
                let mut rem_comments: Vec<String> = Vec::new();
                while i < lines.len() && lines[i].statement_kind == "comment" {
                    if let Some(ref text) = lines[i].comment_text {
                        rem_comments.push(text.clone());
                    }
                    i += 1;
                }

                // Check if the lines following the REM block end with RETURN.
                let body_start = i;
                let mut body_end = i;
                let mut has_return = false;
                while body_end < lines.len() {
                    if lines[body_end].statement_kind == "return_statement" {
                        has_return = true;
                        body_end += 1;
                        break;
                    }
                    // Stop at the next REM block (which would be the start of another subroutine).
                    if lines[body_end].statement_kind == "comment" {
                        break;
                    }
                    body_end += 1;
                }

                if has_return && body_start < body_end {
                    // Derive a function name from the first REM comment.
                    let fn_name = Self::derive_function_name(&rem_comments);
                    let docstring = if rem_comments.is_empty() {
                        None
                    } else {
                        Some(rem_comments.join("\n"))
                    };

                    // The subroutine spans from the first REM line to the RETURN line.
                    let first_node = lines[rem_start].node;
                    let last_node = lines[body_end - 1].node;
                    let start_line = first_node.start_position().row as u32;
                    let end_line = last_node.end_position().row as u32;
                    let start_column = first_node.start_position().column as u32;
                    let end_column = last_node.end_position().column as u32;
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), fn_name);
                    let fn_id = generate_node_id(
                        &state.file_path,
                        &NodeKind::Function,
                        &fn_name,
                        start_line,
                    );

                    // Count complexity by walking body lines' AST nodes.
                    let mut branches: u32 = 0;
                    let mut loops: u32 = 0;
                    let mut returns: u32 = 0;
                    for line in &lines[body_start..body_end] {
                        // Try count_complexity on each line node that has children.
                        let stmt_list =
                            Self::find_child_by_kind(line.node, "statement_list");
                        if let Some(sl) = stmt_list {
                            let stmt = Self::find_child_by_kind(sl, "statement");
                            if let Some(s) = stmt {
                                let mut sc = s.walk();
                                if sc.goto_first_child() {
                                    let inner = sc.node();
                                    match inner.kind() {
                                        "if_statement" => branches += 1,
                                        "for_statement" => loops += 1,
                                        "return_statement" => returns += 1,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    // Use the line number of the first code line (after REMs) as the
                    // signature, so callers can reference it by GOSUB <line_number>.
                    let sig_line_num = lines[body_start].line_number;
                    let signature = format!("GOSUB {}", sig_line_num);

                    let graph_node = Node {
                        id: fn_id.clone(),
                        kind: NodeKind::Function,
                        name: fn_name.clone(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(signature),
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
                            target: fn_id.clone(),
                            kind: EdgeKind::Contains,
                            line: Some(start_line),
                        });
                    }

                    // Extract GOSUB/GOTO call sites from the body.
                    for line in &lines[body_start..body_end] {
                        Self::extract_calls_from_line(state, line, &fn_id);
                    }

                    i = body_end;
                } else {
                    // REM block not followed by RETURN — skip.
                    i = body_end;
                }
            } else {
                i += 1;
            }
        }
    }

    /// Extract GOSUB/GOTO references from top-level lines (those not part of subroutines).
    fn extract_top_level_calls(state: &mut ExtractionState, lines: &[BasicLine<'_>]) {
        // Determine which lines are part of subroutines (between first REM and RETURN).
        let subroutine_ranges = Self::find_subroutine_ranges(lines);

        let file_node_id = state
            .node_stack
            .last()
            .map(|(_, id)| id.clone())
            .unwrap_or_default();

        for (idx, line) in lines.iter().enumerate() {
            // Skip lines that are inside subroutines.
            if subroutine_ranges.iter().any(|(start, end)| idx >= *start && idx < *end) {
                continue;
            }
            Self::extract_calls_from_line(state, line, &file_node_id);
        }
    }

    /// Find ranges of lines that belong to subroutines (REM ... RETURN blocks).
    fn find_subroutine_ranges(lines: &[BasicLine<'_>]) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].statement_kind == "comment" {
                let rem_start = i;
                while i < lines.len() && lines[i].statement_kind == "comment" {
                    i += 1;
                }
                let mut body_end = i;
                let mut has_return = false;
                while body_end < lines.len() {
                    if lines[body_end].statement_kind == "return_statement" {
                        has_return = true;
                        body_end += 1;
                        break;
                    }
                    if lines[body_end].statement_kind == "comment" {
                        break;
                    }
                    body_end += 1;
                }
                if has_return {
                    ranges.push((rem_start, body_end));
                }
                i = body_end;
            } else {
                i += 1;
            }
        }
        ranges
    }

    /// Extract GOSUB/GOTO call references from a single line.
    fn extract_calls_from_line(
        state: &mut ExtractionState,
        line: &BasicLine<'_>,
        from_node_id: &str,
    ) {
        Self::walk_for_calls(state, line.node, from_node_id);
    }

    /// Recursively walk AST nodes looking for gosub_statement and goto_statement.
    fn walk_for_calls(state: &mut ExtractionState, node: TsNode<'_>, from_node_id: &str) {
        let kind = node.kind();
        match kind {
            "gosub_statement" | "goto_statement" => {
                // Extract the target line number.
                if let Some(ln_node) = Self::find_child_by_kind(node, "line_number") {
                    let target = state.node_text(ln_node);
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: from_node_id.to_string(),
                        reference_name: target,
                        reference_kind: EdgeKind::Calls,
                        line: node.start_position().row as u32,
                        column: node.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
            }
            _ => {}
        }
        // Recurse into children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::walk_for_calls(state, child, from_node_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Derive a function name from REM comment text.
    ///
    /// Takes the first REM line text and converts it into a snake_case-like
    /// identifier. For example, "LOG A MESSAGE" becomes "LOG_A_MESSAGE".
    fn derive_function_name(rem_comments: &[String]) -> String {
        if rem_comments.is_empty() {
            return "UNNAMED_SUB".to_string();
        }
        let first = &rem_comments[0];
        // Replace spaces with underscores and keep alphanumeric + underscore.
        let name: String = first
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect();
        // Collapse multiple underscores and trim.
        let mut collapsed = String::new();
        let mut prev_underscore = false;
        for c in name.chars() {
            if c == '_' {
                if !prev_underscore && !collapsed.is_empty() {
                    collapsed.push('_');
                }
                prev_underscore = true;
            } else {
                collapsed.push(c);
                prev_underscore = false;
            }
        }
        let trimmed = collapsed.trim_end_matches('_').to_string();
        if trimmed.is_empty() {
            "UNNAMED_SUB".to_string()
        } else {
            trimmed
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

impl crate::extraction::LanguageExtractor for MsBasic2Extractor {
    fn extensions(&self) -> &[&str] {
        &["bas"]
    }

    fn language_name(&self) -> &str {
        "MS BASIC 2.0"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_msbasic2(file_path, source)
    }
}
