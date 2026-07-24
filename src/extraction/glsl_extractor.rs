/// Tree-sitter based GLSL (OpenGL Shading Language) source code extractor.
///
/// Parses GLSL source files and emits nodes and edges for the code graph.
/// Handles `.glsl`, `.vert`, `.frag`, `.geom`, `.comp`, `.tesc`, `.tese`
/// files, plus Godot's GLSL-dialect shaders (`.gdshader`, `.gdshaderinc`,
/// #270) via a light dialect rewrite before parsing.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, C_COMPLEXITY};
use crate::extraction::ts_state::{find_child_by_kind, has_child_kind, ExtractionState};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from GLSL source files using tree-sitter.
pub struct GlslExtractor;

/// A Godot shader file-level directive (`shader_type`, `render_mode`)
/// captured while rewriting the dialect to plain GLSL.
struct GodotDirective {
    keyword: &'static str,
    text: String,
    line: u32,
}

impl GlslExtractor {
    pub fn extract_source(file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_source_inner(file_path, source, source)
    }

    /// Extract from `parse_src` (a possibly dialect-normalized copy of
    /// `source`) while reading all node text — signatures, excerpts,
    /// docstrings — from the original `source`. The two must have identical
    /// byte length and line structure so tree-sitter byte ranges index both
    /// interchangeably; `rewrite_godot_dialect` guarantees this by blanking
    /// with spaces. This keeps e.g. a `: hint_range(…)` clause visible in a
    /// uniform's indexed signature even though the parser never sees it.
    fn extract_source_inner(file_path: &str, source: &str, parse_src: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(parse_src) {
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

        let root = tree.root_node();
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        state.build_result(start)
    }

    /// Extract a Godot `.gdshader` / `.gdshaderinc` file (#270).
    ///
    /// Godot's shading language is GLSL-family with a handful of dialect
    /// additions the vanilla GLSL grammar rejects: `shader_type` /
    /// `render_mode` / `group_uniforms` / `stencil_mode` file directives,
    /// `global` / `instance` uniform qualifiers, and `: hint_…` clauses on
    /// uniform declarations. Rather than bundling a dedicated grammar, the
    /// source is rewritten to plain GLSL first — every rewrite replaces
    /// text with spaces of identical byte length, so all node positions in
    /// the parsed output still match the original file. Captured
    /// `shader_type` / `render_mode` directives are re-emitted as `Const`
    /// nodes under the file.
    pub fn extract_gdshader(file_path: &str, source: &str) -> ExtractionResult {
        let (rewritten, directives) = Self::rewrite_godot_dialect(source);
        let mut result = Self::extract_source_inner(file_path, source, &rewritten);

        let file_info = result
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::File)
            .map(|n| (n.id.clone(), n.updated_at));
        if let Some((file_id, timestamp)) = file_info {
            for directive in directives {
                let id = generate_node_id(
                    file_path,
                    &NodeKind::Const,
                    directive.keyword,
                    directive.line,
                );
                result.nodes.push(Node {
                    id: id.clone(),
                    kind: NodeKind::Const,
                    name: directive.keyword.to_string(),
                    qualified_name: format!("{file_path}::{}", directive.keyword),
                    file_path: file_path.to_string(),
                    start_line: directive.line,
                    attrs_start_line: directive.line,
                    end_line: directive.line,
                    start_column: 0,
                    end_column: 0,
                    signature: Some(directive.text),
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
                    updated_at: timestamp,
                    parent_id: None,
                });
                result.edges.push(Edge {
                    source: file_id.clone(),
                    target: id,
                    kind: EdgeKind::Contains,
                    line: Some(directive.line),
                });
            }
        }
        result
    }

    /// Rewrite Godot shader dialect constructs into space padding so the
    /// GLSL grammar parses the remainder. Byte length and line structure
    /// are preserved exactly.
    fn rewrite_godot_dialect(source: &str) -> (String, Vec<GodotDirective>) {
        let mut out = String::with_capacity(source.len());
        let mut directives = Vec::new();
        for (line_no, raw) in source.split_inclusive('\n').enumerate() {
            let (line, newline) = match raw.strip_suffix('\n') {
                Some(stripped) => (stripped, "\n"),
                None => (raw, ""),
            };
            out.push_str(&Self::rewrite_godot_line(
                line,
                line_no as u32,
                &mut directives,
            ));
            out.push_str(newline);
        }
        (out, directives)
    }

