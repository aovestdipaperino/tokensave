/// Tree-sitter based Dart source code extractor.
///
/// Parses Dart source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, ComplexityMetrics, DART_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Dart source files using tree-sitter.
pub struct DartExtractor;

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
    /// Tracks depth inside class/mixin/extension/enum bodies.
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

impl DartExtractor {
    /// Extract code graph nodes and edges from a Dart source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Dart source code to parse.
    pub fn extract_dart(file_path: &str, source: &str) -> ExtractionResult {
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

        // Walk the AST using the program-level visitor.
        let root = tree.root_node();
        Self::visit_program_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("dart");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Dart grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit children of the program node. In Dart's grammar, top-level items
    /// like `function_signature` + `function_body` appear as siblings at the program level.
    fn visit_program_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return;
        }

        loop {
            let child = cursor.node();
            match child.kind() {
                "library_name" => Self::visit_library(state, child),
                "import_or_export" => Self::visit_import(state, child),
                "class_definition" | "class_declaration" => Self::visit_class(state, child),
                "mixin_declaration" => Self::visit_mixin(state, child),
                "extension_declaration" => Self::visit_extension(state, child),
                "enum_declaration" => Self::visit_enum(state, child),
                "type_alias" => Self::visit_type_alias(state, child),
                "function_signature" => {
                    // Top-level function: function_signature followed by function_body sibling.
                    let body = child
                        .next_named_sibling()
                        .filter(|s| s.kind() == "function_body");
                    Self::visit_top_level_function(state, child, body);
                }
                "function_declaration" => {
                    // tree-sitter-dart 0.2 wraps top-level functions in
                    // `function_declaration { function_signature, function_body }`.
                    if let Some(sig) = Self::find_child_by_kind(child, "function_signature") {
                        let body = Self::find_child_by_kind(child, "function_body");
                        Self::visit_top_level_function(state, sig, body);
                    }
                }
                "declaration" => Self::visit_declaration(state, child),
                // tree-sitter-dart 0.1 misparses `library foo;` as a variable
                // declaration with type `library`. Detect that shape and treat
                // it as a library directive.
                "top_level_variable_declaration" if Self::is_library_directive(state, child) => {
                    Self::visit_library_misparse(state, child);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Returns true if `node` looks like `library foo;` misparsed as a
    /// `top_level_variable_declaration` whose declared "type" is the
    /// `library` keyword.
    fn is_library_directive(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let Some(ty) = Self::find_child_by_kind(node, "type") else {
            return false;
        };
        let Some(type_id) = Self::find_child_by_kind(ty, "type_identifier") else {
            return false;
        };
        state.node_text(type_id) == "library"
    }

    fn visit_library_misparse(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(list) = Self::find_child_by_kind(node, "initialized_identifier_list") else {
            return;
        };
        let Some(init) = Self::find_child_by_kind(list, "initialized_identifier") else {
            return;
        };
        let Some(ident) = Self::find_child_by_kind(init, "identifier") else {
            return;
        };
        let name = state.node_text(ident);
        Self::push_library_node(state, node, name);
    }

    // ----------------------------------
    // Library
    // ----------------------------------

    fn visit_library(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "dotted_identifier_list").map_or_else(
            || state.node_text(node).trim().to_string(),
            |n| state.node_text(n),
        );
        Self::push_library_node(state, node, name);
    }

    fn push_library_node(state: &mut ExtractionState, node: TsNode<'_>, name: String) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Library, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Library,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node)),
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

    // ----------------------------------
    // Imports
    // ----------------------------------

    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let text = state.node_text(node);
        let path = Self::extract_import_path(&text);
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

        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id,
            reference_name: path,
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Extract the import path from an import/export statement text.
    fn extract_import_path(text: &str) -> String {
        if let Some(start) = text.find('\'') {
            if let Some(end) = text[start + 1..].find('\'') {
                return text[start + 1..start + 1 + end].to_string();
            }
        }
        if let Some(start) = text.find('"') {
            if let Some(end) = text[start + 1..].find('"') {
                return text[start + 1..start + 1 + end].to_string();
            }
        }
        text.trim().to_string()
    }

    // ----------------------------------
    // Top-level function
    // ----------------------------------

    /// Visit a top-level function. In Dart's grammar, `function_signature` and
    /// `function_body` are siblings at the program level.
    fn visit_top_level_function(
        state: &mut ExtractionState,
        sig_node: TsNode<'_>,
        body: Option<TsNode<'_>>,
    ) {
        let name = sig_node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        // Doc-comment siblings live one level above `sig_node` when dart 0.2
        // wraps the function in `function_declaration`. Walk up to find the
        // outermost wrapper before scanning prev siblings.
        let doc_anchor = sig_node
            .parent()
            .filter(|p| p.kind() == "function_declaration")
            .unwrap_or(sig_node);
        let docstring = Self::extract_docstring(state, doc_anchor);

        // Build signature text from the function_signature node.
        let sig_text = state.node_text(sig_node);
        let signature = Some(sig_text.trim().to_string());

        // Detect async from the function body.
        let is_async = match body {
            Some(b) => state.node_text(b).starts_with("async"),
            None => false,
        };

        // Span from the signature to the end of the body (or just the signature).
        let start_line = sig_node.start_position().row as u32;
        let end_line = body.map_or(sig_node.end_position().row as u32, |b| {
            b.end_position().row as u32
        });
        let start_column = sig_node.start_position().column as u32;
        let end_column = body.map_or(sig_node.end_position().column as u32, |b| {
            b.end_position().column as u32
        });
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let metrics = body.map_or(ComplexityMetrics::default(), |b| {
            count_complexity(b, &DART_COMPLEXITY, &state.source)
        });

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Function,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the body.
        if let Some(body_node) = body {
            Self::extract_call_sites(state, body_node, &id);
        }

        // Extract annotation usages from preceding siblings of the signature.
        Self::extract_annotations_from_modifiers(state, sig_node, &id);
    }

    // ----------------------------------
    // Class
    // ----------------------------------

    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let is_abstract = Self::find_child_by_kind(node, "abstract").is_some();

        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, node);
        let signature = Some(Self::extract_signature_to_brace(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        let kind = if is_abstract {
            NodeKind::Interface
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

        // Extract superclass extends reference.
        if let Some(superclass) = node.child_by_field_name("superclass") {
            if let Some(type_id) = Self::find_child_by_kind(superclass, "type_identifier") {
                let type_name = state.node_text(type_id);
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: id.clone(),
                    reference_name: type_name,
                    reference_kind: EdgeKind::Extends,
                    line: superclass.start_position().row as u32,
                    column: superclass.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
            }
        }

        // Extract annotation usages (e.g. @JsonSerializable).
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Visit class body.
        if let Some(body) = node.child_by_field_name("body") {
            state.node_stack.push((name, id.clone()));
            state.class_depth += 1;
            Self::visit_class_body(state, body);
            state.class_depth -= 1;
            state.node_stack.pop();
        }
    }

    // ----------------------------------
    // Mixin
    // ----------------------------------

    fn visit_mixin(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, node);
        let signature = Some(Self::extract_signature_to_brace(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Mixin, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Mixin,
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

        // Extract annotation usages.
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Visit mixin body (it uses class_body).
        if let Some(body) = Self::find_child_by_kind(node, "class_body") {
            state.node_stack.push((name, id.clone()));
            state.class_depth += 1;
            Self::visit_class_body(state, body);
            state.class_depth -= 1;
            state.node_stack.pop();
        }
    }

    // ----------------------------------
    // Extension
    // ----------------------------------

    fn visit_extension(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, node);
        let signature = Some(Self::extract_signature_to_brace(state, node));
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Extension, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Extension,
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

        // Visit extension body.
        if let Some(body) = node.child_by_field_name("body") {
            state.node_stack.push((name, id.clone()));
            state.class_depth += 1;
            Self::visit_body_members(state, body);
            state.class_depth -= 1;
            state.node_stack.pop();
        }
    }

    // ----------------------------------
    // Enum
    // ----------------------------------

    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, node);
        let signature = Some(Self::extract_signature_to_brace(state, node));
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

        // Extract annotation usages.
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Extract enum constants and members from enum_body.
        if let Some(body) = node.child_by_field_name("body") {
            state.node_stack.push((name, id.clone()));
            state.class_depth += 1;
            Self::visit_enum_body(state, body);
            state.class_depth -= 1;
            state.node_stack.pop();
        }
    }

    fn visit_enum_body(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "enum_constant" => Self::visit_enum_constant(state, child),
                    "declaration" => Self::visit_declaration(state, child),
                    "method_signature" => Self::visit_method_signature(state, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_enum_constant(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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
            signature: Some(state.node_text(node)),
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

    // ----------------------------------
    // Type alias
    // ----------------------------------

    fn visit_type_alias(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "type_identifier")
            .or_else(|| Self::find_child_by_kind(node, "identifier"))
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, node);
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
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column,
            signature: Some(state.node_text(node).trim().to_string()),
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // ----------------------------------
    // Class body
    // ----------------------------------

    fn visit_class_body(state: &mut ExtractionState, body: TsNode<'_>) {
        Self::visit_body_members(state, body);
    }

    /// Visit members of a class/mixin/extension/enum body.
    ///
    /// tree-sitter-dart 0.1 wraps every member in a `class_member` node, so
    /// declarations and method definitions are one level deeper than the
    /// extractor originally expected. We unwrap when we see one and dispatch
    /// to the same handlers as before.
    fn visit_body_members(state: &mut ExtractionState, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if !cursor.goto_first_child() {
            return;
        }

        loop {
            let child = cursor.node();
            match child.kind() {
                "class_member" => Self::visit_body_members(state, child),
                "declaration" => Self::visit_declaration(state, child),
                "method_signature" => Self::visit_method_signature(state, child),
                "method_declaration" => Self::visit_method_declaration(state, child),
                "function_signature" => {
                    // Function signature followed by function_body sibling in body context.
                    let body_node = child
                        .next_named_sibling()
                        .filter(|s| s.kind() == "function_body");
                    Self::visit_method_from_sig(state, child, body_node);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// `method_declaration` in tree-sitter-dart 0.1 wraps a `method_signature`
    /// and a `function_body` (for non-abstract methods). Find the signature
    /// and forward to the existing `visit_method_signature` path, passing the
    /// body so the method's complexity metrics get computed.
    fn visit_method_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        let sig = node
            .child_by_field_name("signature")
            .or_else(|| Self::find_child_by_kind(node, "method_signature"));
        let Some(sig) = sig else {
            return;
        };

        // Walk into the `method_signature` to find the actual function /
        // constructor / getter / setter / operator signature, then dispatch.
        let mut cursor = sig.walk();
        if !cursor.goto_first_child() {
            return;
        }
        let body = node
            .child_by_field_name("body")
            .or_else(|| Self::find_child_by_kind(node, "function_body"));
        loop {
            let child = cursor.node();
            match child.kind() {
                "function_signature" => {
                    Self::visit_method_from_sig(state, child, body);
                    return;
                }
                "constructor_signature"
                | "constant_constructor_signature"
                | "factory_constructor_signature"
                | "redirecting_factory_constructor_signature" => {
                    Self::visit_constructor(state, sig, child);
                    return;
                }
                "getter_signature" | "setter_signature" => {
                    Self::visit_getter_or_setter(state, sig, child);
                    return;
                }
                "operator_signature" => {
                    Self::visit_operator(state, sig, child);
                    return;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn visit_method_signature(state: &mut ExtractionState, node: TsNode<'_>) {
        // method_signature wraps constructor_signature, function_signature, etc.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "function_signature" => {
                        // Check for function_body as next sibling of the method_signature node.
                        let body = node
                            .next_named_sibling()
                            .filter(|s| s.kind() == "function_body");
                        Self::visit_method_from_sig(state, child, body);
                        return;
                    }
                    "constructor_signature"
                    | "constant_constructor_signature"
                    | "factory_constructor_signature"
                    | "redirecting_factory_constructor_signature" => {
                        Self::visit_constructor(state, node, child);
                        return;
                    }
                    "getter_signature" | "setter_signature" => {
                        Self::visit_getter_or_setter(state, node, child);
                        return;
                    }
                    "operator_signature" => {
                        Self::visit_operator(state, node, child);
                        return;
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // ----------------------------------
    // Declarations (inside class bodies and top-level)
    // ----------------------------------

    fn visit_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // A "declaration" node wraps various things.
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return;
        }

        let mut func_sig: Option<TsNode<'_>> = None;
        let mut func_body: Option<TsNode<'_>> = None;
        let mut has_static = false;

        loop {
            let child = cursor.node();
            match child.kind() {
                "function_signature" => func_sig = Some(child),
                "function_body" | "block" => func_body = Some(child),
                "static" => has_static = true,
                "constructor_signature"
                | "constant_constructor_signature"
                | "factory_constructor_signature"
                | "redirecting_factory_constructor_signature" => {
                    Self::visit_constructor(state, node, child);
                    return;
                }
                "getter_signature" | "setter_signature" => {
                    Self::visit_getter_or_setter(state, node, child);
                    return;
                }
                "operator_signature" => {
                    Self::visit_operator(state, node, child);
                    return;
                }
                "initialized_variable_definition" => {
                    Self::visit_initialized_var_def(state, node, child, has_static);
                    return;
                }
                "initialized_identifier_list" => {
                    // Field declaration with type: `Type name;`
                    Self::visit_identifier_list_field(state, node, child);
                    return;
                }
                "static_final_declaration_list" => {
                    Self::visit_static_final_declarations(state, node, child);
                    return;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }

        // Process function signature if found.
        if let Some(sig) = func_sig {
            if state.class_depth > 0 {
                Self::visit_method_from_sig(state, sig, func_body);
            } else {
                Self::visit_top_level_function(state, sig, func_body);
            }
        }
    }

    /// Visit a method from a `function_signature` node (inside a class body).
    fn visit_method_from_sig(
        state: &mut ExtractionState,
        sig_node: TsNode<'_>,
        body: Option<TsNode<'_>>,
    ) {
        let name = sig_node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, sig_node);
        let sig_text = state.node_text(sig_node);
        let signature = Some(sig_text.trim().to_string());

        let is_async = match body {
            Some(b) => state.node_text(b).starts_with("async"),
            None => false,
        };

        let start_line = sig_node.start_position().row as u32;
        let end_line = body.map_or(sig_node.end_position().row as u32, |b| {
            b.end_position().row as u32
        });
        let start_column = sig_node.start_position().column as u32;
        let end_column = body.map_or(sig_node.end_position().column as u32, |b| {
            b.end_position().column as u32
        });
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let metrics = body.map_or(ComplexityMetrics::default(), |b| {
            count_complexity(b, &DART_COMPLEXITY, &state.source)
        });

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        if let Some(body_node) = body {
            Self::extract_call_sites(state, body_node, &id);
        }

        // Extract annotation usages from the signature node and its parent
        // (method_signature or declaration wrapper).
        Self::extract_annotations_from_modifiers(state, sig_node, &id);
        if let Some(parent) = sig_node.parent() {
            Self::extract_annotations_from_modifiers(state, parent, &id);
        }
    }

    // ----------------------------------
    // Constructor
    // ----------------------------------

    fn visit_constructor(state: &mut ExtractionState, decl_node: TsNode<'_>, sig_node: TsNode<'_>) {
        let name = Self::extract_constructor_name(state, sig_node);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let signature = text.find('{').map_or_else(
            || Some(text.trim().trim_end_matches(';').trim().to_string()),
            |pos| Some(text[..pos].trim().to_string()),
        );

        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(decl_node, &DART_COMPLEXITY, &state.source);

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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    fn extract_constructor_name(state: &ExtractionState, sig_node: TsNode<'_>) -> String {
        let mut parts = Vec::new();
        let mut cursor = sig_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    parts.push(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        if parts.is_empty() {
            "<constructor>".to_string()
        } else {
            parts.join(".")
        }
    }

    // ----------------------------------
    // Getter / Setter
    // ----------------------------------

    fn visit_getter_or_setter(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        sig_node: TsNode<'_>,
    ) {
        let name = Self::find_child_by_kind(sig_node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::dart_visibility(&name);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let signature = text.find('{').map_or_else(
            || {
                text.find("=>").map_or_else(
                    || Some(text.trim().to_string()),
                    |pos| Some(text[..pos].trim().to_string()),
                )
            },
            |pos| Some(text[..pos].trim().to_string()),
        );

        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let metrics = count_complexity(decl_node, &DART_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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

    // ----------------------------------
    // Operator
    // ----------------------------------

    fn visit_operator(state: &mut ExtractionState, decl_node: TsNode<'_>, _sig_node: TsNode<'_>) {
        let text = state.node_text(decl_node);
        let name = text.find("operator").map_or_else(
            || "operator".to_string(),
            |pos| {
                let after = &text[pos + 8..];
                after
                    .trim()
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            },
        );
        let name = format!("operator {name}");

        let docstring = Self::extract_docstring(state, decl_node);
        let signature = text.find('{').map_or_else(
            || Some(text.trim().to_string()),
            |pos| Some(text[..pos].trim().to_string()),
        );

        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let metrics = count_complexity(decl_node, &DART_COMPLEXITY, &state.source);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Method,
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    // ----------------------------------
    // Fields
    // ----------------------------------

    /// Visit `initialized_variable_definition`: `Type name = value;`
    fn visit_initialized_var_def(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        var_def: TsNode<'_>,
        _has_static: bool,
    ) {
        let name = var_def.child_by_field_name("name").map_or_else(
            || {
                Self::find_child_by_kind(var_def, "identifier")
                    .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n))
            },
            |n| state.node_text(n),
        );

        Self::emit_field(state, decl_node, &name);
    }

    /// Visit `initialized_identifier_list` field: `Type name;` pattern
    /// where declaration has `type_identifier` + `initialized_identifier_list` children.
    fn visit_identifier_list_field(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        list_node: TsNode<'_>,
    ) {
        // initialized_identifier_list -> initialized_identifier -> identifier
        let mut cursor = list_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "initialized_identifier" {
                    if let Some(ident) = Self::find_child_by_kind(child, "identifier") {
                        let name = state.node_text(ident);
                        Self::emit_field(state, decl_node, &name);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit `static_final_declaration_list` fields.
    fn visit_static_final_declarations(
        state: &mut ExtractionState,
        decl_node: TsNode<'_>,
        list_node: TsNode<'_>,
    ) {
        let mut cursor = list_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "static_final_declaration" {
                    if let Some(ident) = Self::find_child_by_kind(child, "identifier") {
                        let name = state.node_text(ident);
                        Self::emit_field(state, decl_node, &name);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Emit a Field node.
    fn emit_field(state: &mut ExtractionState, decl_node: TsNode<'_>, name: &str) {
        let visibility = Self::dart_visibility(name);
        let docstring = Self::extract_docstring(state, decl_node);
        let text = state.node_text(decl_node);
        let start_line = decl_node.start_position().row as u32;
        let end_line = decl_node.end_position().row as u32;
        let start_column = decl_node.start_position().column as u32;
        let end_column = decl_node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Field,
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
    // Call site extraction
    // ----------------------------

    /// Recursively find call expressions inside a given node and create unresolved Calls references.
    /// Dart AST structure for calls: `identifier` followed by `selector` siblings containing
    /// `argument_part` / `arguments`.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return;
        }

        loop {
            let child = cursor.node();
            match child.kind() {
                "expression_statement"
                | "block"
                | "if_statement"
                | "for_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
                | "try_statement"
                | "return_statement"
                | "unary_expression"
                | "await_expression"
                | "conditional_expression"
                | "argument"
                | "named_argument"
                | "arguments"
                | "parenthesized_expression"
                | "assignment_expression"
                | "assignment_expression_without_cascade"
                | "local_variable_declaration"
                | "initialized_variable_definition"
                | "catch_clause"
                | "finally_clause"
                | "yield_statement"
                | "yield_each_statement"
                | "throw_expression"
                | "throw_expression_without_cascade"
                | "spread_element"
                | "for_element"
                | "if_element"
                | "list_literal"
                | "set_or_map_literal"
                | "cascade_section"
                | "switch_expression"
                | "switch_expression_case"
                | "switch_statement_case"
                | "switch_statement_default"
                | "pattern_assignment"
                | "pattern_variable_declaration"
                | "assert_statement"
                | "assert_builtin"
                | "assertion" => {
                    // Recurse into these container nodes.
                    Self::extract_call_sites(state, child, fn_node_id);
                }
                // tree-sitter-dart 0.2 wraps every function call in a
                // dedicated `call_expression { identifier, arguments }` node.
                // Earlier 0.1 code looked for `identifier + selector(arguments)`
                // sibling pairs (still kept below for compatibility with method
                // calls of the form `obj.method()`).
                "call_expression" => {
                    if let Some(ident) = Self::find_child_by_kind(child, "identifier") {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_node_id.to_string(),
                            reference_name: state.node_text(ident),
                            reference_kind: EdgeKind::Calls,
                            line: ident.start_position().row as u32,
                            column: ident.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                    // Recurse into arguments to catch nested calls.
                    Self::extract_call_sites(state, child, fn_node_id);
                }
                // An identifier node: check if followed by selector with arguments.
                "identifier" => {
                    let callee_name = state.node_text(child);
                    // Check if the next sibling is a selector containing argument_part.
                    if let Some(next) = child.next_named_sibling() {
                        if next.kind() == "selector"
                            && (Self::find_child_by_kind(next, "argument_part").is_some()
                                || Self::find_child_by_kind(next, "arguments").is_some())
                        {
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
                }
                // A selector that contains an identifier and argument_part: method call.
                "selector" => {
                    if Self::find_child_by_kind(child, "argument_part").is_some()
                        || Self::find_child_by_kind(child, "arguments").is_some()
                    {
                        // Look for identifier inside unconditional_assignable_selector.
                        if let Some(uas) =
                            Self::find_child_by_kind(child, "unconditional_assignable_selector")
                        {
                            if let Some(ident) = Self::find_child_by_kind(uas, "identifier") {
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
                    }
                    // Also recurse into selectors for nested calls in arguments.
                    Self::extract_call_sites(state, child, fn_node_id);
                }
                "argument_part" => {
                    // Recurse into argument_part for nested calls.
                    Self::extract_call_sites(state, child, fn_node_id);
                }
                // Skip nested function expressions to avoid polluting call sites.
                "function_expression" | "lambda_expression" => {}
                _ => {
                    // Recurse into other nodes.
                    Self::extract_call_sites(state, child, fn_node_id);
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract a signature by trimming at the first `{`.
    fn extract_signature_to_brace(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            text[..brace_pos].trim().to_string()
        } else {
            text.trim().to_string()
        }
    }

    /// Extract docstrings from preceding `documentation_comment` or `///` comment nodes.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "documentation_comment" => {
                    let text = state.node_text(sibling);
                    comments.push(text);
                    current = sibling.prev_named_sibling();
                }
                "comment" => {
                    let text = state.node_text(sibling);
                    if text.trim_start().starts_with("///") {
                        comments.push(text);
                        current = sibling.prev_named_sibling();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        let cleaned: Vec<String> = comments
            .iter()
            .map(|c| Self::clean_doc_comment(c))
            .collect();
        let result = cleaned.join("\n").trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Clean a Dart doc comment. Handles `///` style and `/** ... */` style.
    fn clean_doc_comment(comment: &str) -> String {
        let trimmed = comment.trim();
        if trimmed.contains("///") {
            return trimmed
                .lines()
                .map(|line| {
                    let l = line.trim();
                    if let Some(stripped) = l.strip_prefix("///") {
                        stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
        }
        if trimmed.starts_with("/**") && trimmed.ends_with("*/") {
            let inner = &trimmed[3..trimmed.len() - 2];
            return inner
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
                .to_string();
        }
        trimmed.to_string()
    }

    /// Determine Dart visibility: names starting with `_` are private, everything else is public.
    fn dart_visibility(name: &str) -> Visibility {
        if name.starts_with('_') {
            Visibility::Private
        } else {
            Visibility::Pub
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

    /// Walk previous siblings and children of a declaration looking for
    /// `annotation` or `marker_annotation` nodes and extract annotation usages.
    fn extract_annotations_from_modifiers(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        // Check previous siblings of the declaration.
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            match sibling.kind() {
                "annotation" | "marker_annotation" => {
                    Self::extract_annotations_from_node(state, sibling, target_id);
                    current = sibling.prev_named_sibling();
                }
                "comment" | "documentation_comment" => {
                    current = sibling.prev_named_sibling();
                }
                _ => break,
            }
        }
        // Also check children of the declaration node.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "annotation" || child.kind() == "marker_annotation" {
                    Self::extract_annotations_from_node(state, child, target_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Create an `AnnotationUsage` node and edges for a single annotation node.
    fn extract_annotations_from_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let annot_name = Self::extract_annotation_name(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::@{}", state.qualified_prefix(), annot_name);
        let id = generate_node_id(
            &state.file_path,
            &NodeKind::AnnotationUsage,
            &annot_name,
            start_line,
        );

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::AnnotationUsage,
            name: annot_name.clone(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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

        // Annotates unresolved ref.
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id.clone(),
            reference_name: annot_name,
            reference_kind: EdgeKind::Annotates,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });

        // Direct Annotates edge from the annotation to the target.
        state.edges.push(Edge {
            source: id,
            target: target_id.to_string(),
            kind: EdgeKind::Annotates,
            line: Some(start_line),
        });
    }

    /// Extract the name from a Dart annotation node.
    ///
    /// Looks for an `identifier` child, or falls back to text after `@`, before `(`.
    fn extract_annotation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(ident) = Self::find_child_by_kind(node, "identifier") {
            return state.node_text(ident);
        }
        // Fallback: text after '@', before '('.
        let text = state.node_text(node);
        text.trim()
            .strip_prefix('@')
            .unwrap_or(&text)
            .split('(')
            .next()
            .unwrap_or(&text)
            .trim()
            .to_string()
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

impl crate::extraction::LanguageExtractor for DartExtractor {
    fn extensions(&self) -> &[&str] {
        &["dart"]
    }

    fn language_name(&self) -> &'static str {
        "Dart"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        DartExtractor::extract_dart(file_path, source)
    }
}
