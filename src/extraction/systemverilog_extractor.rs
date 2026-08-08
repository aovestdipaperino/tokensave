/// Tree-sitter based Verilog / `SystemVerilog` source code extractor.
///
/// Handles `.v`, `.vh`, `.sv`, and `.svh` with a single `SystemVerilog` grammar,
/// since `SystemVerilog` is a superset of Verilog and a `.v` file parses under it
/// unchanged.
///
/// Scope is deliberately structural (#344): modules, interfaces, programs,
/// packages, classes, functions, tasks, parameters, and typedefs. Internal nets
/// and variables are *not* indexed — an RTL design declares them by the
/// thousand, and they would swamp the graph without answering the questions the
/// hierarchy is consulted for.
///
/// The relationship that matters in HDL is the design hierarchy, which is
/// emitted as [`EdgeKind::Instantiates`] rather than folded into `Calls`. A
/// module instantiation is structure, not invocation: treating it as a call
/// would put RTL hierarchy into callers/callees, impact, and dead-code results
/// for every other language.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::ts_state::{find_child_by_kind, ExtractionState};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from Verilog/SystemVerilog sources.
pub struct SystemVerilogExtractor;

impl SystemVerilogExtractor {
    pub fn extract_source(file_path: &str, source: &str) -> ExtractionResult {
        let start = Instant::now();
        let mut state = ExtractionState::new(file_path, source);

        let tree = match Self::parse_source(source) {
            Ok(tree) => tree,
            Err(msg) => {
                state.errors.push(msg);
                return state.build_result(start);
            }
        };

        let file_node_id = generate_node_id(file_path, &NodeKind::File, file_path, 0);
        state.nodes.push(Self::make_node(
            file_node_id.clone(),
            NodeKind::File,
            file_path.to_string(),
            file_path.to_string(),
            0,
            source.lines().count().saturating_sub(1) as u32,
            None,
            &state,
        ));
        state.node_stack.push((file_path.to_string(), file_node_id));

        Self::visit_children(&mut state, tree.root_node());

        state.node_stack.pop();
        state.build_result(start)
    }

