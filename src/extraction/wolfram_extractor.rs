/// Tree-sitter based Wolfram Language source code extractor.
///
/// The `tree-sitter-wolfram` grammar is expression-oriented: it has no
/// dedicated definition nodes. Assignments parse as generic `binary` nodes
/// whose anonymous operator child is `=`, `:=`, `^=` or `^:=`, so this
/// extractor classifies each top-level `binary` by its operator and its
/// left-hand side:
///
/// - `f[patterns] = ...` / `f[patterns] := ...` (a `call` with a `symbol`
///   head) is a function definition; the node takes the head symbol name.
/// - `name = ...` / `name := ...` (a bare `symbol`, or a lone `pattern`
///   such as `x_`) is a constant.
/// - Anything else on the left (`part` like `a[[1]]`, `message_name` like
///   `f::usage`, compound heads) updates an existing value instead of
///   declaring a new one, and is skipped.
///
/// Only top-level statements are visited, descending through `infix`
/// (`;`-separated compounds) and `group` nodes. Right-hand sides are never
/// scanned for definitions, so locals inside `Module`/`Block` do not leak
/// out as top-level nodes. `call` nodes with a `symbol` head inside a
/// definition body are recorded as `Calls` unresolved refs.
///
/// Complexity counters stay at zero: Wolfram control flow is call-based
/// (`If`, `While`, `Do`, ...), so the grammar has no branch or loop node
/// kinds to count, and treating every `call` as a branch would be wrong.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::ts_state::ExtractionState;
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

pub struct WolframExtractor;

/// Binary operators that introduce a definition: `Set`, `SetDelayed`,
/// `UpSet`, and `UpSetDelayed`.
const DEFINITION_OPS: &[&str] = &["=", ":=", "^=", "^:="];

impl WolframExtractor {
    pub fn extract_wolfram(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

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

        match Self::parse_source(source) {
            Ok(tree) => Self::visit_top_level_children(&mut state, tree.root_node()),
            Err(msg) => state.errors.push(msg),
        }

        state.node_stack.pop();
        state.build_result(start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("wolfram");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Wolfram grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visits the direct children of `node`, which is either the `source_file`
    /// root or a transparent wrapper (`infix` compound, `group`).
    fn visit_top_level_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit_top_level(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_top_level(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "binary" => Self::visit_binary(state, node),
            "infix" | "group" => Self::visit_top_level_children(state, node),
            _ => {}
        }
    }

    /// Handles one `binary` node, splitting it into left operand, operator,
    /// and right operand.
    fn visit_binary(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some((lhs, op, rhs)) = Self::split_binary(node) else {
            return;
        };
        if !DEFINITION_OPS.contains(&op.as_str()) {
            // Not a definition (comparison, arithmetic, rule, ...). A real
            // definition can still hide inside, e.g. the `g[x] = v` half of
            // TagSet `f /: g[x] = v`, so keep looking through the operands.
            Self::visit_top_level_children(state, node);
            return;
        }
        let Some((kind, name)) = Self::classify_lhs(state, lhs) else {
            // Unrecognized left-hand side such as `(x = y) = 5`: the outer
            // assignment defines nothing itself, but its left side may hold
            // a real definition.
            Self::visit_top_level(state, lhs);
            return;
        };
        Self::emit_definition(state, node, lhs, rhs, kind, &name);
    }

    /// Splits a `binary` node into its (left, operator, right) parts,
    /// skipping comments that may sit between the operands. Returns `None`
    /// when the node does not have the expected shape.
    fn split_binary(node: TsNode<'_>) -> Option<(TsNode<'_>, String, TsNode<'_>)> {
        let mut lhs: Option<TsNode<'_>> = None;
        let mut rhs: Option<TsNode<'_>> = None;
        let mut op: Option<String> = None;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    if child.kind() == "comment" {
                        // Fall through: comments carry no operands.
                    } else if lhs.is_none() {
                        lhs = Some(child);
                    } else if rhs.is_none() {
                        rhs = Some(child);
                    }
                } else if op.is_none() {
                    op = Some(child.kind().to_string());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        match (lhs, op, rhs) {
            (Some(lhs), Some(op), Some(rhs)) => Some((lhs, op, rhs)),
            _ => None,
        }
    }

    /// Classifies the left-hand side of a `=`/`:=`/`^=`/`^:=` assignment.
    /// Returns the node kind and defined name, or `None` when the left side
    /// is an update to an existing value rather than a new definition.
    fn classify_lhs(state: &ExtractionState, lhs: TsNode<'_>) -> Option<(NodeKind, String)> {
        // Unwrap parenthesized left-hand sides iteratively: `(x) = 5` still
        // defines `x`.
        let mut current = lhs;
        for _ in 0..16 {
            if current.kind() != "group" {
                break;
            }
            let mut cursor = current.walk();
            let mut inner: Option<TsNode<'_>> = None;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "comment" {
                        inner = Some(child);
                        break;
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            current = inner?;
        }
        match current.kind() {
            "symbol" => {
                let name = state.node_text(current);
                if name.is_empty() {
                    None
                } else {
                    Some((NodeKind::Const, name))
                }
            }
            "call" => {
                let head = current.child_by_field_name("head")?;
                if head.kind() != "symbol" {
                    return None;
                }
                let name = state.node_text(head);
                if name.is_empty() {
                    None
                } else {
                    Some((NodeKind::Function, name))
                }
            }
            "pattern" => {
                let name_node = current.child_by_field_name("name")?;
                if name_node.kind() != "symbol" {
                    return None;
                }
                let name = state.node_text(name_node);
                if name.is_empty() {
                    None
                } else {
                    Some((NodeKind::Const, name))
                }
            }
            _ => None,
        }
    }

    /// Emits a `Function` or `Const` node plus its `Contains` edge, and
    /// records the `call` sites in the right-hand side.
    fn emit_definition(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        lhs: TsNode<'_>,
        rhs: TsNode<'_>,
        kind: NodeKind,
        name: &str,
    ) {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let qualified_name = format!("{}::{name}", state.qualified_prefix());
        let id = generate_node_id(&state.file_path, &kind, name, start_line);
        let signature = if kind == NodeKind::Function {
            Self::signature_text(state, lhs)
        } else {
            None
        };
        let docstring = Self::extract_docstring(state, node);

        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
            end_line,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
                resolved_by: None,
            });
        }

