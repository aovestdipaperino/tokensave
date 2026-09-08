/// Tree-sitter based VHDL source code extractor.
///
/// Handles `.vhd` and `.vhdl`. Scope is deliberately structural, as for
/// Verilog (#344): entities, architectures, packages, package bodies,
/// subprograms, labelled processes, generics, constants, types, and `use`
/// clauses. Signals, variables, and ports are *not* indexed — an RTL design
/// declares them by the thousand, and they would swamp the graph without
/// answering the questions the hierarchy is consulted for.
///
/// VHDL is case-insensitive: `Child`, `child`, and `CHILD` name one design
/// unit. Every name this extractor emits is lower-cased so that an
/// instantiation resolves to its entity whatever case each file used.
///
/// The design hierarchy is emitted as [`EdgeKind::Instantiates`], from the
/// architecture that contains the instantiation to the instantiated entity.
/// An architecture is a separate design unit from its entity, possibly in a
/// separate file, so it becomes its own `Module` node, named `entity(arch)`
/// as VHDL itself writes it, with an `Implements` reference to the entity.
use std::time::Instant;

use tree_sitter::{Node as TsNode, Parser, Tree};

use crate::extraction::ts_state::{find_child_by_kind, ExtractionState};
use crate::types::{
    generate_node_id, Edge, EdgeKind, ExtractionResult, Node, NodeKind, UnresolvedRef, Visibility,
};

/// Extracts code graph nodes and edges from VHDL sources.
pub struct VhdlExtractor;