    fn parse_source(source: &str) -> Result<Tree, String> {
        let mut parser = Parser::new();
        let language = crate::extraction::ts_provider::language("systemverilog");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load SystemVerilog grammar: {e}"))?;
        parser
            .parse(source, None)
            .ok_or_else(|| "tree-sitter parse returned None".to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn make_node(
        id: String,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        start_line: u32,
        end_line: u32,
        signature: Option<String>,
        state: &ExtractionState,
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
            start_column: 0,
            end_column: 0,
            signature,
            docstring: None,
            // HDL has no access modifiers at the level this indexes; a module or
            // package item is visible to anything that elaborates it.
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

    fn visit_node(state: &mut ExtractionState, node: TsNode<'_>) {
        match node.kind() {
            "module_declaration" | "program_declaration" => {
                Self::visit_scope(state, node, NodeKind::Module);
            }
            // A SystemVerilog interface is a bundle of signals and modports, and
            // is instantiated exactly like a module — so it is the same shape of
            // graph node, not a Java-style contract.
            "interface_declaration" => {
                Self::visit_scope(state, node, NodeKind::Interface);
            }
            "package_declaration" => {
                Self::visit_scope(state, node, NodeKind::Package);
            }
            "class_declaration" => {
                Self::visit_class(state, node);
            }
            "function_declaration" => {
                Self::visit_subroutine(state, node, "function_body_declaration");
            }
            "task_declaration" => {
                Self::visit_subroutine(state, node, "task_body_declaration");
            }
            "type_declaration" => {
                Self::visit_typedef(state, node);
            }
            "module_instantiation" => {
                Self::visit_instantiation(state, node);
            }
            "package_import_declaration" => Self::visit_package_import(state, node),
            "parameter_declaration" | "local_parameter_declaration" => {
                Self::visit_params(state, node);
            }
            _ => Self::visit_children(state, node),
        }
    }

    /// The declared name of a construct: the first identifier under its header.
    ///
    /// Both the ANSI (`module m #(...) (...)`) and non-ANSI (`module m;`) header
    /// forms put the name first, so the first identifier in the declaration
    /// subtree is the name in either case.
    fn declared_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        Self::first_identifier(state, node)
    }

    fn first_identifier(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if node.kind() == "simple_identifier" {
            return Some(state.node_text(node));
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if let Some(found) = Self::first_identifier(state, cursor.node()) {
                    return Some(found);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// The first line of a construct, for use as its signature.
    fn header_line(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        state
            .node_text(node)
            .lines()
            .next()
            .map(|line| line.trim_end().to_string())
    }

    /// Pushes a named scope node and visits its body inside it.
    fn visit_scope(state: &mut ExtractionState, node: TsNode<'_>, kind: NodeKind) -> Option<()> {
        let name = Self::declared_name(state, node)?;
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        state.nodes.push(Self::make_node(
            id.clone(),
            kind,
            name.clone(),
            qualified_name,
            start_line,
            node.end_position().row as u32,
            Self::header_line(state, node),
            state,
        ));
        Self::contain(state, &id, start_line);

        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    /// A class, plus its `extends` base as an `Extends` reference.
    fn visit_class(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let name = Self::declared_name(state, node)?;
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Class, &name, start_line);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        state.nodes.push(Self::make_node(
            id.clone(),
            NodeKind::Class,
            name.clone(),
            qualified_name,
            start_line,
            node.end_position().row as u32,
            Self::header_line(state, node),
            state,
        ));
        Self::contain(state, &id, start_line);

        // `class_type` is present only when the class extends something, so its
        // absence is the discriminator rather than a keyword search.
        if let Some(base) = find_child_by_kind(node, "class_type") {
            if let Some(base_name) = Self::first_identifier(state, base) {
                state.unresolved_refs.push(UnresolvedRef {
                    from_node_id: id.clone(),
                    reference_name: base_name,
                    reference_kind: EdgeKind::Extends,
                    line: base.start_position().row as u32,
                    column: base.start_position().column as u32,
                    file_path: state.file_path.clone(),
                });
            }
        }

        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    /// A `function` or `task`. The name lives in the body declaration, after the
    /// return type, so the wrapper node's first identifier would be the type.
    fn visit_subroutine(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        body_kind: &str,
    ) -> Option<()> {
        let body = find_child_by_kind(node, body_kind)?;
        let name = Self::first_identifier(state, body)?;
        let start_line = node.start_position().row as u32;
        // A subroutine inside a class is a method; at package or module scope it
        // is an ordinary function, which is what `dead_code` and `callers` expect.
        let kind = if state.node_stack.len() > 1 {
            NodeKind::Method
        } else {
            NodeKind::Function
        };
        let id = generate_node_id(&state.file_path, &kind, &name, start_line);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        state.nodes.push(Self::make_node(
            id.clone(),
            kind,
            name.clone(),
            qualified_name,
            start_line,
            node.end_position().row as u32,
            Self::header_line(state, node),
            state,
        ));
        Self::contain(state, &id, start_line);

        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    fn visit_typedef(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        // The typedef's *name* is the last identifier — `typedef enum {A, B}
        // state_t;` puts the enum members first, so taking the first identifier
        // would name the type after one of its own values.
        let name = Self::last_identifier(state, node)?;
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &NodeKind::Typedef, &name, start_line);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);

        state.nodes.push(Self::make_node(
            id.clone(),
            NodeKind::Typedef,
            name,
            qualified_name,
            start_line,
            node.end_position().row as u32,
            Self::header_line(state, node),
            state,
        ));
        Self::contain(state, &id, start_line);
        Some(())
    }

    fn last_identifier(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        let mut found = None;
        Self::collect_last_identifier(state, node, &mut found);
        found
    }

    fn collect_last_identifier(
        state: &ExtractionState,
        node: TsNode<'_>,
        out: &mut Option<String>,
    ) {
        if node.kind() == "simple_identifier" {
            *out = Some(state.node_text(node));
            return;
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_last_identifier(state, cursor.node(), out);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// `parameter` / `localparam` declarations, one node per assignment.
    fn visit_params(state: &mut ExtractionState, node: TsNode<'_>) {
        let Some(list) = find_child_by_kind(node, "list_of_param_assignments") else {
            return;
        };
        let mut cursor = list.walk();
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            let child = cursor.node();
            if child.kind() == "param_assignment" {
                if let Some(name) = Self::first_identifier(state, child) {
                    let start_line = child.start_position().row as u32;
                    let id =
                        generate_node_id(&state.file_path, &NodeKind::Const, &name, start_line);
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
                    state.nodes.push(Self::make_node(
                        id.clone(),
                        NodeKind::Const,
                        name,
                        qualified_name,
                        start_line,
                        child.end_position().row as u32,
                        Some(state.node_text(child).trim().to_string()),
                        state,
                    ));
                    Self::contain(state, &id, start_line);
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// A module or interface instantiation: the design hierarchy edge.
    ///
    /// The grammar produces `module_instantiation` for interfaces too, so this
    /// one path covers both. Only the instantiated *type* becomes a reference —
    /// the instance name is a label within the parent, not a symbol anything
    /// else can refer to.
    ///
    /// The reference is emitted unresolved, so an instantiation of a cell from a
    /// vendor library that is not in the index simply produces no edge, rather
    /// than binding to whatever happens to share its name (#344's requirement
    /// that unresolved instance names never become valid edges).
    fn visit_instantiation(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let type_name = Self::first_identifier(state, node)?;
        let from_node_id = state.parent_node_id()?.to_string();
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id,
            reference_name: type_name,
            reference_kind: EdgeKind::Instantiates,
            line: node.start_position().row as u32,
            column: node.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
        Some(())
    }

    /// `import pkg::*;` / `import pkg::item;` — a Use node, as for every other
    /// language, so `tokensave_imports` and unused-import analysis see it.
    fn visit_package_import(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return;
        }
        loop {
            let child = cursor.node();
            if child.kind() == "package_import_item" {
                if let Some(pkg) = Self::first_identifier(state, child) {
                    let start_line = child.start_position().row as u32;
                    let id = generate_node_id(&state.file_path, &NodeKind::Use, &pkg, start_line);
                    let qualified_name = format!("{}::{}", state.qualified_prefix(), pkg);
                    state.nodes.push(Self::make_node(
                        id.clone(),
                        NodeKind::Use,
                        pkg.clone(),
                        qualified_name,
                        start_line,
                        child.end_position().row as u32,
                        Some(state.node_text(node).trim().to_string()),
                        state,
                    ));
                    Self::contain(state, &id, start_line);
                    state.unresolved_refs.push(UnresolvedRef {
                        from_node_id: id,
                        reference_name: pkg,
                        reference_kind: EdgeKind::Uses,
                        line: start_line,
                        column: child.start_position().column as u32,
                        file_path: state.file_path.clone(),
                    });
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Emits the `Contains` edge from the enclosing scope.
    fn contain(state: &mut ExtractionState, id: &str, line: u32) {
        if let Some(parent_id) = state.parent_node_id() {
            let parent_id = parent_id.to_string();
            state.edges.push(Edge {
                source: parent_id,
                target: id.to_string(),
                kind: EdgeKind::Contains,
                line: Some(line),
            });
        }
    }
}

impl crate::extraction::LanguageExtractor for SystemVerilogExtractor {
    fn extensions(&self) -> &[&str] {
        &["v", "vh", "sv", "svh"]
    }

    fn language_name(&self) -> &'static str {
        "SystemVerilog"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        SystemVerilogExtractor::extract_source(file_path, source)
    }
}
