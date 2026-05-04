/// Tree-sitter based Pascal source code extractor.
///
/// Parses Pascal source files (.pas, .pp, .dpr, .lpr) and emits nodes and edges
/// for the code graph. Supports programs, units, classes, records, interfaces,
/// functions, procedures, constructors, destructors, properties, constants,
/// variables, uses clauses, and visibility sections.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, PASCAL_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Pascal source files using tree-sitter.
pub struct PascalExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    /// Stack of `(name, node_id)` for building qualified names and parent edges.
    node_stack: Vec<(String, String)>,
    file_path: String,
    source: Vec<u8>,
    timestamp: u64,
    /// Track nesting depth for classes.
    class_depth: usize,
    /// Current visibility context (from declSection nodes).
    current_visibility: Visibility,
    /// Whether we are in the implementation section (items default to Private).
    in_implementation: bool,
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
            class_depth: 0,
            current_visibility: Visibility::Pub,
            in_implementation: false,
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

impl PascalExtractor {
    /// Extract code graph nodes and edges from a Pascal source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Pascal source code to parse.
    pub fn extract_pascal(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("pascal");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Pascal grammar: {e}"))?;
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
            "program" => Self::visit_program(state, node),
            "unit" => Self::visit_unit(state, node),
            "interface" => Self::visit_interface_section(state, node),
            "implementation" => Self::visit_implementation_section(state, node),
            "declUses" => Self::visit_uses_clause(state, node),
            "declTypes" => Self::visit_type_section(state, node),
            "declConsts" => Self::visit_const_section(state, node),
            "declVars" => Self::visit_var_section(state, node),
            "defProc" => Self::visit_def_proc(state, node),
            _ => {
                // Recurse into children for unmatched nodes.
                Self::visit_children(state, node);
            }
        }
    }

    /// Extract a program declaration.
    fn visit_program(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_module_name(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::PascalProgram,
            &name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::PascalProgram,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("program {name}")),
            docstring: Self::extract_docstring(state, node),
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

        // Contains edge from File.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Push program onto stack and visit children.
        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
    }

    /// Extract a unit declaration.
    fn visit_unit(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_module_name(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::PascalUnit, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::PascalUnit,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("unit {name}")),
            docstring: Self::extract_docstring(state, node),
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

        // Contains edge from File.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Push unit onto stack and visit children.
        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
    }

    /// Visit the interface section of a unit.
    fn visit_interface_section(state: &mut ExtractionState, node: TsNode<'_>) {
        state.in_implementation = false;
        state.current_visibility = Visibility::Pub;
        Self::visit_children(state, node);
    }

    /// Visit the implementation section of a unit.
    fn visit_implementation_section(state: &mut ExtractionState, node: TsNode<'_>) {
        state.in_implementation = true;
        state.current_visibility = Visibility::Private;
        Self::visit_children(state, node);
    }

    /// Extract a uses clause (e.g., `uses SysUtils, Classes;`).
    fn visit_uses_clause(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "moduleName" {
                    Self::visit_single_use(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single uses reference as a Use node.
    fn visit_single_use(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| state.node_text(node), |n| state.node_text(n));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("uses {name}")),
            docstring: None,
            visibility: Visibility::Private,
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

        // Unresolved Uses reference.
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id,
            reference_name: name,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Visit a type section (declTypes), processing each type declaration.
    fn visit_type_section(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "declType" {
                    Self::visit_type_decl(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit a single type declaration (declType).
    /// Dispatches based on whether it's a class, record, interface, or type alias.
    fn visit_type_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        // Look for the type body: declClass, declIntf, or plain type.
        if let Some(class_node) = Self::find_child_by_kind(node, "declClass") {
            // Check if it's a record or class.
            if Self::find_child_by_kind(class_node, "kRecord").is_some() {
                Self::visit_record_type(state, &name, class_node, node);
            } else {
                Self::visit_class_type(state, &name, class_node, node);
            }
        } else if let Some(intf_node) = Self::find_child_by_kind(node, "declIntf") {
            Self::visit_interface_type(state, &name, intf_node, node);
        } else {
            // Plain type alias (e.g., TMyAlias = Integer).
            Self::visit_type_alias(state, &name, node);
        }
    }

    /// Extract a class type declaration.
    fn visit_class_type(
        state: &mut ExtractionState,
        name: &str,
        class_node: TsNode<'_>,
        decl_node: TsNode<'_>,
    ) {
        let visibility = if state.in_implementation {
            Visibility::Private
        } else {
            Visibility::Pub
        };
        let docstring = Self::extract_docstring(state, decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, name, start_line);

        // Build signature: "TMyClass = class(TObject)"
        let mut sig = format!("{name} = class");
        // Check for parent class.
        if let Some(parent_ref) = Self::find_child_by_kind(class_node, "typeref") {
            let parent_name = state.node_text(parent_ref);
            sig = format!("{name} = class({parent_name})");
        }

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(sig),
            docstring,
            visibility,
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

        // Extract parent class as Extends reference.
        if let Some(parent_ref) = Self::find_child_by_kind(class_node, "typeref") {
            let parent_name = state.node_text(parent_ref);
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: id.clone(),
                reference_name: parent_name,
                reference_kind: EdgeKind::Extends,
                line: parent_ref.start_position().row as u32,
                column: parent_ref.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
        }

        // Visit class body: fields, methods, properties, visibility sections.
        state.class_depth += 1;
        let saved_visibility = state.current_visibility.clone();
        // Default visibility inside a class is Pub (for undeclared section).
        state.current_visibility = Visibility::Pub;
        state.node_stack.push((name.to_string(), id));
        Self::visit_class_body(state, class_node);
        state.node_stack.pop();
        state.current_visibility = saved_visibility;
        state.class_depth -= 1;
    }

    /// Extract a record type declaration.
    fn visit_record_type(
        state: &mut ExtractionState,
        name: &str,
        class_node: TsNode<'_>,
        decl_node: TsNode<'_>,
    ) {
        let visibility = if state.in_implementation {
            Visibility::Private
        } else {
            Visibility::Pub
        };
        let docstring = Self::extract_docstring(state, decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::PascalRecord, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::PascalRecord,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("{name} = record")),
            docstring,
            visibility,
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

        // Visit record fields.
        state.node_stack.push((name.to_string(), id));
        Self::visit_record_body(state, class_node);
        state.node_stack.pop();
    }

    /// Extract an interface type declaration (Pascal interface, not the section).
    fn visit_interface_type(
        state: &mut ExtractionState,
        name: &str,
        intf_node: TsNode<'_>,
        decl_node: TsNode<'_>,
    ) {
        let visibility = if state.in_implementation {
            Visibility::Private
        } else {
            Visibility::Pub
        };
        let docstring = Self::extract_docstring(state, decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Interface, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Interface,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("{name} = interface")),
            docstring,
            visibility,
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

        // Visit interface methods.
        state.node_stack.push((name.to_string(), id));
        Self::visit_interface_body(state, intf_node);
        state.node_stack.pop();
    }

    /// Extract a type alias declaration.
    fn visit_type_alias(state: &mut ExtractionState, name: &str, decl_node: TsNode<'_>) {
        let visibility = if state.in_implementation {
            Visibility::Private
        } else {
            Visibility::Pub
        };
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::TypeAlias, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::TypeAlias,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring,
            visibility,
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

    /// Visit children of a class body, handling visibility sections, fields, etc.
    fn visit_class_body(state: &mut ExtractionState, class_node: TsNode<'_>) {
        let mut cursor = class_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "declSection" => Self::visit_visibility_section(state, child),
                    "declField" => Self::visit_field(state, child),
                    "declProc" => Self::visit_class_method_decl(state, child),
                    "declProp" => Self::visit_property(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit children of a record body (fields only).
    fn visit_record_body(state: &mut ExtractionState, class_node: TsNode<'_>) {
        let mut cursor = class_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "declField" {
                    Self::visit_field(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit children of an interface body (method declarations).
    fn visit_interface_body(state: &mut ExtractionState, intf_node: TsNode<'_>) {
        let mut cursor = intf_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "declProc" {
                    Self::visit_interface_method_decl(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit a visibility section (public, private, protected) and update `current_visibility`.
    fn visit_visibility_section(state: &mut ExtractionState, node: TsNode<'_>) {
        // First child should be the visibility keyword.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "kPublic" => state.current_visibility = Visibility::Pub,
                    "kPrivate" => state.current_visibility = Visibility::Private,
                    "kProtected" => state.current_visibility = Visibility::PubSuper,
                    "declField" => Self::visit_field(state, child),
                    "declProc" => Self::visit_class_method_decl(state, child),
                    "declProp" => Self::visit_property(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a field declaration.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility: state.current_visibility.clone(),
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

    /// Extract a method declaration inside a class.
    fn visit_class_method_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        // Determine the kind from the keyword child.
        let (_kind_str, node_kind) = Self::determine_proc_kind(node);
        let name = Self::find_proc_name(state, node);
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &node_kind, &name, start_line);
        let metrics = count_complexity(node, &PASCAL_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: node_kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility: state.current_visibility.clone(),
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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a method declaration inside an interface type.
    fn visit_interface_method_decl(state: &mut ExtractionState, node: TsNode<'_>) {
        let (_, node_kind) = Self::determine_proc_kind(node);
        let name = Self::find_proc_name(state, node);
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &node_kind, &name, start_line);
        let metrics = count_complexity(node, &PASCAL_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: node_kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
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

        // Contains edge from parent (interface).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a property declaration.
    fn visit_property(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Property, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Property,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility: state.current_visibility.clone(),
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

    /// Visit a const section (declConsts), processing each constant.
    fn visit_const_section(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "declConst" {
                    Self::visit_const(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single constant declaration.
    fn visit_const(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);

        let visibility = if state.in_implementation {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility,
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

    /// Visit a var section (declVars), processing each variable.
    fn visit_var_section(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "declVar" {
                    Self::visit_var(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single variable declaration.
    fn visit_var(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Static, &name, start_line);

        let visibility = if state.in_implementation {
            Visibility::Private
        } else {
            Visibility::Pub
        };

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Static,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
            docstring: None,
            visibility,
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

    /// Visit a function/procedure definition (defProc), which has a declProc and block.
    fn visit_def_proc(state: &mut ExtractionState, node: TsNode<'_>) {
        let decl = Self::find_child_by_kind(node, "declProc");
        let block = Self::find_child_by_kind(node, "block");

        if let Some(decl_node) = decl {
            let (kind_str, node_kind) = Self::determine_proc_kind(decl_node);
            let name = Self::find_proc_name(state, decl_node);
            let decl_text = state.node_text(decl_node);
            let docstring = Self::extract_docstring(state, node);
            let start_line = node.start_position().row as u32;
            let end_line = node.end_position().row as u32;
            let start_column = node.start_position().column as u32;
            let end_column = node.end_position().column as u32;

            // For method implementations like TMyClass.DoSomething, use the full name
            // but detect if it's a class method.
            let (is_method, class_name, method_name) = Self::parse_dotted_name(state, decl_node);
            let display_name = if is_method {
                method_name.clone()
            } else {
                name.clone()
            };

            let actual_kind = if is_method {
                if kind_str == "constructor" {
                    NodeKind::Constructor
                } else {
                    NodeKind::Method
                }
            } else {
                node_kind.clone()
            };

            let qualified_name = format!("{}::{}", state.qualified_prefix(), display_name);
            let id = generate_node_id(&state.file_path, &actual_kind, &display_name, start_line);

            let visibility = if state.in_implementation {
                Visibility::Private
            } else {
                Visibility::Pub
            };

            let metrics = count_complexity(node, &PASCAL_COMPLEXITY, &state.source);
            let graph_node = Node {
                id: id.clone(),
                kind: actual_kind,
                name: display_name,
                qualified_name,
                file_path: state.file_path.clone(),
                start_line,
                attrs_start_line: start_line,
                end_line,
                start_column,
                end_column,
                signature: Some(decl_text.trim().trim_end_matches(';').trim().to_string()),
                docstring,
                visibility,
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

            // If this is a method implementation, create a Receives edge to the class.
            if is_method && !class_name.is_empty() {
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: id.clone(),
                    reference_name: class_name,
                    reference_kind: EdgeKind::Receives,
                    line: start_line,
                    column: start_column,
                    file_path: state.file_path.clone(),
                });
            }

            // Extract call sites from the block.
            if let Some(block_node) = block {
                Self::extract_call_sites(state, block_node, &id);
            }
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Find the module name from a program or unit node.
    fn find_module_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(mod_name) = Self::find_child_by_kind(node, "moduleName") {
            if let Some(ident) = Self::find_child_by_kind(mod_name, "identifier") {
                return state.node_text(ident);
            }
            return state.node_text(mod_name);
        }
        "<unknown>".to_string()
    }

    /// Determine the procedure kind from a declProc node.
    /// Returns (`kind_str`, `NodeKind`).
    fn determine_proc_kind(node: TsNode<'_>) -> (&'static str, NodeKind) {
        if Self::find_child_by_kind(node, "kConstructor").is_some() {
            ("constructor", NodeKind::Constructor)
        } else if Self::find_child_by_kind(node, "kDestructor").is_some() {
            ("destructor", NodeKind::Method)
        } else if Self::find_child_by_kind(node, "kProcedure").is_some() {
            ("procedure", NodeKind::Procedure)
        } else if Self::find_child_by_kind(node, "kFunction").is_some() {
            ("function", NodeKind::Function)
        } else {
            ("unknown", NodeKind::Function)
        }
    }

    /// Find the procedure/function name from a declProc node.
    /// Handles both simple names and dotted names (e.g., TMyClass.DoSomething).
    fn find_proc_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // Check for genericDot first (dotted name like TMyClass.DoSomething).
        if let Some(dot_node) = Self::find_child_by_kind(node, "genericDot") {
            return state.node_text(dot_node);
        }
        // Otherwise look for a simple identifier.
        Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
    }

    /// Parse a dotted name from a declProc node.
    /// Returns (`is_method`, `class_name`, `method_name`).
    fn parse_dotted_name(state: &ExtractionState, decl_node: TsNode<'_>) -> (bool, String, String) {
        if let Some(dot_node) = Self::find_child_by_kind(decl_node, "genericDot") {
            // genericDot has identifiers separated by kDot.
            let mut identifiers = Vec::new();
            let mut cursor = dot_node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "identifier" {
                        identifiers.push(state.node_text(child));
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            if identifiers.len() >= 2 {
                let class_name = identifiers[0].clone();
                let method_name = identifiers[identifiers.len() - 1].clone();
                return (true, class_name, method_name);
            }
        }
        (false, String::new(), String::new())
    }

    /// Recursively find exprCall nodes and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "exprCall" => {
                        // The callee is the first child (identifier or expression).
                        if let Some(callee) = child.named_child(0) {
                            let callee_name = state.node_text(callee);
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: callee_name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        // Recurse into the call expression for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    "statement" => {
                        // Check if statement has a direct identifier child that looks like
                        // a procedure call without parentheses (e.g., `DoOtherThing;`).
                        // In tree-sitter-pascal, a bare procedure call appears as
                        // statement > identifier > ;
                        let first_child = child.named_child(0);
                        if let Some(fc) = first_child {
                            if fc.kind() == "identifier" {
                                // Check there's no exprCall - just a bare identifier.
                                let has_call =
                                    Self::find_child_by_kind(child, "exprCall").is_some();
                                if !has_call {
                                    let callee_name = state.node_text(fc);
                                    // Skip some keywords that aren't calls.
                                    if !matches!(
                                        callee_name.as_str(),
                                        "inherited" | "break" | "continue" | "exit"
                                    ) {
                                        state.unresolved_refs.push(UnresolvedRef {
                                            from_node_id: fn_node_id.to_string(),
                                            reference_name: callee_name,
                                            reference_kind: EdgeKind::Calls,
                                            line: fc.start_position().row as u32,
                                            column: fc.start_position().column as u32,
                                            file_path: state.file_path.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
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

    /// Extract docstrings from preceding comment nodes.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            if sibling.kind() == "comment" {
                let text = state.node_text(sibling);
                comments.push(text);
                current = sibling.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        // Comments are collected in reverse order (closest first).
        comments.reverse();
        let cleaned: Vec<String> = comments.iter().map(|c| Self::clean_comment(c)).collect();
        let result = cleaned.join("\n").trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Strip comment markers from a single Pascal comment text.
    fn clean_comment(comment: &str) -> String {
        let trimmed = comment.trim();
        if let Some(stripped) = trimmed.strip_prefix("//") {
            // Line comment.
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if trimmed.starts_with('{') && trimmed.ends_with('}') {
            // Brace comment { ... }.
            let inner = &trimmed[1..trimmed.len() - 1];
            inner.trim().to_string()
        } else if trimmed.starts_with("(*") && trimmed.ends_with("*)") {
            // Old-style comment (* ... *).
            let inner = &trimmed[2..trimmed.len() - 2];
            inner
                .lines()
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Find the first named child of a node with a given kind.
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

impl crate::extraction::LanguageExtractor for PascalExtractor {
    fn extensions(&self) -> &[&str] {
        &["pas", "pp", "dpr", "lpr"]
    }

    fn language_name(&self) -> &'static str {
        "Pascal"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        PascalExtractor::extract_pascal(file_path, source)
    }
}