    /// Rewrite a single line of Godot shader dialect (see
    /// [`Self::rewrite_godot_dialect`]).
    fn rewrite_godot_line(
        line: &str,
        line_no: u32,
        directives: &mut Vec<GodotDirective>,
    ) -> String {
        let trimmed = line.trim();
        let first = trimmed.split_whitespace().next().unwrap_or("");
        match first {
            "shader_type" | "render_mode" => {
                let keyword = if first == "shader_type" {
                    "shader_type"
                } else {
                    "render_mode"
                };
                directives.push(GodotDirective {
                    keyword,
                    text: trimmed.trim_end_matches(';').trim().to_string(),
                    line: line_no,
                });
                return Self::blank_span(line, 0, line.len());
            }
            // Godot-only grouping/stencil directives carry no symbols.
            "group_uniforms" | "stencil_mode" => {
                return Self::blank_span(line, 0, line.len());
            }
            _ => {}
        }

        // Uniform declarations: blank the Godot-only `global` / `instance`
        // qualifier and any `: hint_…` clause (up to the default value or
        // terminator) so the declaration parses as plain GLSL.
        if matches!(first, "uniform" | "global" | "instance") {
            let mut rewritten = line.to_string();
            if first != "uniform" {
                if let Some(pos) = rewritten.find(first) {
                    rewritten = Self::blank_span(&rewritten, pos, pos + first.len());
                }
            }
            if let Some(colon) = rewritten.find(':') {
                let tail = &rewritten[colon..];
                let end_rel = tail.find(['=', ';']).unwrap_or(tail.len());
                rewritten = Self::blank_span(&rewritten, colon, colon + end_rel);
            }
            return rewritten;
        }
        line.to_string()
    }

    /// Replace `[start, end)` (byte offsets) with spaces, one per byte, so
    /// the result has identical byte length and all positions outside the
    /// span are unchanged.
    fn blank_span(line: &str, start: usize, end: usize) -> String {
        let mut out = String::with_capacity(line.len());
        for (i, ch) in line.char_indices() {
            if i >= start && i < end {
                for _ in 0..ch.len_utf8() {
                    out.push(' ');
                }
            } else {
                out.push(ch);
            }
        }
        out
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
            cognitive_complexity: metrics.cognitive_complexity,
            distinct_operators: metrics.distinct_operators,
            distinct_operands: metrics.distinct_operands,
            total_operators: metrics.total_operators,
            total_operands: metrics.total_operands,
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
            });
        }

        // Extract call sites from the function body.
        if let Some(body) = find_child_by_kind(node, "compound_statement") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    fn extract_function_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(declarator) = Self::find_descendant_by_kind(node, "function_declarator") {
            if let Some(ident) = find_child_by_kind(declarator, "identifier") {
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
        if has_child_kind(node, "struct_specifier") {
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
            });
        }
    }

    fn extract_variable_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // init_declarator: `int x = 0;`
        if let Some(init_decl) = find_child_by_kind(node, "init_declarator") {
            if let Some(ident) = find_child_by_kind(init_decl, "identifier") {
                return Some(state.node_text(ident));
            }
            // array declarator: `float arr[3] = ...`
            if let Some(arr) = find_child_by_kind(init_decl, "array_declarator") {
                if let Some(ident) = find_child_by_kind(arr, "identifier") {
                    return Some(state.node_text(ident));
                }
            }
        }
        // Direct identifier: `uniform vec3 lightPos;`
        if let Some(ident) = find_child_by_kind(node, "identifier") {
            return Some(state.node_text(ident));
        }
        // Array declarator without init: `in vec2 texCoords[];`
        if let Some(arr) = find_child_by_kind(node, "array_declarator") {
            if let Some(ident) = find_child_by_kind(arr, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    // -------------------------------------------------------
    // struct_specifier
    // -------------------------------------------------------

    fn visit_standalone_struct(state: &mut ExtractionState, node: TsNode<'_>) {
        if find_child_by_kind(node, "field_declaration_list").is_none() {
            return;
        }

        let name = find_child_by_kind(node, "type_identifier")
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
            });
        }

        // Extract struct fields.
        if let Some(field_list) = find_child_by_kind(node, "field_declaration_list") {
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
            });
        }
    }

    fn find_field_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // field_identifier is used in struct field declarations
        if let Some(fi) = find_child_by_kind(node, "field_identifier") {
            return Some(state.node_text(fi));
        }
        if let Some(ident) = find_child_by_kind(node, "identifier") {
            return Some(state.node_text(ident));
        }
        // Array field: `float values[4];`
        if let Some(arr) = find_child_by_kind(node, "array_declarator") {
            if let Some(fi) = find_child_by_kind(arr, "field_identifier") {
                return Some(state.node_text(fi));
            }
            if let Some(ident) = find_child_by_kind(arr, "identifier") {
                return Some(state.node_text(ident));
            }
        }
        None
    }

    // -------------------------------------------------------
    // Preprocessor
    // -------------------------------------------------------

    fn visit_preproc_def(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = find_child_by_kind(node, "identifier")
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
            });
        }
    }

    fn visit_preproc_include(state: &mut ExtractionState, node: TsNode<'_>) {
        let include_path = find_child_by_kind(node, "string_literal")
            .or_else(|| find_child_by_kind(node, "system_lib_string"))
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
                    if let Some(inner) = find_child_by_kind(child, qualifier) {
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
}

impl crate::extraction::LanguageExtractor for GlslExtractor {
    fn extensions(&self) -> &[&str] {
        &[
            "glsl",
            "vert",
            "frag",
            "geom",
            "comp",
            "tesc",
            "tese",
            "gdshader",
            "gdshaderinc",
        ]
    }

    fn language_name(&self) -> &'static str {
        "GLSL"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        if file_path.ends_with(".gdshader") || file_path.ends_with(".gdshaderinc") {
            GlslExtractor::extract_gdshader(file_path, source)
        } else {
            GlslExtractor::extract_source(file_path, source)
        }
    }
}
