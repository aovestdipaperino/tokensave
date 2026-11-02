use std::time::Instant;

use crate::extraction::collapse_ws;
use crate::extraction::ts_state::{find_child_by_kind, ExtractionState};
use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct CommonLispExtractor;

impl CommonLispExtractor {
    pub fn extract_commonlisp(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("commonlisp");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Common Lisp grammar: {e}"))?;
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
            "defun" => Self::visit_defun(state, node),
            "list_lit" => Self::visit_list(state, node),
            _ => Self::visit_children(state, node),
        }
    }

    /// Dispatches a plain `list_lit` on its head symbol.
    ///
    /// A `(defun ...)` form parses as a `list_lit` wrapping a `defun` node,
    /// so that case delegates first. Other `def*` forms are plain lists whose
    /// first symbol names the definition kind.
    fn visit_list(state: &mut ExtractionState, node: TsNode<'_>) {
        if let Some(defun) = find_child_by_kind(node, "defun") {
            Self::visit_defun(state, defun);
            return;
        }
        let head = Self::list_head(state, node);
        let normalized = head
            .as_deref()
            .map(Self::normalize_head)
            .unwrap_or_default();
        match normalized.as_str() {
            "defvar" | "defparameter" => Self::visit_var(state, node, NodeKind::Static),
            "defconstant" => Self::visit_var(state, node, NodeKind::Const),
            "defclass" | "defstruct" => Self::visit_class(state, node),
            "defpackage" | "in-package" => Self::visit_package(state, node),
            // Uppercase `DEFUN` etc. do not match the grammar keyword, so they
            // parse as plain lists; handle them through the same emitters.
            "defun" | "defmacro" | "defgeneric" => {
                Self::visit_plain_defun(state, node, NodeKind::Function);
            }
            "defmethod" => Self::visit_plain_defun(state, node, NodeKind::Method),
            _ => Self::visit_children(state, node),
        }
    }

    /// Handles a `defun` node: `defun`, `defmacro`, `defgeneric`, `defmethod`,
    /// and `lambda` forms. Anonymous `lambda` forms carry no name and only
    /// contribute nested definitions and call sites.
    fn visit_defun(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(header) = find_child_by_kind(node, "defun_header") else {
            Self::visit_children(state, node);
            return;
        };
        let keyword = header
            .child_by_field_name("keyword")
            .map(|k| state.node_text(k))
            .unwrap_or_default();
        let kind = match Self::normalize_head(&keyword).as_str() {
            "defun" | "defmacro" | "defgeneric" => NodeKind::Function,
            "defmethod" => NodeKind::Method,
            _ => {
                Self::visit_children(state, node);
                return;
            }
        };
        let Some(name_node) = header.child_by_field_name("function_name") else {
            Self::visit_children(state, node);
            return;
        };
        let Some(name) = Self::defun_name(state, name_node) else {
            Self::visit_children(state, node);
            return;
        };
        let docstring = Self::extract_string_child(state, node);
        Self::emit_function(state, node, name, kind, docstring);
    }

    /// Handles `defun`-family forms that parsed as plain lists (uppercase
    /// keywords the grammar does not special-case).
    fn visit_plain_defun(state: &mut ExtractionState, node: TsNode<'_>, kind: NodeKind) {
        let Some(name) = Self::nth_arg_name(state, node, 1) else {
            Self::visit_children(state, node);
            return;
        };
        Self::emit_function(state, node, name, kind, None);
    }

    /// Emits a function or method node with a `Contains` edge and collects
    /// call sites from its body.
    fn emit_function(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        name: String,
        kind: NodeKind,
        docstring: Option<String>,
    ) {
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let sig = Self::first_line(state, node);

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
            signature: sig,
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

        Self::collect_body_calls(state, node, &id);
    }

    /// Handles `defvar` and `defparameter` (mutable special variables, mapped
    /// to `Static`) and `defconstant` (mapped to `Const`).
    fn visit_var(state: &mut ExtractionState, node: TsNode<'_>, kind: NodeKind) {
        let Some(name) = Self::nth_arg_name(state, node, 1) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let sig = Self::first_line(state, node);

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
            signature: sig,
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

    /// Handles `defclass` and `defstruct`, both mapped to `Class` like the
    /// model extractor maps `deftype`.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name) = Self::nth_arg_name(state, node, 1) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);
        let sig = Self::first_line(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            signature: sig,
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

    /// Handles `defpackage` and `in-package`, mapped to `Module` like the
    /// model extractor maps `ns`.
    fn visit_package(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name) = Self::nth_arg_name(state, node, 1) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let qualified_name = format!("{}::{}", state.file_path, name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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

        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
    }

    /// Collects call sites from a function body, skipping the parameter list.
    ///
    /// For a `defun` node the header child holds the lambda list; for a plain
    /// list the head symbol and name are single symbols with no children, so
    /// recursing into every child is equivalent and stays linear.
    fn collect_body_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() != "defun_header" {
                    Self::collect_calls(state, child, fn_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Records one `Calls` reference per nested list head, mirroring the
    /// model extractor, which excludes only definition forms.
    fn collect_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        match node.kind() {
            "defun" => Self::collect_body_calls(state, node, fn_id),
            "list_lit" => {
                if let Some(defun) = find_child_by_kind(node, "defun") {
                    Self::collect_body_calls(state, defun, fn_id);
                    return;
                }
                if let Some(head) = Self::list_head(state, node) {
                    if !Self::is_definition_head(&Self::normalize_head(&head)) {
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
                Self::collect_child_calls(state, node, fn_id);
            }
            _ => Self::collect_child_calls(state, node, fn_id),
        }
    }

    fn collect_child_calls(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_calls(state, cursor.node(), fn_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Returns the head symbol of a plain list: the first `sym_lit` or
    /// package-qualified `package_lit` child.
    fn list_head(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "sym_lit" || child.kind() == "package_lit" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Normalizes a head or keyword for comparison: drops any package prefix
    /// (`cl:defun` becomes `defun`), drops keyword colons (`:foo` becomes
    /// `foo`), and lowercases.
    fn normalize_head(text: &str) -> String {
        text.rsplit(':')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
    }

    fn is_definition_head(normalized: &str) -> bool {
        matches!(
            normalized,
            "defun"
                | "defmacro"
                | "defgeneric"
                | "defmethod"
                | "defvar"
                | "defparameter"
                | "defconstant"
                | "defclass"
                | "defstruct"
                | "defpackage"
                | "in-package"
                | "lambda"
        )
    }

    /// Extracts the defined name from a `defun_header` name form: a plain or
    /// package-qualified symbol, or a `(setf name)` list.
    fn defun_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        match node.kind() {
            "sym_lit" | "package_lit" => Some(state.node_text(node)),
            "list_lit" => Some(collapse_ws(&state.node_text(node))),
            _ => {
                let text = collapse_ws(&state.node_text(node));
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        }
    }

    /// Returns the name carried by the Nth meaningful child of a plain
    /// definition list (0 is the head symbol, so 1 is the defined name).
    /// Comments and `#_`-discarded forms do not count. Package names may be
    /// keywords or strings in addition to symbols.
    fn nth_arg_name(state: &ExtractionState, node: TsNode<'_>, n: usize) -> Option<String> {
        let mut index = 0;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() && child.kind() != "comment" && child.kind() != "dis_expr" {
                    if index == n {
                        return Self::arg_name(state, child);
                    }
                    index += 1;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    fn arg_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        match node.kind() {
            "sym_lit" | "package_lit" => Some(state.node_text(node)),
            "kwd_lit" => Some(Self::normalize_head(&state.node_text(node))),
            "str_lit" => Some(state.node_text(node).trim_matches('"').to_string()),
            // `(defstruct (name ...) ...)` carries the name in a list.
            "list_lit" => Self::list_head(state, node),
            _ => None,
        }
    }

    /// Returns the first `str_lit` child, used for docstrings.
    fn extract_string_child(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "str_lit" {
                    let text = state.node_text(child);
                    return Some(text.trim_matches('"').to_string());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    fn first_line(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        text.lines().next().map(|l| l.trim().to_string())
    }
}

impl crate::extraction::LanguageExtractor for CommonLispExtractor {
    fn extensions(&self) -> &[&str] {
        &["lisp", "lsp", "cl", "asd"]
    }

    fn language_name(&self) -> &'static str {
        "Common Lisp"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_commonlisp(file_path, source)
    }
}
