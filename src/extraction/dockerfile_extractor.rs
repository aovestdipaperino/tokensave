/// Tree-sitter based Dockerfile source code extractor.
///
/// Parses Dockerfile source files and emits nodes and edges for the code graph.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

/// Extracts code graph nodes and edges from Dockerfile source files using tree-sitter.
pub struct DockerfileExtractor;

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
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
            errors: Vec::new(),
            node_stack: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            timestamp,
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

impl DockerfileExtractor {
    /// Extract code graph nodes and edges from a Dockerfile source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Dockerfile source code to parse.
    pub fn extract_dockerfile(file_path: &str, source: &str) -> ExtractionResult {
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
        let language = crate::extraction::ts_provider::language("dockerfile");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Dockerfile grammar: {e}"))?;
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
            "from_instruction" => Self::visit_from(state, node),
            "env_instruction" => Self::visit_env(state, node),
            "arg_instruction" => Self::visit_arg(state, node),
            "expose_instruction" => Self::visit_expose(state, node),
            "label_instruction" => Self::visit_label(state, node),
            "copy_instruction" => Self::visit_copy(state, node),
            _ => {}
        }
    }

    /// Extract a FROM instruction.
    ///
    /// `FROM image AS alias` creates a Module node for the stage.
    /// `FROM image` (no alias) creates a Use node for the base image.
    ///
    /// When a named stage is created, subsequent instructions until the next
    /// FROM are considered children of that stage.
    fn visit_from(state: &mut ExtractionState, node: TsNode<'_>) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;

        // If there's a previous stage on the stack (not the file root), pop it
        // so the new FROM starts a new scope. The file root is always at index 0.
        if state.node_stack.len() > 1 {
            state.node_stack.pop();
        }

        // Check for AS alias (named stage).
        let alias = node.child_by_field_name("as").map(|n| state.node_text(n));

        // Get the image spec text.
        let image_spec = Self::find_child_by_kind(node, "image_spec")
            .map(|n| state.node_text(n))
            .unwrap_or_default();

        let text = state.node_text(node);

        if let Some(alias_name) = alias {
            // Named stage -> Module node.
            let kind = NodeKind::Module;
            let qualified_name = format!("{}::{}", state.qualified_prefix(), alias_name);
            let id = generate_node_id(&state.file_path, &kind, &alias_name, start_line);

            let graph_node = Node {
                id: id.clone(),
                kind,
                name: alias_name.clone(),
                qualified_name,
                file_path: state.file_path.clone(),
                start_line,
                attrs_start_line: start_line,
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

            // Contains edge from parent.
            if let Some(parent_id) = state.parent_node_id() {
                state.edges.push(Edge {
                    source: parent_id.to_string(),
                    target: id.clone(),
                    kind: EdgeKind::Contains,
                    line: Some(start_line),
                });
            }

            // Push stage onto the stack so subsequent instructions belong to it.
            state.node_stack.push((alias_name, id));
        } else {
            // Unnamed FROM -> Use node for the base image.
            let kind = NodeKind::Use;
            let qualified_name = format!("{}::{}", state.qualified_prefix(), image_spec);
            let id = generate_node_id(&state.file_path, &kind, &image_spec, start_line);

            let graph_node = Node {
                id: id.clone(),
                kind,
                name: image_spec,
                qualified_name,
                file_path: state.file_path.clone(),
                start_line,
                attrs_start_line: start_line,
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

    /// Extract ENV instruction variables as Const nodes.
    ///
    /// Each `env_pair` child has a `name` field and a `value` field.
    fn visit_env(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "env_pair" {
                    Self::extract_env_pair(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extract a single `env_pair` as a Const node.
    fn extract_env_pair(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = match node.child_by_field_name("name") {
            Some(n) => state.node_text(n),
            None => return,
        };

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
            signature: Some(format!("ENV {}", text.trim())),
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

    /// Extract ARG instruction as a Const node.
    ///
    /// `arg_instruction` has a `name` field (the variable name) and an optional
    /// `default` field (the default value).
    fn visit_arg(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = match node.child_by_field_name("name") {
            Some(n) => state.node_text(n),
            None => return,
        };

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

    /// Extract EXPOSE instruction ports as Field nodes.
    fn visit_expose(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "expose_port" {
                    let port_text = state.node_text(child);
                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), port_text);
                    let id = generate_node_id(
                        &state.file_path,
                        &NodeKind::Field,
                        &port_text,
                        start_line,
                    );

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::Field,
                        name: port_text,
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

    /// Extract LABEL instruction key-value pairs as Field nodes.
    fn visit_label(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "label_pair" {
                    let key = if let Some(n) = child.child_by_field_name("key") {
                        state.node_text(n)
                    } else {
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                        continue;
                    };

                    let start_line = child.start_position().row as u32;
                    let end_line = child.end_position().row as u32;
                    let start_column = child.start_position().column as u32;
                    let end_column = child.end_position().column as u32;
                    let text = state.node_text(child);
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), key);
                    let id = generate_node_id(&state.file_path, &NodeKind::Field, &key, start_line);

                    let graph_node = Node {
                        id: id.clone(),
                        kind: NodeKind::Field,
                        name: key,
                        qualified_name,
                        file_path: state.file_path.clone(),
                        start_line,
                        attrs_start_line: start_line,
                        end_line,
                        start_column,
                        end_column,
                        signature: Some(format!("LABEL {}", text.trim())),
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

    /// Extract COPY instruction, looking for `--from=STAGE` references.
    ///
    /// `COPY --from=builder ...` creates a Uses edge from the current stage
    /// (or file) to the referenced stage.
    fn visit_copy(state: &mut ExtractionState, node: TsNode<'_>) {
        let start_line = node.start_position().row as u32;

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "param" {
                    let param_text = state.node_text(child);
                    if let Some(stage_name) = param_text.strip_prefix("--from=") {
                        // Create a Uses edge from the current parent to the
                        // referenced stage. We generate the target ID using the
                        // same convention as visit_from for Module nodes.
                        let target_id = generate_node_id(
                            &state.file_path,
                            &NodeKind::Module,
                            stage_name,
                            0, // We don't know the exact line; use 0 as placeholder
                        );

                        // Find the actual target node ID by searching existing nodes.
                        let resolved_target = state
                            .nodes
                            .iter()
                            .find(|n| n.kind == NodeKind::Module && n.name == stage_name)
                            .map(|n| n.id.clone());

                        let target = resolved_target.unwrap_or(target_id);

                        if let Some(parent_id) = state.parent_node_id() {
                            state.edges.push(Edge {
                                source: parent_id.to_string(),
                                target,
                                kind: EdgeKind::Uses,
                                line: Some(start_line),
                            });
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // ----------------------------
    // Helper methods
    // ----------------------------

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

    /// Build the final `ExtractionResult` from the accumulated state.
    fn build_result(state: ExtractionState, start: Instant) -> ExtractionResult {
        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: Vec::new(),
            errors: state.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl crate::extraction::LanguageExtractor for DockerfileExtractor {
    fn extensions(&self) -> &[&str] {
        &["dockerfile", "Dockerfile"]
    }

    fn language_name(&self) -> &'static str {
        "Dockerfile"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_dockerfile(file_path, source)
    }
}
