//! Tree-sitter based `GDScript` (Godot 4.x) source code extractor.
//!
//! Built on the vendored `tree-sitter-gdscript` grammar (`PrestonKnopp`, ABI
//! 14, external `scanner.c`; key `"gdscript"`). Every `.gd` file is an
//! implicit script class: the file root optionally carries a
//! `class_name_statement` (-> a `Class` node) or falls back to a `Module`
//! node named after the file stem when absent. `extends` (either embedded in
//! `class_name Foo extends Bar` or as a standalone statement) becomes an
//! `Extends` edge from that node.
//!
//! Node kind notes verified against `vendor/tree-sitter-gdscript/src/node-types.json`:
//! - `func _init(...):` parses as a dedicated `constructor_definition` node
//!   (not `function_definition`), so no name-based `_init` sniffing is
//!   needed — it maps directly to `NodeKind::Constructor`.
//! - `class_body` (inner `class X:`) does not accept `constructor_definition`
//!   per the grammar's own node-types (a grammar gap, not ours): `_init`
//!   inside a nested class is only reachable as a `function_definition`
//!   there, which this extractor emits as a plain `Method`.
//! - `@export`/`@onready` vars are their own node kinds
//!   (`export_variable_statement` / `onready_variable_statement`), mapped to
//!   `Field` alongside plain `variable_statement`.
//! - Local `var` inside a function body is never visited as a Field: the
//!   member dispatcher only walks `source` (file root) and `class_body`
//!   children; function bodies are only walked by `extract_call_sites`,
//!   which looks solely for `call`/`attribute_call` nodes.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, GDSCRIPT_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from `GDScript` source files.
pub struct GdScriptExtractor;

/// Kind of lexical scope on the scope stack. Distinct from the emitted
/// `NodeKind` of the scope-owning node: the script's own File/Class/Module
/// wrapper node still pushes a `Script` scope so its members nest correctly
/// via `Contains`, while `Function`-vs-`Method` classification depends on
/// whether we're directly at that top `Script` scope or inside a nested
/// `class X:` block (`Class` scope) — matching the grammar's own split
/// between `source`/`body`-level statements and `class_body`-level ones.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    File,
    Script,
    Class,
}

/// One entry on the scope stack — used to build qualified names and parent
/// `Contains` edges.
struct Scope {
    kind: ScopeKind,
    qual: String,
    id: String,
}

/// Internal state used during AST traversal.
struct ExtractionState {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    unresolved_refs: Vec<UnresolvedRef>,
    errors: Vec<String>,
    scope_stack: Vec<Scope>,
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
            scope_stack: Vec::new(),
            file_path: file_path.to_string(),
            source: source.as_bytes().to_vec(),
            timestamp,
        }
    }

    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }

    fn current_scope(&self) -> &Scope {
        match self.scope_stack.last() {
            Some(scope) => scope,
            None => unreachable!("scope stack underflow"),
        }
    }

    fn parent_node_id(&self) -> Option<&str> {
        self.scope_stack.last().map(|s| s.id.as_str())
    }

    /// Build a member's qualified name from the current scope. The outer
    /// `File` scope uses the `path::name` convention shared with the other
    /// extractors; nested (script/class) members use dotted notation.
    fn member_qualified_name(&self, name: &str) -> String {
        let s = self.current_scope();
        match s.kind {
            ScopeKind::File => format!("{}::{}", s.qual, name),
            ScopeKind::Script | ScopeKind::Class => format!("{}.{}", s.qual, name),
        }
    }

    fn push_contains_edge(&mut self, child_id: &str, line: u32) {
        if let Some(parent_id) = self.parent_node_id() {
            self.edges.push(Edge {
                source: parent_id.to_string(),
                target: child_id.to_string(),
                kind: EdgeKind::Contains,
                line: Some(line),
            });
        }
    }

    /// Build and store a node with sane defaults, returning its id.
    #[allow(clippy::too_many_arguments)]
    fn add_node(
        &mut self,
        kind: NodeKind,
        name: &str,
        qualified_name: String,
        node: TsNode<'_>,
        attrs_start_line: u32,
        signature: Option<String>,
        docstring: Option<String>,
        visibility: Visibility,
        metrics: crate::extraction::complexity::ComplexityMetrics,
    ) -> Option<String> {
        if name.is_empty() {
            return None;
        }
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&self.file_path, &kind, name, start_line);
        let graph_node = Node {
            id: id.clone(),
            kind,
            name: name.to_string(),
            qualified_name,
            file_path: self.file_path.clone(),
            start_line,
            attrs_start_line,
            end_line: node.end_position().row as u32,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
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
            updated_at: self.timestamp,
            parent_id: None,
        };
        self.nodes.push(graph_node);
        self.push_contains_edge(&id, start_line);
        Some(id)
    }
}

