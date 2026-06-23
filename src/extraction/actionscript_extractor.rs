//! Tree-sitter based ActionScript source code extractor.
//!
//! Targets ActionScript 2 (AVM1) as emitted by the JPEXS / FFDec decompiler,
//! and also parses ActionScript 3 (`package { ... }` wrapped). Built on the
//! vendored `tree-sitter-actionscript` grammar (key `"actionscript"`).
//!
//! Emits the standard graph shape: classes, interfaces, methods, free
//! functions, fields/consts, imports, plus `Contains`, `Extends`,
//! `Implements`, `TypeOf`, and `Calls` edges.
//!
//! AS2 note: decompiled classes are top-level with a dotted name
//! (`class com.example.app.Account extends com.example.app.Handler`); the `name` field is a
//! `scoped_data_type`. AS3 wraps the same declarations in a `package`.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, ACTIONSCRIPT_COMPLEXITY};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from ActionScript source files.
pub struct ActionScriptExtractor;

/// Kind of lexical scope on the scope stack.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    File,
    Class,
    Interface,
    Namespace,
}

/// One entry on the scope stack — used to build qualified names, parent
/// `Contains` edges, and to detect constructors (method named like its class).
struct Scope {
    kind: ScopeKind,
    short: String,
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

    /// Gets the source text of a tree-sitter node.
    fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }

    fn current_scope(&self) -> &Scope {
        // The File scope is always pushed first, so this never panics.
        match self.scope_stack.last() {
            Some(scope) => scope,
            None => unreachable!("scope stack underflow"),
        }
    }

    fn parent_node_id(&self) -> Option<&str> {
        self.scope_stack.last().map(|s| s.id.as_str())
    }

    /// Innermost enclosing class short-name, if the current scope is a class.
    fn enclosing_class_short(&self) -> Option<&str> {
        let s = self.current_scope();
        (s.kind == ScopeKind::Class).then_some(s.short.as_str())
    }

    /// Build a member's qualified name from the current scope.
    ///
    /// Class/interface/namespace members use dotted AS notation
    /// (`com.example.app.Account.logon`); file-level symbols use the `path::name`
    /// convention shared with the other extractors.
    fn member_qualified_name(&self, name: &str) -> String {
        let s = self.current_scope();
        match s.kind {
            ScopeKind::File => format!("{}::{}", s.qual, name),
            _ => format!("{}.{}", s.qual, name),
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

impl ActionScriptExtractor {
    /// Extract code graph nodes and edges from an ActionScript source file.
    pub fn extract_actionscript(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return Self::build_result(state, start);
            }
        };

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
            short: file_path.to_string(),
            qual: file_path.to_string(),
            id: file_id,
        });

        Self::visit_children(&mut state, tree.root_node());

        state.scope_stack.pop();
        Self::build_result(state, start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("actionscript");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load ActionScript grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    /// Visit all direct children of a node.
    fn visit_children(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::visit_node(state, cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Dispatch a node by kind. Structural wrappers are descended through;
    /// declarations are handled; expressions/bodies are not auto-descended
    /// (call sites are walked explicitly from function bodies).
    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "package_declaration" | "namespace_declaration" => {
                Self::visit_namespace(state, node);
            }
            "class_declaration" => Self::visit_class(state, node),
            "interface_declaration" => Self::visit_interface(state, node),
            "function_declaration" => Self::visit_function(state, node),
            "method_declaration" => Self::visit_method_signature(state, node),
            "variable_declaration" | "constant_declaration" => Self::visit_field(state, node),
            "import_statement" => Self::visit_import(state, node),
            // Transparent containers: descend to reach declarations within.
            "program" | "statement" | "statement_block" | "declaration" => {
                Self::visit_children(state, node);
            }
            _ => {}
        }
    }

    /// AS3 `package name { ... }` or `namespace`. Recurse into the body so the
    /// contained declarations are picked up, scoped by the package name.
    fn visit_namespace(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = node
            .child_by_field_name("name")
            .map_or_else(String::new, |n| state.node_text(n));
        let attrs_start = Self::attrs_start_line(state, node);
        let qn = if name.is_empty() {
            state.member_qualified_name("<anonymous>")
        } else {
            state.member_qualified_name(&name)
        };
        let display = if name.is_empty() { "<package>" } else { &name };
        let id = state.add_node(
            NodeKind::Namespace,
            display,
            qn.clone(),
            node,
            attrs_start,
            Some(format!("package {name}")),
            Self::extract_docstring(state, node),
            Visibility::Pub,
            Default::default(),
        );
        let Some(id) = id else { return };

        state.scope_stack.push(Scope {
            kind: ScopeKind::Namespace,
            short: name.clone(),
            qual: if name.is_empty() {
                state.current_scope().qual.clone()
            } else {
                qn
            },
            id,
        });
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.scope_stack.pop();
    }

    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let dotted = state.node_text(name_node); // e.g. "com.example.app.Account"
        let short = dotted.rsplit('.').next().unwrap_or(&dotted).to_string();
        let qualified_name = if dotted.contains('.') {
            dotted.clone()
        } else {
            state.member_qualified_name(&dotted)
        };
        let attrs_start = Self::attrs_start_line(state, node);
        let visibility = Self::visibility_of(state, node);
        let signature = Some(Self::first_line(state, node));

        let id = state.add_node(
            NodeKind::Class,
            &short,
            qualified_name.clone(),
            node,
            attrs_start,
            signature,
            Self::extract_docstring(state, node),
            visibility,
            Default::default(),
        );
        let Some(id) = id else { return };

        // Inheritance edges (resolved cross-file later via UnresolvedRef).
        // The `superclass`/`interfaces` fields are attached to BOTH the
        // `extends`/`implements` keyword and the type node(s), so walk children
        // by field name and keep only the type nodes.
        for ty in Self::types_in_field(state, node, "superclass") {
            Self::push_ref(state, &id, &ty, EdgeKind::Extends, node);
        }
        for ty in Self::types_in_field(state, node, "interfaces") {
            Self::push_ref(state, &id, &ty, EdgeKind::Implements, node);
        }

        state.scope_stack.push(Scope {
            kind: ScopeKind::Class,
            short,
            qual: qualified_name,
            id,
        });
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        }
        state.scope_stack.pop();
    }

    fn visit_interface(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let dotted = state.node_text(name_node);
        let short = dotted.rsplit('.').next().unwrap_or(&dotted).to_string();
        let qualified_name = if dotted.contains('.') {
            dotted.clone()
        } else {
            state.member_qualified_name(&dotted)
        };
        let attrs_start = Self::attrs_start_line(state, node);
        let id = state.add_node(
            NodeKind::Interface,
            &short,
            qualified_name.clone(),
            node,
            attrs_start,
            Some(Self::first_line(state, node)),
            Self::extract_docstring(state, node),
            Self::visibility_of(state, node),
            Default::default(),
        );
        let Some(id) = id else { return };

        for ty in Self::types_in_field(state, node, "supertype") {
            Self::push_ref(state, &id, &ty, EdgeKind::Extends, node);
        }

        state.scope_stack.push(Scope {
            kind: ScopeKind::Interface,
            short,
            qual: qualified_name,
            id,
        });
        if let Some(body) = node.child_by_field_name("body") {
            Self::visit_children(state, body);
        } else {
            Self::visit_children(state, node);
        }
        state.scope_stack.pop();
    }

    /// `function foo(...)` — a method when inside a class/interface, otherwise a
    /// free function. A function named like its enclosing class is a constructor.
    fn visit_function(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::function_name(state, node);
        if name.is_empty() {
            return;
        }

        let in_type = matches!(
            state.current_scope().kind,
            ScopeKind::Class | ScopeKind::Interface
        );
        let kind = if state.enclosing_class_short() == Some(name.as_str()) {
            NodeKind::Constructor
        } else if in_type {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let metrics = node
            .child_by_field_name("body")
            .map(|b| count_complexity(b, &ACTIONSCRIPT_COMPLEXITY, &state.source))
            .unwrap_or_default();
        let attrs_start = Self::attrs_start_line(state, node);
        let qn = state.member_qualified_name(&name);
        let id = state.add_node(
            kind,
            &name,
            qn,
            node,
            attrs_start,
            Some(Self::signature(state, node)),
            Self::extract_docstring(state, node),
            Self::visibility_of(state, node),
            metrics,
        );
        let Some(id) = id else { return };

        // return type -> TypeOf
        if let Some(rt) = node.child_by_field_name("return_type") {
            let ty = Self::type_hint_text(state, rt);
            if !ty.is_empty() {
                Self::push_ref(state, &id, &ty, EdgeKind::TypeOf, rt);
            }
        }
        // call sites in the body
        if let Some(body) = node.child_by_field_name("body") {
            Self::extract_call_sites(state, body, &id);
        }
    }

    /// Bodyless `method_declaration` (interface member signature).
    fn visit_method_signature(state: &mut ExtractionState, node: TsNode<'_>) {
        let name = Self::function_name(state, node);
        if name.is_empty() {
            return;
        }
        let attrs_start = Self::attrs_start_line(state, node);
        let qn = state.member_qualified_name(&name);
        let id = state.add_node(
            NodeKind::AbstractMethod,
            &name,
            qn,
            node,
            attrs_start,
            Some(Self::signature(state, node)),
            Self::extract_docstring(state, node),
            Visibility::Pub,
            Default::default(),
        );
        let Some(id) = id else { return };
        if let Some(rt) = node.child_by_field_name("return_type") {
            let ty = Self::type_hint_text(state, rt);
            if !ty.is_empty() {
                Self::push_ref(state, &id, &ty, EdgeKind::TypeOf, rt);
            }
        }
    }

    /// `var`/`const` — a Field inside a class/interface, otherwise a Const
    /// (file/package-level constant).
    fn visit_field(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = state.node_text(name_node);
        if name.is_empty() {
            return;
        }
        let in_type = matches!(
            state.current_scope().kind,
            ScopeKind::Class | ScopeKind::Interface
        );
        let kind = if node.kind() == "constant_declaration" || !in_type {
            NodeKind::Const
        } else {
            NodeKind::Field
        };
        let attrs_start = Self::attrs_start_line(state, node);
        let qn = state.member_qualified_name(&name);
        let id = state.add_node(
            kind,
            &name,
            qn,
            node,
            attrs_start,
            Some(Self::first_line(state, node)),
            Self::extract_docstring(state, node),
            Self::visibility_of(state, node),
            Default::default(),
        );
        let Some(id) = id else { return };
        if let Some(ty_node) = node.child_by_field_name("type") {
            let ty = Self::type_hint_text(state, ty_node);
            if !ty.is_empty() {
                Self::push_ref(state, &id, &ty, EdgeKind::TypeOf, ty_node);
            }
        }
    }

    /// `import a.b.C;` -> a Use node + a Uses edge from the enclosing scope.
    fn visit_import(state: &mut ExtractionState, node: TsNode<'_>) {
        // The imported path is the concatenation of the statement's named
        // children (identifier / scoped_data_type).
        let mut path = String::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let c = cursor.node();
                if matches!(
                    c.kind(),
                    "identifier" | "scoped_data_type" | "generic_data_type" | "any_type"
                ) {
                    path = state.node_text(c);
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        if path.is_empty() {
            return;
        }
        let short = path.rsplit('.').next().unwrap_or(&path).to_string();
        let attrs_start = node.start_position().row as u32;
        let id = state.add_node(
            NodeKind::Use,
            &short,
            format!("{}::{}", state.file_path, path),
            node,
            attrs_start,
            Some(format!("import {path}")),
            None,
            Visibility::Private,
            Default::default(),
        );
        if let (Some(id), Some(parent)) = (id, state.parent_node_id().map(str::to_string)) {
            let _ = id;
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: parent,
                reference_name: path,
                reference_kind: EdgeKind::Uses,
                line: node.start_position().row as u32,
                column: node.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
        }
    }

    // ----------------------------
    // Helpers
    // ----------------------------

    /// Record an unresolved cross-file reference (extends/implements/typeof/calls).
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

    /// Function/method name. Handles `get`/`set` accessors by prefixing them.
    fn function_name(state: &ExtractionState, node: TsNode<'_>) -> String {
        let Some(name_node) = node.child_by_field_name("name") else {
            return String::new();
        };
        let base = state.node_text(name_node);
        // An accessor child (`get`/`set`) sits alongside the name.
        if let Some(acc) = Self::find_child_by_kind(node, "accessor") {
            let kw = state.node_text(acc);
            if kw == "get" || kw == "set" {
                return format!("{kw} {base}");
            }
        }
        base
    }

    /// Collect type identifiers assigned to a named field (`superclass`,
    /// `interfaces`, `supertype`) of `parent`, keeping only type nodes.
    ///
    /// The grammar attaches these fields to both the `extends`/`implements`
    /// keyword token and each type node, so we filter by `is_type_kind`. This
    /// also yields every entry for comma-separated interface lists.
    fn types_in_field(state: &ExtractionState, parent: TsNode<'_>, field: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cursor = parent.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.field_name() == Some(field) && Self::is_type_kind(cursor.node().kind()) {
                    out.push(state.node_text(cursor.node()));
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        out
    }

    fn is_type_kind(kind: &str) -> bool {
        matches!(
            kind,
            "identifier" | "scoped_data_type" | "generic_data_type" | "any_type"
        )
    }

    /// Text of a `type_hint` (`:Number`) with the leading colon removed.
    fn type_hint_text(state: &ExtractionState, node: TsNode<'_>) -> String {
        let raw = state.node_text(node);
        raw.trim_start_matches(':').trim().to_string()
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

    /// Function signature: everything up to (but excluding) the body.
    fn signature(state: &ExtractionState, node: TsNode<'_>) -> String {
        if let Some(body) = node.child_by_field_name("body") {
            let start = node.start_byte();
            let end = body.start_byte().min(state.source.len());
            if end > start {
                if let Ok(s) = std::str::from_utf8(&state.source[start..end]) {
                    return s.trim().trim_end_matches('{').trim().to_string();
                }
            }
        }
        Self::first_line(state, node)
    }

    /// Visibility from a `class_attribut` / `property_attribut` child.
    fn visibility_of(state: &ExtractionState, node: TsNode<'_>) -> Visibility {
        for kind in ["class_attribut", "property_attribut", "interface_attribut"] {
            if let Some(attr) = Self::find_child_by_kind(node, kind) {
                let t = state.node_text(attr);
                // AS `protected` is part of the inheritance API → treat as public;
                // only `private` narrows visibility.
                if t.contains("private") {
                    return Visibility::Private;
                }
            }
        }
        Visibility::Pub
    }

    /// Start line including a leading line/block comment block, so refactoring
    /// tools can select the doc + the declaration together.
    fn attrs_start_line(state: &ExtractionState, node: TsNode<'_>) -> u32 {
        let mut start = node.start_position().row as u32;
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            if matches!(p.kind(), "line_comment" | "block_comment") {
                start = p.start_position().row as u32;
                prev = p.prev_named_sibling();
            } else {
                break;
            }
        }
        let _ = state;
        start
    }

    /// Leading `//` or `/* */` comments immediately preceding a declaration.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(p) = prev {
            match p.kind() {
                "line_comment" => {
                    let t = state.node_text(p);
                    comments.push(t.trim_start_matches('/').trim().to_string());
                    prev = p.prev_named_sibling();
                }
                "block_comment" => {
                    let t = state.node_text(p);
                    let cleaned = t
                        .trim_start_matches("/*")
                        .trim_end_matches("*/")
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .collect::<Vec<_>>()
                        .join("\n");
                    comments.push(cleaned.trim().to_string());
                    prev = p.prev_named_sibling();
                }
                _ => break,
            }
        }
        if comments.is_empty() {
            return None;
        }
        comments.reverse();
        Some(comments.join("\n"))
    }

    /// Walk a function body, recording `call_expression` / `new_expression`
    /// callees as unresolved `Calls` references. Nested functions are skipped.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call_expression" | "new_expression" => {
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
                        Self::extract_call_sites(state, child, fn_id);
                    }
                    // Don't descend into nested function definitions.
                    "function_declaration" | "anonymous_function" => {}
                    _ => Self::extract_call_sites(state, child, fn_id),
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// The callee identifier of a call/new expression. For `a.b.c()` returns the
    /// trailing member (`c`); for `foo()` returns `foo`.
    fn callee_name(state: &ExtractionState, call: TsNode<'_>) -> Option<String> {
        let func = call
            .child_by_field_name("function")
            .or_else(|| call.child_by_field_name("constructor"))
            .or_else(|| call.named_child(0))?;
        let text = match func.kind() {
            "member_expression" => func
                .child_by_field_name("property")
                .map_or_else(|| state.node_text(func), |p| state.node_text(p)),
            _ => state.node_text(func),
        };
        let text = text.trim();
        if text.is_empty() {
            None
        } else {
            // For dotted callees captured whole, keep the trailing segment.
            Some(text.rsplit('.').next().unwrap_or(text).to_string())
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

impl crate::extraction::LanguageExtractor for ActionScriptExtractor {
    fn extensions(&self) -> &[&str] {
        &["as"]
    }

    fn language_name(&self) -> &'static str {
        "ActionScript"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_actionscript(file_path, source)
    }
}
