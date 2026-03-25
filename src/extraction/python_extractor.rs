/// Tree-sitter based Python source code extractor.
///
/// Parses Python source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, PYTHON_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Python source files using tree-sitter.
pub struct PythonExtractor;

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
    /// Depth of class nesting. > 0 means we are inside a class.
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

impl PythonExtractor {
    /// Extract code graph nodes and edges from a Python source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Python source code to parse.
    pub fn extract_python(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = tree_sitter_python::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| format!("failed to load Python grammar: {e}"))?;
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
            "function_definition" => {
                let is_async = Self::has_async_keyword(node);
                Self::visit_function(state, node, is_async);
            }
            "class_definition" => Self::visit_class(state, node),
            "decorated_definition" => Self::visit_decorated_definition(state, node),
            "import_statement" => Self::visit_import(state, node),
            "import_from_statement" => Self::visit_import_from(state, node),
            "expression_statement" if state.class_depth == 0 => {
                // Check for module-level assignments that look like constants.
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "assignment" {
                            Self::visit_assignment(state, child);
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Extract a function definition. If inside a class (class_depth > 0), it becomes a Method.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>, is_async: bool) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let in_class = state.class_depth > 0;
        let kind = if in_class {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = Self::python_visibility(&name);
        let signature = Self::extract_function_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &PYTHON_COMPLEXITY);

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
            visibility,
            is_async,
            branches: metrics.branches,
            loops: metrics.loops,
            returns: metrics.returns,
            max_nesting: metrics.max_nesting,
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
        if let Some(body) = Self::find_child_by_kind(node, "block") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Extract a class definition.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::find_child_by_kind(node, "identifier")
            .map(|n| state.node_text(n))
            .unwrap_or_else(|| "<anonymous>".to_string());

        let visibility = Self::python_visibility(&name);
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

        // Extract base classes (inheritance).
        Self::extract_base_classes(state, node, &id);

        // Visit class body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        if let Some(body) = Self::find_child_by_kind(node, "block") {
            Self::visit_children(state, body);
        }
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a decorated definition (decorator + function or class).
    fn visit_decorated_definition(state: &mut ExtractionState, node: TsNode<'_>) {
        // First, find the inner definition (function_definition or class_definition).
        let inner_def = Self::find_child_by_kind(node, "function_definition")
            .or_else(|| Self::find_child_by_kind(node, "class_definition"));

        // Check if the inner def is an async function (could be wrapped in decorated_definition)
        let is_async = Self::has_async_keyword(node);

        // Determine the inner definition's node ID ahead of time so we can
        // create Annotates edges from decorators to it.
        let inner_kind_and_name = if let Some(inner) = inner_def {
            let name = Self::find_child_by_kind(inner, "identifier")
                .map(|n| state.node_text(n))
                .unwrap_or_else(|| "<anonymous>".to_string());
            let kind = match inner.kind() {
                "class_definition" => NodeKind::Class,
                _ => {
                    if state.class_depth > 0 {
                        NodeKind::Method
                    } else {
                        NodeKind::Function
                    }
                }
            };
            let start_line = inner.start_position().row as u32;
            Some((kind, name, start_line))
        } else {
            None
        };

        // Extract decorator nodes.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "decorator" {
                    let text = state.node_text(child);
                    // Get the decorator name (strip @ and potential arguments).
                    let raw = text.trim_start_matches('@');
                    let name = raw
                        .split('(')
                        .next()
                        .unwrap_or(raw)
                        .trim()
                        .to_string();
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::@{}", state.qualified_prefix(), name);
                    let dec_id = generate_node_id(
                        &state.file_path,
                        &NodeKind::Decorator,
                        &name,
                        start_line,
                    );

                    let graph_node = Node {
                        id: dec_id.clone(),
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
            updated_at: state.timestamp,
                    };
                    state.nodes.push(graph_node);

                    // Annotates edge from decorator to the decorated item.
                    if let Some((ref kind, ref inner_name, inner_line)) = inner_kind_and_name {
                        let target_id =
                            generate_node_id(&state.file_path, kind, inner_name, inner_line);
                        state.edges.push(Edge {
                            source: dec_id,
                            target: target_id,
                            kind: EdgeKind::Annotates,
                            line: Some(start_line),
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Now visit the inner definition itself.
        if let Some(inner) = inner_def {
            match inner.kind() {
                "function_definition" => Self::visit_function(state, inner, is_async),
                "class_definition" => Self::visit_class(state, inner),
                _ => {}
            }
        }
    }

    /// Extract an import statement (e.g., `import os`).
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        // import_statement children include dotted_name nodes.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                    let import_name = if child.kind() == "aliased_import" {
                        // aliased_import has a dotted_name child
                        Self::find_child_by_kind(child, "dotted_name")
                            .map(|n| state.node_text(n))
                            .unwrap_or_else(|| state.node_text(child))
                    } else {
                        state.node_text(child)
                    };
                    Self::create_use_node(state, &import_name, node);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a from-import statement (e.g., `from os.path import join, exists`).
    fn visit_import_from(state: &mut ExtractionState, node: TsNode<'_>) {
        // Get the module being imported from.
        let module_name = Self::find_child_by_kind(node, "dotted_name")
            .or_else(|| Self::find_child_by_kind(node, "relative_import"))
            .map(|n| state.node_text(n))
            .unwrap_or_default();

        // Find the imported names in the import list or a single name.
        // Look for import_prefix children that represent the imported symbols.
        let mut found_names = false;

        // Check for wildcard import: from X import *
        if Self::find_child_by_kind(node, "wildcard_import").is_some() {
            let full_name = format!("{}.*", module_name);
            Self::create_use_node(state, &full_name, node);
            return;
        }

        // Look for individual imported names
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "dotted_name" => {
                        // Skip the module name (first dotted_name) — only process
                        // dotted_names that appear after "import" keyword.
                        // We use position: module dotted_name is before "import",
                        // imported names are after.
                    }
                    "aliased_import" => {
                        // aliased_import has a dotted_name child for the original name
                        let import_name = Self::find_child_by_kind(child, "dotted_name")
                            .map(|n| state.node_text(n))
                            .unwrap_or_else(|| state.node_text(child));
                        let full_name = if module_name.is_empty() {
                            import_name
                        } else {
                            format!("{}.{}", module_name, import_name)
                        };
                        Self::create_use_node(state, &full_name, node);
                        found_names = true;
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // If we didn't find aliased imports, look for the import list pattern
        // where names appear as direct identifiers or dotted_names after "import"
        if !found_names {
            Self::extract_from_import_names(state, node, &module_name);
        }
    }

    /// Extract individual import names from a from-import statement.
    fn extract_from_import_names(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        module_name: &str,
    ) {
        // In tree-sitter-python, `from X import a, b` has children:
        // "from", dotted_name, "import", dotted_name, ",", dotted_name
        // We need to skip past the "import" keyword to find the imported names.
        let mut cursor = node.walk();
        let mut past_import_keyword = false;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "import" {
                    past_import_keyword = true;
                } else if past_import_keyword {
                    match child.kind() {
                        "dotted_name" => {
                            let import_name = state.node_text(child);
                            let full_name = if module_name.is_empty() {
                                import_name
                            } else {
                                format!("{}.{}", module_name, import_name)
                            };
                            Self::create_use_node(state, &full_name, node);
                        }
                        "aliased_import" => {
                            let import_name = Self::find_child_by_kind(child, "dotted_name")
                                .map(|n| state.node_text(n))
                                .unwrap_or_else(|| state.node_text(child));
                            let full_name = if module_name.is_empty() {
                                import_name
                            } else {
                                format!("{}.{}", module_name, import_name)
                            };
                            Self::create_use_node(state, &full_name, node);
                        }
                        _ => {}
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Create a Use node for an import.
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
            reference_name: name.to_string(),
            reference_kind: EdgeKind::Uses,
            line: start_line,
            column: start_column,
            file_path: state.file_path.clone(),
        });
    }

    /// Visit an assignment at module level and check if it's a constant (UPPER_CASE).
    fn visit_assignment(state: &mut ExtractionState, node: TsNode<'_>) {
        // Get the left side of the assignment.
        let left = node.child_by_field_name("left");
        if let Some(left_node) = left {
            let name = state.node_text(left_node);
            if Self::is_upper_snake_case(&name) {
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

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract base classes from a class definition's argument_list.
    fn extract_base_classes(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        if let Some(arg_list) = Self::find_child_by_kind(node, "argument_list") {
            let mut cursor = arg_list.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    match child.kind() {
                        "identifier" => {
                            let base_name = state.node_text(child);
                            let line = child.start_position().row as u32;
                            let column = child.start_position().column as u32;
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: class_id.to_string(),
                                reference_name: base_name,
                                reference_kind: EdgeKind::Extends,
                                line,
                                column,
                                file_path: state.file_path.clone(),
                            });
                        }
                        "attribute" => {
                            // e.g., module.ClassName
                            let base_name = state.node_text(child);
                            let line = child.start_position().row as u32;
                            let column = child.start_position().column as u32;
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: class_id.to_string(),
                                reference_name: base_name,
                                reference_kind: EdgeKind::Extends,
                                line,
                                column,
                                file_path: state.file_path.clone(),
                            });
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

    /// Extract the function signature (def name(params) or async def name(params)).
    fn extract_function_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Use the block child's start byte to find where the body begins,
        // so we don't truncate at `:` inside type annotations.
        if let Some(block) = Self::find_child_by_kind(node, "block") {
            let text = state.node_text(node);
            let block_offset = block.start_byte() - node.start_byte();
            let before_block = &text[..block_offset];
            // Strip the trailing `:` and whitespace before the block.
            Some(before_block.trim().trim_end_matches(':').trim().to_string())
        } else {
            Some(state.node_text(node).trim().to_string())
        }
    }

    /// Extract the class signature (class Name or class Name(Base)).
    fn extract_class_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if let Some(block) = Self::find_child_by_kind(node, "block") {
            let text = state.node_text(node);
            let block_offset = block.start_byte() - node.start_byte();
            let before_block = &text[..block_offset];
            Some(before_block.trim().trim_end_matches(':').trim().to_string())
        } else {
            Some(state.node_text(node).trim().to_string())
        }
    }

    /// Extract docstrings from the first statement in a function/class body.
    /// Python convention: first expression_statement containing a string literal.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let body = Self::find_child_by_kind(node, "block")?;
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "expression_statement" {
                    // Look for a string child.
                    if let Some(string_node) = Self::find_child_by_kind(child, "string") {
                        let text = state.node_text(string_node);
                        return Some(Self::strip_docstring_quotes(&text));
                    }
                    // If the first expression_statement isn't a string, stop looking.
                    return None;
                }
                // Skip comment nodes at the top of the block.
                if child.kind() != "comment" {
                    return None;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Strip triple-quote markers from a docstring.
    fn strip_docstring_quotes(text: &str) -> String {
        let trimmed = text.trim();
        // Handle triple double quotes
        if trimmed.starts_with("\"\"\"") && trimmed.ends_with("\"\"\"") && trimmed.len() >= 6 {
            return trimmed[3..trimmed.len() - 3].trim().to_string();
        }
        // Handle triple single quotes
        if trimmed.starts_with("'''") && trimmed.ends_with("'''") && trimmed.len() >= 6 {
            return trimmed[3..trimmed.len() - 3].trim().to_string();
        }
        // Handle single quotes
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
        trimmed.to_string()
    }

    /// Check if a function_definition (possibly inside decorated_definition) has async keyword.
    fn has_async_keyword(node: TsNode<'_>) -> bool {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "async" {
                    return true;
                }
                // Also check inside function_definition for `async` keyword
                if child.kind() == "function_definition" {
                    return Self::has_async_keyword(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call" => {
                        // Get the callee: the first named child (function being called).
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
                        // Recurse into the call for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested function definitions to avoid polluting call sites.
                    "function_definition" => {}
                    "class_definition" => {}
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

    /// Determine Python visibility:
    /// - `__dunder__` (starts and ends with __) → Pub
    /// - `__mangled` (starts with __ but doesn't end with __) → Private
    /// - `_private` (starts with _) → Private
    /// - everything else → Pub
    fn python_visibility(name: &str) -> Visibility {
        if name.starts_with("__") && name.ends_with("__") && name.len() > 4 {
            Visibility::Pub // dunder methods
        } else if name.starts_with('_') {
            Visibility::Private // name mangling or convention private
        } else {
            Visibility::Pub
        }
    }

    /// Check if a name is UPPER_SNAKE_CASE (module-level constant convention).
    fn is_upper_snake_case(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        // Must contain at least one uppercase letter
        let has_upper = name.chars().any(|c| c.is_ascii_uppercase());
        // All chars must be uppercase letters, digits, or underscores
        let all_valid = name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        // Must not start with a digit
        let starts_ok = !name.starts_with(|c: char| c.is_ascii_digit());
        has_upper && all_valid && starts_ok
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

impl crate::extraction::LanguageExtractor for PythonExtractor {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn language_name(&self) -> &str {
        "Python"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        PythonExtractor::extract_python(file_path, source)
    }
}
