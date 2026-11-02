use std::time::Instant;

use crate::extraction::ts_state::ExtractionState;
use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct SchemeExtractor;

impl SchemeExtractor {
    pub fn extract_scheme(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(t) => t,
            Err(msg) => {
                state.errors.push(msg);
                return state.build_result(start);
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

        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();
        state.build_result(start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("scheme");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Scheme grammar: {e}"))?;
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
        // Quoted data and comments are not code: never descend into them, so a
        // quoted or commented-out `(define ...)` emits nothing.
        if matches!(
            node.kind(),
            "quote" | "comment" | "block_comment" | "sexp_comment" | "directive"
        ) {
            return;
        }
        if node.kind() != "list" {
            Self::visit_children(state, node);
            return;
        }

        // The grammar is a generic S-expression grammar, so definitions are
        // recognized by the head symbol of a list.
        let head_text = Self::first_symbol(state, node);
        let head = head_text.as_deref().unwrap_or("");

        match head {
            "define" => Self::visit_define(state, node),
            "define-syntax" => Self::visit_define_syntax(state, node),
            "define-library" | "library" => Self::visit_library(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    fn visit_define(state: &mut ExtractionState, node: TsNode<'_>) {
        let children = Self::code_children(node);
        if children.len() < 2 {
            return;
        }
        let target = children[1];
        if target.kind() == "list" {
            // Function form: (define (name args ...) body ...).
            let inner = Self::code_children(target);
            let Some(name_node) = inner.first().filter(|n| n.kind() == "symbol") else {
                return;
            };
            let name = state.node_text(*name_node);
            let start_line = node.start_position().row as u32;
            let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
            let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
            let sig = Self::first_line(state, node);
            Self::emit_node(
                state,
                id.clone(),
                NodeKind::Function,
                name,
                qualified_name,
                node,
                start_line,
                sig,
            );
            // Collect call sites from the body, skipping `(define (name ...))`.
            Self::extract_calls(state, node, &id, 2);
        } else if target.kind() == "symbol" {
            // Variable form: (define name value).
            let name = state.node_text(target);
            let start_line = node.start_position().row as u32;
            let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
            let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
            let sig = Self::first_line(state, node);
            Self::emit_node(
                state,
                id,
                NodeKind::Const,
                name,
                qualified_name,
                node,
                start_line,
                sig,
            );
        }
        // Other shapes (e.g. a malformed target) are skipped without failing.
    }

    fn visit_define_syntax(state: &mut ExtractionState, node: TsNode<'_>) {
        // (define-syntax name ...).
        let children = Self::code_children(node);
        if children.len() < 2 || children[1].kind() != "symbol" {
            return;
        }
        let name = state.node_text(children[1]);
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Macro, &name, start_line);
        let sig = Self::first_line(state, node);
        Self::emit_node(
            state,
            id,
            NodeKind::Macro,
            name,
            qualified_name,
            node,
            start_line,
            sig,
        );
    }

    fn visit_library(state: &mut ExtractionState, node: TsNode<'_>) {
        // R7RS (define-library (path parts ...) ...) and R6RS
        // (library (path parts ...) ...) share the same name shape.
        let children = Self::code_children(node);
        if children.len() < 2 {
            return;
        }
        let name_node = children[1];
        let name = if name_node.kind() == "list" {
            let parts: Vec<String> = Self::code_children(name_node)
                .iter()
                .filter(|n| n.kind() == "symbol")
                .map(|n| state.node_text(*n))
                .collect();
            if parts.is_empty() {
                return;
            }
            parts.join(".")
        } else if name_node.kind() == "symbol" {
            state.node_text(name_node)
        } else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);
        Self::emit_node(
            state,
            id.clone(),
            NodeKind::Module,
            name.clone(),
            qualified_name,
            node,
            start_line,
            None,
        );

        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
    }

    /// Pushes a node plus its `Contains` edge from the current parent scope.
    #[allow(clippy::too_many_arguments)]
    fn emit_node(
        state: &mut ExtractionState,
        id: String,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        node: TsNode<'_>,
        start_line: u32,
        signature: Option<String>,
    ) {
        let graph_node = Node {
            id: id.clone(),
            kind,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    /// Direct named children that carry code, skipping comments and directives.
    fn code_children(node: TsNode<'_>) -> Vec<TsNode<'_>> {
        let mut out = Vec::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && !matches!(
                        child.kind(),
                        "comment" | "block_comment" | "sexp_comment" | "directive"
                    )
                {
                    out.push(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        out
    }

    /// Text of the first `symbol` among the code children of a list.
    fn first_symbol(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        Self::code_children(node)
            .iter()
            .find(|n| n.kind() == "symbol")
            .map(|n| state.node_text(*n))
    }

    /// Collects call sites from a function body, skipping the first `skip`
    /// code children (the head symbol and the name/target form).
    fn extract_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str, skip: usize) {
        for child in Self::code_children(node).iter().skip(skip) {
            Self::collect_calls(state, *child, fn_id);
        }
    }

    fn collect_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        if matches!(
            node.kind(),
            "quote" | "comment" | "block_comment" | "sexp_comment" | "directive"
        ) {
            return;
        }
        if node.kind() == "list" {
            if let Some(head) = Self::first_symbol(state, node) {
                if !matches!(
                    head.as_str(),
                    "define" | "define-syntax" | "define-library" | "library"
                ) {
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: fn_id.to_string(),
                        reference_name: head,
                        reference_kind: EdgeKind::Calls,
                        line: node.start_position().row as u32,
                        column: node.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
            }
        }
        for child in Self::code_children(node) {
            // The head symbol itself is not a call target of its own list, but
            // descending uniformly keeps this a plain subtree walk; nested
            // definition heads are filtered by name above.
            Self::collect_calls(state, child, fn_id);
        }
    }

    fn first_line(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        text.lines().next().map(|l| l.trim().to_string())
    }
}

impl crate::extraction::LanguageExtractor for SchemeExtractor {
    fn extensions(&self) -> &[&str] {
        &["scm", "ss"]
    }

    fn language_name(&self) -> &'static str {
        "Scheme"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_scheme(file_path, source)
    }
}
