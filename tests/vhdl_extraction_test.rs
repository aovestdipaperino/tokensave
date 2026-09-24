//! VHDL structural extraction and design hierarchy.
//!
//! Mirrors the Verilog feature test for #344. Scope is structural — entities,
//! architectures, packages, subprograms, labelled processes, generics,
//! constants, types, and `use` clauses. Signals, variables, and ports are not
//! indexed.
//!
//! VHDL adds two constraints Verilog does not have. The language is
//! case-insensitive, so an entity declared as `Child` must be found by an
//! instantiation that writes `child`. And an architecture is a design unit
//! separate from its entity, so the hierarchy edge starts at the architecture
//! and the architecture needs its own link back to the entity.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

/// A package, a leaf entity, and a top entity that instantiates the leaf both
/// ways VHDL allows, split across files so cross-file resolution is exercised.
async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("rtl")).unwrap();

    fs::write(
        project.join("rtl/my_pkg.vhd"),
        "library ieee;\n\
         use ieee.std_logic_1164.all;\n\
         \n\
         package my_pkg is\n\
        \x20 constant WIDTH : integer := 8;\n\
        \x20 type state_t is (IDLE, RUN);\n\
        \x20 subtype byte_t is std_logic_vector(7 downto 0);\n\
        \x20 function add(a, b : integer) return integer;\n\
        \x20 procedure reset_all(signal x : out std_logic);\n\
         end package my_pkg;\n\
         \n\
         package body my_pkg is\n\
        \x20 function add(a, b : integer) return integer is\n\
        \x20 begin\n\
        \x20   return a + b;\n\
        \x20 end function add;\n\
        \x20 procedure reset_all(signal x : out std_logic) is\n\
        \x20 begin\n\
        \x20   x <= '0';\n\
        \x20 end procedure;\n\
         end package body my_pkg;\n",
    )
    .unwrap();

    // Mixed case on purpose: `Child` here, `child` at the instantiation.
    fs::write(
        project.join("rtl/child.vhdl"),
        "library ieee;\n\
         use ieee.std_logic_1164.all;\n\
         \n\
         entity Child is\n\
        \x20 generic (N : integer := 4);\n\
        \x20 port (clk : in std_logic; q : out std_logic);\n\
         end entity Child;\n\
         \n\
         architecture rtl of Child is\n\
        \x20 signal cnt : integer;\n\
         begin\n\
        \x20 process (clk)\n\
        \x20 begin\n\
        \x20   if rising_edge(clk) then cnt <= cnt + 1; end if;\n\
        \x20 end process;\n\
         end architecture rtl;\n",
    )
    .unwrap();

    // A package reached through a user library rather than `work`.
    fs::write(
        project.join("rtl/loot_pkg.vhd"),
        "package loot_pkg is\n\
        \x20 constant K : integer := 1;\n\
         end package loot_pkg;\n",
    )
    .unwrap();

    // Component instantiation, direct entity instantiation, an unresolvable
    // vendor cell, and a labelled process.
    fs::write(
        project.join("rtl/top.vhd"),
        "library ieee;\n\
         library loot;\n\
         use ieee.std_logic_1164.all;\n\
         use work.my_pkg.all;\n\
         use loot.loot_pkg.all;\n\
         \n\
         entity top is\n\
        \x20 port (clk : in std_logic);\n\
         end top;\n\
         \n\
         architecture struct of top is\n\
        \x20 component child\n\
        \x20   generic (N : integer := 4);\n\
        \x20   port (clk : in std_logic; q : out std_logic);\n\
        \x20 end component;\n\
        \x20 signal q1, q2 : std_logic;\n\
         begin\n\
        \x20 u1 : child port map (clk => clk, q => q1);\n\
        \x20 u2 : entity work.child generic map (N => 2) port map (clk => clk, q => q2);\n\
        \x20 u3 : VENDOR_PAD port map (pad => clk);\n\
        \x20 main_proc : process (clk)\n\
        \x20 begin\n\
        \x20   reset_all(q1);\n\
        \x20 end process main_proc;\n\
         end struct;\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn node_names(cg: &TokenSave, kind: &NodeKind) -> Vec<String> {
    let mut names: Vec<String> = cg
        .get_all_nodes()
        .await
        .unwrap()
        .into_iter()
        .filter(|n| n.kind == *kind)
        .map(|n| n.name)
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn vhdl_files_are_indexed() {
    let (_dir, cg) = fixture().await;
    let mut paths: Vec<String> = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    paths.sort();

    for expected in ["rtl/my_pkg.vhd", "rtl/child.vhdl", "rtl/top.vhd"] {
        assert!(
            paths.contains(&expected.to_string()),
            "{expected} must be indexed, got {paths:?}"
        );
    }
}

#[tokio::test]
async fn entities_are_searchable() {
    let (_dir, cg) = fixture().await;
    let hits = cg.search("child", 10).await.unwrap();
    assert!(
        hits.iter().any(|h| h.node.name == "child"),
        "entity must be searchable, got {:?}",
        hits.iter().map(|h| &h.node.name).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn the_structural_construct_kinds_are_extracted() {
    let (_dir, cg) = fixture().await;

    let modules = node_names(&cg, &NodeKind::Module).await;
    assert!(
        modules.contains(&"child".to_string()),
        "an entity declared as `Child` must be indexed lower-case: {modules:?}"
    );
    assert!(modules.contains(&"top".to_string()), "{modules:?}");
    assert!(
        modules.contains(&"child(rtl)".to_string()),
        "an architecture must be named entity(arch): {modules:?}"
    );
    assert!(modules.contains(&"top(struct)".to_string()), "{modules:?}");

    // The declaration is the one `Package`; the body is an `Impl` of it, so
    // a `use` clause has exactly one package to bind to.
    let packages = node_names(&cg, &NodeKind::Package).await;
    assert_eq!(
        packages.iter().filter(|p| *p == "my_pkg").count(),
        1,
        "exactly one package node for the declaration: {packages:?}"
    );
    let impls = node_names(&cg, &NodeKind::Impl).await;
    assert!(
        impls.contains(&"my_pkg".to_string()),
        "the package body must be an impl of the package: {impls:?}"
    );

    let typedefs = node_names(&cg, &NodeKind::Typedef).await;
    assert!(
        typedefs.contains(&"state_t".to_string()),
        "a type must be named after itself, not an enum member: {typedefs:?}"
    );
    assert!(typedefs.contains(&"byte_t".to_string()), "{typedefs:?}");
}

#[tokio::test]
async fn generics_and_constants_are_extracted() {
    let (_dir, cg) = fixture().await;
    let consts = node_names(&cg, &NodeKind::Const).await;
    assert!(
        consts.contains(&"width".to_string()),
        "a package constant, even one the grammar tags as a library type: {consts:?}"
    );
    assert!(
        consts.contains(&"n".to_string()),
        "an entity generic: {consts:?}"
    );
}

#[tokio::test]
async fn subprograms_and_labelled_processes_are_extracted() {
    let (_dir, cg) = fixture().await;
    let functions = node_names(&cg, &NodeKind::Function).await;
    assert!(
        functions.iter().filter(|f| *f == "add").count() == 1,
        "the body is indexed once; the header declaration is not a second node: {functions:?}"
    );
    assert!(
        functions.contains(&"main_proc".to_string()),
        "a labelled process: {functions:?}"
    );

    let procedures = node_names(&cg, &NodeKind::Procedure).await;
    assert_eq!(procedures, vec!["reset_all".to_string()]);
}

#[tokio::test]
async fn an_architecture_implements_its_entity() {
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let arch = nodes.iter().find(|n| n.name == "child(rtl)").unwrap();
    let entity = nodes
        .iter()
        .find(|n| n.name == "child" && n.kind == NodeKind::Module)
        .unwrap();

    let edges = cg.get_all_edges().await.unwrap();
    assert!(
        edges.iter().any(|e| e.kind == EdgeKind::Implements
            && e.source == arch.id
            && e.target == entity.id),
        "child(rtl) must implement child across files"
    );
}

#[tokio::test]
async fn both_instantiation_forms_resolve_across_files_and_case() {
    // `u1 : child` names the component; `u2 : entity work.child` names the
    // entity directly. Both are hierarchy, both point at the entity, and the
    // entity was declared as `Child`.
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let arch = nodes.iter().find(|n| n.name == "top(struct)").unwrap();
    let child = nodes
        .iter()
        .find(|n| n.name == "child" && n.kind == NodeKind::Module)
        .unwrap();

    let edges = cg.get_all_edges().await.unwrap();
    let hierarchy: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Instantiates && e.source == arch.id && e.target == child.id)
        .collect();
    assert_eq!(
        hierarchy.len(),
        2,
        "top(struct) must instantiate child twice, got {hierarchy:?}"
    );
}

#[tokio::test]
async fn an_unresolved_instance_name_produces_no_edge() {
    let (_dir, cg) = fixture().await;
    let edges = cg.get_all_edges().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();

    for edge in edges.iter().filter(|e| e.kind == EdgeKind::Instantiates) {
        let target = nodes.iter().find(|n| n.id == edge.target);
        assert!(
            target.is_some_and(|n| n.kind == NodeKind::Module),
            "an instantiates edge must point at an entity, got {target:?}"
        );
    }
    assert!(
        !nodes.iter().any(|n| n.name == "vendor_pad"),
        "an uninstantiated vendor cell must not be invented as a node"
    );
}

#[tokio::test]
async fn a_component_declaration_is_not_a_node() {
    // The component re-states child's interface inside top. Indexing it would
    // give `u1 : child` a local target that shadows the real entity.
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let children: Vec<_> = nodes.iter().filter(|n| n.name == "child").collect();
    assert_eq!(children.len(), 1, "{children:?}");
    assert_eq!(children[0].file_path, "rtl/child.vhdl");
}

#[tokio::test]
async fn signals_and_ports_are_not_indexed() {
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    for name in ["cnt", "q1", "q2", "clk", "q"] {
        assert!(
            !nodes.iter().any(|n| n.name == name),
            "signal or port `{name}` must stay out of the graph"
        );
    }
}

#[tokio::test]
async fn a_use_clause_resolves_to_the_package() {
    let (_dir, cg) = fixture().await;
    let uses = node_names(&cg, &NodeKind::Use).await;
    assert!(
        uses.contains(&"my_pkg".to_string()),
        "`use work.my_pkg.all` must be recorded, got {uses:?}"
    );
    assert!(
        uses.contains(&"std_logic_1164".to_string()),
        "an IEEE package must be recorded even though it is not indexed: {uses:?}"
    );

    let nodes = cg.get_all_nodes().await.unwrap();
    let use_node = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Use && n.name == "my_pkg" && n.file_path == "rtl/top.vhd")
        .unwrap();
    let edges = cg.get_all_edges().await.unwrap();
    let targets: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses && e.source == use_node.id)
        .map(|e| nodes.iter().find(|n| n.id == e.target).unwrap())
        .collect();
    assert!(
        targets
            .iter()
            .any(|n| n.kind == NodeKind::Package && n.name == "my_pkg"),
        "the use clause must resolve to the package, got {targets:?}"
    );
}

