use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, R_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct RExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
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

    fn qualified_prefix(&self) -> String {
        let mut parts = vec![self.file_path.clone()];
        for (name, _) in &self.node_stack {
            parts.push(name.clone());
        }
        parts.join("::")
    }

    fn parent_node_id(&self) -> Option<&str> {
        self.node_stack.last().map(|(_, id)| id.as_str())
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }
}

impl RExtractor {
    pub fn extract_r(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(t) => t,
            Err(msg) => {
                state.errors.push(msg);
                return Self::build_result(state, start);
            }
        };

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
            updated_at: state.timestamp,
        };
        let file_node_id = file_node.id.clone();
        state.nodes.push(file_node);
        state.node_stack.push((file_path.to_string(), file_node_id));

        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();
        Self::build_result(state, start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("r");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load R grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit_node(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "binary_operator" => Self::visit_assignment(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    /// Handles `name <- function(...)` and `name = function(...)` assignments.
    fn visit_assignment(state: &mut ExtractionState, node: TsNode<'_>) {
        // tree-sitter-r binary_operator: child(0)=lhs, child(1)=op, child(2)=rhs
        let (Some(lhs), Some(op_node), Some(rhs)) = (node.child(0), node.child(1), node.child(2))
        else {
            return;
        };

        let op = state.node_text(op_node);
        if op != "<-" && op != "=" && op != "<<-" {
            // Not an assignment; still recurse to find nested assignments.
            Self::visit_children(state, node);
            return;
        }

        if rhs.kind() != "function_definition" {
            // Could be a nested assignment; recurse into rhs.
            Self::visit_node(state, rhs);
            return;
        }

        // Extract function name — handle simple identifiers and `pkg::fn` forms.
        let name = match lhs.kind() {
            "identifier" => state.node_text(lhs),
            "namespace_operator" => {
                // pkg::fn — use the rightmost identifier
                lhs.child((lhs.child_count() - 1) as u32)
                    .map(|n| state.node_text(n))
                    .unwrap_or_else(|| state.node_text(lhs))
            }
            _ => state.node_text(lhs),
        };

        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_signature(state, rhs);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let metrics = if rhs.child_count() > 0 {
            count_complexity(rhs, &R_COMPLEXITY, &state.source)
        } else {
            Default::default()
        };

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature,
            docstring,
            visibility: Visibility::Pub,
            is_async: false,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
            unsafe_blocks: 0,
            unchecked_calls: metrics.unchecked_calls,
            assertions: metrics.assertions,
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Collect call sites from the function body.
        if let Some(body) = rhs.child_by_field_name("body") {
            Self::extract_calls(state, body, &id);
        }
    }

    fn extract_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "call" {
                    // function name is the first child (the callee)
                    if let Some(callee) = child.child(0) {
                        let name = state.node_text(callee);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_id.to_string(),
                            reference_name: name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    Self::extract_calls(state, child, fn_id);
                } else if child.kind() != "function_definition" {
                    Self::extract_calls(state, child, fn_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn extract_signature(state: &ExtractionState, fn_def: TsNode<'_>) -> Option<String> {
        // Signature = "function(param1, param2, ...)"
        if let Some(params) = fn_def.child_by_field_name("parameters") {
            return Some(format!("function{}", state.node_text(params)));
        }
        let text = state.node_text(fn_def);
        text.lines().next().map(|l| l.trim().to_string())
    }

    /// Roxygen2-style docstrings start with `#'`.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if p.kind() == "comment" {
                let text = state.node_text(p);
                if text.starts_with("#'") {
                    comments.push(text.trim_start_matches("#'").trim().to_string());
                    prev = p.prev_named_sibling();
                } else {
                    break;
                }
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

impl crate::extraction::LanguageExtractor for RExtractor {
    fn extensions(&self) -> &[&str] {
        &["r", "R"]
    }

    fn language_name(&self) -> &'static str {
        "R"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_r(file_path, source)
    }
}
