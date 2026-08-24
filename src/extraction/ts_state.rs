//! Shared traversal state and tree-sitter helpers for language extractors.
//!
//! Most tree-sitter based extractors need the same bookkeeping while walking
//! an AST: accumulators for nodes/edges, a stack of enclosing scopes for
//! qualified names, and small node-search utilities. Extractors with extra
//! per-language state (e.g. C++ access specifiers) keep their own state
//! structs; everything else shares this one.

use std::collections::HashSet;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tree_sitter::Node as TsNode;

use crate::types::{Edge, ExtractionResult, Node, UnresolvedRef, Visibility};

/// What a Python class body declares. The Python extractor keeps one per
/// enclosing class so a `self.<name>` read in a value position can be told
/// apart: a method reference when `name` is a method the class defines and
/// never binds as an attribute, a field read otherwise. Other extractors
/// leave the stack empty.
#[derive(Default)]
pub(crate) struct PythonClassAttrs {
    /// Methods defined directly in the class body.
    pub(crate) methods: HashSet<String>,
    /// Names bound as attributes: `self.<name> = ...` or `cls.<name> = ...`
    /// in the class's own methods, or `<name> = ...` in the class body.
    pub(crate) assigned: HashSet<String>,
    /// Methods that are descriptors (`@property`, `@cached_property`,
    /// `@<name>.setter` and friends). An assignment to one invokes it, so
    /// it does not shadow the method.
    pub(crate) descriptors: HashSet<String>,
}

/// Internal state used during AST traversal.
pub(crate) struct ExtractionState {
    pub(crate) nodes: Vec<Node>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) unresolved_refs: Vec<UnresolvedRef>,
    pub(crate) errors: Vec<String>,
    /// Stack of (name, `node_id`) for building qualified names and parent edges.
    pub(crate) node_stack: Vec<(String, String)>,
    pub(crate) file_path: String,
    pub(crate) source: Vec<u8>,
    pub(crate) timestamp: u64,
    /// Nesting depth of enclosing class-like scopes (used by extractors that
    /// treat top-level and member functions differently; others leave it 0).
    pub(crate) class_depth: usize,
    /// One entry per enclosing Python class, innermost last. See
    /// [`PythonClassAttrs`]. Other extractors leave it empty.
    pub(crate) python_class_attrs: Vec<PythonClassAttrs>,
    /// Current Ruby visibility mode inside a class/module body (private/protected/
    /// public switches). Other extractors leave it at the default Pub.
    pub(crate) visibility_mode: Visibility,
    /// Whether a Ruby `module_function` mode switch is currently active: the
    /// next `def`s in this module body become a private instance method
    /// *and* a public singleton method of the same name. A strict companion
    /// of `visibility_mode` rather than independent state — Ruby treats
    /// `public`/`private`/`protected`/`module_function` as four values of
    /// one default-definition-mode frame, each cancelling the previous
    /// (confirmed against Ruby 3.4.7), so this flag is saved/reset/restored
    /// at exactly the same sites as `visibility_mode`, and setting either
    /// one clears the other. Other extractors leave it `false`.
    pub(crate) module_function_mode: bool,
    /// Node IDs of Ruby singleton methods that belong to the enclosing class
    /// (`def self.foo`, `def obj.foo` where `obj` resolves to `self`/the
    /// enclosing constant), so retroactive visibility (`private_class_method
    /// :foo` vs `private :foo`) can tell a singleton from a same-named
    /// instance method — they share a kind and qualified name. Other
    /// extractors leave it empty.
    pub(crate) singleton_method_ids: Vec<String>,
    /// Node IDs of Ruby singleton methods whose receiver is *not* the
    /// enclosing class (`def obj.foo`, or anything defined inside
    /// `class << some_other_object`). These belong to neither the instance
    /// nor the class-method bucket, so visibility directives must skip them
    /// rather than let them fall into the instance-method branch by default.
    /// Other extractors leave it empty.
    pub(crate) foreign_singleton_method_ids: Vec<String>,
    /// Which Ruby singleton scope the traversal is currently inside. Other
    /// extractors leave it at `Outside`.
    pub(crate) singleton_scope: SingletonScope,
    /// Whether the traversal is currently inside a Ruby module body that has
    /// evidence of being an `ActiveSupport::Concern` (an `extend
    /// ActiveSupport::Concern` seen so far in this body, or a receiverless
    /// `concern`/`concerning` block, which Rails builds pre-extended). Gates
    /// the `included`/`prepended`/`class_methods` DSL classification in
    /// `classify_block_scope` — those names raise `NoMethodError` without
    /// Concern, so without this evidence they're ordinary calls. Scoped by
    /// what `self` denotes in the current body: it survives into a `def
    /// self.x` singleton-method body (where `self` is still the module), but
    /// not into a plain `def x` or `class << self` body (where `self` is the
    /// instance or the singleton class instead). Other extractors leave it
    /// `false`.
    pub(crate) in_concern_scope: bool,
    /// Whether `self` in the body currently being traversed is an *instance* the
    /// extractor cannot name, rather than the enclosing class/module. True inside a
    /// plain `def foo` body (and a `def foo` inside `class << some_object`); false
    /// in class/module bodies, `def self.foo` bodies, and `class << …` bodies,
    /// where `self` is a module. Other extractors leave it `false`.
    pub(crate) self_is_instance: bool,
    /// The class/module node that owns direct Ruby body calls while the
    /// traversal is outside a method or self-retargeting block. Other
    /// extractors leave it `None`.
    pub(crate) ruby_body_call_owner_id: Option<String>,
    /// Whether the traversal is currently inside a Concern `included`/
    /// `prepended`/`class_methods` block, where `self` at runtime is the
    /// includer — a receiver the extractor cannot resolve statically, and
    /// whose actual type (`Class` vs `Module`) determines whether
    /// `module_function` even raises (confirmed against Ruby 3.4.7 and
    /// activesupport 8.1.3.1: `included do; module_function; def a; end;
    /// end` raises `NameError` for a `Class` includer, but silently
    /// succeeds — on the includer, not the concern module itself — for a
    /// `Module` includer). `classify_block_scope`/`visit_block_body`
    /// attribute a plain `def` inside these blocks to the concern module as
    /// a deliberate, already-accepted approximation (the includer's actual
    /// identity is unknowable), but that approximation does not extend to
    /// `module_function`: its private-instance-plus-public-singleton effect
    /// depends on which concrete receiver it runs against, not just on
    /// "some includer exists". So `visit_module_function_directive` treats
    /// this flag as blocking evidence rather than trying to model it. Other
    /// extractors leave it `false`.
    pub(crate) in_concern_self_retargeting_block: bool,
}