        Self::collect_calls(state, rhs, &id);
    }

    /// Records one `Calls` unresolved ref per `call` with a `symbol` head in
    /// the definition body. Nested definitions are entered through their
    /// right-hand side only: the defined head call on the left is not a
    /// call site.
    fn collect_calls(state: &mut ExtractionState, node: TsNode<'_>, from_id: &str) {
        if node.kind() == "call" {
            if let Some(head) = node.child_by_field_name("head") {
                if head.kind() == "symbol" {
                    let name = state.node_text(head);
                    if !name.is_empty() {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: from_id.to_string(),
                            reference_name: name,
                            reference_kind: EdgeKind::Calls,
                            line: node.start_position().row as u32,
                            column: node.start_position().column as u32,
                            file_path: state.file_path.clone(),
                        });
                    }
                }
            }
        }
        if node.kind() == "binary" {
            if let Some((_, op, rhs)) = Self::split_binary(node) {
                if DEFINITION_OPS.contains(&op.as_str()) {
                    Self::collect_calls(state, rhs, from_id);
                    return;
                }
            }
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_calls(state, cursor.node(), from_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// The left-hand side pattern (e.g. `f[x_, y_]`) as a single-line
    /// signature, truncated to a reasonable length.
    fn signature_text(state: &ExtractionState, lhs: TsNode<'_>) -> Option<String> {
        let text = state.node_text(lhs);
        let first = text.lines().next().unwrap_or("").trim();
        if first.is_empty() {
            None
        } else {
            Some(first.chars().take(200).collect())
        }
    }

    /// Joins contiguous `(* ... *)` comments directly above the definition.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if p.kind() != "comment" {
                break;
            }
            let text = state.node_text(p);
            let stripped = text
                .strip_prefix("(*")
                .and_then(|t| t.strip_suffix("*)"))
                .map_or_else(|| text.trim().to_string(), |t| t.trim().to_string());
            if !stripped.is_empty() {
                comments.push(stripped);
            }
            prev = p.prev_named_sibling();
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        Some(comments.join("\n"))
    }
}

impl crate::extraction::LanguageExtractor for WolframExtractor {
    fn extensions(&self) -> &[&str] {
        &["wl", "wls", "wlt"]
    }

    fn language_name(&self) -> &'static str {
        "Wolfram"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_wolfram(file_path, source)
    }
}
