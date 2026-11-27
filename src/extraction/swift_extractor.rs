/// Tree-sitter based Swift source code extractor.
///
/// Parses Swift source files and emits nodes and edges for the code graph.
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, SWIFT_COMPLEXITY};
use crate::extraction::swift_constants::{
    MAX_DECL_SIGNATURE_CHARS, SWIFT_CALLBACK_ARG_LABELS, SWIFT_HIGHER_ORDER_CALLEES,
    SWIFT_LITERAL_IDENTIFIERS, SWIFT_TRAILING_CLOSURE_LABEL,
};
use crate::extraction::ts_state::{find_child_by_kind, ExtractionState};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Lexical type and binding scope tracked per type/function body while extracting
/// Swift call sites so typed receivers resolve precisely without global guesses.
#[derive(Default)]
struct SwiftScope {
    enclosing_types: Vec<String>,
    /// Bound names per lexical frame with their inferred type, `None` when the
    /// binding's type is unknown: an untyped local still shadows outer bindings.
    type_frames: Vec<HashMap<String, Option<String>>>,
    local_frames: Vec<HashSet<String>>,
    /// Function/closure-only locals that shadow enclosing type members.
    local_value_frames: Vec<HashSet<String>>,
    /// Nested local function names (`func helper()`) bound in lexical frames.
    local_fn_frames: Vec<HashSet<String>>,
    /// Callable member names (methods + computed properties) per enclosing type.
    callable_frames: Vec<HashSet<String>>,
    /// Computed property names per enclosing type.
    computed_prop_frames: Vec<HashSet<String>>,
    /// Depth of enclosing `private` / `fileprivate` extensions whose members
    /// default to file-private visibility.
    private_extension_depth: usize,
}

impl SwiftScope {
    fn enclosing_type(&self) -> Option<&str> {
        self.enclosing_types.last().map(String::as_str)
    }

    fn push_type(&mut self, name: &str) {
        self.enclosing_types.push(name.to_string());
        self.type_frames.push(HashMap::new());
        self.local_frames.push(HashSet::new());
        self.callable_frames.push(HashSet::new());
        self.computed_prop_frames.push(HashSet::new());
    }

    fn pop_type(&mut self) {
        self.computed_prop_frames.pop();
        self.callable_frames.pop();
        self.local_frames.pop();
        self.type_frames.pop();
        self.enclosing_types.pop();
    }

    fn push_frame(&mut self) {
        self.type_frames.push(HashMap::new());
        self.local_frames.push(HashSet::new());
        self.local_value_frames.push(HashSet::new());
        self.local_fn_frames.push(HashSet::new());
    }

    fn pop_frame(&mut self) {
        self.local_fn_frames.pop();
        self.local_value_frames.pop();
        self.local_frames.pop();
        self.type_frames.pop();
    }

    fn bind_local_function(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        if let Some(fns) = self.local_fn_frames.last_mut() {
            fns.insert(name);
        }
    }

    fn is_local_function(&self, name: &str) -> bool {
        self.local_fn_frames.iter().rev().any(|f| f.contains(name))
    }

    fn in_private_extension(&self) -> bool {
        self.private_extension_depth > 0
    }

    fn bind(&mut self, name: String, ty: Option<String>, is_local_value: bool) {
        if name.is_empty() {
            return;
        }
        if let Some(locals) = self.local_frames.last_mut() {
            locals.insert(name.clone());
        }
        if is_local_value {
            if let Some(vals) = self.local_value_frames.last_mut() {
                vals.insert(name.clone());
            }
        }
        if let Some(frames) = self.type_frames.last_mut() {
            frames.insert(name, ty);
        }
    }

    fn register_member_callable(&mut self, name: String, is_computed_prop: bool) {
        if name.is_empty() {
            return;
        }
        if let Some(callables) = self.callable_frames.last_mut() {
            callables.insert(name.clone());
        }
        if is_computed_prop {
            if let Some(props) = self.computed_prop_frames.last_mut() {
                props.insert(name);
            }
        }
    }

    fn lookup_type(&self, name: &str) -> Option<&str> {
        self.type_frames
            .iter()
            .rev()
            .find_map(|frame| frame.get(name))
            .and_then(|ty| ty.as_deref())
    }

    fn is_bound_local(&self, name: &str) -> bool {
        self.local_frames.iter().rev().any(|f| f.contains(name))
    }

    fn is_shadowed_by_local_value(&self, name: &str) -> bool {
        self.local_value_frames
            .iter()
            .rev()
            .any(|f| f.contains(name))
    }

    fn owning_type_for_callable(&self, name: &str) -> Option<&str> {
        self.enclosing_types
            .iter()
            .zip(self.callable_frames.iter())
            .rev()
            .find(|(_, callables)| callables.contains(name))
            .map(|(ty, _)| ty.as_str())
    }

    fn owning_type_for_computed_prop(&self, name: &str) -> Option<&str> {
        self.enclosing_types
            .iter()
            .zip(self.computed_prop_frames.iter())
            .rev()
            .find(|(_, props)| props.contains(name))
            .map(|(ty, _)| ty.as_str())
    }
}

/// Extracts code graph nodes and edges from Swift source files using tree-sitter.
pub struct SwiftExtractor;

