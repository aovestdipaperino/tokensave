/// Tree-sitter based Quint source code extractor.
///
/// The vendored `tree-sitter-quint` grammar is a *shallow* highlighter: it
/// emits only leaf token classes (`keyword`, `storage_modifier`,
/// `identifier`, `string`, `number`, comments, etc.) under a flat
/// `source_file` root. There is no expression or scope tree. We rebuild the
/// minimum structural information ourselves by:
///
/// 1. Walking the token stream of `source_file` in order.
/// 2. Recognising `module Foo`, `def f`, `val v`, `var v`, `const c`,
///    `type T`, `assume a` as "pending kind + name" pairs (the kind comes
///    from a `keyword`/`storage_modifier`, the name from the next
///    `identifier`).
/// 3. Counting `{` / `}` characters in the gaps *between* named tokens so
///    we know when a `module` body opens and closes, so we can parent
///    inner definitions to the correct enclosing module.
///
/// This is intentionally heuristic, not a real Quint parser, but it is
/// sufficient to populate `Module` / `Function` / `Const` / `Static` /
/// `TypeAlias` nodes and `Contains` edges from each definition to its
/// enclosing scope.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

pub struct QuintExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    file_path: String,
    source: Vec<u8>,
    file_node_id: String,
    timestamp: u64,
    /// Nested scopes. The first entry is the file; subsequent entries are
    /// modules. Each tuple is `(qualified_name_prefix, parent_node_id,
    /// brace_depth_at_open)`. A frame is popped when the live brace depth
    /// drops below `brace_depth_at_open`.
    scope_stack: Vec<(String, String, u32)>,
}

impl ExtractionState {
    fn new(file_path: &str, source: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let file_node_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            file_node_id,
            timestamp,
            scope_stack: Vec::new(),
        }
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }
}

/// Pending "kind + parent" picked up from a `module`/`def`/... token,
/// waiting for the next `identifier` to provide the name.
#[derive(Clone, Copy)]
enum PendingKind {
    Module,
    Function,
    Const,
    Static,
    TypeAlias,
}

