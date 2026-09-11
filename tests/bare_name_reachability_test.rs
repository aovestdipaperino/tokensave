//! #522: a bare name with one candidate is not evidence of a binding.
//!
//! #508 taught the dotted-receiver fallback (`recv.method`) that being the
//! only symbol of that name in the project proves nothing about whether the
//! call site can reach it. The **bare-name** path never learned the same
//! thing, so a production function declaring a local variable named `exe`
//! acquired a `uses` edge pointing at a pytest fixture named `exe` — in a
//! different directory, in a file importing nothing from the test tree —
//! purely because the fixture was the only `exe` anywhere.
//!
//! The direction is not incidental. Test fixtures are deliberately named after
//! the values they supply, so they collide with exactly the ordinary variable
//! names production code uses: `exe`, `cfg_path`, `node`, `payload`.
//!
//! The gate is scoped by language on measured grounds — see
//! `bare_name_needs_evidence`. Applying it to Rust cost 11.4% of every call
//! edge for no reduction in impossible edges, so the Rust tests below are as
//! load-bearing as the Python ones.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;

/// Whether any edge runs from a node in `from_file` to a node named `to_name`
/// in `to_file`.
async fn has_edge(cg: &TokenSave, from_file: &str, to_name: &str, to_file: &str) -> bool {
    let nodes = cg.db().get_all_nodes().await.expect("nodes");
    let edges = cg.db().get_all_edges().await.expect("edges");
    let by_id: std::collections::HashMap<&str, &tokensave::types::Node> =
        nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    edges.iter().any(|e| {
        let (Some(s), Some(t)) = (by_id.get(e.source.as_str()), by_id.get(e.target.as_str()))
        else {
            return false;
        };
        s.file_path == from_file && t.name == to_name && t.file_path == to_file
    })
}

/// The reported shape: a fixture in the test tree is the only symbol of its
/// name, and production code in another directory declares a local with that
/// name. No import ties them; the edge is impossible.
#[tokio::test]
async fn a_python_bare_name_does_not_bind_to_an_unreachable_sole_candidate() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();

    std::fs::write(
        root.join("tests/test_runner.py"),
        "import pytest\n\n\n@pytest.fixture\ndef exe(tmp_path):\n    p = tmp_path / \"tool.exe\"\n    p.write_text(\"\", encoding=\"utf-8\")\n    return str(p)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/cli.py"),
        "import os\n\n\ndef resolve():\n    exe = (os.environ.get(\"TOOL\") or \"\").strip()\n    if not exe:\n        raise RuntimeError(\"missing\")\n    return exe\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    assert!(
        !has_edge(&cg, "src/cli.py", "exe", "tests/test_runner.py").await,
        "production code must not bind a local name to a test fixture it cannot reach"
    );
}

/// The gate must not swallow the legitimate case: an explicit import is
/// exactly the evidence it asks for, so the edge survives.
#[tokio::test]
async fn a_python_bare_name_still_binds_when_the_caller_imports_it() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("pkg")).unwrap();

    std::fs::write(
        root.join("pkg/helpers.py"),
        "def shared_helper():\n    return 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("pkg/app.py"),
        "from pkg.helpers import shared_helper\n\n\ndef run():\n    return shared_helper()\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    assert!(
        has_edge(&cg, "pkg/app.py", "shared_helper", "pkg/helpers.py").await,
        "an imported name is the evidence the gate asks for; the edge must survive"
    );
}

/// Same file is evidence too, and is the most common case of all.
#[tokio::test]
async fn a_python_bare_name_still_binds_within_one_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("solo.py"),
        "def helper():\n    return 2\n\n\ndef caller():\n    return helper()\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    assert!(
        has_edge(&cg, "solo.py", "helper", "solo.py").await,
        "a same-file call must always resolve"
    );
}

/// The scoping decision, pinned. Rust resolves bare names through a module
/// system the gate cannot see, so a cross-module call with no same-directory
/// or import evidence *must still resolve* — this is the 11.4% of call edges
/// the unscoped version destroyed.
#[tokio::test]
async fn a_rust_bare_name_across_modules_is_not_gated() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src/deep/nested")).unwrap();

    std::fs::write(
        root.join("src/deep/nested/util.rs"),
        "pub fn compute_widget_total() -> i32 {\n    7\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod deep;\n\npub fn run() -> i32 {\n    compute_widget_total()\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    assert!(
        has_edge(
            &cg,
            "src/lib.rs",
            "compute_widget_total",
            "src/deep/nested/util.rs"
        )
        .await,
        "Rust bare-name resolution must be unaffected by the gate"
    );
}
