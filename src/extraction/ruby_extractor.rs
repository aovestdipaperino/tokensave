/// Tree-sitter based Ruby source code extractor.
///
/// Parses Ruby source files and emits nodes and edges for the code graph.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::complexity::{count_complexity, RUBY_COMPLEXITY};
use crate::extraction::ts_state::{find_child_by_kind, ExtractionState, SingletonScope};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Ruby source files using tree-sitter.
pub struct RubyExtractor;

/// Which same-named node a Ruby visibility directive should retroactively
/// mark. Instance methods, the enclosing class's singleton methods, and
/// methods on an unresolvable receiver all share `NodeKind::Method` and
/// `qualified_name`, so the id lists in `ExtractionState` are what tell them
/// apart.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VisibilityTarget {
    /// The instance method, or a file-scope `Function`.
    Instance,
    /// A singleton method of the enclosing class (`def self.foo`, or `def
    /// foo` inside `class << self`).
    EnclosingSingleton,
    /// A method defined on an unresolvable receiver — inside `class <<
    /// other`, or `def self.foo` inside a `class << …` body.
    Foreign,
}

impl RubyExtractor {
    /// Extract code graph nodes and edges from a Ruby source file.
    ///
    /// `file_path` is used for qualified names and node IDs (not for I/O).
    /// `source` is the Ruby source code to parse.
    pub fn extract_ruby(file_path: &str, source: &str) -> ExtractionResult {
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
        Self::visit_children(&mut state, root);

        state.node_stack.pop();

        state.build_result(start)
    }

