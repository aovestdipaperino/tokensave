//! Verilog / SystemVerilog structural extraction and design hierarchy.
//!
//! Feature test for #344. A project containing tracked `.v` files could
//! `init` and `sync` successfully while none of its RTL appeared in the graph:
//! the files were not listed and no module was searchable.
//!
//! Scope is deliberately structural — modules, interfaces, programs, packages,
//! classes, functions, tasks, parameters, typedefs. Internal nets and variables
//! are not indexed; an RTL design declares them by the thousand and they would
//! swamp the graph without answering the questions the hierarchy is consulted
//! for.
//!
//! The load-bearing constraint is the reporter's last acceptance criterion:
//! traversal must never treat an *unresolved* instance name as a valid edge. A
//! vendor cell that is not in the index has to produce no edge at all, rather
//! than binding to whatever else happens to share its name.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

/// The issue's minimal example, plus enough SystemVerilog to cover the rest of
/// the requested scope, split across `.v` and `.sv` so both are exercised.
async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("rtl")).unwrap();

    fs::write(
        project.join("rtl/child.v"),
        "module child (\n\
        \x20   input wire clk\n\
         );\n\
         endmodule\n",
    )
    .unwrap();

    // Cross-file instantiation, an unresolvable vendor cell, and a parameter.
    fs::write(
        project.join("rtl/top.v"),
        "module top #(parameter WIDTH = 8) (\n\
        \x20   input wire clk\n\
         );\n\
        \x20   localparam DEPTH = 4;\n\
        \x20   child u_child (.clk(clk));\n\
        \x20   VENDOR_PAD u_pad (.pad(clk));\n\
         endmodule\n",
    )
    .unwrap();

    fs::write(
        project.join("rtl/pkg.sv"),
        "package pkg_a;\n\
        \x20 typedef enum {A, B} state_t;\n\
        \x20 function automatic int add(int x, int y);\n\
        \x20   return x + y;\n\
        \x20 endfunction\n\
         endpackage\n",
    )
    .unwrap();

    fs::write(
        project.join("rtl/bus.sv"),
        "interface bus_if;\n\
        \x20 logic valid;\n\
         endinterface\n\
         \n\
         class Base;\n\
        \x20 virtual task run(); endtask\n\
         endclass\n\
         \n\
         class Derived extends Base;\n\
        \x20 task run(); endtask\n\
         endclass\n\
         \n\
         module wrapper;\n\
        \x20 import pkg_a::*;\n\
        \x20 bus_if u_bus();\n\
         endmodule\n",
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
async fn hdl_files_are_indexed() {
    // The reported symptom: init succeeded and the file was simply absent.
    let (_dir, cg) = fixture().await;
    let mut paths: Vec<String> = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    paths.sort();

    for expected in ["rtl/child.v", "rtl/top.v", "rtl/pkg.sv", "rtl/bus.sv"] {
        assert!(
            paths.contains(&expected.to_string()),
            "{expected} must be indexed, got {paths:?}"
        );
    }
}

#[tokio::test]
async fn modules_are_searchable() {
    let (_dir, cg) = fixture().await;
    let hits = cg.search("child", 10).await.unwrap();
    assert!(
        hits.iter().any(|h| h.node.name == "child"),
        "module must be searchable, got {:?}",
        hits.iter().map(|h| &h.node.name).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn the_requested_construct_kinds_are_extracted() {
    let (_dir, cg) = fixture().await;

    let modules = node_names(&cg, &NodeKind::Module).await;
    assert!(modules.contains(&"child".to_string()), "{modules:?}");
    assert!(modules.contains(&"top".to_string()), "{modules:?}");
    assert!(modules.contains(&"wrapper".to_string()), "{modules:?}");

    let interfaces = node_names(&cg, &NodeKind::Interface).await;
    assert!(interfaces.contains(&"bus_if".to_string()), "{interfaces:?}");

    let packages = node_names(&cg, &NodeKind::Package).await;
    assert!(packages.contains(&"pkg_a".to_string()), "{packages:?}");

    let classes = node_names(&cg, &NodeKind::Class).await;
    assert!(classes.contains(&"Base".to_string()), "{classes:?}");
    assert!(classes.contains(&"Derived".to_string()), "{classes:?}");

    let typedefs = node_names(&cg, &NodeKind::Typedef).await;
    assert!(
        typedefs.contains(&"state_t".to_string()),
        "a typedef must be named after the type, not one of its enum members: {typedefs:?}"
    );
}

#[tokio::test]
async fn parameters_and_localparams_are_extracted() {
    let (_dir, cg) = fixture().await;
    let consts = node_names(&cg, &NodeKind::Const).await;
    assert!(consts.contains(&"WIDTH".to_string()), "{consts:?}");
    assert!(consts.contains(&"DEPTH".to_string()), "{consts:?}");
}

#[tokio::test]
async fn functions_and_tasks_are_extracted() {
    let (_dir, cg) = fixture().await;
    let all = cg.get_all_nodes().await.unwrap();
    assert!(
        all.iter().any(|n| n.name == "add"),
        "a package function must be indexed"
    );
    assert!(
        all.iter().any(|n| n.name == "run"),
        "a class task must be indexed"
    );
}

#[tokio::test]
async fn a_cross_file_instantiation_resolves_to_its_definition() {
    // The design hierarchy is the whole point: `top` instantiates `child`,
    // defined in another file.
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let top = nodes.iter().find(|n| n.name == "top").unwrap();
    let child = nodes.iter().find(|n| n.name == "child").unwrap();

    let edges = cg.get_all_edges().await.unwrap();
    assert!(
        edges.iter().any(|e| e.kind == EdgeKind::Instantiates
            && e.source == top.id
            && e.target == child.id),
        "top must instantiate child across files"
    );
}

#[tokio::test]
async fn an_interface_instantiation_is_also_a_hierarchy_edge() {
    // The grammar parses an interface instance as a module instantiation, and
    // for hierarchy purposes it is one.
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let wrapper = nodes.iter().find(|n| n.name == "wrapper").unwrap();
    let bus = nodes.iter().find(|n| n.name == "bus_if").unwrap();

    let edges = cg.get_all_edges().await.unwrap();
    assert!(
        edges.iter().any(|e| e.kind == EdgeKind::Instantiates
            && e.source == wrapper.id
            && e.target == bus.id),
        "wrapper must instantiate bus_if"
    );
}

#[tokio::test]
async fn an_unresolved_instance_name_produces_no_edge() {
    // The reporter's explicit requirement. `VENDOR_PAD` is not in the index, so
    // it must yield nothing — binding it to any same-named symbol would
    // fabricate a hierarchy that does not exist.
    let (_dir, cg) = fixture().await;
    let edges = cg.get_all_edges().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();

    for edge in edges.iter().filter(|e| e.kind == EdgeKind::Instantiates) {
        let target = nodes.iter().find(|n| n.id == edge.target);
        assert!(
            target.is_some_and(|n| matches!(
                n.kind,
                NodeKind::Module | NodeKind::Interface | NodeKind::InterfaceType
            )),
            "an instantiates edge must point at a module or interface, got {target:?}"
        );
    }
    assert!(
        !nodes.iter().any(|n| n.name == "VENDOR_PAD"),
        "an uninstantiated vendor cell must not be invented as a node"
    );
}

#[tokio::test]
async fn class_inheritance_is_extracted() {
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let derived = nodes.iter().find(|n| n.name == "Derived").unwrap();
    let base = nodes.iter().find(|n| n.name == "Base").unwrap();

    let edges = cg.get_all_edges().await.unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e.kind == EdgeKind::Extends && e.source == derived.id && e.target == base.id),
        "Derived must extend Base"
    );
}

#[tokio::test]
async fn internal_signals_are_not_indexed() {
    // Deliberately out of scope: `logic valid` inside the interface. Indexing
    // every net would multiply graph size for no retrieval benefit.
    let (_dir, cg) = fixture().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        !nodes.iter().any(|n| n.name == "valid"),
        "internal nets must stay out of the graph"
    );
}

#[tokio::test]
async fn a_package_import_is_recorded() {
    // So the module import graph (#334) and unused-import analysis see HDL too.
    let (_dir, cg) = fixture().await;
    let uses = node_names(&cg, &NodeKind::Use).await;
    assert!(
        uses.contains(&"pkg_a".to_string()),
        "an `import pkg_a::*` must be recorded, got {uses:?}"
    );
}