impl VhdlExtractor {
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
        let language = crate::extraction::ts_provider::language("vhdl");
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to load VHDL grammar: {e}"))?;
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
            // A VHDL design unit is visible to anything that elaborates it.
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
            "entity_declaration" => {
                Self::visit_scope(state, node, NodeKind::Module);
            }
            "architecture_definition" => {
                Self::visit_architecture(state, node);
            }
            "package_declaration" => {
                Self::visit_scope(state, node, NodeKind::Package);
            }
            // `package body my_pkg is ... end;` is the implementation of a
            // package declared elsewhere, the way a Rust `impl` block is the
            // implementation of a type. It gets its own scope so the
            // subprograms defined in it have a parent, but not a second
            // `Package` node with the same name: two identical candidates
            // would tie, and a tied bare-name reference resolves to nothing,
            // which left every `use work.my_pkg.all` without an edge.
            "package_definition" => {
                Self::visit_scope(state, node, NodeKind::Impl);
            }
            "subprogram_definition" => {
                Self::visit_subprogram(state, node);
            }
            "process_statement" => {
                Self::visit_process(state, node);
            }
            "type_declaration" | "subtype_declaration" => {
                Self::visit_typedef(state, node);
            }
            "constant_declaration" | "generic_clause" => {
                Self::visit_constants(state, node);
            }
            "component_instantiation_statement" => {
                Self::visit_instantiation(state, node);
            }
            "use_clause" => Self::visit_use_clause(state, node),
            // A component declaration re-states an entity's interface locally
            // so that it can be instantiated by name. It is not a design unit,
            // and indexing it would give every instantiation a second, local
            // target that shadows the real entity.
            "component_declaration" => {}
            _ => Self::visit_children(state, node),
        }
    }

    /// True for a leaf that names something the user wrote.
    ///
    /// The grammar labels an identifier by what it thinks it is: `identifier`
    /// for an unknown word, `label` after a colon, and `library_function`,
    /// `library_type`, and so on for words that match an IEEE library symbol.
    /// A user constant called `WIDTH` therefore parses as `library_type`, and a
    /// function called `add` as `library_function`. `library_namespace` is
    /// excluded: it is the `work` or `ieee` prefix, never the name itself.
    fn is_name_leaf(node: TsNode<'_>) -> bool {
        let kind = node.kind();
        kind == "identifier"
            || kind == "label"
            || (kind.starts_with("library_") && kind != "library_namespace")
    }

    /// The first name leaf under `node`, lower-cased. VHDL is
    /// case-insensitive, so this is the canonical form.
    fn first_name(state: &ExtractionState, node: TsNode<'_>) -> Option<String> {
        if Self::is_name_leaf(node) {
            return Some(state.node_text(node).to_lowercase());
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if let Some(found) = Self::first_name(state, cursor.node()) {
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

    /// Pushes a node for a named construct, with its `Contains` edge, and
    /// returns its id.
    fn push_named(
        state: &mut ExtractionState,
        node: TsNode<'_>,
        kind: NodeKind,
        name: &str,
        signature: Option<String>,
    ) -> String {
        let start_line = node.start_position().row as u32;
        let id = generate_node_id(&state.file_path, &kind, name, start_line);
        let qualified_name = format!("{}::{}", state.qualified_prefix(), name);
        state.nodes.push(Self::make_node(
            id.clone(),
            kind,
            name.to_string(),
            qualified_name,
            start_line,
            node.end_position().row as u32,
            signature,
            state,
        ));
        Self::contain(state, &id, start_line);
        id
    }

    /// Pushes a named scope node and visits its body inside it.
    fn visit_scope(state: &mut ExtractionState, node: TsNode<'_>, kind: NodeKind) -> Option<()> {
        let name = Self::first_name(state, node)?;
        let id = Self::push_named(state, node, kind, &name, Self::header_line(state, node));
        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    /// `architecture rtl of child is ... end;` — a `Module` named `child(rtl)`
    /// with an `Implements` reference to entity `child`.
    ///
    /// The entity is a separate design unit, often in a separate file, so the
    /// architecture cannot simply be nested under it. The combined name keeps
    /// an architecture searchable by either half, and tells `rtl` of `child`
    /// apart from `rtl` of every other entity in the design.
    fn visit_architecture(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let arch = Self::first_name(state, node)?;
        let entity = Self::first_name(state, find_child_by_kind(node, "name")?)?;
        let name = format!("{entity}({arch})");
        let id = Self::push_named(
            state,
            node,
            NodeKind::Module,
            &name,
            Self::header_line(state, node),
        );
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id: id.clone(),
            reference_name: entity,
            reference_kind: EdgeKind::Implements,
            line: node.start_position().row as u32,
            column: node.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    /// A function or procedure body. Bare declarations (`function f return
    /// integer;` in a package header) are not indexed: the body carries the
    /// same name and would make every subprogram appear twice.
    fn visit_subprogram(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let (spec, kind) = if let Some(spec) = find_child_by_kind(node, "function_specification") {
            (spec, NodeKind::Function)
        } else {
            (
                find_child_by_kind(node, "procedure_specification")?,
                NodeKind::Procedure,
            )
        };
        let name = Self::first_name(state, spec)?;
        let signature = Some(state.node_text(spec).trim().to_string());
        let id = Self::push_named(state, node, kind, &name, signature);
        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    /// A labelled process. An unlabelled one has no name anything can refer
    /// to, so it is walked for nested declarations but gets no node.
    fn visit_process(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let Some(label) = find_child_by_kind(node, "label_declaration") else {
            Self::visit_children(state, node);
            return None;
        };
        let name = Self::first_name(state, label)?;
        let id = Self::push_named(
            state,
            node,
            NodeKind::Function,
            &name,
            Self::header_line(state, node),
        );
        state.node_stack.push((name, id));
        Self::visit_children(state, node);
        state.node_stack.pop();
        Some(())
    }

    fn visit_typedef(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let name = Self::first_name(state, node)?;
        Self::push_named(
            state,
            node,
            NodeKind::Typedef,
            &name,
            Self::header_line(state, node),
        );
        Some(())
    }

    /// One `Const` node per name in each `identifier_list` under `node`: a
    /// `constant a, b : integer := 0;` or a `generic (N, M : natural);`.
    fn visit_constants(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut lists = Vec::new();
        Self::collect_kind(node, "identifier_list", &mut lists);
        for list in lists {
            let decl = list.parent().unwrap_or(list);
            let signature = Some(state.node_text(decl).trim().to_string());
            let mut cursor = list.walk();
            if !cursor.goto_first_child() {
                continue;
            }
            loop {
                let leaf = cursor.node();
                if Self::is_name_leaf(leaf) {
                    let name = state.node_text(leaf).to_lowercase();
                    Self::push_named(state, decl, NodeKind::Const, &name, signature.clone());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn collect_kind<'a>(node: TsNode<'a>, kind: &str, out: &mut Vec<TsNode<'a>>) {
        if node.kind() == kind {
            out.push(node);
            return;
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                Self::collect_kind(cursor.node(), kind, out);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// `u1 : child port map (...)` or `u1 : entity work.child port map (...)`:
    /// the design hierarchy edge, from the enclosing architecture to the
    /// instantiated entity.
    ///
    /// Only the instantiated *unit* becomes a reference; the label is a name
    /// within the parent that nothing else can refer to. The reference is
    /// emitted unresolved, so an instantiation of a vendor cell that is not in
    /// the index produces no edge rather than binding to whatever shares its
    /// name. The `work.` library prefix is skipped by [`Self::is_name_leaf`].
    fn visit_instantiation(state: &mut ExtractionState, node: TsNode<'_>) -> Option<()> {
        let unit = find_child_by_kind(node, "instantiated_unit")
            .or_else(|| find_child_by_kind(node, "name"))?;
        let unit_name = Self::first_name(state, unit)?;
        let from_node_id = state.parent_node_id()?.to_string();
        state.unresolved_refs.push(UnresolvedRef {
            from_node_id,
            reference_name: unit_name,
            reference_kind: EdgeKind::Instantiates,
            line: node.start_position().row as u32,
            column: node.start_position().column as u32,
            file_path: state.file_path.clone(),
        });
        Some(())
    }

    /// `use work.my_pkg.all;` — a Use node per selected name, so
    /// `tokensave_imports` and unused-import analysis see it, plus a `Uses`
    /// reference to the package.
    ///
    /// The grammar labels the segments of a selected name `library` and
    /// `package`. The package is what the clause imports: `use
    /// loot.cc_lut_pkg.all` names `cc_lut_pkg`, and a user library such as
    /// `loot` is an `identifier` like any other, so the first name leaf
    /// would be the library. A two-segment `use my_pkg.all` has no
    /// `package` field; its `library` field is the package.
    fn visit_use_clause(state: &mut ExtractionState, node: TsNode<'_>) {
        let mut selected = Vec::new();
        Self::collect_kind(node, "selected_name", &mut selected);
        for sel in selected {
            let Some(pkg) = sel
                .child_by_field_name("package")
                .or_else(|| sel.child_by_field_name("library"))
                .and_then(|n| Self::first_name(state, n))
            else {
                continue;
            };
            let signature = Some(state.node_text(node).trim().to_string());
            let id = Self::push_named(state, sel, NodeKind::Use, &pkg, signature);
            state.unresolved_refs.push(UnresolvedRef {
                from_node_id: id,
                reference_name: pkg,
                reference_kind: EdgeKind::Uses,
                line: sel.start_position().row as u32,
                column: sel.start_position().column as u32,
                file_path: state.file_path.clone(),
            });
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

impl crate::extraction::LanguageExtractor for VhdlExtractor {
    fn extensions(&self) -> &[&str] {
        &["vhd", "vhdl"]
    }

    fn language_name(&self) -> &'static str {
        "VHDL"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        VhdlExtractor::extract_source(file_path, source)
    }
}
