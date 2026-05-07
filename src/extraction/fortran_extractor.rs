/// Tree-sitter based Fortran source code extractor.
///
/// Parses Fortran source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, FORTRAN_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Fortran source files using tree-sitter.
pub struct FortranExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    /// Stack of (name, `node_id`) for building qualified names and parent edges.
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

impl FortranExtractor {
    /// Extract code graph nodes and edges from a Fortran source file.
    pub fn extract_fortran(file_path: &str, source: &str) -> ExtractionResult {
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

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("fortran");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Fortran grammar: {e}"))?;
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
        match node.kind() {
            "module" => Self::visit_module(state, node),
            "program" => Self::visit_program(state, node),
            "subroutine" => Self::visit_subroutine(state, node),
            "function" => Self::visit_function(state, node),
            "derived_type_definition" => Self::visit_derived_type(state, node),
            "interface" => Self::visit_interface(state, node),
            "variable_declaration" => Self::visit_variable_declaration(state, node),
            "use_statement" => Self::visit_use_statement(state, node),
            "internal_procedures" => Self::visit_children(state, node),
            _ => {}
        }
    }

    /// Extract a module definition.
    fn visit_module(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_module_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
            name: name.clone(),
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Visit module body.
        state.node_stack.push((name.clone(), id));
        Self::visit_children(state, node);
        state.node_stack.pop();
    }

    /// Extract a program definition (treated as a Function node).
    fn visit_program(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_program_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());

        let metrics = count_complexity(node, &FORTRAN_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
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

        // Visit program body for use statements, call sites, etc.
        state.node_stack.push((name.clone(), id.clone()));
        Self::visit_children(state, node);
        // Extract call sites from the program body.
        Self::extract_call_sites(state, node, &id);
        state.node_stack.pop();
    }

