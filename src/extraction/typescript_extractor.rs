/// Tree-sitter based TypeScript/JavaScript source code extractor.
///
/// Parses TypeScript (.ts, .tsx) and JavaScript (.js, .jsx) source files and
/// emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, TYPESCRIPT_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from TypeScript/JavaScript source files
/// using tree-sitter.
pub struct TypeScriptExtractor;

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
    /// Whether the current declaration is inside an export_statement.
    in_export: bool,
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
            in_export: false,
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

impl TypeScriptExtractor {
    /// Extract code graph nodes and edges from a TypeScript/JavaScript source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the source code to parse.
    pub fn extract_typescript(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let ext = file_path.rsplit('.').next().unwrap_or("ts");

        let tree = match Self::parse_source(source, ext) {
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

    /// Parse source code into a tree-sitter AST, selecting grammar by file extension.
    fn parse_source(source: &str, extension: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        match extension {
            "ts" => {
                let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
                parser
                    .set_language(&language.into())
                    .map_err(|e| format!("failed to load TypeScript grammar: {e}"))?;
            }
            "tsx" => {
                let language = tree_sitter_typescript::LANGUAGE_TSX;
                parser
                    .set_language(&language.into())
                    .map_err(|e| format!("failed to load TSX grammar: {e}"))?;
            }
            "js" | "jsx" => {
                let language = tree_sitter_javascript::LANGUAGE;
                parser
                    .set_language(&language.into())
                    .map_err(|e| format!("failed to load JavaScript grammar: {e}"))?;
            }
            _ => {
                // Default to TypeScript.
                let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
                parser
                    .set_language(&language.into())
                    .map_err(|e| format!("failed to load TypeScript grammar: {e}"))?;
            }
        }
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
            "export_statement" => Self::visit_export_statement(state, node),
            "function_declaration" => Self::visit_function(state, node),
            "lexical_declaration" => Self::visit_lexical_declaration(state, node),
            "class_declaration" => Self::visit_class(state, node),
            "interface_declaration" => Self::visit_interface(state, node),
            "enum_declaration" => Self::visit_enum(state, node),
            "type_alias_declaration" => Self::visit_type_alias(state, node),
            "import_statement" => Self::visit_import(state, node),
            "expression_statement" => {
                // Namespace declarations appear as expression_statement > internal_module.
                if let Some(internal) = Self::find_child_by_kind(node, "internal_module") {
                    Self::visit_namespace(state, internal);
                }
            }
            _ => {
                // For other node types, skip — children are visited explicitly
                // by the specific visit_* methods when needed.
            }
        }
    }

    /// Visit an export_statement. Sets in_export flag and recurses into the
    /// inner declaration.
    fn visit_export_statement(state: &mut ExtractionState, node: TsNode<'_>) {
        let prev_in_export = state.in_export;
        state.in_export = true;

        // Also create an Export node to track the export itself.
        let start_line = node.start_position().row as u32;

        // Recurse into children to find the actual declaration.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "function_declaration" => Self::visit_function(state, child),
                    "class_declaration" => Self::visit_class(state, child),
                    "interface_declaration" => Self::visit_interface(state, child),
                    "enum_declaration" => Self::visit_enum(state, child),
                    "type_alias_declaration" => Self::visit_type_alias(state, child),
                    "lexical_declaration" => Self::visit_lexical_declaration(state, child),
                    // Re-export or bare export like `export { foo }`
                    "export_clause" => {
                        let text = state.node_text(node);
                        let name = "export";
                        let qualified_name =
                            format!("{}::{}", state.qualified_prefix(), name);
                        let id = generate_node_id(
                            &state.file_path,
                            &NodeKind::Export,
                            &text,
                            start_line,
                        );
                        let graph_node = Node {
                            id: id.clone(),
                            kind: NodeKind::Export,
                            name: name.to_string(),
                            qualified_name,
                            file_path: state.file_path.clone(),
                            start_line,
                            end_line: node.end_position().row as u32,
                            start_column: node.start_position().column as u32,
                            end_column: node.end_position().column as u32,
                            signature: Some(text),
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
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        state.in_export = prev_in_export;
    }

    /// Extract a function declaration node.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let is_async = Self::has_child_kind(node, "async");
        let signature = Self::extract_signature(state, node);
        let docstring = Self::extract_jsdoc(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let metrics = count_complexity(node, &TYPESCRIPT_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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
        if let Some(body) = Self::find_child_by_kind(node, "statement_block") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a lexical declaration (const/let/var) looking for arrow functions
    /// and constant declarations.
    fn visit_lexical_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_const = Self::has_child_kind(node, "const");

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    // Check if this is an arrow function assignment.
                    if let Some(arrow) = Self::find_child_by_kind(child, "arrow_function") {
                        Self::visit_arrow_function(state, child, arrow);
                    } else if is_const {
                        // It's a const variable (not an arrow function).
                        Self::visit_const_variable(state, child);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an arrow function from a variable_declarator node.
    fn visit_arrow_function(
        state: &mut ExtractionState,
        declarator: TsNode<'_>,
        arrow_node: TsNode<'_>,
    ) {
        let name = Self::find_child_by_kind(declarator, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let is_async = Self::has_child_kind(arrow_node, "async");

        // Use the declarator's parent (lexical_declaration) for docstring lookup.
        let docstring = if let Some(parent) = declarator.parent() {
            Self::extract_jsdoc(state, parent)
        } else {
            None
        };

        let signature = Self::extract_arrow_signature(state, declarator);
        let start_line = declarator.start_position().row as u32;
        let end_line = arrow_node.end_position().row as u32;
        let start_column = declarator.start_position().column as u32;
        let end_column = arrow_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id =
            generate_node_id(&state.file_path, &NodeKind::ArrowFunction, &name, start_line);
        let metrics = count_complexity(arrow_node, &TYPESCRIPT_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::ArrowFunction,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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

        // Extract call sites from the arrow function body.
        if let Some(body) = Self::find_child_by_kind(arrow_node, "statement_block") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a const variable declaration (not an arrow function).
    fn visit_const_variable(state: &mut ExtractionState, declarator: TsNode<'_>) {
        let name = Self::find_child_by_kind(declarator, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let text = state.node_text(declarator);
        let start_line = declarator.start_position().row as u32;
        let end_line = declarator.end_position().row as u32;
        let start_column = declarator.start_position().column as u32;
        let end_column = declarator.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);

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

    /// Extract a class declaration node.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        // TS uses type_identifier, JS uses identifier for class names.
        let name = Self::find_child_by_kind(node, "type_identifier")
            .or_else(|| Self::find_child_by_kind(node, "identifier"))
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let docstring = Self::extract_jsdoc(state, node);
        let signature = Self::extract_signature(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Class,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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

        // Extract decorators from the class declaration itself.
        Self::extract_decorators(state, node, &id);

        // Extract extends/implements from class_heritage.
        Self::extract_class_heritage(state, node, &id);

        // Recurse into class_body for methods and fields.
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            state.node_stack.push((name, id.clone()));
            Self::visit_class_body(state, body);
            state.node_stack.pop();
        }
    }

    /// Visit the body of a class, extracting methods and fields.
    fn visit_class_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "method_definition" => Self::visit_method(state, child),
                    "public_field_definition" => Self::visit_field(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a method_definition from a class body.
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "property_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let kind = if name == "constructor" {
            NodeKind::Constructor
        } else {
            NodeKind::Method
        };

        // Check for accessibility_modifier (public/private/protected).
        let visibility = Self::extract_ts_accessibility(state, node);
        let is_async = Self::has_child_kind(node, "async");
        let signature = Self::extract_signature(state, node);
        let docstring = Self::extract_jsdoc(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &TYPESCRIPT_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (the class).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the method body.
        if let Some(body) = Self::find_child_by_kind(node, "statement_block") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a field from a class body (public_field_definition).
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "property_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = Self::extract_ts_accessibility(state, node);
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
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (the class).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract an interface declaration node.
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let docstring = Self::extract_jsdoc(state, node);
        let signature = Self::extract_signature(state, node);
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

        // Extract methods from interface body.
        if let Some(body) = Self::find_child_by_kind(node, "interface_body") {
            state.node_stack.push((name, id.clone()));
            Self::visit_interface_body(state, body);
            state.node_stack.pop();
        }
    }

    /// Visit the body of an interface, extracting method signatures.
    fn visit_interface_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "method_signature" {
                    Self::visit_interface_method(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a method_signature from an interface body.
    fn visit_interface_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "property_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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

        // Contains edge from parent (the interface).
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract an enum declaration node.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let docstring = Self::extract_jsdoc(state, node);
        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());
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

        // Extract enum members from enum_body.
        if let Some(body) = Self::find_child_by_kind(node, "enum_body") {
            state.node_stack.push((name, id.clone()));
            Self::visit_enum_body(state, body);
            state.node_stack.pop();
        }
    }

    /// Visit the body of an enum, extracting variants.
    fn visit_enum_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "property_identifier" {
                    Self::visit_enum_member(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an enum member (variant).
    fn visit_enum_member(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = state.node_text(node);
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

    /// Extract a type alias declaration.
    fn visit_type_alias(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "type_identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let text = state.node_text(node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::TypeAlias, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::TypeAlias,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
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

    /// Extract an import statement.
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        // Extract the module path from the string literal.
        let module_path = Self::extract_import_path(state, node);
        let name = module_path.clone().unwrap_or_else(|| text.clone());
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
            updated_at: state.timestamp,
        };
        state.nodes.push(graph_node);

        // Contains edge from parent (File).
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

    /// Extract a namespace (internal_module) declaration.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let visibility = if state.in_export {
            Visibility::Pub
        } else {
            Visibility::Private
        };
        let docstring = Self::extract_jsdoc(state, node);
        let text = state.node_text(node);
        let signature = text.find('{').map(|pos| text[..pos].trim().to_string());
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

        // Recurse into the namespace body.
        if let Some(body) = Self::find_child_by_kind(node, "statement_block") {
            state.node_stack.push((name, id));
            Self::visit_children(state, body);
            state.node_stack.pop();
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract decorators from a class or method declaration.
    fn extract_decorators(state: &mut ExtractionState, node: TsNode<'_>, parent_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "decorator" {
                    let text = state.node_text(child);
                    // Get the decorator name (strip @ and potential arguments).
                    let name = text
                        .trim_start_matches('@')
                        .split('(')
                        .next()
                        .unwrap_or(&text)
                        .trim()
                        .to_string();
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), name);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::Decorator,
                        &name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::Decorator,
                        name: name.clone(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(text),
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

                    // Annotates edge from decorator to parent.
                    state.edges.push(Edge {
                        source: id,
                        target: parent_id.to_string(),
                        kind: EdgeKind::Annotates,
                        line: Some(start_line),
                    });
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract extends/implements from a class heritage clause.
    fn extract_class_heritage(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        if let Some(heritage) = Self::find_child_by_kind(node, "class_heritage") {
            let mut cursor = heritage.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    match child.kind() {
                        "extends_clause" => {
                            // Find the extended class name (identifier or type_identifier).
                            let ext_name = Self::find_child_by_kind(child, "identifier")
                                .or_else(|| Self::find_child_by_kind(child, "type_identifier"))
                                .map(|n| state.node_text(n));
                            if let Some(name) = ext_name {
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: class_id.to_string(),
                                    reference_name: name,
                                    reference_kind: EdgeKind::Extends,
                                    line: child.start_position().row as u32,
                                    column: child.start_position().column as u32,
                                    file_path: state.file_path.clone(),
                                });
                            }
                        }
                        "implements_clause" => {
                            // May implement multiple interfaces.
                            let mut inner = child.walk();
                            if inner.goto_first_child() {
                                loop {
                                    let iface = inner.node();
                                    if iface.kind() == "type_identifier" {
                                        let name = state.node_text(iface);
                                        state.unresolved_refs.push(UnresolvedRef {
                                            from_node_id: class_id.to_string(),
                                            reference_name: name,
                                            reference_kind: EdgeKind::Implements,
                                            line: iface.start_position().row as u32,
                                            column: iface.start_position().column as u32,
                                            file_path: state.file_path.clone(),
                                        });
                                    }
                                    if !inner.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Extract the import path from an import_statement.
    fn extract_import_path(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // The string child contains the module path.
        if let Some(string_node) = Self::find_child_by_kind(node, "string") {
            let text = state.node_text(string_node);
            // Strip quotes.
            let path = text
                .trim()
                .trim_matches('\'')
                .trim_matches('"')
                .to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
        None
    }

    /// Recursively find call_expression nodes inside a node and create
    /// unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        // Get the callee name.
                        let callee = child.named_child(0);
                        if let Some(callee) = callee {
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
                        // Also recurse into the call expression for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested arrow functions to avoid polluting call sites.
                    "arrow_function" | "function" | "function_declaration" => {}
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

    /// Extract the function/method signature (everything up to the body `{`).
    fn extract_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            Some(text.trim().to_string())
        }
    }

    /// Extract the signature for an arrow function from its variable_declarator.
    fn extract_arrow_signature(state: &ExtractionState, declarator: TsNode<'_>) -> Option<String> {
        let text = state.node_text(declarator);
        // For arrow functions, the signature is "name = (params) => ..."
        // We want everything up to the arrow body.
        if let Some(arrow_pos) = text.find("=>") {
            Some(text[..arrow_pos + 2].trim().to_string())
        } else {
            Some(text.trim().to_string())
        }
    }

    /// Extract JSDoc docstrings from preceding comment nodes.
    /// Only picks up `/** ... */` style comments (JSDoc).
    fn extract_jsdoc(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // In TS, we also need to check the parent if this is inside an export_statement.
        let mut target = node;
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                target = parent;
            }
        }

        let current = target.prev_named_sibling();
        if let Some(sibling) = current {
            if sibling.kind() == "comment" {
                let text = state.node_text(sibling);
                if text.starts_with("/**") {
                    return Some(Self::clean_jsdoc(&text));
                }
            }
        }
        None
    }

    /// Clean JSDoc comment markers.
    fn clean_jsdoc(comment: &str) -> String {
        let trimmed = comment.trim();
        if trimmed.starts_with("/**") && trimmed.ends_with("*/") {
            let inner = &trimmed[3..trimmed.len() - 2];
            inner
                .lines()
                .map(|line| {
                    let l = line.trim();
                    l.strip_prefix("* ")
                        .or_else(|| l.strip_prefix('*'))
                        .unwrap_or(l)
                })
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Extract TypeScript accessibility modifier (public/private/protected).
    fn extract_ts_accessibility(state: &ExtractionState, node: TsNode<'_>) -> Visibility {
        if let Some(modifier) = Self::find_child_by_kind(node, "accessibility_modifier") {
            let text = state.node_text(modifier);
            match text.as_str() {
                "public" => Visibility::Pub,
                "private" => Visibility::Private,
                "protected" => Visibility::PubSuper,
                _ => Visibility::Pub,
            }
        } else {
            // In TypeScript, class members without explicit modifier are public by default.
            Visibility::Pub
        }
    }

    /// Check if a node has a direct child of a given kind.
    fn has_child_kind(node: TsNode<'_>, kind: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == kind {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
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

impl crate::extraction::LanguageExtractor for TypeScriptExtractor {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn language_name(&self) -> &str {
        "TypeScript"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        TypeScriptExtractor::extract_typescript(file_path, source)
    }
}
