//! Module-level import dependencies, cycles, and cut simulation.
//!
//! Feature test for #334. `tokensave_circular` answers dependency questions at
//! the *file* level using `calls`/`uses` edges, which is the wrong unit and the
//! wrong edge for planning a decomposition: one `from mod_b import X` produces
//! zero `calls` edges or fifty depending on how the name is used, so counting
//! calls says nothing about how many statements a refactor must touch.
//!
//! The three questions this has to answer are the reporter's: which modules are
//! mutually reachable, how many import statements hold a given pair together,
//! and whether cutting one dependency would break a cycle or leave everything
//! still mutually reachable — the last being the one that decides whether a
//! proposed cut is worth making at all.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

/// Two packages in a genuine import cycle, plus a third that only depends
/// inward, so a cut can be shown to break one cycle and not another.
async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    for pkg in ["core", "plugins", "cli"] {
        fs::create_dir_all(project.join(pkg)).unwrap();
    }

    // core -> plugins, from two separate statements, one of them lazy.
    fs::write(
        project.join("core/engine.py"),
        "from plugins.registry import lookup_plugin\n\
         \n\
         def run(name):\n\
        \x20   return lookup_plugin(name)\n",
    )
    .unwrap();
    fs::write(
        project.join("core/loader.py"),
        "def load(name):\n\
        \x20   from plugins.registry import lookup_plugin\n\
        \x20   return lookup_plugin(name)\n",
    )
    .unwrap();

    // plugins -> core, closing the cycle.
    fs::write(
        project.join("plugins/registry.py"),
        "from core.settings import DEFAULTS\n\
         \n\
         def lookup_plugin(name):\n\
        \x20   return DEFAULTS.get(name)\n",
    )
    .unwrap();
    fs::write(project.join("core/settings.py"), "DEFAULTS = {}\n").unwrap();

    // cli depends on core but nothing depends on cli.
    fs::write(
        project.join("cli/main.py"),
        "from core.engine import run\n\
         \n\
         def main():\n\
        \x20   return run(\"x\")\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

#[tokio::test]
async fn a_module_cycle_is_reported() {
    let (_dir, cg) = fixture().await;
    let cycles = cg.build_module_import_graph(1).await.unwrap().cycles();

    assert_eq!(
        cycles.len(),
        1,
        "expected exactly one cycle, got {cycles:?}"
    );
    assert_eq!(
        cycles[0],
        vec!["core".to_string(), "plugins".to_string()],
        "the mutually-reachable pair must be the cycle"
    );
}

#[tokio::test]
async fn a_module_that_only_depends_inward_is_not_in_a_cycle() {
    // `cli` imports `core` but nothing imports `cli`. Reporting it would make
    // the cycle look bigger than it is and mislead a decomposition plan.
    let (_dir, cg) = fixture().await;
    let cycles = cg.build_module_import_graph(1).await.unwrap().cycles();
    assert!(
        !cycles.iter().any(|c| c.contains(&"cli".to_string())),
        "cli is not mutually reachable, got {cycles:?}"
    );
}

#[tokio::test]
async fn every_import_statement_holding_a_dependency_is_enumerated() {
    // This is the count a refactor actually has to pay: two separate `import`
    // statements create core -> plugins, and the number of `calls` edges says
    // nothing about that.
    let (_dir, cg) = fixture().await;
    let graph = cg.build_module_import_graph(1).await.unwrap();
    let dep = graph
        .dependencies()
        .into_iter()
        .find(|d| d.from == "core" && d.to == "plugins")
        .expect("core -> plugins must be a dependency");

    assert_eq!(
        dep.sites.len(),
        2,
        "both import statements must be listed, got {:?}",
        dep.sites
    );
    let files: Vec<&str> = dep.sites.iter().map(|s| s.file.as_str()).collect();
    assert!(files.contains(&"core/engine.py"));
    assert!(files.contains(&"core/loader.py"));
    assert!(
        dep.sites.iter().all(|s| s.line > 0),
        "each site needs the statement's line, got {:?}",
        dep.sites
    );
}

#[tokio::test]
async fn a_function_body_import_is_flagged_as_lazy() {
    // A lazy import is usually there *because* of a cycle and costs far less to
    // remove than a module-level one, so the two must not read alike.
    let (_dir, cg) = fixture().await;
    let graph = cg.build_module_import_graph(1).await.unwrap();
    let dep = graph
        .dependencies()
        .into_iter()
        .find(|d| d.from == "core" && d.to == "plugins")
        .unwrap();

    let lazy = dep
        .sites
        .iter()
        .find(|s| s.file == "core/loader.py")
        .expect("loader site must be present");
    assert!(lazy.lazy, "an import inside a function body is lazy");

    let eager = dep
        .sites
        .iter()
        .find(|s| s.file == "core/engine.py")
        .expect("engine site must be present");
    assert!(!eager.lazy, "a module-level import is not lazy");
}

#[tokio::test]
async fn cutting_a_dependency_that_closes_the_cycle_breaks_it() {
    // The decisive question: does this cut buy anything?
    let (_dir, cg) = fixture().await;
    let graph = cg.build_module_import_graph(1).await.unwrap();
    assert_eq!(graph.cycles().len(), 1);

    let after = graph.cycles_without("plugins", "core");
    assert!(
        after.is_empty(),
        "removing the back-edge must break the cycle, got {after:?}"
    );
}

#[tokio::test]
async fn cutting_an_unrelated_dependency_changes_nothing() {
    // A cut that leaves every module still mutually reachable is worthless, and
    // nothing short of recomputing the components can tell it from a good one.
    let (_dir, cg) = fixture().await;
    let graph = cg.build_module_import_graph(1).await.unwrap();

    let after = graph.cycles_without("cli", "core");
    assert_eq!(
        after,
        graph.cycles(),
        "cutting a dependency outside the cycle must leave it intact"
    );
}

#[tokio::test]
async fn grouping_depth_changes_the_unit_of_analysis() {
    // At depth 2 the packages split into their subdirectories, so the same
    // codebase has a different — and equally valid — module decomposition.
    let (_dir, cg) = fixture().await;
    let deep = cg.build_module_import_graph(2).await.unwrap();
    let names: Vec<String> = deep
        .dependencies()
        .into_iter()
        .map(|d| format!("{}->{}", d.from, d.to))
        .collect();
    assert!(
        !names.is_empty(),
        "a deeper grouping must still produce dependencies"
    );
}

#[tokio::test]
async fn imports_within_one_module_are_not_dependencies() {
    // `core/engine.py` importing `core/settings.py` is internal cohesion, not
    // a dependency between modules; counting it would make every package
    // depend on itself.
    let (_dir, cg) = fixture().await;
    let graph = cg.build_module_import_graph(1).await.unwrap();
    assert!(
        !graph.dependencies().iter().any(|d| d.from == d.to),
        "a module must never depend on itself"
    );
}