impl QuintExtractor {
    pub fn extract_quint(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let file_node = Node {
            id: state.file_node_id.clone(),
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
        state.nodes.push(file_node);
        state
            .scope_stack
            .push((file_path.to_string(), state.file_node_id.clone(), 0));

        match Self::parse(source) {
            Ok(tree) => Self::walk_tokens(&mut state, tree.root_node()),
            Err(_msg) => {
                // Parse failed; skip extraction rather than emitting bogus structure.
            }
        }

        ExtractionResult {
            nodes: state.nodes,
            edges: state.edges,
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn parse(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("quint");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Quint grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn walk_tokens(state: &mut ExtractionState, root: TsNode<'_>) {
        let mut depth: u32 = 0;
        let mut pending: Option<PendingKind> = None;
        // When we just emitted a Module node, hold its info here. We push
        // it onto the scope stack on the next `{` so we know the depth at
        // which the matching `}` should pop it.
        let mut pending_open: Option<(String, String)> = None; // (qualified_name, id)
                                                               // Active import collection: dotted parts seen since the `import`
                                                               // keyword, plus the line of that keyword. Committed (i.e. emits a
                                                               // Uses edge) when we hit any token that doesn't extend the path
                                                               // (`from`, `as`, end of stream, another keyword, a storage_modifier,
                                                               // etc.). The shallow grammar gives us identifiers and `.` operators
                                                               // — that's enough to reconstruct the dotted import path.
        let mut import_collect: Option<(Vec<String>, u32)> = None;

        let mut cursor = root.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                let kind = child.kind();
                // The `.` operator extends an import path; everything else
                // is a terminator handled below.
                let extends_import = matches!(kind, "identifier")
                    || (kind == "operator" && state.node_text(child) == ".");
                if !extends_import {
                    if let Some((parts, line)) = import_collect.take() {
                        Self::commit_import(state, &parts, line);
                    }
                }

                match kind {
                    "{" => {
                        depth += 1;
                        if let Some((qn, id)) = pending_open.take() {
                            state.scope_stack.push((qn, id, depth));
                        }
                    }
                    "}" => {
                        depth = depth.saturating_sub(1);
                        // Pop module frames whose opening depth is now
                        // above the live depth. Never pop the file frame.
                        while state.scope_stack.len() > 1 {
                            let opened_at = match state.scope_stack.last() {
                                Some((_, _, d)) => *d,
                                None => 0,
                            };
                            if opened_at > depth {
                                state.scope_stack.pop();
                            } else {
                                break;
                            }
                        }
                    }
                    "keyword" => {
                        let txt = state.node_text(child);
                        if txt == "module" {
                            pending = Some(PendingKind::Module);
                        } else if txt == "import" {
                            import_collect = Some((Vec::new(), child.start_position().row as u32));
                            pending = None;
                        }
                    }
                    "storage_modifier" => {
                        // `pure` prefixes `def`/`val`; we just keep
                        // updating `pending` until the meaningful
                        // storage_modifier arrives.
                        if let Some(p) = quint_storage_kind(&state.node_text(child)) {
                            pending = Some(p);
                        }
                    }
                    "identifier" => {
                        if let Some((parts, _)) = import_collect.as_mut() {
                            parts.push(state.node_text(child));
                        } else if let Some(p) = pending.take() {
                            let name = state.node_text(child);
                            let id = Self::emit_node(state, child, p, &name);
                            if matches!(p, PendingKind::Module) {
                                let qualified = match state.scope_stack.last() {
                                    Some((qn, _, _)) => format!("{qn}::{name}"),
                                    None => name.clone(),
                                };
                                pending_open = Some((qualified, id));
                            }
                        }
                    }
                    "operator" => {
                        // `.` inside an import path is part of the dotted
                        // module name and shouldn't reset pending.
                        if import_collect.is_none() {
                            pending = None;
                        }
                    }
                    "line_comment" | "block_comment" | "hashbang" => {
                        // Comments don't reset `pending`: `def /* doc */ f`
                        // is still a definition of `f`.
                    }
                    _ => {
                        // Any other token (string, number, constant,
                        // storage_type, punctuation `( ) [ ] , ;`, etc.)
                        // breaks a pending pattern.
                        pending = None;
                    }
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // End-of-file flush: an import at the very end of the file with
        // no following terminator still needs its Uses edge.
        if let Some((parts, line)) = import_collect.take() {
            Self::commit_import(state, &parts, line);
        }
    }

    /// Joins the collected dotted-path parts and emits a `Uses` edge from
    /// the current scope to a synthetic file node for the imported module.
    /// `parts` may be empty (e.g. `import` with nothing after it) — in
    /// that case nothing is emitted.
    fn commit_import(state: &mut ExtractionState, parts: &[String], line: u32) {
        if parts.is_empty() {
            return;
        }
        let target_path = parts.join(".");
        let target_id = generate_node_id(&target_path, &NodeKind::File, &target_path, 0);
        let parent_id = match state.scope_stack.last() {
            Some((_, id, _)) => id.clone(),
            None => state.file_node_id.clone(),
        };
        state.edges.push(Edge {
            source: parent_id,
            target: target_id,
            kind: EdgeKind::Uses,
            line: Some(line),
        });
    }

    /// Emits a node + Contains edge attached to the current scope. Returns
    /// the new node id so the caller can promote modules onto the scope stack.
    fn emit_node(
        state: &mut ExtractionState,
        ident_node: TsNode<'_>,
        pending: PendingKind,
        name: &str,
    ) -> String {
        let kind = match pending {
            PendingKind::Module => NodeKind::Module,
            PendingKind::Function => NodeKind::Function,
            PendingKind::Const => NodeKind::Const,
            PendingKind::Static => NodeKind::Static,
            PendingKind::TypeAlias => NodeKind::TypeAlias,
        };
        let start_line = ident_node.start_position().row as u32;
        let end_line = ident_node.end_position().row as u32;
        let start_column = ident_node.start_position().column as u32;
        let end_column = ident_node.end_position().column as u32;

        let parent_qn = match state.scope_stack.last() {
            Some((qn, _, _)) => qn.clone(),
            None => state.file_path.clone(),
        };
        let qualified_name = format!("{parent_qn}::{name}");
        let id = generate_node_id(&state.file_path, &kind, name, start_line);

        let node_obj = Node {
            id: id.clone(),
            kind,
            name: name.to_string(),
            qualified_name,
            file_path: state.file_path.clone(),
            start_line,
            attrs_start_line: start_line,
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
        state.nodes.push(node_obj);

        if let Some((_, parent_id, _)) = state.scope_stack.last() {
            state.edges.push(Edge {
                source: parent_id.clone(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        id
    }
}

/// Maps a Quint `storage_modifier` keyword to the kind of definition it
/// introduces. Returns `None` for `pure`/`nondet` because those only
/// modify the *next* storage modifier (`pure def`, `pure val`, `nondet`
/// is a binding form, not a top-level kind).
fn quint_storage_kind(text: &str) -> Option<PendingKind> {
    match text {
        "def" | "action" | "temporal" | "run" => Some(PendingKind::Function),
        "val" | "const" | "assume" => Some(PendingKind::Const),
        "var" => Some(PendingKind::Static),
        "type" => Some(PendingKind::TypeAlias),
        _ => None,
    }
}

impl crate::extraction::LanguageExtractor for QuintExtractor {
    fn extensions(&self) -> &[&str] {
        &["qnt"]
    }

    fn language_name(&self) -> &'static str {
        "Quint"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_quint(file_path, source)
    }
}
