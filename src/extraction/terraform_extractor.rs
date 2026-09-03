use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser};

use crate::extraction::ts_state::ExtractionState;
use crate::extraction::LanguageExtractor;
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct TerraformExtractor;

impl TerraformExtractor {
    pub fn extract_terraform(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);
        let file_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        state.nodes.push(Self::node(
            &state,
            NodeKind::File,
            file_path.to_string(),
            file_path.to_string(),
            file_id.clone(),
            0,
            source.lines().count().saturating_sub(1) as u32,
            0,
            None,
        ));

        let mut parser = Parser::new();
        if let Err(error) = parser.set_language(&tree_sitter_hcl::LANGUAGE.into()) {
            state
                .errors
                .push(format!("failed to load Terraform grammar: {error}"));
            return state.build_result(start);
        }
        let Some(tree) = parser.parse(source, None) else {
            state
                .errors
                .push("tree-sitter parse returned None".to_string());
            return state.build_result(start);
        };

        let root = tree.root_node();
        if root.has_error() {
            Self::visit_recovered_blocks(&mut state, root, &file_id, file_path);
        } else if let Some(body) = Self::direct_child(root, "body") {
            Self::visit_body(&mut state, body, &file_id, file_path, true);
        }
        state.build_result(start)
    }

    fn visit_recovered_blocks(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        file_id: &str,
        file_path: &str,
    ) {
        if node.kind() == "block" && node.start_position().column == 0 {
            Self::emit_block(state, node, file_id, file_path);
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit_recovered_blocks(state, cursor.node(), file_id, file_path);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_body(
        state: &mut ExtractionState,
        body: TsNode<'_>,
        parent_id: &str,
        parent_qn: &str,
        top_level: bool,
    ) {
        let mut cursor = body.walk();
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            let child = cursor.node();
            match child.kind() {
                "block" if top_level => Self::emit_block(state, child, parent_id, parent_qn),
                "attribute" if top_level => {
                    Self::emit_attribute(state, child, parent_id, parent_qn, None);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn emit_block(
        state: &mut ExtractionState,
        block: TsNode<'_>,
        parent_id: &str,
        parent_qn: &str,
    ) {
        let (block_type, labels) = Self::block_header(state, block);
        let Some(name) = Self::block_name(&block_type, &labels) else {
            return;
        };
        let start_line = block.start_position().row as u32;
        let end_line = block.end_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);
        let qualified_name = format!("{parent_qn}::{name}");
        let signature = state
            .node_text(block)
            .lines()
            .next()
            .map(str::trim)
            .map(str::to_string);
        state.nodes.push(Self::node(
            state,
            NodeKind::Module,
            name.clone(),
            qualified_name.clone(),
            id.clone(),
            start_line,
            end_line,
            block.start_position().column as u32,
            signature,
        ));
        state.edges.push(Edge {
            source: parent_id.to_string(),
            target: id.clone(),
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });

        if let Some(body) = Self::direct_child(block, "body") {
            let mut cursor = body.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "attribute" {
                        let prefix = (block_type == "locals").then_some("local");
                        Self::emit_attribute(state, child, &id, &qualified_name, prefix);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            if !block.has_error() {
                Self::collect_references(state, body, &id);
            }
        }
    }

    fn emit_attribute(
        state: &mut ExtractionState,
        attribute: TsNode<'_>,
        parent_id: &str,
        parent_qn: &str,
        prefix: Option<&str>,
    ) {
        let Some(identifier) = Self::direct_child(attribute, "identifier") else {
            return;
        };
        let key = state.node_text(identifier);
        let name = prefix.map_or_else(|| key.clone(), |prefix| format!("{prefix}.{key}"));
        let start_line = attribute.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
        let signature = state
            .node_text(attribute)
            .lines()
            .next()
            .map(str::trim)
            .map(str::to_string);
        state.nodes.push(Self::node(
            state,
            NodeKind::Const,
            name.clone(),
            format!("{parent_qn}::{name}"),
            id.clone(),
            start_line,
            attribute.end_position().row as u32,
            attribute.start_position().column as u32,
            signature,
        ));
        state.edges.push(Edge {
            source: parent_id.to_string(),
            target: id,
            kind: EdgeKind::Contains,
            line: Some(start_line),
        });
    }

    fn block_header(state: &ExtractionState, block: TsNode<'_>) -> (String, Vec<String>) {
        let mut block_type = String::new();
        let mut labels = Vec::new();
        let mut cursor = block.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "identifier" if block_type.is_empty() => block_type = state.node_text(child),
                    "string_lit" => {
                        labels.push(state.node_text(child).trim_matches('"').to_string());
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        (block_type, labels)
    }

    fn collect_references(state: &mut ExtractionState, node: TsNode<'_>, owner_id: &str) {
        if node.kind() == "variable_expr" {
            let parts = Self::traversal_parts(state, node);
            if let Some(reference_name) = Self::normalize_reference(&parts) {
                let line = node.start_position().row as u32;
                let column = node.start_position().column as u32;
                let duplicate = state.unresolved_refs.iter().any(|reference| {
                    reference.from_node_id == owner_id
                        && reference.reference_name == reference_name
                        && reference.line == line
                        && reference.column == column
                });
                if !duplicate {
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: owner_id.to_string(),
                        reference_name,
                        reference_kind: EdgeKind::Uses,
                        line,
                        column,
                        file_path: state.file_path.clone(),
                    });
                }
            }
        }

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_references(state, cursor.node(), owner_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn traversal_parts(state: &ExtractionState, variable: TsNode<'_>) -> Vec<String> {
        let mut parts = Vec::new();
        if let Some(identifier) = Self::direct_child(variable, "identifier") {
            parts.push(state.node_text(identifier));
        }
        let mut sibling = variable.next_named_sibling();
        while let Some(node) = sibling {
            if node.kind() != "get_attr" {
                break;
            }
            if let Some(identifier) = Self::direct_child(node, "identifier") {
                parts.push(state.node_text(identifier));
            }
            sibling = node.next_named_sibling();
        }
        parts
    }

    fn normalize_reference(parts: &[String]) -> Option<String> {
        match parts {
            [root, name, ..] if root == "var" => Some(format!("var.{name}")),
            [root, name, ..] if root == "local" => Some(format!("local.{name}")),
            [root, name, ..] if root == "module" => Some(format!("module.{name}")),
            [root, name, ..] if root == "output" => Some(format!("output.{name}")),
            [root, kind, name, ..] if root == "data" => Some(format!("data.{kind}.{name}")),
            [kind, name, ..]
                if !matches!(
                    kind.as_str(),
                    "path" | "terraform" | "count" | "each" | "self"
                ) =>
            {
                Some(format!("resource.{kind}.{name}"))
            }
            _ => None,
        }
    }

    fn block_name(block_type: &str, labels: &[String]) -> Option<String> {
        match (block_type, labels) {
            ("resource", [kind, name, ..]) => Some(format!("resource.{kind}.{name}")),
            ("data", [kind, name, ..]) => Some(format!("data.{kind}.{name}")),
            ("module", [name, ..]) => Some(format!("module.{name}")),
            ("variable", [name, ..]) => Some(format!("var.{name}")),
            ("output", [name, ..]) => Some(format!("output.{name}")),
            ("provider", [name, ..]) => Some(format!("provider.{name}")),
            ("terraform", _) => Some("terraform".to_string()),
            ("locals", _) => Some("locals".to_string()),
            _ => None,
        }
    }

    fn direct_child<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
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

    #[allow(clippy::too_many_arguments)]
    fn node(
        state: &ExtractionState,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        id: String,
        start_line: u32,
        end_line: u32,
        start_column: u32,
        signature: Option<String>,
    ) -> Node {
        Node {
            id,
            kind,
            name,
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column,
            end_column: 0,
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
        }
    }
}

impl LanguageExtractor for TerraformExtractor {
    fn extensions(&self) -> &[&str] {
        &["tf", "tfvars"]
    }

    fn language_name(&self) -> &'static str {
        "Terraform"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_terraform(file_path, source)
    }
}
