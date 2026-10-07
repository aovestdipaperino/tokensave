/// Tree-sitter based C# source code extractor.
///
/// Parses C# source files and emits nodes and edges for the code graph.
use std::collections::HashMap;
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, CSHARP_COMPLEXITY};
use crate::extraction::ts_state::ExtractionState;
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from C# source files using tree-sitter.
pub struct CSharpExtractor;

impl CSharpExtractor {
    /// Extract code graph nodes and edges from a C# source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the C# source code to parse.
    pub fn extract_csharp(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return state.build_result(start);
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

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        state.build_result(start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("c_sharp");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load C# grammar: {e}"))?;
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
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                Self::visit_namespace(state, node);
            }
            "using_directive" => Self::visit_using(state, node),
            "class_declaration" => Self::visit_class(state, node),
            "struct_declaration" => Self::visit_struct(state, node),
            "interface_declaration" => Self::visit_interface(state, node),
            "enum_declaration" => Self::visit_enum(state, node),
            "method_declaration" => Self::visit_method(state, node),
            "constructor_declaration" => Self::visit_constructor(state, node),
            "property_declaration" => Self::visit_property(state, node),
            "indexer_declaration" => Self::visit_indexer(state, node),
            "field_declaration" => Self::visit_field(state, node),
            "record_declaration" | "record_struct_declaration" => Self::visit_record(state, node),
            "delegate_declaration" => Self::visit_delegate(state, node),
            "event_declaration" | "event_field_declaration" => Self::visit_event(state, node),
            "attribute_list" => Self::visit_attribute_list(state, node),
            _ => {
                // Recurse into children for any unhandled node types.
                Self::visit_children(state, node);
            }
        }
    }

    /// Extract a namespace declaration.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| {
            // For file-scoped namespaces or qualified names
            Self::extract_qualified_name_child(state, node)
                .unwrap_or_else(|| "<anonymous>".to_string())
        });
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Namespace, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Namespace,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("namespace {name}")),
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Visit namespace body.
        state.node_stack.push((name, id));
        // For braced namespaces, visit the body (declaration_list)
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        } else {
            // For file-scoped namespace, visit remaining children
            Self::visit_children(state, node);
        }
        state.node_stack.pop();
    }

    /// Extract a using directive as a Use node.
    fn visit_using(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Strip "using " prefix and trailing ";"
        let path = text
            .trim()
            .strip_prefix("using ")
            .unwrap_or(&text)
            .trim()
            .strip_prefix("static ")
            .unwrap_or(text.trim().strip_prefix("using ").unwrap_or(&text).trim())
            .trim_end_matches(';')
            .trim()
            .to_string();

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), path);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, &path, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: path.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(text.trim().to_string()),
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Unresolved Uses reference.
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id,
            reference_name: path,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Extract a class declaration.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if state.class_depth > 0 {
            NodeKind::InnerClass
        } else {
            NodeKind::Class
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind,
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
            visibility,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Extract attributes on this class.
        Self::extract_attributes_from_declaration(state, node, &id);

        // Extract base list (extends/implements).
        Self::extract_base_list(state, node, &id, true);

        // Visit class body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a struct declaration.
    fn visit_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, &name, start_line);

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
            visibility,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Extract base list (struct can implement interfaces).
        Self::extract_base_list(state, node, &id, false);

        // Visit struct body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an interface declaration.
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Interface, &name, start_line);

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
            visibility,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Extract base list (interfaces can extend other interfaces).
        Self::extract_base_list(state, node, &id, false);

        // Visit interface body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an enum declaration with its members.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Enum, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Enum,
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
            visibility,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Extract enum members from the body.
        state.node_stack.push((name, id));
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_enum_members(state, body);
        }
        state.node_stack.pop();
    }

    /// Extract enum members from an `enum_member_declaration_list`.
    fn extract_enum_members(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_member_declaration" {
                    Self::extract_single_enum_member(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single enum member as an `EnumVariant` node.
    fn extract_single_enum_member(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| {
            // Fallback: try to get the identifier child directly
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "identifier" {
                        return state.node_text(child);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            "<anonymous>".to_string()
        });
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::EnumVariant, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::EnumVariant,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).trim().to_string()),
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

        // Contains edge from parent (the enum).
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

    /// Extract a method declaration.
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let is_async = Self::has_modifier(node, state, "async");

        // If inside a class/struct, it's a Method; otherwise Function
        let kind = if state.class_depth > 0 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &CSHARP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind,
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
            visibility,
            is_async,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Extract attributes on this method.
        Self::extract_attributes_from_declaration(state, node, &id);

        // Extract call sites from the method body.
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
            Self::extract_typed_calls(state, node, body, &id);
        }
    }

    /// Extract a constructor declaration.
    fn visit_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &CSHARP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Constructor,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature,
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
            cognitive_complexity: metrics.cognitive_complexity,
            distinct_operators: metrics.distinct_operators,
            distinct_operands: metrics.distinct_operands,
            total_operators: metrics.total_operators,
            total_operands: metrics.total_operands,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Extract attributes on this constructor.
        Self::extract_attributes_from_declaration(state, node, &id);

        // Extract call sites from the constructor body.
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
            Self::extract_typed_calls(state, node, body, &id);
        }
    }

    /// Extract a property declaration.
    fn visit_property(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let type_str = node
            .child_by_field_name("type")
            .map(|n| state.node_text(n))
            .unwrap_or_default();
        let sig = format!("{type_str} {name}");
        Self::push_property_node(state, node, name, sig);
    }

    /// Extract an indexer declaration (`T this[int i] { get; set; }`) as a
    /// `CSharpProperty` named `this`, so calls in its accessors have a
    /// caller (#637).
    fn visit_indexer(state: &mut ExtractionState, node: TsNode<'_>) {
        let type_str = node
            .child_by_field_name("type")
            .map(|n| state.node_text(n))
            .unwrap_or_default();
        let params = node
            .child_by_field_name("parameters")
            .map(|n| state.node_text(n))
            .unwrap_or_default();
        let sig = format!("{type_str} this{params}");
        Self::push_property_node(state, node, "this".to_string(), sig);
    }

    /// Push a `CSharpProperty` node for a property or indexer declaration,
    /// its `Contains` edge, and the call sites in its accessor bodies.
    fn push_property_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        name: String,
        sig: String,
    ) {
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::CSharpProperty,
            &name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::CSharpProperty,
            name,
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        Self::extract_member_call_sites(state, node, &id);
    }

    /// Extract field declarations.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let visibility = Self::extract_csharp_visibility(node, state);

        // In C# tree-sitter, field_declaration contains a variable_declaration
        // which has variable_declarator children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declaration" {
                    Self::extract_variable_declarators(state, child, &visibility, node);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract variable declarators from a `variable_declaration` node.
    fn extract_variable_declarators(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        visibility: &Visibility,
        field_decl: TsNode<'_>,
    ) {
        let start_line = field_decl.start_position().row as u32;
        let end_line = field_decl.end_position().row as u32;
        let start_column = field_decl.start_position().column as u32;
        let end_column = field_decl.end_position().column as u32;
        let signature_text = state.node_text(field_decl).trim().to_string();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    let field_name = child
                        .child_by_field_name("name")
                        .or_else(|| {
                            // Try direct identifier child
                            let mut inner = child.walk();
                            if inner.goto_first_child() {
                                loop {
                                    let ic = inner.node();
                                    if ic.kind() == "identifier" {
                                        return Some(ic);
                                    }
                                    if !inner.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                            None
                        })
                        .map_or_else(|| state.node_text(child), |n| state.node_text(n));

                    let qualified_name = format!("{}::{}", state.qualified_prefix(), field_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::Field,
                        &field_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::Field,
                        name: field_name,
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(signature_text.clone()),
                        docstring: None,
                        visibility: visibility.clone(),
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

                    // Contains edge from parent.
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a record declaration.
    fn visit_record(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let docstring = Self::extract_xml_docstring(state, node);
        let signature = Some(Self::extract_declaration_signature(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Record, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Record,
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
            visibility,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Visit record body if present.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a delegate declaration.
    fn visit_delegate(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_csharp_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Delegate, &name, start_line);
        let signature_text = state
            .node_text(node)
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_string();

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Delegate,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(signature_text),
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
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: state.timestamp,
            parent_id: None,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent.
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

    /// Extract an event declaration.
    fn visit_event(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let name = Self::extract_event_name(state, node).unwrap_or_else(|| {
            // Fallback: parse from text
            text.split_whitespace()
                .last()
                .unwrap_or("<anonymous>")
                .trim_end_matches(';')
                .to_string()
        });

        let visibility = Self::extract_csharp_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Event, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Event,
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
            visibility,
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

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        // Calls in `add`/`remove` accessor bodies (#637). An
        // `event_field_declaration` has no accessors, so this is a no-op there.
        Self::extract_member_call_sites(state, node, &id);
    }

    /// Extract attribute lists as `AnnotationUsage` nodes with Annotates edges.
    fn visit_attribute_list(state: &mut ExtractionState, node: TsNode<'_>) {
        // Find the next declaration sibling - that's the declaration this attribute annotates.
        let target_id = Self::find_next_declaration_id(state, node);

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute" {
                    let attr_name = Self::extract_attribute_name(state, child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), attr_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::AnnotationUsage,
                        &attr_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::AnnotationUsage,
                        name: attr_name.clone(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(format!("[{}]", state.node_text(child).trim())),
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
                        cognitive_complexity: 0,
                        distinct_operators: 0,
                        distinct_operands: 0,
                        total_operators: 0,
                        total_operands: 0,
                        updated_at: state.timestamp,
                        parent_id: None,
                    };
                    state.nodes.push(graph_node);

                    // Annotates unresolved ref.
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: id.clone(),
                        reference_name: attr_name,
                        reference_kind: EdgeKind::Annotates,
                        line: start_line,
                        column: start_column,
                        file_path: state.file_path.clone(),
                    });

                    // If we found the target, create a direct Annotates edge.
                    if let Some(ref tid) = target_id {
                        state.edges.push(Edge {
                            source: id,
                            target: tid.clone(),
                            kind: EdgeKind::Annotates,
                            line: Some(start_line),
                            resolved_by: None,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the name of a node by looking for a "name" field child.
    fn extract_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        node.child_by_field_name("name").map(|n| state.node_text(n))
    }

    /// Try to extract a `qualified_name` child for namespace declarations.
    fn extract_qualified_name_child(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "qualified_name" || child.kind() == "identifier" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Extract the event name from an event declaration.
    fn extract_event_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Try the "name" field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(state.node_text(name_node));
        }
        // For event_field_declaration, look for variable_declaration > variable_declarator
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declaration" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let ic = inner.node();
                            if ic.kind() == "variable_declarator" {
                                if let Some(name_node) = ic.child_by_field_name("name") {
                                    return Some(state.node_text(name_node));
                                }
                                // Try identifier child
                                let mut deep = ic.walk();
                                if deep.goto_first_child() {
                                    loop {
                                        let dc = deep.node();
                                        if dc.kind() == "identifier" {
                                            return Some(state.node_text(dc));
                                        }
                                        if !deep.goto_next_sibling() {
                                            break;
                                        }
                                    }
                                }
                                return Some(state.node_text(ic));
                            }
                            if !inner.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Extract C# visibility from modifier keywords.
    fn extract_csharp_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifier" {
                    let text = state.node_text(child);
                    match text.as_str() {
                        "public" => return Visibility::Pub,
                        "private" => return Visibility::Private,
                        "internal" => return Visibility::PubCrate,
                        "protected" => return Visibility::PubSuper,
                        _ => {}
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // No modifier -> Private for class members
        Visibility::Private
    }

    /// Check if a node has a specific modifier keyword.
    fn has_modifier(node: TsNode<'_>, state: &ExtractionState, modifier: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifier" {
                    let text = state.node_text(child);
                    if text == modifier {
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

    /// Extract the declaration signature (text from start up to the opening `{`).
    fn extract_declaration_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            text[..brace_pos].trim().to_string()
        } else {
            text.trim_end_matches(';').trim().to_string()
        }
    }

    /// Extract XML doc comments (/// ...) preceding a declaration.
    fn extract_xml_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_sibling();
        while let Some(sibling) = current {
            let text = state.node_text(sibling);
            let trimmed = text.trim();
            if trimmed.starts_with("///") {
                comments.push(trimmed.to_string());
                current = sibling.prev_sibling();
            } else if sibling.kind() == "attribute_list" {
                // Skip attribute lists between comments and the declaration
                current = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if comments.is_empty() {
            return None;
        }

        // Comments are collected in reverse order (bottom-up), so reverse them.
        comments.reverse();

        // Clean the comments: strip ///, strip XML tags for clean text.
        let cleaned: Vec<String> = comments
            .iter()
            .map(|line| {
                let stripped = line.strip_prefix("///").unwrap_or(line).trim();
                // Strip XML tags like <summary>, </summary>, <param>, etc.
                Self::strip_xml_tags(stripped)
            })
            .filter(|s| !s.is_empty())
            .collect();

        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned.join("\n").trim().to_string())
        }
    }

    /// Strip XML tags from a string.
    fn strip_xml_tags(s: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        for c in s.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(c);
            }
        }
        result.trim().to_string()
    }

    /// Extract base list (extends/implements) from a type declaration.
    /// For classes, the first base type is Extends, the rest are Implements.
    /// For structs/interfaces, all are Implements.
    fn extract_base_list(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        type_id: &str,
        is_class: bool,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "base_list" {
                    Self::extract_base_types(state, child, type_id, is_class);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract types from a `base_list` node.
    fn extract_base_types(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        type_id: &str,
        is_class: bool,
    ) {
        let mut is_first = true;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "identifier"
                        || child.kind() == "generic_name"
                        || child.kind() == "qualified_name")
                {
                    // `IProducer<TRecord>` names the type `IProducer`; the
                    // type arguments would keep the ref from ever matching
                    // the declaration (#643).
                    let type_name = strip_type_arguments(&state.node_text(child));
                    // A class's first base may be a class or an interface;
                    // the syntax cannot tell them apart, so it is recorded as
                    // `Extends` and the resolver turns it into `Implements`
                    // when the target is an interface (#643).
                    let edge_kind = if is_class && is_first {
                        is_first = false;
                        EdgeKind::Extends
                    } else {
                        EdgeKind::Implements
                    };

                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: type_id.to_string(),
                        reference_name: type_name,
                        reference_kind: edge_kind,
                        line: child.start_position().row as u32,
                        column: child.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract attributes from a declaration node's `attribute_list` children.
    /// Creates `AnnotationUsage` nodes and Annotates edges pointing to the target declaration.
    fn extract_attributes_from_declaration(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute_list" {
                    Self::visit_attribute_list_for_target(state, child, target_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit an `attribute_list` node and create `AnnotationUsage` nodes targeting a known declaration.
    fn visit_attribute_list_for_target(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute" {
                    let attr_name = Self::extract_attribute_name(state, child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), attr_name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::AnnotationUsage,
                        &attr_name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::AnnotationUsage,
                        name: attr_name,
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(state.node_text(child).trim().to_string()),
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

                    // Annotates edge from annotation to target declaration.
                    state.edges.push(Edge {
                        source: id.clone(),
                        target: target_id.to_string(),
                        kind: EdgeKind::Annotates,
                        line: Some(start_line),
                        resolved_by: None,
                    });

                    // Contains edge from parent.
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the attribute name from an attribute node.
    fn extract_attribute_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(name_node) = node.child_by_field_name("name") {
            return state.node_text(name_node);
        }
        // Fallback: find the first named child that is an identifier
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" || child.kind() == "qualified_name" {
                    return state.node_text(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        state.node_text(node).trim().to_string()
    }

    /// Find the next declaration sibling after an `attribute_list` and compute its ID.
    fn find_next_declaration_id(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut current = node.next_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "attribute_list" => {
                    current = sibling.next_named_sibling();
                }
                "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "method_declaration"
                | "constructor_declaration"
                | "property_declaration"
                | "field_declaration"
                | "record_declaration"
                | "record_struct_declaration"
                | "delegate_declaration"
                | "event_declaration"
                | "event_field_declaration" => {
                    let name = Self::extract_name(state, sibling)
                        .unwrap_or_else(|| "<anonymous>".to_string());
                    let kind = match sibling.kind() {
                        "class_declaration" => {
                            if state.class_depth > 0 {
                                NodeKind::InnerClass
                            } else {
                                NodeKind::Class
                            }
                        }
                        "struct_declaration" => NodeKind::Struct,
                        "interface_declaration" => NodeKind::Interface,
                        "enum_declaration" => NodeKind::Enum,
                        "method_declaration" => {
                            if state.class_depth > 0 {
                                NodeKind::Method
                            } else {
                                NodeKind::Function
                            }
                        }
                        "constructor_declaration" => NodeKind::Constructor,
                        "property_declaration" => NodeKind::CSharpProperty,
                        "record_declaration" | "record_struct_declaration" => NodeKind::Record,
                        "delegate_declaration" => NodeKind::Delegate,
                        "event_declaration" | "event_field_declaration" => NodeKind::Event,
                        _ => return None,
                    };
                    let start_line = sibling.start_position().row as u32;
                    return Some(generate_node_id(&state.file_path, &kind, &name, start_line));
                }
                _ => return None,
            }
        }
        None
    }

    /// Record call sites in the bodies of a property, indexer, or event (#637).
    ///
    /// Walks each accessor's body (`get`/`set`/`init`/`add`/`remove`, block or
    /// expression-bodied) and the declaration's `value` (an expression-bodied
    /// member's `=> expr`, or a property initializer `= expr`), attributing
    /// every call to `member_id`. Attributes and the declared type are left
    /// out, so `[Display(Name = nameof(X))]` is not recorded as a call.
    fn extract_member_call_sites(state: &mut ExtractionState, node: TsNode<'_>, member_id: &str) {
        if let Some(accessors) = node.child_by_field_name("accessors") {
            let mut cursor = accessors.walk();
            for accessor in accessors.named_children(&mut cursor) {
                if accessor.kind() != "accessor_declaration" {
                    continue;
                }
                if let Some(body) = accessor.child_by_field_name("body") {
                    Self::scan_call_site(state, body, member_id);
                }
            }
        }
        if let Some(value) = node.child_by_field_name("value") {
            Self::scan_call_site(state, value, member_id);
        }
    }

    /// Recursively find `invocation_expression` nodes among `node`'s
    /// descendants and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::scan_call_site(state, cursor.node(), fn_node_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Record `node` itself if it is a call site, then recurse into it.
    fn scan_call_site(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        match node.kind() {
            "invocation_expression" => {
                let callee_name = Self::extract_invocation_name(state, node);
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: fn_node_id.to_string(),
                    reference_name: callee_name,
                    reference_kind: EdgeKind::Calls,
                    line: node.start_position().row as u32,
                    column: node.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
                // Recurse for nested calls inside arguments.
                Self::extract_call_sites(state, node, fn_node_id);
            }
            "object_creation_expression" => {
                let type_name = Self::extract_object_creation_type(state, node);
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: fn_node_id.to_string(),
                    reference_name: format!("new {type_name}"),
                    reference_kind: EdgeKind::Calls,
                    line: node.start_position().row as u32,
                    column: node.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
                Self::extract_call_sites(state, node, fn_node_id);
            }
            // Skip nested declarations.
            "method_declaration" | "constructor_declaration" | "class_declaration" => {}
            _ => {
                Self::extract_call_sites(state, node, fn_node_id);
            }
        }
    }

    /// Extract the name from an `invocation_expression` node.
    ///
    /// Type arguments are not part of the name: `f.Create<T>(x)` records
    /// `f.Create`, so the trailing segment can match the declaration (#642).
    fn extract_invocation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // invocation_expression: function + argument_list
        if let Some(func_node) = node.child_by_field_name("function") {
            let name = match func_node.kind() {
                "member_access_expression" => match (
                    func_node.child_by_field_name("expression"),
                    func_node.child_by_field_name("name"),
                ) {
                    (Some(recv), Some(name)) => format!(
                        "{}.{}",
                        state.node_text(recv),
                        Self::simple_member_name(state, name)
                    ),
                    _ => state.node_text(func_node),
                },
                "generic_name" => Self::simple_member_name(state, func_node),
                _ => state.node_text(func_node),
            };
            // `global::A.B()` is a dotted call, not a `::` type expression.
            return name
                .strip_prefix("global::")
                .map_or(name.clone(), str::to_string);
        }
        // Fallback: first child
        if let Some(first) = node.child(0) {
            if first.kind() != "argument_list" {
                return state.node_text(first);
            }
        }
        state.node_text(node)
    }

    /// Extract the type name from an `object_creation_expression`.
    fn extract_object_creation_type(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(type_node) = node.child_by_field_name("type") {
            return state.node_text(type_node);
        }
        // Fallback: look for type identifier children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named()
                    && (child.kind() == "identifier"
                        || child.kind() == "generic_name"
                        || child.kind() == "qualified_name")
                {
                    return state.node_text(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        "<unknown>".to_string()
    }
}

/// Variable name to the static type expression of its value: a class name,
/// optionally followed by `::member` and `::method()` steps that the resolver
/// evaluates against the indexed declarations (#642).
type VarTypes = HashMap<String, String>;

/// Steps beyond which a receiver chain is not worth typing.
const MAX_TYPE_STEPS: usize = 8;

/// Typed-receiver calls (#642).
///
/// For `recv.Method(...)` whose receiver has a static type the extractor can
/// see, a `Type[::step]*::Method` Calls ref is recorded at the same position as
/// the `recv.Method` ref. The resolver evaluates it against the indexed
/// classes; when it resolves, the receiver-qualified sibling is dropped, and
/// when the evidence runs out (an unindexed type, an extension method) the
/// sibling decides as before.
///
/// Receiver types come from the enclosing type's fields, properties and
/// primary-constructor parameters, the method's parameters, and its locals:
/// declared types, and for `var` the initializer's type (`new T()`, casts,
/// `as`, and a call or member read whose declared type the resolver looks up).
impl CSharpExtractor {
    fn extract_typed_calls(
        state: &mut ExtractionState,
        decl: TsNode<'_>,
        body: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let owner = Self::enclosing_type_decl(decl);
        let self_type = owner
            .and_then(|c| c.child_by_field_name("name"))
            .map(|n| state.node_text(n));
        let mut vars = VarTypes::new();
        if let Some(owner) = owner {
            Self::collect_member_types(state, owner, &mut vars);
        }
        if let Some(params) = decl.child_by_field_name("parameters") {
            Self::collect_parameter_types(state, params, &mut vars);
        }
        Self::collect_local_types(state, body, self_type.as_deref(), &mut vars);
        Self::emit_typed_calls(state, body, fn_node_id, self_type.as_deref(), &vars);
    }

    fn enclosing_type_decl(node: TsNode<'_>) -> Option<TsNode<'_>> {
        let mut cur = node.parent();
        while let Some(n) = cur {
            if matches!(
                n.kind(),
                "class_declaration"
                    | "struct_declaration"
                    | "record_declaration"
                    | "record_struct_declaration"
                    | "interface_declaration"
            ) {
                return Some(n);
            }
            cur = n.parent();
        }
        None
    }

    /// The declared type of a `type` node, as a bare class name.
    fn declared_type(state: &ExtractionState, ty: TsNode<'_>) -> Option<String> {
        if ty.kind() == "implicit_type" {
            return None;
        }
        let text = state.node_text(ty);
        crate::resolution::csharp_type_name(&text).map(str::to_string)
    }

    /// Fields, properties and primary-constructor parameters of `owner`.
    fn collect_member_types(state: &ExtractionState, owner: TsNode<'_>, vars: &mut VarTypes) {
        let mut cursor = owner.walk();
        for child in owner.children(&mut cursor) {
            if child.kind() == "parameter_list" {
                Self::collect_parameter_types(state, child, vars);
            }
        }
        let Some(body) = owner.child_by_field_name("body") else {
            return;
        };
        let mut cursor = body.walk();
        for member in body.children(&mut cursor) {
            match member.kind() {
                "field_declaration" => {
                    let mut inner = member.walk();
                    for decl in member.children(&mut inner) {
                        if decl.kind() == "variable_declaration" {
                            Self::collect_declaration(state, decl, None, vars);
                        }
                    }
                }
                "property_declaration" => {
                    if let (Some(ty), Some(name)) = (
                        member.child_by_field_name("type"),
                        member.child_by_field_name("name"),
                    ) {
                        if let Some(t) = Self::declared_type(state, ty) {
                            vars.insert(state.node_text(name), t);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_parameter_types(state: &ExtractionState, params: TsNode<'_>, vars: &mut VarTypes) {
        let mut cursor = params.walk();
        for p in params.children(&mut cursor) {
            if p.kind() != "parameter" {
                continue;
            }
            if let (Some(ty), Some(name)) =
                (p.child_by_field_name("type"), p.child_by_field_name("name"))
            {
                if let Some(t) = Self::declared_type(state, ty) {
                    vars.insert(state.node_text(name), t);
                }
            }
        }
    }

    /// One `variable_declaration`: the declared type, or for `var` each
    /// declarator's initializer type.
    fn collect_declaration(
        state: &ExtractionState,
        decl: TsNode<'_>,
        self_type: Option<&str>,
        vars: &mut VarTypes,
    ) {
        let declared = decl
            .child_by_field_name("type")
            .and_then(|t| Self::declared_type(state, t));
        let mut cursor = decl.walk();
        for d in decl.children(&mut cursor) {
            if d.kind() != "variable_declarator" {
                continue;
            }
            let Some(name) = d.child_by_field_name("name") else {
                continue;
            };
            let ty = declared.clone().or_else(|| {
                let mut c = d.walk();
                let init = d
                    .named_children(&mut c)
                    .filter(|n| n.id() != name.id())
                    .last()?;
                Self::expr_type(state, init, self_type, vars)
            });
            if let Some(ty) = ty {
                vars.insert(state.node_text(name), ty);
            }
        }
    }

    /// Locals declared anywhere in `node`, in source order, skipping nested
    /// type and member declarations.
    fn collect_local_types(
        state: &ExtractionState,
        node: TsNode<'_>,
        self_type: Option<&str>,
        vars: &mut VarTypes,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "variable_declaration" => {
                    Self::collect_declaration(state, child, self_type, vars);
                }
                "foreach_statement" => {
                    if let (Some(ty), Some(left)) = (
                        child.child_by_field_name("type"),
                        child.child_by_field_name("left"),
                    ) {
                        if left.kind() == "identifier" {
                            if let Some(t) = Self::declared_type(state, ty) {
                                vars.insert(state.node_text(left), t);
                            }
                        }
                    }
                }
                "method_declaration" | "constructor_declaration" | "class_declaration" => {
                    continue;
                }
                _ => {}
            }
            Self::collect_local_types(state, child, self_type, vars);
        }
    }

    /// The method name of a member access or generic call, without type
    /// arguments.
    fn simple_member_name(state: &ExtractionState, name: TsNode<'_>) -> String {
        if name.kind() == "generic_name" {
            let mut cursor = name.walk();
            let id = name
                .named_children(&mut cursor)
                .find(|c| c.kind() == "identifier");
            if let Some(id) = id {
                return state.node_text(id);
            }
        }
        state.node_text(name)
    }

    /// The static type expression of a value expression, when it can be told.
    fn expr_type(
        state: &ExtractionState,
        expr: TsNode<'_>,
        self_type: Option<&str>,
        vars: &VarTypes,
    ) -> Option<String> {
        let ty = match expr.kind() {
            "object_creation_expression" | "cast_expression" => {
                Self::declared_type(state, expr.child_by_field_name("type")?)?
            }
            "as_expression" => Self::declared_type(state, expr.child_by_field_name("right")?)?,
            "parenthesized_expression" => {
                let mut c = expr.walk();
                let inner = expr.named_children(&mut c).next()?;
                Self::expr_type(state, inner, self_type, vars)?
            }
            "await_expression" => {
                let mut c = expr.walk();
                let inner = expr.named_children(&mut c).next()?;
                if inner.kind() != "invocation_expression" {
                    return None;
                }
                Self::invocation_type(state, inner, self_type, vars, true)?
            }
            "invocation_expression" => Self::invocation_type(state, expr, self_type, vars, false)?,
            "identifier" => vars.get(&state.node_text(expr))?.clone(),
            "this" => self_type?.to_string(),
            "member_access_expression" => {
                let recv = expr.child_by_field_name("expression")?;
                let name = expr.child_by_field_name("name")?;
                let recv_ty = Self::receiver_type(state, recv, self_type, vars)?;
                format!("{recv_ty}::{}", state.node_text(name))
            }
            _ => return None,
        };
        (ty.matches("::").count() <= MAX_TYPE_STEPS).then_some(ty)
    }

    /// The return type expression of a call: `Recv::Method()`, or
    /// `Recv::await Method()` when the call is awaited.
    fn invocation_type(
        state: &ExtractionState,
        call: TsNode<'_>,
        self_type: Option<&str>,
        vars: &VarTypes,
        awaited: bool,
    ) -> Option<String> {
        let func = call.child_by_field_name("function")?;
        let (recv_ty, name) = match func.kind() {
            "member_access_expression" => {
                let recv = func.child_by_field_name("expression")?;
                let name = func.child_by_field_name("name")?;
                (
                    Self::receiver_type(state, recv, self_type, vars)?,
                    Self::simple_member_name(state, name),
                )
            }
            "identifier" | "generic_name" => (
                self_type?.to_string(),
                Self::simple_member_name(state, func),
            ),
            _ => return None,
        };
        let await_prefix = if awaited { "await " } else { "" };
        Some(format!("{recv_ty}::{await_prefix}{name}()"))
    }

    /// The static type expression of a call receiver. An identifier that is
    /// no known variable and starts upper-case is read as a class name, for a
    /// static call (`Factory.Create()`).
    fn receiver_type(
        state: &ExtractionState,
        recv: TsNode<'_>,
        self_type: Option<&str>,
        vars: &VarTypes,
    ) -> Option<String> {
        match recv.kind() {
            "identifier" => {
                let name = state.node_text(recv);
                if let Some(ty) = vars.get(&name) {
                    return Some(ty.clone());
                }
                name.chars()
                    .next()
                    .is_some_and(char::is_uppercase)
                    .then_some(name)
            }
            "generic_name" => Self::declared_type(state, recv),
            _ => Self::expr_type(state, recv, self_type, vars),
        }
    }

    fn emit_typed_calls(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        fn_node_id: &str,
        self_type: Option<&str>,
        vars: &VarTypes,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "method_declaration" | "constructor_declaration" | "class_declaration" => continue,
                "invocation_expression" => {
                    let typed = child
                        .child_by_field_name("function")
                        .filter(|f| f.kind() == "member_access_expression")
                        .and_then(|f| {
                            let recv = f.child_by_field_name("expression")?;
                            let name = f.child_by_field_name("name")?;
                            let ty = Self::receiver_type(state, recv, self_type, vars)?;
                            Some(format!("{ty}::{}", Self::simple_member_name(state, name)))
                        });
                    if let Some(reference_name) = typed {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                }
                _ => {}
            }
            Self::emit_typed_calls(state, child, fn_node_id, self_type, vars);
        }
    }
}

impl crate::extraction::LanguageExtractor for CSharpExtractor {
    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn language_name(&self) -> &'static str {
        "C#"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        CSharpExtractor::extract_csharp(file_path, source)
    }
}

/// Drops every `<...>` type-argument list from a C# type name, so
/// `App.IProducer<int>` becomes `App.IProducer` and `Outer<T>.Inner` becomes
/// `Outer.Inner`. Nested lists (`IMap<K, List<V>>`) are handled by depth.
fn strip_type_arguments(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0usize;
    for c in name.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 && !c.is_whitespace() => out.push(c),
            _ => {}
        }
    }
    out
}