impl SwiftExtractor {
    /// Extract code graph nodes and edges from a Swift source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Swift source code to parse.
    pub fn extract_swift(file_path: &str, source: &str) -> ExtractionResult {
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
        let mut scope = SwiftScope::default();
        scope.push_frame();
        Self::visit_children(&mut state, &mut scope, root);
        scope.pop_frame();

        state.node_stack.pop();

        state.build_result(start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("swift");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Swift grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit all children of a node.
    fn visit_children(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                Self::visit_node(state, scope, child);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Visit a single AST node, dispatching on its type.
    fn visit_node(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        match node.kind() {
            "import_declaration" => Self::visit_import(state, node),
            "class_declaration" => Self::visit_class_declaration(state, scope, node),
            "protocol_declaration" => Self::visit_protocol(state, scope, node),
            "function_declaration" => Self::visit_function(state, scope, node),
            "init_declaration" => Self::visit_init(state, scope, node),
            "property_declaration" => Self::visit_property(state, scope, node),
            "typealias_declaration" => Self::visit_typealias(state, node),
            // Any other statement (top-level code in `main.swift`, a `deinit`
            // or `subscript` in a type body) can hold call sites. Attribute
            // them to the enclosing scope: the File node at top level, the
            // type in a type body. `extract_call_sites` stops at nested
            // declarations, so their bodies are not attributed here.
            _ => {
                if let Some(scope_id) = state.parent_node_id().map(str::to_string) {
                    Self::extract_call_sites(state, scope, node, &scope_id);
                }
            }
        }
    }

    // ----------------------------------
    // Import
    // ----------------------------------

    /// Extract an import declaration (e.g. `import Foundation`).
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = find_child_by_kind(node, "identifier").map_or_else(
            || {
                // Fallback: get everything after "import "
                let text = state.node_text(node);
                text.strip_prefix("import ")
                    .unwrap_or(&text)
                    .trim()
                    .to_string()
            },
            |n| state.node_text(n),
        );

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

    // ----------------------------------
    // class_declaration (class, struct, enum, extension)
    // ----------------------------------

    /// Dispatch `class_declaration` based on the `declaration_kind` field.
    ///
    /// tree-sitter-swift uses `class_declaration` for class, struct, enum, and extension.
    /// The first anonymous child carries the keyword: "class", "struct", "enum", or "extension".
    fn visit_class_declaration(
        state: &mut ExtractionState,
        scope: &mut SwiftScope,
        node: TsNode<'_>,
    ) {
        let decl_kind = Self::declaration_kind_keyword(state, node);
        match decl_kind.as_str() {
            "struct" => Self::visit_struct(state, scope, node),
            "enum" => Self::visit_enum(state, scope, node),
            "extension" => Self::visit_extension(state, scope, node),
            _ => Self::visit_class(state, scope, node),
        }
    }

    /// Returns the declaration keyword ("class", "struct", "enum", "extension") for a
    /// `class_declaration` node by reading the first child with field "`declaration_kind`".
    fn declaration_kind_keyword(state: &ExtractionState, node: TsNode<'_>) -> String {
        // The keyword is stored as the first anonymous child with field name "declaration_kind".
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.field_name() == Some("declaration_kind") {
                    return state.node_text(cursor.node());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        // Fallback: check first child text.
        if let Some(first) = node.child(0) {
            return state.node_text(first);
        }
        "class".to_string()
    }

    /// Extract a class definition.
    fn visit_class(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = Self::extract_type_name(state, node);
        let visibility = Self::extract_member_visibility(state, scope, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_decl_signature(state, node);
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

        // Extract inheritance (class Foo: Bar).
        Self::extract_inheritance(state, node, &id);

        // Extract attribute annotations (e.g. @objc, @available).
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Visit class body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        scope.push_type(&name);
        if let Some(body) = node.child_by_field_name("body") {
            Self::seed_type_member_bindings(state, scope, body);
            Self::visit_children(state, scope, body);
        }
        scope.pop_type();
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a struct definition.
    fn visit_struct(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = Self::extract_type_name(state, node);
        let visibility = Self::extract_member_visibility(state, scope, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_decl_signature(state, node);
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

        // Extract protocol conformances (struct Foo: Bar).
        Self::extract_inheritance(state, node, &id);

        // Extract attribute annotations.
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Visit struct body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        scope.push_type(&name);
        if let Some(body) = node.child_by_field_name("body") {
            Self::seed_type_member_bindings(state, scope, body);
            Self::visit_children(state, scope, body);
        }
        scope.pop_type();
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract an enum definition.
    fn visit_enum(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = Self::extract_type_name(state, node);
        let visibility = Self::extract_member_visibility(state, scope, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_decl_signature(state, node);
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

        // Extract raw type and protocol conformances (enum Foo: String, Bar).
        Self::extract_inheritance(state, node, &id);

        // Visit enum body (enum_class_body with enum_entry children).
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        scope.push_type(&name);
        if let Some(body) = node.child_by_field_name("body") {
            Self::seed_type_member_bindings(state, scope, body);
            Self::visit_enum_body(state, scope, body);
        }
        scope.pop_type();
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit enum body children, extracting enum entries as `EnumVariant` nodes.
    fn visit_enum_body(state: &mut ExtractionState, scope: &mut SwiftScope, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "enum_entry" => Self::visit_enum_entry(state, child),
                    "function_declaration" => Self::visit_function(state, scope, child),
                    "init_declaration" => Self::visit_init(state, scope, child),
                    "property_declaration" => Self::visit_property(state, scope, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract an enum entry as an `EnumVariant` node.
    fn visit_enum_entry(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| find_child_by_kind(node, "simple_identifier").map(|n| state.node_text(n)))
            .unwrap_or_else(|| {
                // Fallback: parse text after "case "
                let text = state.node_text(node);
                text.strip_prefix("case ")
                    .unwrap_or(&text)
                    .trim()
                    .to_string()
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

    /// Extract an extension declaration.
    fn visit_extension(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        // For extension, the name is in a user_type or type_identifier child with field "name".
        let name = node.child_by_field_name("name").map_or_else(
            || "<anonymous>".to_string(),
            |n| {
                // Could be a user_type wrapping a type_identifier.
                find_child_by_kind(n, "type_identifier")
                    .map_or_else(|| state.node_text(n), |ti| state.node_text(ti))
            },
        );

        let visibility = Self::extract_visibility(state, node);
        let is_private_extension = visibility == Visibility::Private;
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_decl_signature(state, node);
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

        // Extract protocol conformances added by the extension (extension Foo: Bar).
        Self::extract_inheritance(state, node, &id);

        // Visit extension body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        scope.push_type(&name);
        if is_private_extension {
            scope.private_extension_depth += 1;
        }
        if let Some(body) = node.child_by_field_name("body") {
            Self::seed_type_member_bindings(state, scope, body);
            Self::visit_children(state, scope, body);
        }
        if is_private_extension {
            scope.private_extension_depth -= 1;
        }
        scope.pop_type();
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    // ----------------------------------
    // Protocol
    // ----------------------------------

    /// Extract a protocol declaration (maps to Interface).
    fn visit_protocol(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Self::extract_member_visibility(state, scope, node);
        let docstring = Self::extract_docstring(state, node);
        let signature = Self::extract_decl_signature(state, node);
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

        // Extract inherited protocols (protocol Foo: Bar). These stay `Extends`.
        Self::extract_inheritance(state, node, &id);

        // Extract attribute annotations.
        Self::extract_annotations_from_modifiers(state, node, &id);

        // Visit protocol body. Protocol functions are protocol_function_declaration.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        scope.push_type(&name);
        if let Some(body) = node.child_by_field_name("body") {
            Self::seed_type_member_bindings(state, scope, body);
            Self::visit_protocol_body(state, scope, body);
        }
        scope.pop_type();
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Visit protocol body children, extracting protocol function declarations as Method nodes.
    fn visit_protocol_body(state: &mut ExtractionState, scope: &mut SwiftScope, body: TsNode<'_>) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "protocol_function_declaration" => {
                        Self::visit_protocol_function(state, child);
                    }
                    "property_declaration" => Self::visit_property(state, scope, child),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a protocol function declaration as a Method node.
    fn visit_protocol_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| find_child_by_kind(node, "simple_identifier").map(|n| state.node_text(n)))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Method, &name, start_line);
        let signature = Self::extract_decl_signature(state, node);

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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    // ----------------------------------
    // Function / Method
    // ----------------------------------

    /// Extract a function or method declaration.
    ///
    /// If `class_depth` > 0, it becomes a Method; otherwise a Function.
    fn visit_function(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| find_child_by_kind(node, "simple_identifier").map(|n| state.node_text(n)))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let in_class = state.class_depth > 0;
        let kind = if in_class {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = Self::extract_member_visibility(state, scope, node);
        let is_async = Self::has_async_keyword(node, &state.source);
        let signature = Self::extract_decl_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &SWIFT_COMPLEXITY, &state.source);

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

        // Seed parameter bindings, then extract call sites from the function body.
        scope.push_frame();
        Self::seed_parameter_bindings(state, scope, node);
        Self::extract_default_param_func_refs(state, scope, node, &id);
        Self::extract_call_sites(state, scope, node, &id);
        scope.pop_frame();

        // Extract attribute annotations (e.g. @discardableResult, @objc).
        Self::extract_annotations_from_modifiers(state, node, &id);
    }

    // ----------------------------------
    // Init (Constructor)
    // ----------------------------------

    /// Extract an init declaration as a Constructor node.
    fn visit_init(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = "init".to_string();
        let visibility = Self::extract_member_visibility(state, scope, node);
        let signature = Self::extract_decl_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Constructor, &name, start_line);
        let metrics = count_complexity(node, &SWIFT_COMPLEXITY, &state.source);

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

        // Seed parameter bindings, then extract call sites from the init body.
        scope.push_frame();
        Self::seed_parameter_bindings(state, scope, node);
        Self::extract_default_param_func_refs(state, scope, node, &id);
        Self::extract_call_sites(state, scope, node, &id);
        scope.pop_frame();

        // Extract attribute annotations.
        Self::extract_annotations_from_modifiers(state, node, &id);
    }

    // ----------------------------------
    // Property
    // ----------------------------------

    /// Extract a property declaration (`let`/`var`).
    ///
    /// Inside a class/struct/protocol body, stored properties (including those with
    /// `willSet`/`didSet` observers) become `Field`, while computed properties become
    /// `Property`. At top level, becomes `Const`.
    fn visit_property(state: &mut ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        let name = Self::extract_property_name(state, node);
        let in_class = state.class_depth > 0;
        let kind = if in_class {
            if Self::is_computed_property_node(node) {
                NodeKind::Property
            } else {
                NodeKind::Field
            }
        } else {
            NodeKind::Const
        };
        let visibility = Self::extract_member_visibility(state, scope, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let text = state.node_text(node);
        let docstring = Self::extract_docstring(state, node);

        if state.class_depth == 0 {
            let inferred = Self::infer_property_type(state, scope, node);
            scope.bind(name.clone(), inferred, true);
        }

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
            signature: Some(text.lines().next().unwrap_or("").trim().to_string()),
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

        // Calls in an initializer (`let x = Foo.make()`), a computed
        // property body or an accessor belong to the property.
        scope.push_frame();
        Self::extract_call_sites(state, scope, node, &id);
        scope.pop_frame();

        // Extract attribute annotations.
        Self::extract_annotations_from_modifiers(state, node, &id);
    }

    // ----------------------------------
    // Typealias
    // ----------------------------------

    /// Extract a typealias declaration.
    fn visit_typealias(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map(|n| state.node_text(n))
            .or_else(|| find_child_by_kind(node, "type_identifier").map(|n| state.node_text(n)))
            .unwrap_or_else(|| "<anonymous>".to_string());

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
                target: id,
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the type name from a `class_declaration` node.
    ///
    /// The name is in a child with field "name", which is a `type_identifier` for
    /// class/struct/enum, or a `user_type` wrapping `type_identifier` for extensions.
    fn extract_type_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        node.child_by_field_name("name").map_or_else(
            || "<anonymous>".to_string(),
            |n| {
                // For class/struct/enum, this is a type_identifier directly.
                // For extension, it may be a user_type wrapping type_identifier.
                if n.kind() == "user_type" {
                    find_child_by_kind(n, "type_identifier")
                        .map_or_else(|| state.node_text(n), |ti| state.node_text(ti))
                } else {
                    state.node_text(n)
                }
            },
        )
    }

    /// Extract inheritance specifiers (e.g. `class Foo: Bar`, `struct Foo: P`).
    ///
    /// Creates Extends `UnresolvedRef` for each inherited type. A Swift
    /// inheritance clause cannot tell a superclass from a protocol, so the
    /// resolver turns an `Extends` that lands on a protocol into `Implements`.
    fn extract_inheritance(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "inheritance_specifier" {
                    // The inheritance_specifier has a child with field "inherits_from"
                    // which is a user_type containing the parent type name.
                    if let Some(inherits_from) = child.child_by_field_name("inherits_from") {
                        let base_name = find_child_by_kind(inherits_from, "type_identifier")
                            .map_or_else(
                                || state.node_text(inherits_from),
                                |ti| state.node_text(ti),
                            );
                        let line = inherits_from.start_position().row as u32;
                        let column = inherits_from.start_position().column as u32;
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: class_id.to_string(),
                            reference_name: base_name,
                            reference_kind: EdgeKind::Extends,
                            line,
                            column,
                            file_path: state.file_path.clone(),
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the property name from a `property_declaration`.
    ///
    /// The name is in the `pattern` child (field "name") -> `simple_identifier` (field "`bound_identifier`").
    fn extract_property_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(pattern) = node.child_by_field_name("name") {
            if let Some(ident) = pattern.child_by_field_name("bound_identifier") {
                return state.node_text(ident);
            }
            // Fallback: find simple_identifier in pattern.
            if let Some(ident) = find_child_by_kind(pattern, "simple_identifier") {
                return state.node_text(ident);
            }
            return state.node_text(pattern);
        }
        // Fallback: find pattern child then simple_identifier.
        if let Some(pattern) = find_child_by_kind(node, "pattern") {
            if let Some(ident) = find_child_by_kind(pattern, "simple_identifier") {
                return state.node_text(ident);
            }
        }
        "<anonymous>".to_string()
    }

    /// Extract an explicit visibility modifier from a node's `modifiers` children.
    fn extract_explicit_visibility(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> Option<Visibility> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    if let Some(vis_mod) = find_child_by_kind(child, "visibility_modifier") {
                        let text = state.node_text(vis_mod);
                        return Some(match text.as_str() {
                            "private" | "fileprivate" => Visibility::Private,
                            "internal" => Visibility::PubCrate,
                            _ => Visibility::Pub,
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Extract the visibility modifier from a node's modifiers children.
    ///
    /// Looks for a `visibility_modifier` inside `modifiers` to determine visibility.
    fn extract_visibility(state: &ExtractionState, node: TsNode<'_>) -> Visibility {
        // Default: Swift internal (we map to Pub for simplicity, like other extractors).
        Self::extract_explicit_visibility(state, node).unwrap_or(Visibility::Pub)
    }

    /// Extract member visibility, inheriting file-private visibility inside a
    /// `private` or `fileprivate` extension when no explicit modifier is present.
    fn extract_member_visibility(
        state: &ExtractionState,
        scope: &SwiftScope,
        node: TsNode<'_>,
    ) -> Visibility {
        Self::extract_explicit_visibility(state, node).unwrap_or(if scope.in_private_extension() {
            Visibility::Private
        } else {
            Visibility::Pub
        })
    }

    /// Check if a function declaration has the `async` keyword.
    ///
    /// In tree-sitter-swift, `async` is an anonymous child node with kind "async".
    fn has_async_keyword(node: TsNode<'_>, _source: &[u8]) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "async" {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Extract docstrings from `/// comment` lines preceding definitions.
    ///
    /// Swift uses `///` doc comments. We look for `comment` sibling nodes that
    /// immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text
                    .trim_start_matches("///")
                    .trim_start_matches("//")
                    .trim()
                    .to_string();
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

    /// Extract a compact single-line callable signature up to `{`, preserving
    /// multi-line parameter labels and types for overload disambiguation.
    fn extract_decl_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let full = state.node_text(node);
        let header = match node.child_by_field_name("body") {
            Some(body) => {
                let end = body
                    .start_byte()
                    .saturating_sub(node.start_byte())
                    .min(full.len());
                &full[..end]
            }
            None => full.as_str(),
        };
        let compact = header.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.is_empty() {
            return None;
        }
        if compact.len() <= MAX_DECL_SIGNATURE_CHARS {
            return Some(compact);
        }
        let mut cut = MAX_DECL_SIGNATURE_CHARS;
        while cut > 0 && !compact.is_char_boundary(cut) {
            cut -= 1;
        }
        Some(compact[..cut].to_string())
    }

    /// Populate member property type bindings and callable names across a type body.
    fn seed_type_member_bindings(
        state: &ExtractionState,
        scope: &mut SwiftScope,
        body: TsNode<'_>,
    ) {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "property_declaration" => {
                        let name = Self::extract_property_name(state, child);
                        let is_computed = Self::is_computed_property_node(child);
                        if is_computed {
                            scope.register_member_callable(name, true);
                        } else {
                            let ty = Self::infer_property_type(state, scope, child);
                            scope.bind(name, ty, false);
                        }
                    }
                    "function_declaration" | "protocol_function_declaration" => {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| state.node_text(n))
                            .or_else(|| {
                                find_child_by_kind(child, "simple_identifier")
                                    .map(|n| state.node_text(n))
                            })
                            .unwrap_or_default();
                        scope.register_member_callable(name, false);
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn is_computed_property_node(node: TsNode<'_>) -> bool {
        node.child_by_field_name("computed_value").is_some()
            || find_child_by_kind(node, "computed_property").is_some()
    }

    /// Populate parameter name -> type bindings from direct `parameter` children of a callable.
    fn seed_parameter_bindings(
        state: &ExtractionState,
        scope: &mut SwiftScope,
        callable: TsNode<'_>,
    ) {
        let mut cursor = callable.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter" {
                    let local_name = child
                        .child_by_field_name("name")
                        .map(|n| state.node_text(n))
                        .or_else(|| {
                            find_child_by_kind(child, "simple_identifier")
                                .map(|n| state.node_text(n))
                        })
                        .unwrap_or_default();
                    // A `parameter` carries its type in the `type` field, not
                    // in a `type_annotation` child.
                    let ty = Self::first_type_annotation(child)
                        .or_else(|| child.child_by_field_name("type"))
                        .and_then(|ta| Self::extract_base_type_name(state, ta));
                    scope.bind(local_name, ty, true);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Record closure parameter names inside `lambda_literal` so higher-order refs ignore them.
    fn seed_lambda_parameters(state: &ExtractionState, scope: &mut SwiftScope, lambda: TsNode<'_>) {
        let mut cursor = lambda.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "lambda_function_type" {
                    Self::collect_lambda_param_names(state, scope, child);
                } else if child.kind() == "lambda_parameter" {
                    if let Some(name) = child
                        .child_by_field_name("name")
                        .or_else(|| find_child_by_kind(child, "simple_identifier"))
                    {
                        scope.bind(state.node_text(name), None, true);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn collect_lambda_param_names(
        state: &ExtractionState,
        scope: &mut SwiftScope,
        node: TsNode<'_>,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "lambda_parameter" {
                    if let Some(name) = child
                        .child_by_field_name("name")
                        .or_else(|| find_child_by_kind(child, "simple_identifier"))
                    {
                        scope.bind(state.node_text(name), None, true);
                    }
                } else if child.kind() == "lambda_function_type_parameters" {
                    Self::collect_lambda_param_names(state, scope, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Infer the base type name of a `property_declaration` from its annotation or initializer.
    fn infer_property_type(
        state: &ExtractionState,
        scope: &SwiftScope,
        node: TsNode<'_>,
    ) -> Option<String> {
        if let Some(ta) = Self::first_type_annotation(node) {
            if let Some(ty) = Self::extract_base_type_name(state, ta) {
                return Some(ty);
            }
        }
        if let Some(val) = node.child_by_field_name("value") {
            return Self::infer_expr_type(state, scope, val);
        }
        None
    }

    fn first_type_annotation(node: TsNode<'_>) -> Option<TsNode<'_>> {
        find_child_by_kind(node, "type_annotation")
    }

    /// Extract the base Swift nominal type name from a `type_annotation` or type node.
    fn extract_base_type_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        match node.kind() {
            "type_annotation" => node
                .child_by_field_name("type")
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| node.named_child(0))
                .and_then(|inner| Self::extract_base_type_name(state, inner)),
            "optional_type" | "implicitly_unwrapped_optional_type" => node
                .named_child(0)
                .and_then(|inner| Self::extract_base_type_name(state, inner)),
            "array_type" => Some("Array".to_string()),
            "dictionary_type" => Some("Dictionary".to_string()),
            "user_type" => {
                // Take the last direct `type_identifier` child so `Foundation.URL` -> `URL`
                // and `CustomStore<T>` -> `CustomStore`.
                let mut last = None;
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "type_identifier" {
                            last = Some(state.node_text(child));
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                last
            }
            "type_identifier" => Some(state.node_text(node)),
            _ => None,
        }
    }

    /// Infer the static base type name of a Swift expression when syntactically obvious.
    fn infer_expr_type(
        state: &ExtractionState,
        scope: &SwiftScope,
        node: TsNode<'_>,
    ) -> Option<String> {
        match node.kind() {
            "await_expression" | "try_expression" => node
                .child_by_field_name("expr")
                .and_then(|inner| Self::infer_expr_type(state, scope, inner)),
            "postfix_expression" => node
                .child_by_field_name("target")
                .and_then(|inner| Self::infer_expr_type(state, scope, inner)),
            "array_literal" => Some("Array".to_string()),
            "dictionary_literal" => Some("Dictionary".to_string()),
            "line_string_literal" | "multi_line_string_literal" | "raw_string_literal" => {
                Some("String".to_string())
            }
            "as_expression" => node
                .child_by_field_name("type")
                .and_then(|ty| Self::extract_base_type_name(state, ty)),
            "constructor_expression" => node
                .child_by_field_name("constructed_type")
                .and_then(|ty| Self::extract_base_type_name(state, ty)),
            "call_expression" => {
                let callee = node.named_child(0)?;
                match callee.kind() {
                    "simple_identifier" => {
                        let name = state.node_text(callee);
                        name.chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_uppercase())
                            .then_some(name)
                    }
                    "user_type" | "type_identifier" | "array_type" | "dictionary_type" => {
                        Self::extract_base_type_name(state, callee)
                    }
                    "constructor_expression" => Self::infer_expr_type(state, scope, callee),
                    "navigation_expression" => {
                        // Outer.Inner() or StaticFactory.init(...)
                        let suffix = callee.child_by_field_name("suffix")?;
                        let method = suffix
                            .child_by_field_name("suffix")
                            .map(|s| state.node_text(s))?;
                        if method == "init" {
                            return Self::receiver_type_name(state, callee);
                        }
                        method
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_uppercase())
                            .then_some(method)
                    }
                    _ => None,
                }
            }
            "simple_identifier" => {
                let ident = state.node_text(node);
                scope.lookup_type(&ident).map(str::to_string)
            }
            "self_expression" => scope.enclosing_type().map(str::to_string),
            _ => None,
        }
    }

    /// Record local variable bindings introduced by `for` or `if let`/`guard let`.
    fn record_block_bindings(state: &ExtractionState, scope: &mut SwiftScope, node: TsNode<'_>) {
        match node.kind() {
            "property_declaration" => {
                let name = Self::extract_property_name(state, node);
                let ty = Self::infer_property_type(state, scope, node);
                scope.bind(name, ty, true);
            }
            "for_statement" => {
                if let Some(item) = node.child_by_field_name("item") {
                    let mut names = Self::pattern_bindings(state, item);
                    if names.is_empty()
                        && find_child_by_kind(item, "value_binding_pattern").is_none()
                    {
                        // `for item in items`: the bare identifier is the binding.
                        names.extend(
                            find_child_by_kind(item, "simple_identifier")
                                .map(|ident| state.node_text(ident)),
                        );
                    }
                    for name in names {
                        scope.bind(name, None, true);
                    }
                }
            }
            "if_statement" | "guard_statement" => {
                // `if let x = value` / `guard let x: T = value` / shorthand
                // `if let x`. Types are inferred in the enclosing scope, before
                // any of the new names shadow it.
                let mut bindings: Vec<(String, Option<String>)> = Vec::new();
                let mut pending: Option<String> = None;
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        match cursor.field_name() {
                            Some("bound_identifier") if child.kind() == "simple_identifier" => {
                                if let Some(previous) = pending.take() {
                                    let ty = scope.lookup_type(&previous).map(str::to_string);
                                    bindings.push((previous, ty));
                                }
                                pending = Some(state.node_text(child));
                            }
                            Some("name") => {
                                if let Some(bound) = pending.take() {
                                    let ty = Self::extract_base_type_name(state, child);
                                    bindings.push((bound, ty));
                                }
                            }
                            // `if case .file(let path, _) = value`.
                            Some("condition") if child.kind() == "pattern" => {
                                bindings.extend(
                                    Self::pattern_bindings(state, child)
                                        .into_iter()
                                        .map(|name| (name, None)),
                                );
                            }
                            Some("condition")
                                if child.is_named()
                                    && !matches!(
                                        child.kind(),
                                        "value_binding_pattern"
                                            | "where_clause"
                                            | "availability_condition"
                                    ) =>
                            {
                                if let Some(bound) = pending.take() {
                                    let ty = Self::infer_expr_type(state, scope, child);
                                    bindings.push((bound, ty));
                                }
                            }
                            _ => {}
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                if let Some(shorthand) = pending {
                    let ty = scope.lookup_type(&shorthand).map(str::to_string);
                    bindings.push((shorthand, ty));
                }
                for (name, ty) in bindings {
                    scope.bind(name, ty, true);
                }
            }
            _ => {}
        }
    }

    /// Recursively find call nodes, computed-property reads, function references,
    /// and `#selector(...)` targets inside a given node and create unresolved Calls refs.
    fn extract_call_sites(
        state: &mut ExtractionState,
        scope: &mut SwiftScope,
        node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" => {
                        Self::visit_call_expression(state, scope, child, fn_node_id, 0);
                    }
                    "switch_entry" => {
                        // `case .file(let path, _):` binds `path` for the entry body.
                        scope.push_frame();
                        if let Some(pattern) = find_child_by_kind(child, "switch_pattern") {
                            for name in Self::pattern_bindings(state, pattern) {
                                scope.bind(name, None, true);
                            }
                        }
                        Self::extract_call_sites(state, scope, child, fn_node_id);
                        scope.pop_frame();
                    }
                    "selector_expression" => {
                        Self::extract_selector_ref(state, scope, child, fn_node_id);
                    }
                    "simple_identifier" => {
                        Self::maybe_emit_computed_prop_read(state, scope, child, fn_node_id);
                    }
                    // Nested local `func helper()` attributes inner calls to the enclosing callable,
                    // while suppressing bare calls/reads to the local helper name itself.
                    "function_declaration" => {
                        let local_fn_name = child
                            .child_by_field_name("name")
                            .map(|n| state.node_text(n))
                            .or_else(|| {
                                find_child_by_kind(child, "simple_identifier")
                                    .map(|n| state.node_text(n))
                            })
                            .unwrap_or_default();
                        scope.bind_local_function(local_fn_name);
                        scope.push_frame();
                        Self::seed_parameter_bindings(state, scope, child);
                        Self::extract_default_param_func_refs(state, scope, child, fn_node_id);
                        if let Some(body) = child.child_by_field_name("body") {
                            Self::extract_call_sites(state, scope, body, fn_node_id);
                        }
                        scope.pop_frame();
                    }
                    // Skip nested type/init definitions to avoid polluting call sites.
                    "init_declaration" | "class_declaration" | "protocol_declaration" => {}
                    "lambda_literal" => {
                        scope.push_frame();
                        Self::seed_lambda_parameters(state, scope, child);
                        Self::extract_call_sites(state, scope, child, fn_node_id);
                        scope.pop_frame();
                    }
                    "property_declaration" => {
                        // Extract initializer calls before binding the new local variable.
                        Self::extract_call_sites(state, scope, child, fn_node_id);
                        Self::record_block_bindings(state, scope, child);
                    }
                    "guard_statement" => {
                        // Guard bindings enter the enclosing scope after its condition/else run.
                        Self::extract_call_sites(state, scope, child, fn_node_id);
                        Self::record_block_bindings(state, scope, child);
                    }
                    "if_statement" | "for_statement" | "while_statement" => {
                        scope.push_frame();
                        Self::record_block_bindings(state, scope, child);
                        Self::extract_call_sites(state, scope, child, fn_node_id);
                        scope.pop_frame();
                    }
                    _ => {
                        Self::extract_call_sites(state, scope, child, fn_node_id);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Emit a `Calls` reference when a `simple_identifier` reads an enclosing
    /// computed property (`controls`, `chart` in `chart.frame(...)`, `self.controls`).
    fn maybe_emit_computed_prop_read(
        state: &mut ExtractionState,
        scope: &SwiftScope,
        ident_node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let ident = state.node_text(ident_node);
        if SWIFT_LITERAL_IDENTIFIERS.contains(&ident.as_str())
            || scope.is_shadowed_by_local_value(&ident)
            || scope.is_local_function(&ident)
        {
            return;
        }
        let Some(owner) = scope.owning_type_for_computed_prop(&ident) else {
            return;
        };
        if Self::is_direct_callee_ident(ident_node)
            || Self::is_non_read_identifier_position(ident_node)
            || Self::is_member_of_foreign_receiver(state, ident_node)
            || Self::is_leading_dot_shorthand(ident_node)
        {
            return;
        }
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: fn_node_id.to_string(),
            reference_name: format!("{owner}::{ident}"),
            reference_kind: EdgeKind::Calls,
            line: ident_node.start_position().row as u32,
            column: ident_node.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
    }

    /// Record one call site and walk into it.
    ///
    /// tree-sitter-swift parses `let x = f(a: 1) { }` as a call on the call
    /// `f(a: 1)` whose suffix holds only the trailing closure. That wrapper is
    /// the same call: its closures are counted on the inner call's selector and
    /// no separate reference is recorded for the wrapper.
    fn visit_call_expression(
        state: &mut ExtractionState,
        scope: &mut SwiftScope,
        call: TsNode<'_>,
        fn_node_id: &str,
        extra_trailing_closures: usize,
    ) {
        if let Some((inner, closures)) = Self::trailing_closure_wrapper(call) {
            Self::visit_call_expression(
                state,
                scope,
                inner,
                fn_node_id,
                extra_trailing_closures + closures,
            );
            if let Some(suffix) = find_child_by_kind(call, "call_suffix") {
                Self::extract_call_sites(state, scope, suffix, fn_node_id);
            }
            return;
        }
        if !Self::is_subscript_call(call) {
            if let Some(name) =
                Self::extract_callee_name(state, scope, call, extra_trailing_closures)
            {
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: fn_node_id.to_string(),
                    reference_name: name,
                    reference_kind: EdgeKind::Calls,
                    line: call.start_position().row as u32,
                    column: call.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
            }
            Self::extract_function_reference_args(state, scope, call, fn_node_id);
        }
        Self::extract_call_sites(state, scope, call, fn_node_id);
    }

    /// The inner call and closure count when `call` only adds trailing closures
    /// to the call it wraps (its suffix has no argument list).
    fn trailing_closure_wrapper(call: TsNode<'_>) -> Option<(TsNode<'_>, usize)> {
        // `try f(a: 1) { }` / `await f(a: 1) { }` wrap the inner call too.
        let mut inner = call.named_child(0)?;
        while matches!(inner.kind(), "try_expression" | "await_expression") {
            inner = inner.child_by_field_name("expr")?;
        }
        if inner.kind() != "call_expression" {
            return None;
        }
        let suffix = find_child_by_kind(call, "call_suffix")?;
        if find_child_by_kind(suffix, "value_arguments").is_some() {
            return None;
        }
        let mut closures = 0;
        let mut cursor = suffix.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == "lambda_literal" {
                    closures += 1;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        (closures > 0).then_some((inner, closures))
    }

    /// Names a pattern binds: `let path` in `.file(let path, _)`, `name` in
    /// `case let .other(name)`, tuple items in `for (key, value) in`. Names
    /// matched against an existing value (`case .some(x)`) are not bindings.
    fn pattern_bindings(state: &ExtractionState, pattern: TsNode<'_>) -> Vec<String> {
        let mut names = Vec::new();
        // (node, inside a `let`/`var` pattern)
        let mut stack = vec![(pattern, false)];
        while let Some((node, binding)) = stack.pop() {
            let binding = binding || find_child_by_kind(node, "value_binding_pattern").is_some();
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "simple_identifier" {
                        let is_bound = cursor.field_name() == Some("bound_identifier")
                            || (binding
                                && node.kind() == "pattern"
                                && node.named_child_count() == 1);
                        if is_bound {
                            names.push(state.node_text(child));
                        }
                    } else if child.kind().contains("pattern") {
                        stack.push((child, binding));
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        names
    }

    /// Whether `node` is the member in `.member` leading-dot shorthand (e.g. `.body`).
    fn is_leading_dot_shorthand(node: TsNode<'_>) -> bool {
        node.parent().is_some_and(|p| {
            p.kind() == "prefix_expression"
                && p.child_by_field_name("operation")
                    .is_some_and(|op| op.kind() == ".")
        })
    }

    /// Whether `call_node` is a subscript (`receiver[idx]`) rather than a call (`receiver(...)`).
    fn is_subscript_call(call_node: TsNode<'_>) -> bool {
        let Some(suffix) = find_child_by_kind(call_node, "call_suffix") else {
            return false;
        };
        let Some(args) = find_child_by_kind(suffix, "value_arguments") else {
            return false;
        };
        args.child(0).is_some_and(|c| c.kind() == "[")
    }

    /// Extract function references passed as parameter default values (`fn: T = Type.helper`).
    fn extract_default_param_func_refs(
        state: &mut ExtractionState,
        scope: &SwiftScope,
        callable: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let mut cursor = callable.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.field_name() == Some("default_value") {
                    Self::emit_func_ref_value(state, scope, cursor.node(), fn_node_id, false);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Whether `node` is the member in `receiver.member` for a receiver other
    /// than `self`: `items.count` reads the array's `count`, not the enclosing
    /// type's computed `count`.
    fn is_member_of_foreign_receiver(state: &ExtractionState, node: TsNode<'_>) -> bool {
        let Some(nav) = node
            .parent()
            .filter(|parent| parent.kind() == "navigation_suffix")
            .and_then(|suffix| suffix.parent())
        else {
            return false;
        };
        let Some(target) = nav.child_by_field_name("target") else {
            return true;
        };
        let target = Self::unwrap_prefix_postfix(target);
        !(target.kind() == "self_expression"
            || (target.kind() == "simple_identifier" && state.node_text(target) == "self"))
    }

    fn is_direct_callee_ident(node: TsNode<'_>) -> bool {
        let Some(parent) = node.parent() else {
            return false;
        };
        match parent.kind() {
            "call_expression" => parent.named_child(0).is_some_and(|c| c.id() == node.id()),
            "navigation_suffix" => parent
                .parent()
                .and_then(|nav| nav.parent())
                .is_some_and(|p| p.kind() == "call_expression"),
            "selector_expression" => true,
            _ => false,
        }
    }

    fn is_non_read_identifier_position(node: TsNode<'_>) -> bool {
        node.parent().is_some_and(|p| {
            matches!(
                p.kind(),
                "value_argument_label"
                    | "pattern"
                    | "parameter"
                    | "function_declaration"
                    | "init_declaration"
                    | "property_declaration"
                    | "class_declaration"
                    | "protocol_declaration"
                    | "type_identifier"
                    | "user_type"
            )
        })
    }

    /// Emit `Calls` references for higher-order function references passed as
    /// arguments (e.g. `.map(csvField)`, `Button(action: lookUpHash)`, `.sorted(by: Self.order)`).
    fn extract_function_reference_args(
        state: &mut ExtractionState,
        scope: &SwiftScope,
        call_node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let Some(suffix) = find_child_by_kind(call_node, "call_suffix") else {
            return;
        };
        let Some(args) = find_child_by_kind(suffix, "value_arguments") else {
            return;
        };
        let is_higher_order_callee = call_node
            .named_child(0)
            .and_then(|target| match target.kind() {
                "simple_identifier" => Some(state.node_text(target)),
                "navigation_expression" => Self::navigation_suffix_ident(state, target),
                _ => None,
            })
            .is_some_and(|callee| SWIFT_HIGHER_ORDER_CALLEES.contains(&callee.as_str()));

        let mut cursor = args.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "value_argument" {
                    let is_callback_label = find_child_by_kind(child, "value_argument_label")
                        .is_some_and(|label| {
                            let name = state.node_text(label);
                            SWIFT_CALLBACK_ARG_LABELS.contains(&name.trim_end_matches(':').trim())
                        });
                    if let Some(val) = child.child_by_field_name("value") {
                        Self::emit_func_ref_value(
                            state,
                            scope,
                            val,
                            fn_node_id,
                            is_higher_order_callee || is_callback_label,
                        );
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn emit_func_ref_value(
        state: &mut ExtractionState,
        scope: &SwiftScope,
        val: TsNode<'_>,
        fn_node_id: &str,
        allow_unqualified: bool,
    ) {
        match val.kind() {
            "simple_identifier" => {
                let ident = state.node_text(val);
                if SWIFT_LITERAL_IDENTIFIERS.contains(&ident.as_str())
                    || ident.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    || scope.is_bound_local(&ident)
                    || scope.is_shadowed_by_local_value(&ident)
                    || scope.is_local_function(&ident)
                {
                    return;
                }
                let reference_name = if let Some(owner) = scope.owning_type_for_callable(&ident) {
                    format!("{owner}::{ident}")
                } else if allow_unqualified {
                    ident
                } else {
                    return;
                };
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: fn_node_id.to_string(),
                    reference_name,
                    reference_kind: EdgeKind::Calls,
                    line: val.start_position().row as u32,
                    column: val.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
            }
            "navigation_expression" => {
                // Skip shorthand `.compact` where target is absent.
                let Some(target) = val.child_by_field_name("target") else {
                    return;
                };
                let Some(method) = Self::navigation_suffix_ident(state, val) else {
                    return;
                };
                if matches!(method.as_str(), "self" | "Type" | "init") {
                    return;
                }
                if let Some(type_name) = Self::receiver_type_name(state, val) {
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: fn_node_id.to_string(),
                        reference_name: format!("{type_name}::{method}"),
                        reference_kind: EdgeKind::Calls,
                        line: val.start_position().row as u32,
                        column: val.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                } else if Self::receiver_is_self_type(state, target)
                    || matches!(target.kind(), "self_expression")
                    || (target.kind() == "simple_identifier" && state.node_text(target) == "self")
                {
                    let reference_name = match scope.owning_type_for_callable(&method) {
                        Some(owner) => format!("{owner}::{method}"),
                        None => method,
                    };
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: fn_node_id.to_string(),
                        reference_name,
                        reference_kind: EdgeKind::Calls,
                        line: val.start_position().row as u32,
                        column: val.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Emit a `Calls` reference for `#selector(...)` (`#selector(statusItemClicked)`,
    /// `#selector(MyType.action(_:))`, `#selector(getter: prop)`).
    fn extract_selector_ref(
        state: &mut ExtractionState,
        scope: &SwiftScope,
        node: TsNode<'_>,
        fn_node_id: &str,
    ) {
        let mut cursor = node.walk();
        let Some(target_expr) = cursor
            .goto_first_child()
            .then(|| {
                let mut found = None;
                loop {
                    let c = cursor.node();
                    if c.is_named() && c.kind() != "selector" {
                        found = Some(c);
                        break;
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                found
            })
            .flatten()
        else {
            return;
        };

        let expr = if target_expr.kind() == "call_expression" {
            match target_expr.named_child(0) {
                Some(callee) => callee,
                None => return,
            }
        } else {
            target_expr
        };

        let reference_name = match expr.kind() {
            "simple_identifier" => {
                let ident = state.node_text(expr);
                match scope.owning_type_for_callable(&ident) {
                    Some(owner) => format!("{owner}::{ident}"),
                    None => ident,
                }
            }
            "navigation_expression" => {
                let Some(method) = Self::navigation_suffix_ident(state, expr) else {
                    return;
                };
                if let Some(type_name) = Self::receiver_type_name(state, expr) {
                    format!("{type_name}::{method}")
                } else {
                    match scope.owning_type_for_callable(&method) {
                        Some(owner) => format!("{owner}::{method}"),
                        None => method,
                    }
                }
            }
            _ => return,
        };

        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: fn_node_id.to_string(),
            reference_name,
            reference_kind: EdgeKind::Calls,
            line: node.start_position().row as u32,
            column: node.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
    }

    fn navigation_suffix_ident(state: &ExtractionState, nav: TsNode<'_>) -> Option<String> {
        let mut last = None;
        let mut cursor = nav.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "navigation_suffix" {
                    if let Some(ident) = find_child_by_kind(child, "simple_identifier") {
                        last = Some(state.node_text(ident));
                    }
                } else if child.kind() == "simple_identifier" && last.is_none() {
                    last = Some(state.node_text(child));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        last
    }

    /// Build Swift argument-label selector `(label1:label2:)` for a call with at
    /// least one argument: `_:` per unlabeled argument, `label:` per labeled one,
    /// and the match-any marker per trailing closure.
    ///
    /// `extra_trailing_closures` counts closures that tree-sitter attached to an
    /// enclosing wrapper call (`let x = f(a: 1) { }` parses as a call on `f(a: 1)`).
    fn extract_call_selector_suffix(
        state: &ExtractionState,
        call_node: TsNode<'_>,
        extra_trailing_closures: usize,
    ) -> String {
        let mut labels: Vec<String> = Vec::new();
        let mut has_named_label = false;
        if let Some(suffix) = find_child_by_kind(call_node, "call_suffix") {
            Self::push_suffix_labels(state, suffix, &mut labels, &mut has_named_label);
        }
        labels.extend(
            std::iter::repeat_with(|| format!("{SWIFT_TRAILING_CLOSURE_LABEL}:"))
                .take(extra_trailing_closures),
        );
        if labels.is_empty() {
            String::new()
        } else {
            format!("({})", labels.join(""))
        }
    }

    /// Append the labels of one `call_suffix`: its arguments, then one marker
    /// per trailing closure.
    fn push_suffix_labels(
        state: &ExtractionState,
        suffix: TsNode<'_>,
        labels: &mut Vec<String>,
        has_named_label: &mut bool,
    ) {
        let mut cursor = suffix.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "value_arguments" => {
                        Self::push_argument_labels(state, child, labels, has_named_label);
                    }
                    // A trailing closure fills the next parameter whatever its
                    // label, so it is recorded with the match-any marker.
                    "lambda_literal" => labels.push(format!("{SWIFT_TRAILING_CLOSURE_LABEL}:")),
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Append one `label:` (or `_:` when unlabeled) per argument in `args`.
    fn push_argument_labels(
        state: &ExtractionState,
        args: TsNode<'_>,
        labels: &mut Vec<String>,
        has_named_label: &mut bool,
    ) {
        let mut cursor = args.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "value_argument" {
                    // `f(id:)` / `Type.f(in:of:)` name a function by its argument
                    // labels: each label is a `reference_specifier`.
                    let mut specifiers = Vec::new();
                    let mut arg_cursor = child.walk();
                    if arg_cursor.goto_first_child() {
                        loop {
                            if arg_cursor.field_name() == Some("reference_specifier") {
                                specifiers.push(state.node_text(arg_cursor.node()));
                            }
                            if !arg_cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    let label = child
                        .child_by_field_name("name")
                        .map(|label_node| state.node_text(label_node))
                        .filter(|label| !label.is_empty());
                    if !specifiers.is_empty() {
                        *has_named_label = true;
                        labels.extend(
                            specifiers
                                .iter()
                                .map(|label| format!("{}:", label.trim_end_matches(':'))),
                        );
                    } else if let Some(label) = label {
                        *has_named_label = true;
                        labels.push(format!("{label}:"));
                    } else {
                        labels.push("_:".to_string());
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract the callee name from a `call_expression`.
    ///
    /// In Swift, `call_expression` children are: callee (`simple_identifier` or `navigation_expression`)
    /// followed by `call_suffix` with `value_arguments`.
    ///
    /// - Type receivers (`Cache.open(...)`, `Outer.Inner.make(...)`) become `Cache::open` / `Inner::make`.
    /// - Value receivers with a statically inferred local/parameter/property type `T` become `T::method`.
    /// - Other value receivers keep their dotted prefix (`worker.start`, `items.allSatisfy`, `self.runHelper`)
    ///   so the resolver treats foreign receivers safely instead of matching by bare name alone.
    /// - Calls with multiple arguments or labeled arguments append `(label1:label2:)` for overload disambiguation.
    fn extract_callee_name(
        state: &ExtractionState,
        scope: &SwiftScope,
        node: TsNode<'_>,
        extra_trailing_closures: usize,
    ) -> Option<String> {
        let callee = Self::callee_expression(node.named_child(0)?)?;
        let base = if callee.kind() == "navigation_expression" {
            let method = Self::navigation_suffix_ident(state, callee)?;
            if let Some(type_name) = Self::receiver_type_name(state, callee) {
                format!("{type_name}::{method}")
            } else if let Some(target) = callee.child_by_field_name("target") {
                if Self::receiver_is_self_type(state, target) {
                    method
                } else if let Some(inferred_ty) =
                    Self::resolve_navigation_receiver_type(state, scope, target)
                {
                    if inferred_ty.is_empty() {
                        method
                    } else {
                        format!("{inferred_ty}::{method}")
                    }
                } else {
                    let receiver_text = Self::receiver_chain_text(state, target);
                    if receiver_text.is_empty() {
                        format!("<expr>.{method}")
                    } else {
                        format!("{receiver_text}.{method}")
                    }
                }
            } else {
                format!(".{method}")
            }
        } else {
            let name = state.node_text(callee);
            if callee.kind() == "simple_identifier" && scope.is_local_function(&name) {
                return None;
            }
            // `.make(...)` names a static member of the contextual type, which
            // is not known here: keep the leading dot so it is not a bare call.
            let is_implicit_member = node.named_child(0).is_some_and(|target| {
                target.kind() == "prefix_expression"
                    && target
                        .child_by_field_name("operation")
                        .is_some_and(|op| op.kind() == ".")
            });
            if is_implicit_member {
                format!(".{name}")
            } else {
                name
            }
        };
        let selector = Self::extract_call_selector_suffix(state, node, extra_trailing_closures);
        if selector.is_empty() {
            Some(base)
        } else {
            Some(format!("{base}{selector}"))
        }
    }

    fn unwrap_prefix_postfix(mut node: TsNode<'_>) -> TsNode<'_> {
        while matches!(node.kind(), "prefix_expression" | "postfix_expression") {
            let Some(next) = node
                .child_by_field_name("target")
                .or_else(|| node.named_child(0))
            else {
                break;
            };
            node = next;
        }
        node
    }

    fn receiver_is_self_type(state: &ExtractionState, target: TsNode<'_>) -> bool {
        let inner = Self::unwrap_prefix_postfix(target);
        matches!(
            inner.kind(),
            "simple_identifier" | "self_expression" | "user_type" | "type_identifier"
        ) && state.node_text(inner) == "Self"
    }

    fn resolve_navigation_receiver_type(
        state: &ExtractionState,
        scope: &SwiftScope,
        target: TsNode<'_>,
    ) -> Option<String> {
        let inner = Self::unwrap_prefix_postfix(target);
        match inner.kind() {
            "simple_identifier" => {
                let ident = state.node_text(inner);
                if ident == "self" || ident == "Self" {
                    return None;
                }
                scope.lookup_type(&ident).map(str::to_string)
            }
            "navigation_expression" => {
                // Support `self.store.saveSnapshot()` by looking up `store` on the enclosing type.
                let base = Self::unwrap_prefix_postfix(inner.child_by_field_name("target")?);
                let suffix = inner.child_by_field_name("suffix")?;
                let member = suffix.child_by_field_name("suffix")?;
                if matches!(base.kind(), "self_expression")
                    || (base.kind() == "simple_identifier" && state.node_text(base) == "self")
                {
                    let member_name = state.node_text(member);
                    return scope.lookup_type(&member_name).map(str::to_string);
                }
                None
            }
            "call_expression" | "array_literal" | "dictionary_literal" | "line_string_literal" => {
                Self::infer_expr_type(state, scope, inner)
            }
            _ => None,
        }
    }

    fn receiver_chain_text(state: &ExtractionState, target: TsNode<'_>) -> String {
        let inner = Self::unwrap_prefix_postfix(target);
        match inner.kind() {
            "simple_identifier" | "self_expression" | "super_expression" => state.node_text(inner),
            "navigation_expression" => {
                let head = inner
                    .child_by_field_name("target")
                    .map(|t| Self::receiver_chain_text(state, t))
                    .unwrap_or_default();
                let tail = inner
                    .child_by_field_name("suffix")
                    .and_then(|s| s.child_by_field_name("suffix"))
                    .map(|m| state.node_text(m))
                    .unwrap_or_default();
                if head.is_empty() || tail.is_empty() {
                    String::new()
                } else {
                    format!("{head}.{tail}")
                }
            }
            _ => String::new(),
        }
    }

    /// The expression a call suffix actually applies to.
    ///
    /// tree-sitter-swift attaches the call suffix of `48 + Double(x)` or
    /// `!ok && items.allSatisfy { ... }` to the whole binary expression, and
    /// of `await f(x) { ... }` to the `await`. The callee is the rightmost
    /// operand. When that operand is itself a call, the call is on a call's
    /// result and the inner call is recorded on its own, so there is no name.
    fn callee_expression(node: TsNode<'_>) -> Option<TsNode<'_>> {
        let mut current = node;
        loop {
            let next = match current.kind() {
                "await_expression" | "try_expression" => current.child_by_field_name("expr"),
                "prefix_expression" | "postfix_expression" => current.child_by_field_name("target"),
                _ => current.child_by_field_name("rhs"),
            };
            match next {
                Some(inner) => current = inner,
                None => break,
            }
        }
        (current.kind() != "call_expression").then_some(current)
    }

    /// The type name a navigation callee is called through, if its receiver
    /// is spelled as a type: `Cache` in `Cache.open`, `Inner` in
    /// `Outer.Inner.make`. Swift types are capitalised by convention, so a
    /// lowercase receiver is a value. `Self` is left out: it names the
    /// enclosing type, which the bare method name already resolves within.
    fn receiver_type_name(state: &ExtractionState, nav: TsNode<'_>) -> Option<String> {
        let target = Self::unwrap_prefix_postfix(nav.child_by_field_name("target")?);
        let ident = match target.kind() {
            "simple_identifier" => target,
            "navigation_expression" => {
                let suffix = target.child_by_field_name("suffix")?;
                suffix.child_by_field_name("suffix")?
            }
            _ => return None,
        };
        if ident.kind() != "simple_identifier" {
            return None;
        }
        let name = state.node_text(ident);
        let is_type = name.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        (is_type && name != "Self").then_some(name)
    }

    /// Walk previous siblings of a declaration looking for `attribute` nodes
    /// and extract annotation usages from each one.
    fn extract_annotations_from_modifiers(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        target_id: &str,
    ) {
        let mut current = node.prev_named_sibling();
        while let Some(sibling) = current {
            if sibling.kind() == "attribute" {
                Self::extract_annotations_from_node(state, sibling, target_id);
                current = sibling.prev_named_sibling();
            } else if sibling.kind() == "comment" || sibling.kind() == "multiline_comment" {
                current = sibling.prev_named_sibling();
            } else {
                break;
            }
        }
        // Also check children of a "modifiers" child node.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "modifiers" {
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let m = inner.node();
                            if m.kind() == "attribute" {
                                Self::extract_annotations_from_node(state, m, target_id);
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

    /// Create an `AnnotationUsage` node and edges for a single `attribute` node.
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
            resolved_by: None,
        });
    }

    /// Extract the name from a Swift `attribute` node.
    ///
    /// Looks for a `user_type` child, or falls back to text after `@`, before `(`.
    fn extract_annotation_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        // Try user_type -> type_identifier path.
        if let Some(ut) = find_child_by_kind(node, "user_type") {
            if let Some(ti) = find_child_by_kind(ut, "type_identifier") {
                return state.node_text(ti);
            }
            return state.node_text(ut);
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
}

impl crate::extraction::LanguageExtractor for SwiftExtractor {
    fn extensions(&self) -> &[&str] {
        &["swift"]
    }

    fn language_name(&self) -> &'static str {
        "Swift"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_swift(file_path, source)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod swift_extractor_unit_tests {
    use super::*;

    #[test]
    fn swift_scope_empty_guards_and_member_lookups() {
        let mut scope = SwiftScope::default();
        assert_eq!(scope.enclosing_type(), None);
        assert_eq!(scope.owning_type_for_callable("missing"), None);
        assert_eq!(scope.owning_type_for_computed_prop("missing"), None);

        scope.push_type("Box");
        assert_eq!(scope.enclosing_type(), Some("Box"));
        scope.bind(String::new(), Some("Int".to_string()), false);
        scope.bind("prop".to_string(), Some("Store".to_string()), false);
        scope.register_member_callable(String::new(), true);
        scope.register_member_callable("computed".to_string(), true);
        assert_eq!(scope.owning_type_for_callable("computed"), Some("Box"));
        assert_eq!(scope.owning_type_for_computed_prop("computed"), Some("Box"));

        scope.push_frame();
        scope.bind("computed".to_string(), Some("Int".to_string()), true);
        assert!(scope.is_shadowed_by_local_value("computed"));
        assert!(scope.is_bound_local("computed"));
        scope.pop_frame();
        scope.pop_type();
        assert_eq!(scope.enclosing_type(), None);
    }

    #[test]
    fn whitespace_only_declaration_produces_no_signature() {
        let state = ExtractionState::new("Empty.swift", "func f() {}");
        let tree = SwiftExtractor::parse_source("   \n   ").expect("parse whitespace");
        assert_eq!(
            SwiftExtractor::extract_decl_signature(&state, tree.root_node()),
            None
        );
    }

    #[test]
    fn helper_fallbacks_exercise_defensive_ast_branches() {
        fn find_kind<'a>(n: TsNode<'a>, target: &str) -> Option<TsNode<'a>> {
            if n.kind() == target {
                return Some(n);
            }
            let mut cursor = n.walk();
            if cursor.goto_first_child() {
                loop {
                    if let Some(found) = find_kind(cursor.node(), target) {
                        return Some(found);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            None
        }

        let sample = "import Foundation\nextension Foo {}\nlet x: MyType = !flag\nlet n = (a).b()\n@available(*, deprecated)\nfunc call() { _ = #selector(tap) }\n";
        let tree = SwiftExtractor::parse_source(sample).expect("parse sample");
        let root = tree.root_node();
        let state_ro = ExtractionState::new("Helpers.swift", sample);

        // `extract_base_type_name` on direct `type_identifier` and unknown node kind.
        let type_id = find_kind(root, "type_identifier").expect("type_identifier");
        assert_eq!(
            SwiftExtractor::extract_base_type_name(&state_ro, type_id).as_deref(),
            Some("Foo")
        );
        assert_eq!(
            SwiftExtractor::extract_base_type_name(&state_ro, root),
            None
        );

        // `is_direct_callee_ident` on root (no parent) and `#selector(tap)` target.
        assert!(!SwiftExtractor::is_direct_callee_ident(root));
        let sel = find_kind(root, "selector_expression").expect("selector_expression");
        let sel_ident = find_kind(sel, "simple_identifier").expect("selector ident");
        assert!(SwiftExtractor::is_direct_callee_ident(sel_ident));

        // Missing `call_suffix` / `value_arguments` guards.
        assert_eq!(
            SwiftExtractor::extract_call_selector_suffix(&state_ro, root, 0),
            ""
        );
        let scope = SwiftScope::default();
        let mut state_mut = ExtractionState::new("Helpers.swift", sample);
        SwiftExtractor::extract_function_reference_args(&mut state_mut, &scope, root, "fn-id");

        // `declaration_kind_keyword` fallback on non-class node with children and childless node.
        let import_node = find_kind(root, "import_declaration").expect("import_declaration");
        assert_eq!(
            SwiftExtractor::declaration_kind_keyword(&state_ro, import_node),
            "import"
        );
        assert_eq!(
            SwiftExtractor::declaration_kind_keyword(&state_ro, type_id),
            "class"
        );

        // `extract_type_name` on node without `name` and on `extension Foo` (`user_type`).
        assert_eq!(
            SwiftExtractor::extract_type_name(&state_ro, root),
            "<anonymous>"
        );
        let ext_decl = find_kind(root, "class_declaration").expect("extension decl");
        assert_eq!(
            SwiftExtractor::extract_type_name(&state_ro, ext_decl),
            "Foo"
        );

        // `extract_property_name` fallback `<anonymous>` on node without `name`/`pattern`.
        assert_eq!(
            SwiftExtractor::extract_property_name(&state_ro, root),
            "<anonymous>"
        );

        // `resolve_navigation_receiver_type` on `Self` and `receiver_type_name` on non-ident target.
        let self_src = "let r = Self.make()\nlet d = 1.description\n";
        let self_tree = SwiftExtractor::parse_source(self_src).expect("parse self_src");
        let self_root = self_tree.root_node();
        let self_state = ExtractionState::new("SelfRef.swift", self_src);
        let nav_self = find_kind(self_root, "navigation_expression").expect("nav_self");
        if let Some(target) = nav_self.child_by_field_name("target") {
            assert_eq!(
                SwiftExtractor::resolve_navigation_receiver_type(&self_state, &scope, target),
                None
            );
        }
        let lit_nav_src = "let d = 1.description\n";
        let lit_tree = SwiftExtractor::parse_source(lit_nav_src).expect("parse lit");
        let lit_state = ExtractionState::new("Lit.swift", lit_nav_src);
        let lit_nav = find_kind(lit_tree.root_node(), "navigation_expression").expect("lit_nav");
        assert_eq!(
            SwiftExtractor::receiver_type_name(&lit_state, lit_nav),
            None
        );

        // `extract_annotation_name` fallback on node without `user_type`.
        assert_eq!(
            SwiftExtractor::extract_annotation_name(&state_ro, type_id),
            "Foo"
        );

        // `visit_import` fallback on a node without an `identifier` child.
        SwiftExtractor::visit_import(&mut state_mut, import_node);
        SwiftExtractor::visit_import(&mut state_mut, type_id);
        assert!(state_mut
            .nodes
            .iter()
            .any(|n| n.kind == NodeKind::Use && n.name == "Foo"));

        // `unwrap_prefix_postfix` on `!flag` (`prefix_expression`).
        let prefix = find_kind(root, "prefix_expression").expect("prefix_expression");
        let unwrapped = SwiftExtractor::unwrap_prefix_postfix(prefix);
        assert_eq!(unwrapped.kind(), "simple_identifier");
    }
}