    /// Parse source code into a tree-sitter AST.
    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("ruby");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load Ruby grammar: {e}"))?;
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
            "method" => Self::visit_method(state, node),
            "singleton_method" => Self::visit_singleton_method(state, node),
            "singleton_class" => Self::visit_singleton_class(state, node),
            "class" => Self::visit_class(state, node),
            "module" => Self::visit_module(state, node),
            "assignment" => Self::visit_assignment_for_const(state, node),
            // Bare `private`/`protected`/`public` mode switches parse as a plain
            // identifier statement; defensively also handle a no-arg call.
            // A receiverless include/prepend/extend is a mixin directive; the two
            // handlers are gated on disjoint method names, so both run.
            "identifier" | "call" | "method_call" => {
                Self::visit_visibility_directive(state, node);
                Self::visit_mixin_directive(state, node);
            }
            // Statement containers: they wrap statements without opening a new
            // definition scope, so the enclosing class/module stays the parent and
            // a mixin/definition nested in one is extracted as if written directly
            // in the body (`include Foo if enabled?`, `if RUBY_VERSION > "3" … end`,
            // `begin … rescue LoadError … end`). Method bodies are NOT reached from
            // here — visit_method routes them through extract_call_sites instead.
            "if" | "unless" | "if_modifier" | "unless_modifier" | "then" | "else" | "elsif"
            | "case" | "when" | "case_match" | "in_clause" | "while" | "until"
            | "while_modifier" | "until_modifier" | "do" | "begin" | "rescue" | "ensure"
            | "rescue_modifier" | "body_statement" | "do_block" | "block" => {
                Self::visit_children(state, node);
            }
            _ => {}
        }
    }

    /// Extract a regular method definition (`def method_name`).
    ///
    /// Inside a `class << self` body (`state.singleton_scope`), this is a
    /// class (singleton) method regardless of `class_depth` — same as
    /// `def self.foo` — and gets registered in `singleton_method_ids` (or
    /// `foreign_singleton_method_ids` if the receiver wasn't the enclosing
    /// class) so retroactive `private_class_method :foo` can find it.
    fn visit_method(state: &mut ExtractionState, node: TsNode<'_>) {
        // tree-sitter-ruby's `method` node exposes a `name` field typed `_method_name`,
        // which covers plain identifiers as well as operator defs (`def []=`, `def <=>`)
        // and setter defs (`def name=`) — neither of which is an `identifier` node kind.
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let in_class = state.class_depth > 0 || state.singleton_scope != SingletonScope::Outside;
        let kind = if in_class {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let visibility = state.visibility_mode.clone();
        let signature = Self::extract_method_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &RUBY_COMPLEXITY, &state.source);

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
        match state.singleton_scope {
            SingletonScope::Enclosing => state.singleton_method_ids.push(id.clone()),
            SingletonScope::Foreign => state.foreign_singleton_method_ids.push(id.clone()),
            SingletonScope::Outside => {}
        }

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the method body.
        Self::extract_call_sites(state, node, &id);
    }

    /// True if a singleton receiver denotes the enclosing class/module: a literal
    /// `self`, or the constant naming the scope we are currently inside
    /// (`class Report; def Report.generate`, equivalent to `def self.generate`).
    ///
    /// A literal `self` only means "the enclosing class" outside a `class << …`
    /// body: inside one, `self` *is* the singleton class, so `def self.foo`
    /// there defines a method one level further out (`Report.singleton_class`,
    /// not `Report`). Constant lookup is unaffected by singleton scope, so the
    /// `"constant"` arm doesn't need the same guard.
    fn is_enclosing_receiver(state: &ExtractionState, receiver: TsNode<'_>) -> bool {
        match receiver.kind() {
            "self" => state.singleton_scope == SingletonScope::Outside,
            "constant" | "scope_resolution" => {
                state.class_depth > 0
                    && Self::matches_enclosing_scope_path(state, &state.node_text(receiver))
            }
            _ => false,
        }
    }

    /// True if `path` (the source text of a `constant` or `scope_resolution`
    /// receiver) names the same object as some suffix of the enclosing
    /// class/module scopes, at node-stack-entry granularity — never splitting
    /// an entry's own name on `::` (a compact `class Outer::Inner` pushes one
    /// entry, and a bare `Inner` must not resolve inside it, since Ruby
    /// wouldn't resolve it either).
    ///
    /// `node_stack[0]` is always the file's own root entry (pushed once for
    /// the whole traversal, never a Ruby scope), so it's excluded from both
    /// the suffix search and the absolute path below.
    ///
    /// A leading `::` anchors the match to the *whole* scope chain (absolute
    /// path): inside `module A; class B`, `::B` names the top-level `B`, not
    /// `A::B`, so it must not match via a relative suffix.
    fn matches_enclosing_scope_path(state: &ExtractionState, path: &str) -> bool {
        let scopes = &state.node_stack[1..];
        let scope_names = || scopes.iter().map(|(name, _)| name.as_str());

        if let Some(absolute) = path.strip_prefix("::") {
            return scope_names().collect::<Vec<_>>().join("::") == absolute;
        }

        (0..scopes.len())
            .any(|start| scope_names().skip(start).collect::<Vec<_>>().join("::") == path)
    }

    /// Extract a singleton method definition (`def self.method_name` or `def obj.method_name`).
    fn visit_singleton_method(state: &mut ExtractionState, node: TsNode<'_>) {
        // singleton_method has: "def", object (self or identifier), ".", identifier, parameters?, body
        // We want the method name (the identifier after ".")
        let name = Self::find_last_identifier_before_params(state, node)
            .unwrap_or_else(|| "<anonymous>".to_string());

        let kind = NodeKind::Method;
        let visibility = Visibility::Pub;
        let signature = Self::extract_singleton_method_signature(state, node);
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let metrics = count_complexity(node, &RUBY_COMPLEXITY, &state.source);

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
        if node
            .child_by_field_name("object")
            .is_some_and(|obj| Self::is_enclosing_receiver(state, obj))
        {
            state.singleton_method_ids.push(id.clone());
        } else {
            state.foreign_singleton_method_ids.push(id.clone());
        }

        // Contains edge from parent.
        if let Some(parent_id) = state.parent_node_id() {
            state.edges.push(Edge {
                source: parent_id.to_string(),
                target: id.clone(),
                kind: EdgeKind::Contains,
                line: Some(start_line),
            });
        }

        // Extract call sites from the method body.
        Self::extract_call_sites(state, node, &id);
    }

    /// Extract a `class << self` (or `class << expr`) body.
    ///
    /// This reopens the enclosing object's singleton class: `def foo` inside
    /// defines a class method exactly like `def self.foo` would. We don't push
    /// onto `node_stack` or bump `class_depth` — the qualified name and the
    /// `Contains` edge source must stay the enclosing class, so
    /// `class << self; def foo; end; end` matches `def self.foo`'s
    /// `file::Report::foo`.
    ///
    /// The block has its own visibility scope (starts public, doesn't leak
    /// either direction), mirroring `visit_class`/`visit_module`. Only
    /// `class << self` (or `class << EnclosingConstant`) marks methods as
    /// singletons of the enclosing class — `class << some_object` still has
    /// its body visited (so methods aren't dropped) but its defs are
    /// registered as foreign, since we can't resolve `some_object` and
    /// registering them as the enclosing class's singletons would let an
    /// unrelated `private_class_method` match them. The scope is assigned
    /// unconditionally (not just when it resolves to `Enclosing`) so a
    /// `class << other` nested inside `class << self` doesn't inherit the
    /// outer `Enclosing` scope.
    ///
    /// `scope` must be computed *before* `state.singleton_scope` is updated
    /// below: it judges this `class << …`'s own receiver against the scope
    /// *enclosing* it, so a `class << self` nested inside another `class <<
    /// self` sees the outer `Enclosing` scope and correctly resolves to
    /// `Foreign` (that inner `self` is the outer singleton class, not
    /// `Report`).
    fn visit_singleton_class(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let scope = match node.child_by_field_name("value") {
            Some(v) if Self::is_enclosing_receiver(state, v) => SingletonScope::Enclosing,
            _ => SingletonScope::Foreign,
        };

        let saved_visibility_mode = state.visibility_mode.clone();
        state.visibility_mode = Visibility::Pub;
        let saved_singleton_scope = state.singleton_scope;
        state.singleton_scope = scope;

        Self::visit_children(state, body);

        state.singleton_scope = saved_singleton_scope;
        state.visibility_mode = saved_visibility_mode;
    }

    /// Extract a class definition.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) {
        // The grammar types the name field as choice($.constant, $.scope_resolution),
        // so a compact `class Outer::Inner` has no direct `constant` child - only the
        // `name` field (whose text is the full path) covers both forms.
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Visibility::Pub;
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
            });
        }

        // Extract superclass (inheritance): `class Foo < Bar`
        Self::extract_superclass(state, node, &id);

        // Visit class body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        let saved_visibility_mode = state.visibility_mode.clone();
        state.visibility_mode = Visibility::Pub;
        let saved_singleton_scope = state.singleton_scope;
        state.singleton_scope = SingletonScope::Outside;
        if let Some(body) = find_child_by_kind(node, "body_statement") {
            Self::visit_children(state, body);
        }
        state.singleton_scope = saved_singleton_scope;
        state.visibility_mode = saved_visibility_mode;
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Extract a module definition.
    fn visit_module(state: &mut ExtractionState, node: TsNode<'_>) {
        // See visit_class: the name field covers both a bare constant and a
        // compact `module A::B`'s scope_resolution.
        let name = node
            .child_by_field_name("name")
            .map_or_else(|| "<anonymous>".to_string(), |n| state.node_text(n));

        let visibility = Visibility::Pub;
        let docstring = Self::extract_docstring(state, node);
        let start_line = node.start_position().row as u32;
        let end_line = node.end_position().row as u32;
        let start_column = node.start_position().column as u32;
        let end_column = node.end_position().column as u32;
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        let id = generate_node_id(&state.file_path, &NodeKind::Module, &name, start_line);

        // Build "module ModuleName" signature
        let text = state.node_text(node);
        let signature = text
            .lines()
            .next()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());

        let graph_node = Node {
            id: id.clone(),
            kind: NodeKind::Module,
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
            });
        }

        // Visit module body.
        state.node_stack.push((name.clone(), id));
        state.class_depth += 1;
        let saved_visibility_mode = state.visibility_mode.clone();
        state.visibility_mode = Visibility::Pub;
        let saved_singleton_scope = state.singleton_scope;
        state.singleton_scope = SingletonScope::Outside;
        if let Some(body) = find_child_by_kind(node, "body_statement") {
            Self::visit_children(state, body);
        }
        state.singleton_scope = saved_singleton_scope;
        state.visibility_mode = saved_visibility_mode;
        state.class_depth -= 1;
        state.node_stack.pop();
    }

    /// Check if an assignment is a Ruby constant (starts with uppercase) and extract it.
    ///
    /// Ruby constants are identifiers that start with an uppercase letter.
    fn visit_assignment_for_const(state: &mut ExtractionState, node: TsNode<'_>) {
        // In tree-sitter-ruby, assignment has left and right children.
        // Constants are represented as "constant" kind nodes on the left side.
        let left = node.child_by_field_name("left");
        if let Some(left_node) = left {
            if left_node.kind() == "constant" {
                let name = state.node_text(left_node);
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
                    });
                }
            }
        }
    }

    /// Resolve a Ruby visibility modifier name to a `Visibility`, if the name
    /// is one of `public`/`private`/`protected`. Ruby has no `protected`
    /// variant in our enum; both `private` and `protected` map to
    /// `Visibility::Private` since the distortion this fixes only requires
    /// distinguishing public API from non-public.
    fn resolve_visibility_keyword(name: &str) -> Option<Visibility> {
        match name {
            "public" => Some(Visibility::Pub),
            "private" | "protected" => Some(Visibility::Private),
            _ => None,
        }
    }

    /// Resolve a Ruby *singleton* visibility directive name to a `Visibility`.
    /// These target `def self.foo` methods by symbol rather than switching the
    /// default mode. Ruby core has no `protected_class_method`, so only these two.
    fn resolve_class_method_keyword(name: &str) -> Option<Visibility> {
        match name {
            "public_class_method" => Some(Visibility::Pub),
            "private_class_method" => Some(Visibility::Private),
            _ => None,
        }
    }

    /// Handle `private`/`protected`/`public`/`private_class_method`/
    /// `public_class_method` directives: bare mode switches (`private`),
    /// symbol-list retroactive marking (`private :foo, :bar`), and inline
    /// `def` (`private def foo; end`).
    fn visit_visibility_directive(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "identifier" => {
                let name = state.node_text(node);
                if let Some(visibility) = Self::resolve_visibility_keyword(&name) {
                    state.visibility_mode = visibility;
                }
            }
            "call" | "method_call" => {
                // Real visibility directives are receiverless. A call with an explicit
                // receiver (e.g. `policy.private`, `config.public(:run)`) is an ordinary
                // method call that merely shares a name — it must not change visibility.
                if node.child_by_field_name("receiver").is_some() {
                    return;
                }
                let Some(method_node) = node.child_by_field_name("method") else {
                    return;
                };
                let name = state.node_text(method_node);
                let class_method_visibility = Self::resolve_class_method_keyword(&name);
                let is_class_method = class_method_visibility.is_some();
                let Some(visibility) =
                    class_method_visibility.or_else(|| Self::resolve_visibility_keyword(&name))
                else {
                    return;
                };
                // Which same-named node this directive should mark. Kept as one
                // arm per (scope, is_class_method) combination, even where two
                // arms produce the same target, so each has room for its own
                // rationale below.
                #[allow(clippy::match_same_arms)]
                let target = match (state.singleton_scope, is_class_method) {
                    (SingletonScope::Outside, false) => VisibilityTarget::Instance,
                    (SingletonScope::Outside, true) => VisibilityTarget::EnclosingSingleton,
                    // Inside `class << self`, `private :foo` (not just
                    // `private_class_method`) must target the singleton method,
                    // since that's what `foo` was registered as — there is no
                    // instance-method node for it to fall back to.
                    (SingletonScope::Enclosing, false) => VisibilityTarget::EnclosingSingleton,
                    // `private_class_method` here would target `def self.foo`, one
                    // level further out (Report.singleton_class, not Report) —
                    // Ruby raises `NameError` if aimed at the plain `def foo`.
                    (SingletonScope::Enclosing, true) => VisibilityTarget::Foreign,
                    (SingletonScope::Foreign, _) => VisibilityTarget::Foreign,
                };

                let Some(args) = node.child_by_field_name("arguments") else {
                    // Bare call with no argument list (e.g. `private()`): only a
                    // mode switch makes sense here.
                    if !is_class_method {
                        state.visibility_mode = visibility;
                    }
                    return;
                };

                let mut saw_arg = false;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let arg = cursor.node();
                        match arg.kind() {
                            "simple_symbol" => {
                                saw_arg = true;
                                let symbol_name =
                                    state.node_text(arg).trim_start_matches(':').to_string();
                                Self::mark_method_visibility(
                                    state,
                                    &symbol_name,
                                    target,
                                    visibility.clone(),
                                );
                            }
                            "delimited_symbol" => {
                                // Any symbol argument counts, so the mode isn't switched
                                // below — even an interpolated one we can't resolve.
                                saw_arg = true;
                                if let Some(symbol_name) =
                                    Self::static_delimited_symbol_name(state, arg)
                                {
                                    Self::mark_method_visibility(
                                        state,
                                        &symbol_name,
                                        target,
                                        visibility.clone(),
                                    );
                                }
                            }
                            "method" => {
                                saw_arg = true;
                                let saved_visibility_mode = state.visibility_mode.clone();
                                state.visibility_mode = visibility.clone();
                                Self::visit_method(state, arg);
                                state.visibility_mode = saved_visibility_mode;
                            }
                            "singleton_method" => {
                                saw_arg = true;
                                Self::visit_singleton_method(state, arg);
                                // `visit_singleton_method` always hardcodes `Pub` (singletons
                                // sit outside the `visibility_mode` path), so a plain `private
                                // def self.foo; end` is correctly left public (a no-op in
                                // Ruby). Only `private_class_method` actually privatizes it.
                                if is_class_method {
                                    if let Some(node) = state.nodes.last_mut() {
                                        node.visibility = visibility.clone();
                                    }
                                }
                            }
                            _ => {
                                // Any other named argument (e.g. `private attr_reader :foo`,
                                // whose arg is a nested `call`) is still an argument: Ruby
                                // applies the directive to it and returns without switching
                                // the default visibility. Unnamed nodes (punctuation like
                                // `(`, `)`, `,`) don't count, so `private()` still switches
                                // the mode.
                                if arg.is_named() {
                                    saw_arg = true;
                                }
                            }
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }

                if !saw_arg && !is_class_method {
                    state.visibility_mode = visibility;
                }
            }
            _ => {}
        }
    }

    /// Retroactively mark the method named `name` defined in the *current*
    /// class/module body (the owner on top of `state.node_stack` at directive
    /// time) as having `visibility`. Matches on `qualified_name` rather than
    /// bare `name` + `file_path`, so a same-named method in an unrelated or
    /// ancestor class elsewhere in the file is left untouched — only the
    /// method actually defined in the enclosing body of this directive is
    /// affected.
    ///
    /// `target` selects which same-named node to mark: instance methods, the
    /// enclosing class's singleton methods, and methods on an unresolvable
    /// receiver all share `NodeKind::Method` and `qualified_name`, so
    /// `state.singleton_method_ids`/`foreign_singleton_method_ids` are what
    /// disambiguate them. One approximation remains: inside a `Foreign` body,
    /// `def foo` and `def self.foo` both land in `foreign_singleton_method_ids`,
    /// so a directive there can't tell them apart.
    fn mark_method_visibility(
        state: &mut ExtractionState,
        name: &str,
        target: VisibilityTarget,
        visibility: Visibility,
    ) {
        let target_qn = format!("{}::{}", state.qualified_prefix(), name);
        let singleton_ids = &state.singleton_method_ids;
        let foreign_ids = &state.foreign_singleton_method_ids;
        if let Some(node) = state.nodes.iter_mut().rev().find(|n| {
            if n.qualified_name != target_qn {
                return false;
            }
            let is_singleton = singleton_ids.contains(&n.id);
            let is_foreign = foreign_ids.contains(&n.id);
            match target {
                VisibilityTarget::Instance => {
                    !is_singleton
                        && !is_foreign
                        && (n.kind == NodeKind::Method || n.kind == NodeKind::Function)
                }
                VisibilityTarget::EnclosingSingleton => is_singleton && n.kind == NodeKind::Method,
                VisibilityTarget::Foreign => is_foreign && n.kind == NodeKind::Method,
            }
        }) {
            node.visibility = visibility;
        }
    }

    /// Extract `include`/`prepend`/`extend` of a named module as an
    /// `Implements` ref from the enclosing class/module to that module.
    ///
    /// All three keywords are receiverless calls (`mod.include Bar` is an
    /// ordinary method call on another object, not a mixin, and is skipped).
    /// Only `constant`/`scope_resolution` arguments are resolvable
    /// statically — `extend self`, `include some_variable`, and
    /// `include mod_returning_method()` name something we can't bind to a
    /// node, so they're skipped rather than fabricating a ref. A top-level
    /// `include` (outside any class/module) mixes into `Object`, and there is
    /// no class/module node to attach the ref to, so it's skipped too.
    ///
    /// `extend` (which adds singleton methods) and `include`/`prepend`
    /// (which add instance methods) all produce the same `Implements` edge
    /// kind — distinguishing them would need a new `EdgeKind` variant.
    fn visit_mixin_directive(state: &mut ExtractionState, node: TsNode<'_>) {
        if node.child_by_field_name("receiver").is_some() {
            return;
        }
        let Some(method_node) = node.child_by_field_name("method") else {
            return;
        };
        let method_name = state.node_text(method_node);
        if !matches!(method_name.as_str(), "include" | "prepend" | "extend") {
            return;
        }
        if state.class_depth == 0 {
            return;
        }
        let Some(from_node_id) = state.parent_node_id().map(str::to_string) else {
            return;
        };
        let Some(arguments) = node.child_by_field_name("arguments") else {
            return;
        };

        let mut cursor = arguments.walk();
        for arg in arguments.named_children(&mut cursor) {
            if matches!(arg.kind(), "constant" | "scope_resolution") {
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: from_node_id.clone(),
                    reference_name: state.node_text(arg),
                    reference_kind: EdgeKind::Implements,
                    line: arg.start_position().row as u32,
                    column: arg.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
            }
        }
    }

    // ----------------------------
    // Helper extraction methods
    // ----------------------------

    /// Extract the superclass from a class definition (`class Foo < Bar`).
    ///
    /// Creates an Extends `UnresolvedRef` from the class to its superclass.
    fn extract_superclass(state: &mut ExtractionState, node: TsNode<'_>, class_id: &str) {
        // In tree-sitter-ruby, the superclass is a child node with field name "superclass"
        // or a "superclass" kind node. The superclass node contains the constant name.
        if let Some(superclass_node) = node.child_by_field_name("superclass") {
            let base_name = state.node_text(superclass_node);
            // Strip any leading whitespace/symbols from the superclass name
            let base_name = base_name.trim().trim_start_matches('<').trim().to_string();
            if !base_name.is_empty() {
                let line = superclass_node.start_position().row as u32;
                let column = superclass_node.start_position().column as u32;
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: class_id.to_string(),
                    reference_name: base_name,
                    reference_kind: EdgeKind::Extends,
                    line,
                    column,
                    file_path: state.file_path.clone(),
                });
            }
        } else {
            // Try finding a superclass child node by kind
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "superclass" {
                        // The superclass node contains "< ConstantName"
                        // Find the constant child inside superclass
                        if let Some(const_node) = find_child_by_kind(child, "constant")
                            .or_else(|| find_child_by_kind(child, "scope_resolution"))
                        {
                            let base_name = state.node_text(const_node);
                            let line = const_node.start_position().row as u32;
                            let column = const_node.start_position().column as u32;
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
    }

    /// Extract the method signature (def name(params) ... end).
    ///
    /// Returns the first line of the method, which contains the signature.
    fn extract_method_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        // The signature is everything on the first line.
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract the singleton method signature (def self.name(params)).
    fn extract_singleton_method_signature(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract the class signature (class Name or class Name < Base).
    fn extract_class_signature(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let text = state.node_text(node);
        let first_line = text.lines().next()?.trim().to_string();
        if first_line.is_empty() {
            None
        } else {
            Some(first_line)
        }
    }

    /// Extract docstrings from `# comment` lines preceding definitions.
    ///
    /// Ruby uses comment lines (# ...) as documentation. We look for `comment`
    /// sibling nodes that immediately precede the given definition node.
    fn extract_docstring(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        // Look at the previous sibling nodes for consecutive comment lines.
        let mut comments: Vec<String> = Vec::new();
        let mut prev = node.prev_named_sibling();
        while let Some(prev_node) = prev {
            if prev_node.kind() == "comment" {
                let text = state.node_text(prev_node);
                let stripped = text.trim_start_matches('#').trim().to_string();
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

    /// Decode a static `delimited_symbol` (`:"foo"`, `:"[]="`) to its method
    /// name. Returns `None` for symbols we can't resolve at extraction time —
    /// anything containing an `interpolation` (`:"#{x}"`) or `escape_sequence`
    /// (method names never need escapes) — so those are skipped rather than
    /// marked on the wrong method.
    fn static_delimited_symbol_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut name = String::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "string_content" {
                name.push_str(&state.node_text(child));
            } else {
                // interpolation / escape_sequence — not statically decodable.
                return None;
            }
        }
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    /// Find the method name identifier in a singleton method.
    ///
    /// In `def self.foo(args)`, we want "foo" (the identifier after the dot).
    /// tree-sitter-ruby's `singleton_method` has: "def", object, ".", name (identifier), parameters?, body
    fn find_last_identifier_before_params(
        state: &ExtractionState,
        node: TsNode<'_>,
    ) -> Option<String> {
        // Walk children and find the last identifier before "method_parameters" or body
        let mut cursor = node.walk();
        let mut last_ident: Option<String> = None;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "identifier" => {
                        last_ident = Some(state.node_text(child));
                    }
                    "method_parameters" | "body_statement" => {
                        break;
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        last_ident
    }

    /// Recursively find call nodes inside a given node and create unresolved Calls references.
    fn extract_call_sites(state: &mut ExtractionState, node: TsNode<'_>, fn_node_id: &str) {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "call" | "method_call" => {
                        // In tree-sitter-ruby, a call node has a "method" field for the method name.
                        // For simple calls like `foo(args)`, the first named child is the method name.
                        let callee_name =
                            if let Some(method_node) = child.child_by_field_name("method") {
                                Some(state.node_text(method_node))
                            } else {
                                // Fall back to first named child
                                child.named_child(0).map(|n| state.node_text(n))
                            };

                        if let Some(name) = callee_name {
                            state.unresolved_refs.push(UnresolvedRef {
                                from_node_id: fn_node_id.to_string(),
                                reference_name: name,
                                reference_kind: EdgeKind::Calls,
                                line: child.start_position().row as u32,
                                column: child.start_position().column as u32,
                                file_path: state.file_path.clone(),
                            });
                        }
                        // Recurse into the call for nested calls.
                        Self::extract_call_sites(state, child, fn_node_id);
                    }
                    // Skip nested method/singleton_method/class/module/singleton_class
                    // definitions to avoid polluting call sites with their internal calls.
                    "method" | "singleton_method" | "class" | "module" | "singleton_class" => {}
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
}

impl crate::extraction::LanguageExtractor for RubyExtractor {
    fn extensions(&self) -> &[&str] {
        &["rb"]
    }

    fn language_name(&self) -> &'static str {
        "Ruby"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        Self::extract_ruby(file_path, source)
    }
}
