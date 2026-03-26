/// Tree-sitter based VB.NET source code extractor.
///
/// Parses VB.NET source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, ComplexityConfig};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Complexity configuration for VB.NET.
pub static VBNET_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "select_case_statement",
        "case_clause",
        "catch_clause",
    ],
    loop_types: &[
        "for_statement",
        "for_each_statement",
        "while_statement",
        "do_statement",
    ],
    return_types: &[
        "return_statement",
        "exit_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["invocation"],
    call_method_field: "target",
    assertion_names: &["Assert", "AreEqual", "IsTrue", "IsFalse"],
    macro_invocation_types: &[],
};

/// Extracts code graph nodes and edges from VB.NET source files using tree-sitter.
pub struct VbNetExtractor;

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
    /// Track nesting depth to distinguish inner classes from top-level classes.
    class_depth: usize,
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

impl VbNetExtractor {
    /// Extract code graph nodes and edges from a VB.NET source file.
    pub fn extract_vbnet(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = tree_sitter_vb_dotnet::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load VB.NET grammar: {e}"))?;
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
            "imports_statement" => Self::visit_imports(state, node),
            "type_declaration" => Self::visit_type_declaration(state, node),
            "ERROR" => Self::visit_error_node(state, node),
            _ => {
                // Recurse into children for any unhandled node types.
                Self::visit_children(state, node);
            }
        }
    }

    /// Visit a type_declaration node and dispatch to the inner block type.
    fn visit_type_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Collect docstring from preceding comment siblings.
        let docstring = Self::extract_xml_docstring(state, node);

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "class_block" => Self::visit_class(state, child, docstring.clone()),
                    "structure_block" => Self::visit_struct(state, child, docstring.clone()),
                    "interface_block" => Self::visit_interface(state, child, docstring.clone()),
                    "enum_block" => Self::visit_enum(state, child, docstring.clone()),
                    "module_block" => Self::visit_module(state, child, docstring.clone()),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Handle ERROR nodes that may be top-level `Const` declarations.
    fn visit_error_node(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let trimmed = text.trim();

        // Check if this looks like a Const declaration: "Const NAME As TYPE = VALUE"
        if trimmed.starts_with("Const ") || trimmed.starts_with("Public Const ")
            || trimmed.starts_with("Private Const ")
        {
            // Extract the constant name
            let after_const = if let Some(rest) = trimmed.strip_prefix("Public Const ") {
                rest
            } else if let Some(rest) = trimmed.strip_prefix("Private Const ") {
                rest
            } else {
                trimmed.strip_prefix("Const ").unwrap_or(trimmed)
            };

            let name = after_const
                .split_whitespace()
                .next()
                .unwrap_or("<anonymous>")
                .to_string();

            let start_line = node.start_position().row as u32;
            let end_line = node.end_position().row as u32;
            let start_column = node.start_position().column as u32;
            let end_column = node.end_position().column as u32;
            let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
            let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
            let docstring = Self::extract_xml_docstring(state, node);

            let visibility = if trimmed.starts_with("Private ") {
                Visibility::Private
            } else if trimmed.starts_with("Public ") {
                Visibility::Pub
            } else {
                Visibility::Private
            };

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
                signature: Some(trimmed.to_string()),
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
    }

    /// Extract an imports statement as a Use node.
    fn visit_imports(state: &mut ExtractionState, node: TsNode<'_>) {
        let path = node
            .child_by_field_name("namespace")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| {
                let text = state.node_text(node);
                text.trim()
                    .strip_prefix("Imports ")
                    .unwrap_or(&text)
                    .trim()
                    .to_string()
            });

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
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).trim().to_string()),
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
            reference_name: path,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Extract a class_block declaration.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>, docstring: Option<String>) {
        let name = Self::extract_block_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
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
        let signature = Self::extract_block_signature(state, node, "Class");

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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

        // Extract Inherits/Implements from text-based analysis of children.
        Self::extract_inherits_implements(state, node, &id);

        // Visit class body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        Self::visit_block_children(state, node);
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a structure_block declaration.
    fn visit_struct(state: &mut ExtractionState, node: TsNode<'_>, docstring: Option<String>) {
        let name = Self::extract_block_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Struct, &name, start_line);
        let signature = Self::extract_block_signature(state, node, "Structure");

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Struct,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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

        // Visit struct body.
        state.node_stack.push((name, id));
        state.class_depth += 1;
        Self::visit_block_children(state, node);
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an interface_block declaration.
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>, docstring: Option<String>) {
        let name = Self::extract_block_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Interface, &name, start_line);
        let signature = Self::extract_block_signature(state, node, "Interface");

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Interface,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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

        // Visit interface body (may contain method signatures).
        state.node_stack.push((name, id));
        state.class_depth += 1;
        Self::visit_block_children(state, node);
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an enum_block declaration with its members.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>, docstring: Option<String>) {
        let name = Self::extract_block_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
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
            end_line,
            start_column,
            end_column,
            signature: Some(format!("Enum {}", name)),
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

        // Extract enum members.
        state.node_stack.push((name, id));
        Self::extract_enum_members(state, node);
        state.node_stack.pop();
    }

    /// Extract a module_block declaration.
    fn visit_module(state: &mut ExtractionState, node: TsNode<'_>, docstring: Option<String>) {
        let name = Self::extract_block_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);
        let signature = Self::extract_block_signature(state, node, "Module");

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
        state.node_stack.push((name, id));
        state.class_depth += 1;
        Self::visit_block_children(state, node);
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit the children of a block node (class_block, structure_block, etc.),
    /// dispatching to the appropriate handler for each child.
    fn visit_block_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "method_declaration" => Self::visit_method(state, child),
                    "constructor_declaration" => Self::visit_constructor(state, child),
                    "property_declaration" => Self::visit_property(state, child),
                    "field_declaration" => Self::visit_field(state, child),
                    "type_declaration" => Self::visit_type_declaration(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a method declaration (Sub or Function).
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_vbnet_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        // If inside a class/struct/module, it's a Method; otherwise Function
        let kind = if state.class_depth > 0 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &VBNET_COMPLEXITY, &state.source);
        let signature = Self::extract_method_signature(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring: None,
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

        // Extract call sites from method body.
        Self::extract_call_sites_from_children(state, node, &id);
    }

    /// Extract a constructor declaration (Sub New).
    fn visit_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = "New".to_string();
        let visibility = Self::extract_vbnet_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &VBNET_COMPLEXITY, &state.source);

        let sig_text = state.node_text(node);
        let signature = sig_text
            .lines()
            .next()
            .map(|l| l.trim().to_string());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Constructor,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature,
            docstring: None,
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

        // Extract call sites from constructor body.
        Self::extract_call_sites_from_children(state, node, &id);
    }

    /// Extract a property declaration.
    fn visit_property(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::extract_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_vbnet_visibility(node, state);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Property, &name, start_line);

        // Extract type from as_clause
        let type_str = Self::extract_as_clause_type(state, node);
        let sig = if let Some(t) = &type_str {
            format!("Property {} As {}", name, t)
        } else {
            format!("Property {}", name)
        };

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Property,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(sig),
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

    /// Extract a field declaration.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let trimmed = text.trim();

        // Skip Inherits/Implements lines that the grammar mis-parses as field_declaration.
        if trimmed.starts_with("Inherits") || trimmed.starts_with("erializable")
            || trimmed.starts_with("Implements")
        {
            return;
        }

        let visibility = Self::extract_vbnet_visibility(node, state);

        // Look for variable_declarator children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    let field_name = child
                        .child_by_field_name("name")
                        .map(|n| state.node_text(n))
                        .unwrap_or_else(|| {
                            // Try direct identifier child
                            let mut inner = child.walk();
                            if inner.goto_first_child() {
                                loop {
                                    let ic = inner.node();
                                    if ic.kind() == "identifier" {
                                        return state.node_text(ic);
                                    }
                                    if !inner.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                            state.node_text(child)
                        });

                    // Skip field names that look like mis-parsed Inherits/Implements
                    if field_name == "Inherits" || field_name.starts_with("erializable") {
                        continue;
                    }

                    let start_line = node.start_position().row as u32;
                    let end_line = node.end_position().row as u32;
                    let start_column = node.start_position().column as u32;
                    let end_column = node.end_position().column as u32;
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
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(trimmed.to_string()),
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract enum members from an enum_block.
    fn extract_enum_members(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_member" {
                    Self::extract_single_enum_member(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single enum member as an EnumVariant node.
    fn extract_single_enum_member(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| {
                // Fallback: try identifier child
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (the enum).
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
    // Helper extraction methods
    // ----------------------------

    /// Extract the name from a block node (class_block, etc.) via the first identifier child.
    fn extract_block_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Block nodes in VB.NET grammar use `name` field
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(state.node_text(name_node));
        }
        // Fallback: find first identifier child
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Extract the name from a node via its "name" field.
    fn extract_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(state.node_text(name_node));
        }
        // Fallback: first identifier child
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    return Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Extract a simple block signature like "Class Foo" or "Module Bar".
    fn extract_block_signature(state: &ExtractionState, node: TsNode<'_>, keyword: &str) -> Option<String> {
        let text = state.node_text(node);
        // Take the first line which contains the declaration keyword and name.
        let first_line = text.lines().next().unwrap_or("").trim();
        if first_line.is_empty() {
            Some(format!("{} {}", keyword, Self::extract_block_name(state, node).unwrap_or_default()))
        } else {
            Some(first_line.to_string())
        }
    }

    /// Extract the method signature (first line of the method declaration).
    fn extract_method_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        text.lines().next().map(|l| l.trim().to_string())
    }

    /// Extract the type from an as_clause child.
    fn extract_as_clause_type(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "as_clause" {
                    // The as_clause contains "As" keyword and a type node
                    if let Some(type_node) = child.child_by_field_name("type") {
                        return Some(state.node_text(type_node));
                    }
                    // Fallback: get type child
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let ic = inner.node();
                            if ic.kind() == "type" {
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

    /// Extract VB.NET visibility from modifier keywords.
    fn extract_vbnet_visibility(node: TsNode<'_>, state: &ExtractionState) -> Visibility {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    // Look inside modifiers for individual modifier nodes
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let mc = inner.node();
                            if mc.kind() == "modifier" {
                                let text = state.node_text(mc);
                                match text.as_str() {
                                    "Public" => return Visibility::Pub,
                                    "Private" => return Visibility::Private,
                                    "Friend" => return Visibility::PubCrate,
                                    "Protected" => return Visibility::PubSuper,
                                    _ => {}
                                }
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
        // No modifier -> Private for class members
        Visibility::Private
    }

    /// Extract Inherits and Implements references from the text of a class block.
    /// The VB.NET tree-sitter grammar mis-parses these as field_declaration/ERROR,
    /// so we do text-based extraction from the full block text.
    fn extract_inherits_implements(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        class_id: &str,
    ) {
        let text = state.node_text(node);
        let base_line = node.start_position().row as u32;

        for (i, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(base_type) = trimmed.strip_prefix("Inherits ") {
                let base_type = base_type.trim();
                if !base_type.is_empty() {
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: class_id.to_string(),
                        reference_name: base_type.to_string(),
                        reference_kind: EdgeKind::Extends,
                        line: base_line + i as u32,
                        column: 0,
                        file_path: state.file_path.clone(),
                    });
                }
            } else if let Some(iface_list) = trimmed.strip_prefix("Implements ") {
                // Can be comma-separated: "Implements IFoo, IBar"
                for iface in iface_list.split(',') {
                    let iface = iface.trim();
                    if !iface.is_empty() {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: class_id.to_string(),
                            reference_name: iface.to_string(),
                            reference_kind: EdgeKind::Implements,
                            line: base_line + i as u32,
                            column: 0,
                            file_path: state.file_path.clone(),
                        });
                    }
                }
            }
        }
    }

    /// Extract XML doc comments (''' ...) preceding a declaration.
    fn extract_xml_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_sibling();
        while let Some(sibling) = current {
            let kind = sibling.kind();
            if kind == "comment" {
                let text = state.node_text(sibling);
                let trimmed = text.trim();
                if trimmed.starts_with("'''") {
                    comments.push(trimmed.to_string());
                    current = sibling.prev_sibling();
                } else {
                    break;
                }
            } else if kind == "blank_line" {
                // Skip blank lines between doc comments
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

        // Clean the comments: strip ''', strip XML tags for clean text.
        let cleaned: Vec<String> = comments
            .iter()
            .map(|line| {
                let stripped = line.strip_prefix("'''").unwrap_or(line).trim();
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

    /// Recursively find invocation nodes and create unresolved Calls references.
    fn extract_call_sites_from_children(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "invocation" => {
                        let callee_name = Self::extract_invocation_name(state, child);
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: callee_name,
                            reference_kind: EdgeKind::Calls,
                            line: child.start_position().row as u32,
                            column: child.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                        // Recurse for nested calls inside arguments.
                        Self::extract_call_sites_from_children(state, child, fn_node_id);
                    }
                    // Skip nested declarations.
                    "method_declaration" | "constructor_declaration" | "class_block" => {}
                    _ => {
                        Self::extract_call_sites_from_children(state, child, fn_node_id);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the name from an invocation node.
    fn extract_invocation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(target) = node.child_by_field_name("target") {
            return state.node_text(target);
        }
        // Fallback: first child
        if let Some(first) = node.child(0) {
            if first.kind() != "argument_list" {
                return state.node_text(first);
            }
        }
        state.node_text(node)
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

impl crate::extraction::LanguageExtractor for VbNetExtractor {
    fn extensions(&self) -> &[&str] {
        &["vb"]
    }

    fn language_name(&self) -> &str {
        "VB.NET"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        VbNetExtractor::extract_vbnet(file_path, source)
    }
}