impl GdScriptExtractor {
    /// Extract code graph nodes and edges from a `GDScript` source file.
    pub fn extract_gdscript(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return Self::build_result(state, start);
            }
        };
        let root = tree.root_node();
        if root.has_error() {
            // Tree-sitter still produced a (partial) tree; keep going and
            // extract whatever parsed cleanly, but flag it for visibility.
            state
                .errors
                .push(format!("parse errors in {file_path} (partial extraction)"));
        }

        // File root node.
        let file_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        state.nodes.push(Node {
            id: file_id.clone(),
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
        });
        state.scope_stack.push(Scope {
            kind: ScopeKind::File,
            qual: file_path.to_string(),
            id: file_id,
        });

        // The script's own identity: `class_name` -> Class, else a Module
        // fallback named after the file stem.
        let class_name_node = Self::find_child_by_kind(root, "class_name_statement");
        let script_name = match class_name_node {
            Some(cn) => cn
                .child_by_field_name("name")
                .map_or_else(String::new, |n| state.node_text(n)),
            None => Self::file_stem(file_path),
        };
        let script_kind = if class_name_node.is_some() {
            NodeKind::Class
        } else {
            NodeKind::Module
        };
        let attrs_start = class_name_node.map_or(0, |n| Self::attrs_start_line(n));
        let docstring = class_name_node.and_then(|n| Self::extract_docstring(&state, n));
        let signature = class_name_node.map(|n| Self::first_line(&state, n));
        let qn = state.member_qualified_name(&script_name);

        let script_id = state.add_node(
            script_kind,
            &script_name,
            qn.clone(),
            root,
            attrs_start,
            signature,
            docstring,
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
        let Some(script_id) = script_id else {
            state.scope_stack.pop();
            return Self::build_result(state, start);
        };

        // Extends target: embedded in `class_name Foo extends Bar`, or a
        // standalone `extends Bar` statement (with or without `class_name`).
        let extends_stmt = class_name_node
            .and_then(|cn| cn.child_by_field_name("extends"))
            .or_else(|| Self::find_child_by_kind(root, "extends_statement"));
        if let Some(es) = extends_stmt {
            if let Some(target) = Self::extends_text(&state, es) {
                Self::push_ref(&mut state, &script_id, &target, EdgeKind::Extends, es);
            }
        }

        state.scope_stack.push(Scope {
            kind: ScopeKind::Script,
            qual: qn,
            id: script_id,
        });
        Self::visit_members(&mut state, root);
        state.scope_stack.pop(); // Script
        state.scope_stack.pop(); // File

        Self::build_result(state, start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("gdscript");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load GDScript grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit direct named children of a member container (`source` file root
    /// or an inner `class_body`), dispatching declarations. `class_name`/
    /// `extends` statements are already consumed by the caller and simply
    /// fall through the dispatcher's default arm.
    fn visit_members(state: &mut ExtractionState, container: TsNode<'_>) {
        let mut cursor = container.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    Self::visit_node(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "function_definition" => Self::visit_function(state, node),
            "constructor_definition" => Self::visit_constructor(state, node),
            "signal_statement" => Self::visit_signal(state, node),
            "variable_statement" | "export_variable_statement" | "onready_variable_statement" => {
                Self::visit_field(state, node);
            }
            "const_statement" => Self::visit_const(state, node),
            "enum_definition" => Self::visit_enum(state, node),
            "class_definition" => Self::visit_inner_class(state, node),
            _ => {}
        }
    }

    /// `func name(...):` — a `Method` inside a nested `class X:` block,
    /// otherwise a `Function` (the script's own top-level members).
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let kind = match state.current_scope().kind {
            ScopeKind::Class => NodeKind::Method,
            ScopeKind::Script | ScopeKind::File => NodeKind::Function,
        };
        let body = node.child_by_field_name("body");
        let metrics = body
            .map(|b| count_complexity(b, &GDSCRIPT_COMPLEXITY, &state.source))
            .unwrap_or_default();
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::signature_text(state, node, body));
        let id = state.add_node(
            kind,
            &name,
            qn,
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            metrics,
        );
        let Some(id) = id else { return };

        if let Some(rt) = node.child_by_field_name("return_type") {
            let ty = state.node_text(rt);
            if !ty.is_empty() {
                Self::push_ref(state, &id, &ty, EdgeKind::TypeOf, rt);
            }
        }
        if let Some(body) = body {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// `func _init(...):` — the grammar emits a dedicated `constructor_definition`
    /// node for this (see module docs), so no name sniffing is needed.
    fn visit_constructor(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = "_init";
        let body = node.child_by_field_name("body");
        let metrics = body
            .map(|b| count_complexity(b, &GDSCRIPT_COMPLEXITY, &state.source))
            .unwrap_or_default();
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(name);
        let signature = Some(Self::signature_text(state, node, body));
        let id = state.add_node(
            NodeKind::Constructor,
            name,
            qn,
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            metrics,
        );
        let Some(id) = id else { return };

        if let Some(rt) = node.child_by_field_name("return_type") {
            let ty = state.node_text(rt);
            if !ty.is_empty() {
                Self::push_ref(state, &id, &ty, EdgeKind::TypeOf, rt);
            }
        }
        if let Some(body) = body {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// `signal foo(a, b)`.
    fn visit_signal(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::first_line(state, node));
        state.add_node(
            NodeKind::Signal,
            &name,
            qn,
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
    }

    /// `var`/`@export var`/`@onready var` at script or class scope -> Field.
    /// Never called for locals: function bodies are only walked by
    /// `extract_call_sites`, which doesn't dispatch to this.
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::first_line(state, node));
        let id = state.add_node(
            NodeKind::Field,
            &name,
            qn,
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
        let Some(id) = id else { return };
        Self::push_type_ref(state, &id, node);
    }

    /// `const NAME = value`.
    fn visit_const(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::first_line(state, node));
        let id = state.add_node(
            NodeKind::Const,
            &name,
            qn,
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
        let Some(id) = id else { return };
        Self::push_type_ref(state, &id, node);
    }

    /// Push a `TypeOf` ref for a `variable_statement`/`const_statement`'s
    /// `type` field, if present and it's an explicit `type` node (not
    /// `inferred_type`, which carries no type text).
    fn push_type_ref(state: &mut ExtractionState, id: &str, node: TsNode<'_>) {
        if let Some(ty_node) = node.child_by_field_name("type") {
            if ty_node.kind() == "type" {
                let ty = state.node_text(ty_node);
                if !ty.is_empty() {
                    Self::push_ref(state, id, &ty, EdgeKind::TypeOf, ty_node);
                }
            }
        }
    }

    /// `enum Name { A, B, C }` (or anonymous `enum { A, B }`).
    fn visit_enum(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::first_line(state, node));
        let id = state.add_node(
            NodeKind::Enum,
            &name,
            qn.clone(),
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
        let Some(id) = id else { return };

        state.scope_stack.push(Scope {
            kind: ScopeKind::Class,
            qual: qn,
            id,
        });
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_enum_variants(state, body);
        }
        state.scope_stack.pop();
    }

    fn visit_enum_variants(state: &mut ExtractionState, list: TsNode<'_>) {
        let mut cursor = list.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enumerator" {
                    Self::visit_enumerator(state, child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn visit_enumerator(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(left) = node.child_by_field_name("left") else {
            return;
        };
        let name = state.node_text(left);
        if name.is_empty() {
            return;
        }
        let attrs_start = node.start_position().row as u32;
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::first_line(state, node));
        state.add_node(
            NodeKind::EnumVariant,
            &name,
            qn,
            node,
            attrs_start,
            signature,
            None,
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
    }

    /// Nested `class Name: ...` (or `class Name extends Base: ...`).
    fn visit_inner_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let attrs_start = Self::attrs_start_line(node);
        let qn = state.member_qualified_name(&name);
        let signature = Some(Self::first_line(state, node));
        let id = state.add_node(
            NodeKind::InnerClass,
            &name,
            qn.clone(),
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            Visibility::Pub,
            crate::extraction::complexity::ComplexityMetrics::default(),
        );
        let Some(id) = id else { return };

        let extends_stmt = node.child_by_field_name("extends").or_else(|| {
            node.child_by_field_name("body")
                .and_then(|b| Self::find_child_by_kind(b, "extends_statement"))
        });
        if let Some(es) = extends_stmt {
            if let Some(target) = Self::extends_text(state, es) {
                Self::push_ref(state, &id, &target, EdgeKind::Extends, es);
            }
        }

        state.scope_stack.push(Scope {
            kind: ScopeKind::Class,
            qual: qn,
            id,
        });
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_members(state, body);
        }
        state.scope_stack.pop();
    }

    // ----------------------------
    // Helpers
    // ----------------------------

    /// Record an unresolved cross-file reference (extends/typeof/calls).
    fn push_ref(
        state: &mut ExtractionState,
        from_id: &str,
        name: &str,
        kind: EdgeKind,
        at: TsNode<'_>,
    ) {
        if name.is_empty() {
            return;
        }
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: from_id.to_string(),
            reference_name: name.to_string(),
            reference_kind: kind,
            line: at.start_position().row as u32,
            column: at.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
    }

    /// The `extends` target's text: a `type` node (class/dotted name) or a
    /// `string` literal (`res://...` path), whichever the `extends_statement`
    /// carries.
    fn extends_text(state: &ExtractionState, extends_stmt: TsNode<'_>) -> Option<String> {
        let mut cursor = extends_stmt.walk();
        if cursor.goto_first_child() {
            loop {
                let c = cursor.node();
                match c.kind() {
                    "type" => return Some(state.node_text(c)),
                    "string" => {
                        let raw = state.node_text(c);
                        return Some(raw.trim_matches(|ch| ch == '"' || ch == '\'').to_string());
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// File stem (basename without the `.gd` extension) used as the Module
    /// fallback name when a script has no `class_name`.
    fn file_stem(file_path: &str) -> String {
        let base = file_path.rsplit('/').next().unwrap_or(file_path);
        base.strip_suffix(".gd").unwrap_or(base).to_string()
    }

    /// First physical line of a node, trimmed — used as a compact signature.
    fn first_line(state: &ExtractionState, node: TsNode<'_>) -> String {
        state
            .node_text(node)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    /// Function/constructor signature: source text from the node start up to
    /// (excluding) its body, trimmed. `GDScript` has no closing brace, so trim
    /// a trailing `:` instead.
    fn signature_text(
        state: &ExtractionState,
        node: TsNode<'_>,
        body: Option<TsNode<'_>>,
    ) -> String {
        if let Some(body) = body {
            let start = node.start_byte();
            let end = body.start_byte().min(state.source.len());
            if end > start {
                if let Ok(s) = std::str::from_utf8(&state.source[start..end]) {
                    return s.trim().trim_end_matches(':').trim().to_string();
                }
            }
        }
        Self::first_line(state, node)
    }

    /// Start line including a leading `#` comment block, so refactoring
    /// tools can select the doc + declaration together.
    fn attrs_start_line(node: TsNode<'_>) -> u32 {
        let mut start = node.start_position().row as u32;
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if p.kind() == "comment" {
                start = p.start_position().row as u32;
                prev = p.prev_named_sibling();
            } else {
                break;
            }
        }
        start
    }

    /// Leading `#` comments immediately preceding a declaration.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if p.kind() == "comment" {
                let t = state.node_text(p);
                comments.push(t.trim_start_matches('#').trim().to_string());
                prev = p.prev_named_sibling();
            } else {
                break;
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Walk a function body, recording `call` / `attribute_call` callees as
    /// unresolved `Calls` references. `GDScript` has no nested named function
    /// definitions (only `lambda` expressions), so unlike `ActionScript` there
    /// is no nested-function guard needed here.
    ///
    /// Two additional dynamic-dispatch idioms this codebase's style relies on
    /// heavily are captured alongside direct calls (both verified against the
    /// live grammar via a parse-tree dump, not inferred from node-types.json
    /// alone, which under-documents the bare-attribute shape):
    /// - `Callable(receiver, "method_name")` / `X.call_deferred("method_name")`
    ///   / `X.connect(callback)` — a small, deliberately narrow allowlist of
    ///   well-known Godot dynamic-dispatch APIs. See `dynamic_dispatch_targets`.
    /// - A bare dotted attribute passed as a call argument with no call
    ///   parens (`foo(bar, MyDb._load_from_registry)`) — a function
    ///   reference passed by value, the shape `BaseDatabaseCache`'s
    ///   lazy-init pattern and similar dispatch tables use. See
    ///   `extract_bare_attribute_args`.
    ///
    /// Both were previously invisible to this extractor: a callee referenced
    /// only this way had zero recorded edges, making `tokensave_dead_code`
    /// misreport it as unreachable even though it's genuinely live.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call" | "attribute_call" => {
                        if let Some(callee) = Self::callee_name(state, child) {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_id.to_string(),
                                reference_name: callee,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        for target in Self::dynamic_dispatch_targets(state, child) {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_id.to_string(),
                                reference_name: target,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                    }
                    "arguments" => {
                        Self::extract_bare_attribute_args(state, child, fn_id);
                    }
                    _ => {}
                }
                Self::extract_call_sites(state, child, fn_id);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Extra `Calls`-shaped targets hidden inside a small, deliberately
    /// narrow allowlist of well-known Godot dynamic-dispatch APIs — on top
    /// of the direct `callee_name` edge already recorded for the call site
    /// itself (`Callable(...)`/`call_deferred(...)`/`connect(...)` are each
    /// still recorded as an ordinary call to themselves too):
    /// - `Callable(receiver, "method_name")` — the method is named by a
    ///   string literal (2-arg constructor shape only).
    /// - `X.call_deferred("method_name")` / bare `call_deferred("method_name")`
    ///   — Godot's deferred-call API, first argument a string literal.
    /// - `X.connect(callback)` — Godot's signal-connect API; `callback` is a
    ///   bare identifier or bare dotted-attribute function reference (no call
    ///   parens) naming the handler directly.
    ///
    /// All three were previously invisible: a handler referenced only this
    /// way showed zero incoming edges and misreported as dead code by
    /// `tokensave_dead_code`. Matching is on method name only (not receiver
    /// type — `GDScript` has no static typing strong enough to verify the
    /// receiver really is e.g. a `Signal`), so this is a heuristic, but a
    /// narrow one: these three names are Godot-API-reserved enough in
    /// practice that a same-named unrelated user method is very unlikely.
    fn dynamic_dispatch_targets(state: &ExtractionState, node: TsNode<'_>) -> Vec<String> {
        let Some(method) = Self::find_child_by_kind(node, "identifier").map(|n| state.node_text(n))
        else {
            return Vec::new();
        };
        let Some(args) = node.child_by_field_name("arguments") else {
            return Vec::new();
        };
        let mut named = Vec::new();
        let mut cursor = args.walk();
        if cursor.goto_first_child() {
            loop {
                let c = cursor.node();
                if c.is_named() {
                    named.push(c);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        let first: Option<TsNode<'_>> = named.first().copied();

        match method.as_str() {
            "Callable" if named.len() == 2 && named[1].kind() == "string" => {
                Self::string_literal_text(state, named[1])
                    .into_iter()
                    .collect()
            }
            "call_deferred" => first
                .filter(|n| n.kind() == "string")
                .and_then(|n| Self::string_literal_text(state, n))
                .into_iter()
                .collect(),
            "connect" => first
                .filter(|n| {
                    n.kind() == "identifier"
                        || (n.kind() == "attribute" && Self::is_bare_dotted_attribute(*n))
                })
                .map(|n| state.node_text(n))
                .filter(|s| !s.is_empty())
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Unquoted text of a `string` node, or `None` if empty after trimming
    /// quotes.
    fn string_literal_text(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let raw = state.node_text(node);
        let trimmed = raw.trim_matches(|c| c == '"' || c == '\'');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Scan a call's `arguments` node for bare dotted-attribute references
    /// passed by value (no call parens) — e.g. the second argument of
    /// `BaseDatabaseCache.get_or_create(_h, MyDb._load_from_registry)`. Each
    /// survivor is recorded as a `Calls`-kind reference (matching the
    /// eventual invocation semantics — it's a function value being handed
    /// off to be called later, not data being read) so the referenced
    /// function isn't misread as dead just because it's never *directly*
    /// invoked at this call site.
    fn extract_bare_attribute_args(state: &mut ExtractionState, args: TsNode<'_>, fn_id: &str) {
        let mut cursor = args.walk();
        if cursor.goto_first_child() {
            loop {
                let c = cursor.node();
                if c.kind() == "attribute" && Self::is_bare_dotted_attribute(c) {
                    let text = state.node_text(c);
                    if !text.is_empty() {
                        state.unresolved_refs.push(UnresolvedRef {
                            from_node_id: fn_id.to_string(),
                            reference_name: text,
                            reference_kind: EdgeKind::Calls,
                            line: c.start_position().row as u32,
                            column: c.start_position().column as u32,
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

    /// True when `node` (an `attribute` node) is a bare dotted chain with no
    /// trailing call/subscript — e.g. `MyDb._load_from_registry`, not
    /// `MyDb._load_from_registry()` or `MyDb._load_from_registry[0]`.
    /// Verified against the live grammar via a parse-tree dump: a bare `a.b`
    /// parses as `attribute(identifier, identifier)` — two plain `identifier`
    /// children, no `attribute_call`/`attribute_subscript` wrapper — while a
    /// called or subscripted chain's outermost `attribute` node has one of
    /// those two kinds as its last child instead.
    fn is_bare_dotted_attribute(node: TsNode<'_>) -> bool {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return false;
        }
        loop {
            let c = cursor.node();
            if c.is_named() && matches!(c.kind(), "attribute_call" | "attribute_subscript") {
                return false;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        true
    }

    /// The callee name of a `call` / `attribute_call` node. `attribute_call`
    /// (`obj.method()`) carries the method name as its `identifier` child
    /// directly; the receiver is a preceding named sibling under the
    /// enclosing `attribute` node (`attribute(receiver, attribute_call(...))`
    /// — confirmed via a parse-tree dump), fetched here via
    /// `prev_named_sibling()` and prefixed on, matching the `receiver.method`
    /// convention the Python/TS/JS extractors already use for the same
    /// shape. Previously only the bare method name was emitted, silently
    /// discarding the receiver — a preload-alias (`XScript.some_method()`) or
    /// direct `class_name` receiver (`EquipmentDatabase.get_equipment()`) gave
    /// the resolver no qualifier to disambiguate a same-named method
    /// elsewhere. `call` (`foo()`) carries the callee as its first named
    /// child ahead of the `arguments` field.
    fn callee_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        match node.kind() {
            "attribute_call" => {
                let method =
                    Self::find_child_by_kind(node, "identifier").map(|n| state.node_text(n))?;
                if let Some(receiver) = node.prev_named_sibling() {
                    let receiver_text = state.node_text(receiver);
                    if !receiver_text.is_empty() {
                        return Some(format!("{receiver_text}.{method}"));
                    }
                }
                Some(method)
            }
            "call" => {
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        let c = cursor.node();
                        if c.is_named() && c.kind() != "arguments" {
                            let text = state.node_text(c);
                            let trimmed = text.trim();
                            return if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed.rsplit('.').next().unwrap_or(trimmed).to_string())
                            };
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn find_child_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let c = cursor.node();
                if c.kind() == kind {
                    return Some(c);
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

impl crate::extraction::LanguageExtractor for GdScriptExtractor {
    fn extensions(&self) -> &[&str] {
        &["gd"]
    }

    fn language_name(&self) -> &'static str {
        "GDScript"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_gdscript(file_path, source)
    }
}
