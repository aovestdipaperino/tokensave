/// Tree-sitter based PHP source code extractor.
///
/// Parses PHP source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, PHP_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from PHP source files using tree-sitter.
pub struct PhpExtractor;

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
    /// Depth of class nesting. > 0 means we are inside a class/interface/trait.
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

impl PhpExtractor {
    /// Extract code graph nodes and edges from a PHP source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the PHP source code to parse.
    pub fn extract_php(file_path: &str, source: &str) -> ExtractionResult {
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

        // Walk the AST.
        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("php");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load PHP grammar: {e}"))?;
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
            "function_definition" => Self::visit_function(state, node),
            "method_declaration" => Self::visit_method(state, node),
            "class_declaration" => Self::visit_class(state, node),
            "interface_declaration" => Self::visit_interface(state, node),
            "trait_declaration" => Self::visit_trait(state, node),
            "enum_declaration" => Self::visit_enum(state, node),
            "namespace_definition" => Self::visit_namespace(state, node),
            "use_declaration" => Self::visit_use_declaration(state, node),
            "const_declaration" => Self::visit_const_declaration(state, node),
            "property_declaration" => Self::visit_property_declaration(state, node),
            // Recurse into program / namespace body.
            _ => Self::visit_children(state, node),
        }
    }

    /// Extract a top-level function definition.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Visibility::Pub;
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let metrics = count_complexity(node, &PHP_COMPLEXITY, &state.source);

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

        Self::extract_annotations(state, node, &id);

        // Extract call sites from the function body.
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a class method declaration.
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Self::extract_visibility(state, node);
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let metrics = count_complexity(node, &PHP_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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

        // Contains edge from parent class/trait/interface.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        Self::extract_annotations(state, node, &id);

        // Extract call sites from the method body.
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a class declaration.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_class_signature(state, node);
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

        Self::extract_annotations(state, node, &id);
        // Extract base class (extends) references.
        Self::extract_class_extends(state, node, &id);
        // Extract interface (implements) references.
        Self::extract_class_implements(state, node, &id);

        // Visit class body members.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "declaration_list") {
            Self::visit_class_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an interface declaration.
    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Trait, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Trait,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("interface {name}")),
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

        // Visit interface body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "declaration_list") {
            Self::visit_class_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a trait declaration.
    fn visit_trait(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Trait, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Trait,
            name: name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(format!("trait {name}")),
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

        // Visit trait body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "declaration_list") {
            Self::visit_class_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an enum declaration.
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
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
            signature: Some(format!("enum {name}")),
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

        // Visit enum body for cases.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "enum_declaration_list") {
            Self::visit_enum_body(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit enum body, extracting enum cases as EnumVariant nodes.
    fn visit_enum_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "enum_case" => Self::visit_enum_case(state, child),
                    "method_declaration" => Self::visit_method(state, child),
                    "const_declaration" => Self::visit_const_declaration(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an enum case as an EnumVariant.
    fn visit_enum_case(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::EnumVariant, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::EnumVariant,
            name: name.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a namespace definition.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "namespace_name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Visibility::Pub;
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

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
            signature: Some(format!("namespace {name}")),
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Visit namespace body (braced namespace) or siblings (unbraced).
        state.node_stack.push((name.clone(), id));
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::visit_children(state, body);
        }
        state.node_stack.pop();
    }

    /// Extract a use declaration (import statement).
    fn visit_use_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // use_declaration contains use_instead_of or namespace_use_clause children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "namespace_use_clause" => {
                        Self::visit_use_clause(state, child, node);
                    }
                    "namespace_use_group" => {
                        Self::visit_use_group(state, child, node);
                    }
                    "qualified_name" | "name" => {
                        let name = state.node_text(child);
                        Self::create_use_node(state, &name, node);
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single use clause (e.g., `Foo\Bar as Baz`).
    fn visit_use_clause(
        state: &mut ExtractionState,
        clause: TsNode<'_>,
        use_node: TsNode<'_>,
    ) {
        let name = Self::find_child_by_kind(clause, "qualified_name")
            .or_else(|| Self::find_child_by_kind(clause, "name"))
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| state.node_text(clause));
        Self::create_use_node(state, &name, use_node);
    }

    /// Extract a grouped use declaration (e.g., `use Foo\{Bar, Baz}`).
    fn visit_use_group(
        state: &mut ExtractionState,
        group: TsNode<'_>,
        use_node: TsNode<'_>,
    ) {
        // The group has a prefix and individual clauses.
        let prefix = Self::find_child_by_kind(group, "namespace_name")
            .map(|n| state.node_text(n))
            .unwrap_or_default();

        let mut cursor = group.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "namespace_use_clause" {
                    let item = Self::find_child_by_kind(child, "name")
                        .or_else(|| Self::find_child_by_kind(child, "qualified_name"))
                        .map(|n| state.node_text(n))
                        .unwrap_or_else(|| state.node_text(child));
                    let full_name = if prefix.is_empty() {
                        item
                    } else {
                        format!("{}\\{}", prefix, item)
                    };
                    Self::create_use_node(state, &full_name, use_node);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Create a Use node for a PHP use/import declaration.
    fn create_use_node(state: &mut ExtractionState, name: &str, node: TsNode<'_>) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Use, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Use,
            name: name.to_string(),
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
            reference_name: name.to_string(),
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Extract a const declaration (can be at file level or inside a class).
    fn visit_const_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // const_declaration contains one or more const_element children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "const_element" {
                    Self::visit_const_element(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single const element.
    fn visit_const_element(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "name")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Const,
            name: name.clone(),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    /// Extract a property declaration inside a class.
    fn visit_property_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let visibility = Self::extract_visibility(state, node);

        // property_declaration contains one or more property_element children.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "property_element" || child.kind() == "variable_name" {
                    let name = if child.kind() == "property_element" {
                        Self::find_child_by_kind(child, "variable_name")
                            .map(|n| state.node_text(n))
                            .unwrap_or_else(|| state.node_text(child))
                    } else {
                        state.node_text(child)
                    };
                    // Strip leading $ from variable names.
                    let name = name.trim_start_matches('$').to_string();

                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                    let id =
                        generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::Field,
                        name: name.clone(),
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(state.node_text(node).trim().to_string()),
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

                    if let Some(parent_id) = state.parent_node_id() {
                        state.edges.push(Edge {
                            source: parent_id.to_string(),
                            target: id.clone(),
                            kind: EdgeKind::Contains,
                            line: Some(start_line),
                        });
                    }

                    // Extract annotations from the enclosing property_declaration.
                    Self::extract_annotations(state, node, &id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit class body, dispatching on member kinds.
    fn visit_class_body(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "method_declaration" => Self::visit_method(state, child),
                    "property_declaration" => Self::visit_property_declaration(state, child),
                    "const_declaration" => Self::visit_const_declaration(state, child),
                    "use_declaration" => Self::visit_use_declaration(state, child),
                    _ => {}
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

    /// Extract the visibility modifier from a node (looks for visibility_modifier child).
    fn extract_visibility(state: &ExtractionState, node: TsNode<'_>) -> Visibility {
        if let Some(vis_node) = Self::find_child_by_kind(node, "visibility_modifier") {
            let text = state.node_text(vis_node);
            match text.trim() {
                "public" => Visibility::Pub,
                "protected" => Visibility::Private,
                "private" => Visibility::Private,
                _ => Visibility::Pub,
            }
        } else {
            // Default visibility in PHP is public for class methods.
            Visibility::Pub
        }
    }

    /// Extract base class from a class declaration (extends clause).
    fn extract_class_extends(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        // Look for base_clause child containing a qualified_name or name.
        if let Some(base_clause) = Self::find_child_by_kind(node, "base_clause") {
            let base_name = Self::find_child_by_kind(base_clause, "qualified_name")
                .or_else(|| Self::find_child_by_kind(base_clause, "name"))
                .map(|n| state.node_text(n));
            if let Some(name) = base_name {
                let line = base_clause.start_position().row as u32;
                let column = base_clause.start_position().column as u32;
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: class_id.to_string(),
                    reference_name: name,
                    reference_kind: EdgeKind::Extends,
                    line,
                    column,
                    file_path: state.file_path.clone(),
                });
            }
        }
    }

    /// Extract implemented interfaces from a class declaration (implements clause).
    fn extract_class_implements(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        class_id: &str,
    ) {
        if let Some(impl_list) = Self::find_child_by_kind(node, "class_implements") {
            let mut cursor = impl_list.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "qualified_name" || child.kind() == "name" {
                        let iface_name = state.node_text(child);
                        let line = child.start_position().row as u32;
                        let column = child.start_position().column as u32;
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: class_id.to_string(),
                            reference_name: iface_name,
                            reference_kind: EdgeKind::Implements,
                            line,
                            column,
                            file_path: state.file_path.clone(),
                        });
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Extract the function/method signature (everything up to the body).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Use the compound_statement body's start byte to find where the body begins.
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            let text = state.node_text(node);
            let body_offset = body.start_byte() - node.start_byte();
            if body_offset <= text.len() {
                let before_body = &text[..body_offset];
                return Some(before_body.trim().to_string());
            }
        }
        Some(state.node_text(node).trim().to_string())
    }

    /// Extract the class signature (class Name extends Base implements Iface).
    fn extract_class_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(body) = Self::find_child_by_kind(node, "declaration_list") {
            let text = state.node_text(node);
            let body_offset = body.start_byte() - node.start_byte();
            if body_offset <= text.len() {
                let before_body = &text[..body_offset];
                return Some(before_body.trim().to_string());
            }
        }
        Some(state.node_text(node).trim().to_string())
    }

    /// Extract PHP doc comments (`/** ... */`) preceding a node.
    ///
    /// Looks for a preceding sibling or leading `comment` node with `/**` prefix.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Walk previous named siblings to find a doc comment immediately before this node.
        let parent = node.parent()?;
        let mut cursor = parent.walk();
        let mut last_comment: Option<String> = None;

        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.id() == node.id() {
                    // Return the last comment seen immediately before this node.
                    return last_comment;
                }
                if child.kind() == "comment" {
                    let text = state.node_text(child);
                    if text.trim_start().starts_with("/**") {
                        last_comment = Some(Self::strip_doc_comment(&text));
                    } else {
                        // Non-doc comment resets the docstring candidate.
                        last_comment = None;
                    }
                } else if !child.is_extra() {
                    // Any non-comment, non-whitespace node resets the candidate.
                    last_comment = None;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Strip `/** ... */` markers from a PHP doc comment.
    fn strip_doc_comment(text: &str) -> String {
        let trimmed = text.trim();
        // Remove /** prefix and */ suffix.
        let inner = trimmed
            .strip_prefix("/**")
            .unwrap_or(trimmed)
            .strip_suffix("*/")
            .unwrap_or(trimmed);
        // Clean up each line: remove leading * markers.
        inner
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim().to_string())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "function_call_expression" => {
                        // The first child is the callee (function name or expression).
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
                        // Recurse for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    "method_call_expression" | "nullsafe_method_call_expression" => {
                        // method_call_expression: object -> name -> arguments
                        // The method name is the "name" field child.
                        let method_name = child
                            .child_by_field_name("name")
                            .map(|n| state.node_text(n));
                        if let Some(name) = method_name {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    "static_call_expression" => {
                        // ClassName::method()
                        let method_name = child
                            .child_by_field_name("name")
                            .map(|n| state.node_text(n));
                        if let Some(name) = method_name {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested function/class definitions to avoid polluting call sites.
                    "function_definition" | "class_declaration" => {}
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

    // -----------------------------------------------------------------------
    // Annotations (PHP 8 Attributes)
    // -----------------------------------------------------------------------

    /// Extract PHP 8 attributes from a declaration node and create
    /// AnnotationUsage nodes and Annotates edges.
    ///
    /// PHP 8 attributes appear as `attribute_list` children of the declaration.
    /// Structure: attribute_list > attribute_group > attribute > name.
    fn extract_annotations(state: &mut ExtractionState, node: TsNode<'_>, target_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "attribute_list" {
                    Self::extract_annotations_from_attribute_list(state, child, target_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Walk an attribute_list node, iterating over attribute_group > attribute
    /// children to create AnnotationUsage nodes.
    fn extract_annotations_from_attribute_list(
        state: &mut ExtractionState,
        attr_list: TsNode<'_>,
        target_id: &str,
    ) {
        let mut cursor = attr_list.walk();
        if cursor.goto_first_child() {
            loop {
                let group = cursor.node();
                if group.kind() == "attribute_group" {
                    let mut inner = group.walk();
                    if inner.goto_first_child() {
                        loop {
                            let child = inner.node();
                            if child.kind() == "attribute" {
                                let attr_name = Self::extract_attribute_name(state, child);
                                let start_line = child.start_position().row as u32;
                                let end_line = child.end_position().row as u32;
                                let start_column = child.start_position().column as u32;
                                let end_column = child.end_position().column as u32;
                                let qualified_name =
                                    format!("{}::@{}", state.qualified_prefix(), attr_name);
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
                                    end_line,
                                    start_column,
                                    end_column,
                                    signature: Some(
                                        format!("#[{}]", state.node_text(child).trim()),
                                    ),
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

                                // Annotates unresolved ref.
                                state.unresolved_refs.push(UnresolvedRef {
                                    from_node_id: id.clone(),
                                    reference_name: attr_name,
                                    reference_kind: EdgeKind::Annotates,
                                    line: start_line,
                                    column: start_column,
                                    file_path: state.file_path.clone(),
                                });

                                // Direct Annotates edge from annotation to target.
                                state.edges.push(Edge {
                                    source: id,
                                    target: target_id.to_string(),
                                    kind: EdgeKind::Annotates,
                                    line: Some(start_line),
                                });
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
    }

    /// Extract the name from a PHP attribute node.
    fn extract_attribute_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // PHP attributes have a "name" or "qualified_name" child.
        if let Some(name) = Self::find_child_by_kind(node, "name") {
            return state.node_text(name);
        }
        if let Some(qn) = Self::find_child_by_kind(node, "qualified_name") {
            return state.node_text(qn);
        }
        // Fallback: full text before '('
        let text = state.node_text(node);
        text.split('(').next().unwrap_or(&text).trim().to_string()
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

impl crate::extraction::LanguageExtractor for PhpExtractor {
    fn extensions(&self) -> &[&str] {
        &["php"]
    }

    fn language_name(&self) -> &str {
        "PHP"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_php(file_path, source)
    }
}
