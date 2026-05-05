/// Tree-sitter based Lean 4 extractor (arborium-lean grammar).
///
/// The grammar's root is a `module` whose body contains `import`,
/// `namespace`, `section`, `declaration`, and various other top-level
/// constructs. `declaration` is a wrapper around the actual definition
/// kind (`def`, `theorem`, `abbrev`, `axiom`, `constant`, `instance`,
/// `structure`, `inductive`, `class_inductive`, `example`).
///
/// Each named declaration exposes its name via the `name` field. We walk
/// the tree, push namespace/section frames onto a scope stack, and emit
/// graph nodes for declarations, parented to the closest enclosing scope.
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, Visibility,
};

pub struct LeanExtractor;

struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    file_path: String,
    source: Vec<u8>,
    file_node_id: String,
    timestamp: u64,
    /// `(qualified_prefix, parent_id)` — top is the active scope. The file
    /// frame is the last resort and is never popped.
    scope_stack: Vec<(String, String)>,
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

impl LeanExtractor {
    pub fn extract_lean(file_path: &str, source: &str) -> ExtractionResult {
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
            .push((file_path.to_string(), state.file_node_id.clone()));

        if let Ok(tree) = Self::parse(source) {
            Self::visit(&mut state, tree.root_node());
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
        let language = crate::extraction::ts_provider::language("lean");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Lean grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    fn visit(state: &mut ExtractionState, node: TsNode<'_>) {
        // Skip anonymous keyword tokens (e.g. the literal `namespace`,
        // `def`, `theorem` words inside their parent named nodes).
        // Their `kind()` matches the named outer kind but they are not
        // structural — emitting nodes for them would produce phantom
        // `<anonymous>` modules and double-counted definitions.
        if !node.is_named() {
            return;
        }
        match node.kind() {
            "module" | "declaration" => Self::visit_children(state, node),
            "namespace" | "section" => Self::visit_namespace(state, node),
            "import" => Self::visit_import(state, node),
            "def" | "abbrev" | "theorem" => {
                Self::emit_named(state, node, NodeKind::Function);
            }
            // Anonymous `instance` blocks (`instance : Add Nat where ...`)
            // have no name and aren't useful as graph nodes; only emit
            // when a name is present.
            "instance" => Self::emit_if_named(state, node, NodeKind::Const),
            "axiom" | "constant" => {
                Self::emit_named(state, node, NodeKind::Const);
            }
            "structure" => {
                Self::emit_named(state, node, NodeKind::Struct);
            }
            "inductive" | "class_inductive" => {
                Self::emit_named(state, node, NodeKind::Enum);
            }
            // `example` has no name; skip it (it's anonymous by design).
            // `open`, `attribute`, `notation`, `mixfix`, `macro_rules`,
            // `variable`, `universe`, `prelude`, `elab`, `syntax`,
            // `hash_command`, `export`, `builtin_initialize` are out of
            // scope for now — they don't define named graph entities we
            // currently track.
            _ => {}
        }
    }

    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Handles `namespace` / `section`. Named blocks emit a `Module`
    /// node and push a new scope so the body is parented to it. Anonymous
    /// `section` blocks (no `name` field) emit nothing — they're scope
    /// markers in the source, but as graph nodes they'd just be noise —
    /// the body is recursed into so contained defs still get parented to
    /// the *surrounding* scope.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node.child_by_field_name("name").map(|n| state.node_text(n));
        let pushed = if let Some(name) = name.as_deref() {
            let id = Self::emit_node(state, node, NodeKind::Module, name);
            let parent_qn = match state.scope_stack.last() {
                Some((qn, _)) => qn.clone(),
                None => state.file_path.clone(),
            };
            state.scope_stack.push((format!("{parent_qn}::{name}"), id));
            true
        } else {
            false
        };

        // Recurse into every child *except* the `name` field — the rest is
        // the body. Iterating all children with their field names works
        // for both `namespace` and `section`.
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                let field = cursor.field_name();
                if field != Some("name") {
                    Self::visit(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        if pushed {
            state.scope_stack.pop();
        }
    }

    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        // The `module` field on `import` holds the dotted module path.
        if let Some(n) = node.child_by_field_name("module") {
            let target = state.node_text(n);
            Self::push_use_edge(state, node, &target);
            return;
        }
        // Fallback: scan for an identifier child (older grammars / shapes).
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == "identifier" {
                    let target = state.node_text(cursor.node());
                    Self::push_use_edge(state, node, &target);
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn push_use_edge(state: &mut ExtractionState, node: TsNode<'_>, target_path: &str) {
        let target_id = generate_node_id(target_path, &NodeKind::File, target_path, 0);
        let parent_id = match state.scope_stack.last() {
            Some((_, id)) => id.clone(),
            None => state.file_node_id.clone(),
        };
        state.edges.push(Edge {
            source: parent_id,
            target: target_id,
            kind: EdgeKind::Uses,
            line: Some(node.start_position().row as u32),
        });
    }

    /// Emits a named declaration (def/theorem/structure/...). Returns the
    /// new node id so callers (namespace/section) can use it as the parent
    /// for nested content.
    fn emit_named(state: &mut ExtractionState, node: TsNode<'_>, kind: NodeKind) -> String {
        let name = match node.child_by_field_name("name") {
            Some(n) => state.node_text(n),
            None => format!("<anonymous_{}>", node.kind()),
        };
        Self::emit_node(state, node, kind, &name)
    }

    /// Emits a node only if a `name` field is present. Used for
    /// declarations that are commonly anonymous (e.g. `instance`).
    fn emit_if_named(state: &mut ExtractionState, node: TsNode<'_>, kind: NodeKind) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = state.node_text(name_node);
            Self::emit_node(state, node, kind, &name);
        }
    }

    fn emit_node(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        kind: NodeKind,
        name: &str,
    ) -> String {
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let parent_qn = match state.scope_stack.last() {
            Some((qn, _)) => qn.clone(),
            None => state.file_path.clone(),
        };
        let qualified_name = format!("{parent_qn}::{name}");
        let id = generate_node_id(&state.file_path, &kind, name, start_line);

        let signature = state
            .node_text(node)
            .lines()
            .next()
            .map(|l| l.trim().to_string());

        let new_node = Node {
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
        state.nodes.push(new_node);

        if let Some((_, parent_id)) = state.scope_stack.last() {
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

impl crate::extraction::LanguageExtractor for LeanExtractor {
    fn extensions(&self) -> &[&str] {
        &["lean"]
    }

    fn language_name(&self) -> &'static str {
        "Lean"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_lean(file_path, source)
    }
}
