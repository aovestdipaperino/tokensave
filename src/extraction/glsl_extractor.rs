/// Tree-sitter based GLSL (OpenGL Shading Language) source code extractor.
///
/// Parses GLSL source files and emits nodes and edges for the code graph.
/// Handles `.glsl`, `.vert`, `.frag`, `.geom`, `.comp`, `.tesc`, `.tese` files.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, C_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from GLSL source files using tree-sitter.
pub struct GlslExtractor;

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

impl GlslExtractor {
    pub fn extract_source(file_path: &str, source: &str) -> ExtractionResult {
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

        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        Self::build_result(state, start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("glsl");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load GLSL grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

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

    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "function_definition" => Self::visit_function_definition(state, node),
            "declaration" => Self::visit_declaration(state, node),
            "struct_specifier" => Self::visit_standalone_struct(state, node),
            "preproc_def" => Self::visit_preproc_def(state, node),
            "preproc_include" => Self::visit_preproc_include(state, node),
            _ => {}
        }
    }

    // -------------------------------------------------------
    // function_definition
    // -------------------------------------------------------

    fn visit_function_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        let name =
            Self::extract_function_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let signature = Some(Self::extract_function_signature(state, node));
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);
        let metrics = count_complexity(node, &C_COMPLEXITY, &state.source);

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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the function body.
        if let Some(body) = Self::find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    fn extract_function_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(declarator) = Self::find_descendant_by_kind(node, "function_declarator") {
            if let Some(ident) = Self::find_child_by_kind(declarator, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        let text = state.node_text(node);
        if let Some(brace_pos) = text.find('{') {
            text[..brace_pos].trim().to_string()
        } else {
            text.trim().trim_end_matches(';').trim().to_string()
        }
    }

    // -------------------------------------------------------
    // declaration (globals, uniforms, in/out, prototypes)
    // -------------------------------------------------------

    fn visit_declaration(state: &mut ExtractionState, node: TsNode<'_>) {
        // Function prototype
        if Self::find_descendant_by_kind(node, "function_declarator").is_some() {
            Self::visit_function_prototype(state, node);
            return;
        }

        // Struct declaration
        if Self::has_child_kind(node, "struct_specifier") {
            Self::visit_children(state, node);
            return;
        }

        // Global variable / uniform / in / out / varying / attribute
        Self::visit_global_variable(state, node);
    }

    fn visit_function_prototype(state: &mut ExtractionState, node: TsNode<'_>) {
        let name =
            Self::extract_function_name(state, node).unwrap_or_else(|| "<anonymous>".to_string());
        let text = state.node_text(node);
        let signature = Some(text.trim().trim_end_matches(';').trim().to_string());
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Function, &name, start_line);

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

    fn visit_global_variable(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name) = Self::extract_variable_name(state, node) else {
            return;
        };

        let text = state.node_text(node);
        let text_trimmed = text.trim();

        // Classify GLSL storage-qualified declarations.
        let (kind, visibility) = if Self::has_qualifier(state, node, "uniform") {
            (NodeKind::Const, Visibility::Pub)
        } else if Self::has_qualifier(state, node, "in")
            || Self::has_qualifier(state, node, "varying")
            || Self::has_qualifier(state, node, "attribute")
            || Self::has_qualifier(state, node, "out")
        {
            (NodeKind::Field, Visibility::Pub)
        } else if text_trimmed.starts_with("const ") || text_trimmed.contains(" const ") {
            (NodeKind::Const, Visibility::Private)
        } else {
            (NodeKind::Static, Visibility::Private)
        };

        let signature = Some(text_trimmed.trim_end_matches(';').trim().to_string());
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);

        let graph_node = Node {
            id: id.clone(),
            kind,
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

    fn extract_variable_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // init_declarator: `int x = 0;`
        if let Some(init_decl) = Self::find_child_by_kind(node, "init_declarator") {
            if let Some(ident) = Self::find_child_by_kind(init_decl, "identifier") {
                return Some(state.node_text(ident));
            }
            // array declarator: `float arr[3] = ...`
            if let Some(arr) = Self::find_child_by_kind(init_decl, "array_declarator") {
                if let Some(ident) = Self::find_child_by_kind(arr, "identifier") {
                    return Some(state.node_text(ident));
                }
            }
        }
        // Direct identifier: `uniform vec3 lightPos;`
        if let Some(ident) = Self::find_child_by_kind(node, "identifier") {
            return Some(state.node_text(ident));
        }
        // Array declarator without init: `in vec2 texCoords[];`
        if let Some(arr) = Self::find_child_by_kind(node, "array_declarator") {
            if let Some(ident) = Self::find_child_by_kind(arr, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    // -------------------------------------------------------
    // struct_specifier
    // -------------------------------------------------------

    fn visit_standalone_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        if Self::find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }

        let name = Self::find_child_by_kind(node, "type_identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let docstring = Self::extract_docstring(state, node);
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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract struct fields.
        if let Some(field_list) = Self::find_child_by_kind(node, "field_declaration_list") {
            state.node_stack.push((name, id));
            Self::visit_struct_fields(state, field_list);
            state.node_stack.pop();
        }
    }

    fn visit_struct_fields(state: &mut ExtractionState, field_list: TsNode<'_>) {
        let mut cursor = field_list.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "field_declaration" {
                    Self::visit_struct_field(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_struct_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name) = Self::find_field_name(state, node) else {
            return;
        };
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let text = state.node_text(node);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Field, &name, start_line);

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
            signature: Some(text.trim().trim_end_matches(';').trim().to_string()),
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

    fn find_field_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // field_identifier is used in struct field declarations
        if let Some(fi) = Self::find_child_by_kind(node, "field_identifier") {
            return Some(state.node_text(fi));
        }
        if let Some(ident) = Self::find_child_by_kind(node, "identifier") {
            return Some(state.node_text(ident));
        }
        // Array field: `float values[4];`
        if let Some(arr) = Self::find_child_by_kind(node, "array_declarator") {
            if let Some(fi) = Self::find_child_by_kind(arr, "field_identifier") {
                return Some(state.node_text(fi));
            }
            if let Some(ident) = Self::find_child_by_kind(arr, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    // -------------------------------------------------------
    // Preprocessor
    // -------------------------------------------------------

    fn visit_preproc_def(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

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

        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }
    }

    fn visit_preproc_include(state: &mut ExtractionState, node: TsNode<'_>) {
        let include_path = Self::find_child_by_kind(node, "string_literal")
            .or_else(|| Self::find_child_by_kind(node, "system_lib_string"))
            .map_or_else(|| "<unknown>".to_string(), |n| state.node_text(n));

        let line = node.start_position().row as u32;
        let column = node.start_position().column as u32;

        if let Some(parent_id) = state.parent_node_id() {
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: parent_id.to_string(),
                reference_name: include_path,
                reference_kind: EdgeKind::Uses,
                line,
                column,
                file_path: state.file_path.clone(),
            });
        }
    }

    // -------------------------------------------------------
    // Call site extraction
    // -------------------------------------------------------

    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "call_expression" {
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
                    Self::extract_call_sites(state, child, fn_node_id);
                } else {
                    Self::extract_call_sites(state, child, fn_node_id);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // -------------------------------------------------------
    // Docstring extraction
    // -------------------------------------------------------

    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            if sibling.kind() == "comment" {
                comments.push(state.node_text(sibling));
                current = sibling.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        let cleaned: Vec<String> = comments.iter().map(|c| Self::clean_comment(c)).collect();
        let result = cleaned.join("\n").trim().to_string();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn clean_comment(comment: &str) -> String {
        let trimmed = comment.trim();
        if let Some(stripped) = trimmed.strip_prefix("//") {
            stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
        } else if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            let inner = &trimmed[2..trimmed.len() - 2];
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

    // -------------------------------------------------------
    // Utility helpers
    // -------------------------------------------------------

    /// Check if a declaration has a GLSL storage qualifier (uniform, in, out, etc.)
    /// or a type qualifier (const, etc.).
    ///
    /// tree-sitter-glsl emits qualifiers either as direct child nodes whose
    /// `kind()` matches the keyword (e.g. `"uniform"`, `"in"`, `"out"`) or
    /// nested inside a `type_qualifier` wrapper (e.g. `type_qualifier > const`).
    fn has_qualifier(_state: &ExtractionState, node: TsNode<'_>, qualifier: &str) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                let kind = child.kind();
                // Direct qualifier keyword: `uniform`, `in`, `out`, `varying`, `attribute`, `buffer`
                if kind == qualifier {
                    return true;
                }
                // Wrapped in type_qualifier: `type_qualifier > const`
                if kind == "type_qualifier" {
                    if let Some(inner) = Self::find_child_by_kind(child, qualifier) {
                        let _ = inner;
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

    fn has_child_kind(node: TsNode<'_>, kind: &str) -> bool {
        Self::find_child_by_kind(node, kind).is_some()
    }

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

    fn find_descendant_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == kind {
                    return Some(child);
                }
                if let Some(found) = Self::find_descendant_by_kind(child, kind) {
                    return Some(found);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
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

impl crate::extraction::LanguageExtractor for GlslExtractor {
    fn extensions(&self) -> &[&str] {
        &["glsl", "vert", "frag", "geom", "comp", "tesc", "tese"]
    }

    fn language_name(&self) -> &'static str {
        "GLSL"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        GlslExtractor::extract_source(file_path, source)
    }
}