    /// Extract a subroutine definition.
    fn visit_subroutine(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_subroutine_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let signature = Self::extract_first_line_signature(state, node);
        let metrics = count_complexity(node, &FORTRAN_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
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

        // Extract call sites from the subroutine body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract a function definition.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_function_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

        let signature = Self::extract_first_line_signature(state, node);
        let metrics = count_complexity(node, &FORTRAN_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
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

        // Extract call sites from the function body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract a derived type definition (Fortran's struct equivalent).
    fn visit_derived_type(state: &mut ExtractionState, node: TsNode<'_>) {
        let (name, base_type) = Self::find_derived_type_info(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, &name, start_line);

        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Struct,
            name: name.clone(),
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extends edge for base type (type, extends(BaseType) :: DerivedType).
        if let Some(base_name) = base_type {
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: id.clone(),
                reference_name: base_name,
                reference_kind: EdgeKind::Extends,
                line: start_line,
                column: start_column,
                file_path: state.file_path.clone(),
            });
        }

        // Visit fields inside the derived type.
        state.node_stack.push((name.clone(), id));
        Self::visit_derived_type_fields(state, node);
        state.node_stack.pop();
    }

    /// Visit `variable_declaration` children inside a `derived_type_definition` to extract fields.
    fn visit_derived_type_fields(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declaration" {
                    Self::visit_field(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a field from a `variable_declaration` inside a derived type.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        // The field name is in the `declarator` field, which is an `identifier`.
        let name = node
            .child_by_field_name("declarator")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);
        let text = state.node_text(node);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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

        // Contains edge from parent (the derived type).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract an interface definition.
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_interface_name(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Interface, &name, start_line);

        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Interface,
            name: name.clone(),
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

    /// Extract a `variable_declaration` at module/program scope.
    ///
    /// If it has a `parameter` `type_qualifier` and an `init_declarator`, treat it as a Const.
    fn visit_variable_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Check for `parameter` attribute (Fortran constant).
        if !Self::has_parameter_attribute(state, node) {
            return;
        }

        // Look for init_declarator child with left (name) and right (value).
        let declarator = node.child_by_field_name("declarator");
        if let Some(decl) = declarator {
            if decl.kind() == "init_declarator" {
                let name = decl
                    .child_by_field_name("left")
                    .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

                let docstring = Self::extract_docstring(state, node);
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;
                let start_column = node.start_position().column as u32;
                let end_column = node.end_position().column as u32;
                let text = state.node_text(node);
                let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);

                let graph_node = Node {
                    id: id.clone(),
                    kind: NodeKind::Const,
                    name,
                    qualified_name,
                    file_path: state.file_path.clone(),
                    start_line,
                    attrs_start_line: start_line,
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
        }
    }

    /// Extract a use statement.
    fn visit_use_statement(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "module_name")
            .map_or_else(|| "<unknown>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
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

    // ----------------------------
    // Name extraction helpers
    // ----------------------------

    /// Find the module name from a module node.
    /// Structure: module -> `module_statement` -> name
    fn find_module_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        Self::find_child_by_kind(node, "module_statement")
            .and_then(|stmt| Self::find_child_by_kind(stmt, "name"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
    }

    /// Find the program name from a program node.
    /// Structure: program -> `program_statement` -> name
    fn find_program_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        Self::find_child_by_kind(node, "program_statement")
            .and_then(|stmt| Self::find_child_by_kind(stmt, "name"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
    }

    /// Find the subroutine name from a subroutine node.
    /// Structure: subroutine -> `subroutine_statement` (field name="name")
    fn find_subroutine_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        Self::find_child_by_kind(node, "subroutine_statement")
            .and_then(|stmt| stmt.child_by_field_name("name"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
    }

    /// Find the function name from a function node.
    /// Structure: function -> `function_statement` (field name="name")
    fn find_function_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        Self::find_child_by_kind(node, "function_statement")
            .and_then(|stmt| stmt.child_by_field_name("name"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
    }

    /// Find the derived type name and optional base type.
    /// Structure: `derived_type_definition` -> `derived_type_statement` -> `type_name`, optional `base_type_specifier`
    fn find_derived_type_info(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> (String, Option<String>) {
        let stmt = Self::find_child_by_kind(node, "derived_type_statement");
        let name = stmt
            .and_then(|s| Self::find_child_by_kind(s, "type_name"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let base_type = stmt
            .and_then(|s| s.child_by_field_name("base"))
            .and_then(|base_spec| Self::find_child_by_kind(base_spec, "identifier"))
            .map(|n| state.node_text(n));

        (name, base_type)
    }

    /// Find the interface name.
    /// Structure: interface -> `interface_statement` -> name
    fn find_interface_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        Self::find_child_by_kind(node, "interface_statement")
            .and_then(|stmt| Self::find_child_by_kind(stmt, "name"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
    }

    /// Check if a `variable_declaration` has a `parameter` attribute.
    fn has_parameter_attribute(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_qualifier" {
                    let text = state.node_text(child);
                    if text.to_lowercase() == "parameter" {
                        return true;
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    // ----------------------------
    // Signature and docstring helpers
    // ----------------------------

    /// Extract the first line of a node as its signature.
    fn extract_first_line_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract docstrings from `! comment` lines preceding definitions.
    ///
    /// Fortran uses comment lines (! ...) as documentation. We look for `comment`
    /// sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text.trim_start_matches('!').trim().to_string();
                comments.push(stripped);
                prev = prev_node.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        // Comments were collected in reverse order; reverse them back.
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "subroutine_call" => {
                        // subroutine_call has a `subroutine` field pointing to the identifier.
                        if let Some(sub_node) = child.child_by_field_name("subroutine") {
                            let callee_name = state.node_text(sub_node);
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: callee_name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        // Recurse into arguments for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    "call_expression" => {
                        // call_expression: first named child is typically the identifier.
                        if let Some(ident) = child.named_child(0) {
                            if ident.kind() == "identifier" {
                                let callee_name = state.node_text(ident);
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: fn_node_id.to_string(),
                                    reference_name: callee_name,
                                    reference_kind: EdgeKind::Calls,
                                    line: child.start_position().row as u32,
                                    column: child.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                            }
                        }
                        // Recurse into arguments for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested subroutine/function definitions.
                    "subroutine" | "function" | "module" | "program" => {}
                    _ => {
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                }
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

    /// Build the final `ExtractionResult` from the accumulated state.
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

impl crate::extraction::LanguageExtractor for FortranExtractor {
    fn extensions(&self) -> &[&str] {
        &["f90", "f95", "f03", "f08", "f18", "f", "for"]
    }

    fn language_name(&self) -> &'static str {
        "Fortran"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_fortran(file_path, source)
    }
}