/// Which Ruby singleton scope the traversal is currently inside. `class << expr`
/// reopens `expr`'s singleton class, so a plain `def foo` there defines a method
/// on `expr`, not an instance method of the enclosing class. Other extractors
/// leave this at `Outside`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingletonScope {
    /// Not inside any `class << …` body.
    Outside,
    /// Inside `class << self` (or `class << EnclosingName`) — defs are class
    /// methods of the enclosing class.
    Enclosing,
    /// Inside `class << some_other_object` — defs belong to an object we cannot
    /// resolve, so they are not members of the enclosing class.
    Foreign,
}

impl ExtractionState {
    pub(crate) fn new(file_path: &str, source: &str) -> Self {
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
            python_class_attrs: Vec::new(),
            visibility_mode: Visibility::Pub,
            module_function_mode: false,
            singleton_method_ids: Vec::new(),
            foreign_singleton_method_ids: Vec::new(),
            singleton_scope: SingletonScope::Outside,
            in_concern_scope: false,
            self_is_instance: false,
            ruby_body_call_owner_id: None,
            in_concern_self_retargeting_block: false,
        }
    }

    /// Returns the current qualified name prefix from the node stack.
    pub(crate) fn qualified_prefix(&self) -> String {
        let mut parts = vec![self.file_path.clone()];
        for (name, _) in &self.node_stack {
            parts.push(name.clone());
        }
        parts.join("::")
    }

    /// Returns the current parent node ID, or None if at file root level.
    pub(crate) fn parent_node_id(&self) -> Option<&str> {
        self.node_stack.last().map(|(_, id)| id.as_str())
    }

    /// Gets the text of a tree-sitter node from the source.
    pub(crate) fn node_text(&self, node: TsNode<'_>) -> String {
        node.utf8_text(&self.source)
            .unwrap_or("<invalid utf8>")
            .to_string()
    }

    /// Consumes the state into an `ExtractionResult`, stamping the duration.
    pub(crate) fn build_result(self, start: Instant) -> ExtractionResult {
        ExtractionResult {
            nodes: self.nodes,
            edges: self.edges,
            unresolved_refs: self.unresolved_refs,
            errors: self.errors,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

/// Find the first direct child of a node with a given kind.
pub(crate) fn find_child_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
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

/// Find the first descendant of a node with a given kind (recursive DFS).
pub(crate) fn find_descendant_by_kind<'a>(node: TsNode<'a>, kind: &str) -> Option<TsNode<'a>> {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if current.kind() == kind {
            return Some(current);
        }
        // Push children via cursor (O(N) per node) and reverse so the
        // first child pops first. Previous revision used `current.child(i)`
        // in a `for i in (0..N).rev()` loop, which is O(N²) per node
        // because `child(i)` walks sibling links from index 0.
        let start = stack.len();
        let mut cursor = current.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        stack[start..].reverse();
    }
    None
}

/// Returns true if the node has a direct child of the given kind.
pub(crate) fn has_child_kind(node: TsNode<'_>, kind: &str) -> bool {
    find_child_by_kind(node, kind).is_some()
}
