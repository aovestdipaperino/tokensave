/// Tree-sitter based AWK source code extractor.
///
/// Parses AWK source files and emits nodes and edges for the code graph:
/// - `function` definitions become `Function` nodes.
/// - `BEGIN`, `END`, `BEGINFILE`, and `ENDFILE` blocks become `Function`
///   nodes named after the keyword.
/// - Pattern-action rules become `Function` nodes named after the collapsed
///   pattern text; action-only rules (no pattern) are named `main`.
/// - User function calls become unresolved `Calls` references. Builtin calls
///   (e.g. `length`, `gsub`) carry no `name` field in the grammar, so they
///   are skipped naturally; indirect calls (`@var(...)`) have a dynamic
///   callee and are skipped deliberately.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::collapse_ws;
use crate::extraction::complexity::{count_complexity, ComplexityConfig, ComplexityMetrics};
use crate::extraction::ts_state::{find_child_by_kind, ExtractionState};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from AWK source files using tree-sitter.
pub struct AwkExtractor;

/// Complexity mapping for the tree-sitter-awk grammar. Every kind listed
/// here was verified against the vendored `node-types.json`.
static AWK_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_case",
        "switch_default",
        "ternary_exp",
    ],
    loop_types: &[
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_while_statement",
    ],
    return_types: &[
        "return_statement",
        "exit_statement",
        "break_statement",
        "continue_statement",
        "next_statement",
        "nextfile_statement",
    ],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["func_call"],
    call_method_field: "name",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

impl AwkExtractor {
    /// Extract code graph nodes and edges from an AWK source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the AWK source code to parse.
    ///
    /// Never panics on malformed input: parse problems are recorded in
    /// `errors` and whatever parsed cleanly is still returned.
    pub fn extract_awk(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        // Create the File root node first so even a total parse failure
        // still yields a partial result.
        let file_node = Node {
            id: generate_node_id(file_path, &NodeKind::File, file_path, 0),
            kind: NodeKind::File,
            name: file_path.to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            start_line: 0,
            attrs_start_line: 0,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state.node_stack.push((file_path.to_string(), file_node_id));

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                state.node_stack.pop();
                return state.build_result(start);
            }
        };
        let root = tree.root_node();
        if root.has_error() {
            // Tree-sitter still produced a (partial) tree; keep going and
            // extract whatever parsed cleanly, but flag it for visibility.
            state
                .errors
                .push(format!("parse errors in {file_path} (partial extraction)"));
        }

        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        state.build_result(start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("awk");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load AWK grammar: {e}"))?;
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

    /// Visit a single top-level AST node, dispatching on its type.
    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "func_def" => Self::visit_func_def(state, node),
            "rule" => Self::visit_rule(state, node),
            _ => {}
        }
    }

    /// Extract a `function name(params) { ... }` definition.
    fn visit_func_def(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let id = Self::emit_function(state, node, &name);
        // Collect calls from the function body.
        if let Some(block) = find_child_by_kind(node, "block") {
            Self::extract_call_sites(state, block, &id);
        }
    }

    /// Extract a pattern-action rule or `BEGIN`/`END` block as a `Function`.
    fn visit_rule(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = match find_child_by_kind(node, "pattern") {
            None => "main".to_string(),
            Some(pattern) => {
                let collapsed = collapse_ws(&state.node_text(pattern));
                if pattern.named_child_count() == 0
                    && matches!(
                        collapsed.as_str(),
                        "BEGIN" | "END" | "BEGINFILE" | "ENDFILE"
                    )
                {
                    collapsed
                } else if collapsed.is_empty() {
                    "main".to_string()
                } else {
                    truncate_name(&collapsed)
                }
            }
        };
        let id = Self::emit_function(state, node, &name);
        // Collect calls from both the pattern and the action block.
        Self::extract_call_sites(state, node, &id);
    }

    /// Emit a `Function` node plus its `Contains` edge. Returns the node id.
    fn emit_function(state: &mut ExtractionState, node: TsNode<'_>, name: &str) -> String {
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{name}", state.qualified_prefix());
        let id = generate_node_id(&state.file_path, &NodeKind::Function, name, start_line);
        let metrics = if node.child_count() > 0 {
            count_complexity(node, &AWK_COMPLEXITY, &state.source)
        } else {
            ComplexityMetrics::default()
        };

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring,
            visibility: Visibility::Pub,
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: metrics.unsafe_blocks,
            unchecked_calls: metrics.unchecked_calls,
            assertions: metrics.assertions,
            cognitive_complexity: metrics.cognitive_complexity,
            distinct_operators: metrics.distinct_operators,
            distinct_operands: metrics.distinct_operands,
            total_operators: metrics.total_operators,
            total_operands: metrics.total_operands,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        id
    }

    /// Extract the signature (collapsed first line of the definition).
    fn extract_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim();
        if first_line.is_empty() {
            None
        } else {
            Some(collapse_ws(first_line))
        }
    }

    /// Extract docstrings from `#` comment lines immediately preceding a definition.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text.trim_start_matches('#').trim().to_string();
                comments.push(stripped);
                prev = prev_node.prev_named_sibling();
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

    /// Recursively find `func_call` nodes and record unresolved `Calls` references.
    ///
    /// Only calls with a `name` field (plain or namespace-qualified
    /// identifiers) are recorded; builtin calls have no `name` field and
    /// indirect calls (`@var(...)`) name no static callee, so both are skipped.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, owner_id: &str) {
        // The callee of an indirect call is a runtime value, not a name.
        if node.kind() == "indirect_func_call" {
            return;
        }
        if node.kind() == "func_call" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let callee = state.node_text(name_node);
                if !callee.is_empty() {
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: owner_id.to_string(),
                        reference_name: callee,
                        reference_kind: EdgeKind::Calls,
                        line: node.start_position().row as u32,
                        column: node.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
            }
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::extract_call_sites(state, child, owner_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

/// Shorten a rule name derived from pattern source so long patterns do not
/// produce unwieldy node names.
fn truncate_name(name: &str) -> String {
    const MAX_CHARS: usize = 80;
    if name.chars().count() <= MAX_CHARS {
        return name.to_string();
    }
    let truncated: String = name.chars().take(MAX_CHARS).collect();
    format!("{truncated}...")
}

impl crate::extraction::LanguageExtractor for AwkExtractor {
    fn extensions(&self) -> &[&str] {
        &["awk"]
    }

    fn language_name(&self) -> &'static str {
        "AWK"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_awk(file_path, source)
    }
}