/// `use loot.cc_lut_pkg.all` in a real design produced a Use node named
/// `loot`: a user library is an `identifier` to the grammar, so the first
/// name leaf was the library, and the package never resolved.
#[tokio::test]
async fn a_use_clause_through_a_user_library_names_the_package() {
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let top_uses: Vec<&str> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use && n.file_path == "rtl/top.vhd")
        .map(|n| n.name.as_str())
        .collect();
    assert!(
        top_uses.contains(&"loot_pkg") && !top_uses.contains(&"loot"),
        "the package, not the library, is what a use clause names: {top_uses:?}"
    );

    let use_node = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Use && n.name == "loot_pkg" && n.file_path == "rtl/top.vhd")
        .unwrap();
    let edges = cg.get_all_edges().await.unwrap();
    let resolved = edges.iter().any(|e| {
        e.kind == EdgeKind::Uses
            && e.source == use_node.id
            && nodes
                .iter()
                .any(|n| n.id == e.target && n.kind == NodeKind::Package && n.name == "loot_pkg")
    });
    assert!(resolved, "the use clause must resolve to package loot_pkg");
}

/// Instantiation is the hierarchy of a design, and `callers`/`callees` are
/// how that hierarchy is read: the callers of an entity are the
/// architectures that instantiate it, tagged with the `instantiates` edge so
/// a reader can tell them from a procedure call.
#[tokio::test]
async fn callers_and_callees_follow_instantiation() {
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let child = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Module && n.name == "child")
        .unwrap();
    let top = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Module && n.name == "top(struct)")
        .unwrap();

    let callers = cg.get_callers(&child.id, 1).await.unwrap();
    assert!(
        callers
            .iter()
            .any(|(n, e)| n.id == top.id && e.kind == EdgeKind::Instantiates),
        "top(struct) instantiates child, so it is a caller of child: {:?}",
        callers
            .iter()
            .map(|(n, e)| (&n.name, &e.kind))
            .collect::<Vec<_>>()
    );

    let callees = cg.get_callees(&top.id, 1).await.unwrap();
    assert!(
        callees
            .iter()
            .any(|(n, e)| n.id == child.id && e.kind == EdgeKind::Instantiates),
        "child is a callee of top(struct): {:?}",
        callees
            .iter()
            .map(|(n, e)| (&n.name, &e.kind))
            .collect::<Vec<_>>()
    );
}

/// `tokensave_rank` accepts `instantiates`, so the most-instantiated entity
/// in a design can be found the way the most-implemented interface can.
#[tokio::test]
async fn rank_accepts_the_instantiates_edge_kind() {
    let defs = tokensave::mcp::tools::get_tool_definitions();
    let rank = defs.iter().find(|d| d.name == "tokensave_rank").unwrap();
    let allowed = rank.input_schema["properties"]["edge_kind"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert!(
        allowed.contains(&"instantiates"),
        "the schema must offer instantiates: {allowed:?}"
    );

    let (_dir, cg) = fixture().await;
    let result = handle_tool_call(
        &cg,
        "tokensave_rank",
        serde_json::json!({"edge_kind": "instantiates", "direction": "incoming"}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = result.value["content"][0]["text"].as_str().unwrap();
    let ranking: serde_json::Value = serde_json::from_str(text).unwrap();
    let names: Vec<&str> = ranking["ranking"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(
        names.contains(&"child"),
        "child must be ranked as an instantiated entity: {names:?}"
    );
}
