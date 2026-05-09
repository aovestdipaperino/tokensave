//! LSP integration tests gated on `cfg(feature = "lsp-integration")`.
//!
//! Run with:
//!
//!     cargo test --features lsp-integration --test lsp_integration_test
//!
//! Each test spawns a real LSP server binary against a temporary fixture
//! project. The whole module compiles to nothing under default features,
//! so `cargo test` on a system without rust-analyzer installed continues
//! to pass.
//!
//! The fixtures live in-process — written to a `tempfile::TempDir` per
//! test — so there is no dependency on external test data.

#![cfg(feature = "lsp-integration")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::time::Duration;

use tempfile::TempDir;
use tokensave::lsp;
use tokensave::lsp::adapters::rust::RustAnalyzerAdapter;
use tokensave::lsp::adapters::{which, LspAdapter};
use tokensave::lsp::manager::LspManager;
use tokensave::lsp::resolver::LspResolver;
use tokensave::types::{EdgeKind, Node, NodeKind, UnresolvedRef, Visibility};

/// Skip a test when the binary isn't actually usable on the runner.
///
/// Two checks:
///   1. `which` finds it on `$PATH`
///   2. `<binary> --version` exits successfully
///
/// The second check catches the rustup-proxy case: `~/.cargo/bin/rust-analyzer`
/// can be a wrapper that delegates to the active toolchain, and on toolchains
/// without the component installed the wrapper exits non-zero with no LSP
/// traffic. `which` would still report success in that case, so the spawn
/// would succeed and the initialize handshake would silently time out.
macro_rules! require_binary {
    ($name:literal) => {{
        let Some(path) = which($name) else {
            eprintln!("[skip] {} not on PATH", $name);
            return;
        };
        let ok = std::process::Command::new(&path)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            eprintln!(
                "[skip] {} present at {} but `--version` failed (likely a broken proxy)",
                $name,
                path.display()
            );
            return;
        }
    }};
}

/// Build a minimal Rust crate with two files and a cross-file call.
/// Returns the project root.
fn build_rust_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "lsp_fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(root.join("src")).unwrap();

    // src/lib.rs declares mod helper and calls helper::greet from main_logic.
    std::fs::write(
        root.join("src/lib.rs"),
        r#"pub mod helper;

pub fn main_logic() -> String {
    helper::greet("world")
}
"#,
    )
    .unwrap();

    std::fs::write(
        root.join("src/helper.rs"),
        r#"pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#,
    )
    .unwrap();

    dir
}

/// Sample Node matching what the tree-sitter Rust extractor would emit
/// for the `greet` function in src/helper.rs.
fn helper_greet_node() -> Node {
    Node {
        id: "lsp_fixture::helper::greet".to_string(),
        kind: NodeKind::Function,
        name: "greet".to_string(),
        qualified_name: "lsp_fixture::helper::greet".to_string(),
        file_path: "src/helper.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 3,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn greet(name: &str) -> String".to_string()),
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
        updated_at: 0,
    }
}

fn main_logic_caller_node() -> Node {
    Node {
        id: "lsp_fixture::main_logic".to_string(),
        kind: NodeKind::Function,
        name: "main_logic".to_string(),
        qualified_name: "lsp_fixture::main_logic".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 3,
        attrs_start_line: 3,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("pub fn main_logic() -> String".to_string()),
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
        updated_at: 0,
    }
}

#[tokio::test]
async fn rust_analyzer_resolves_cross_file_call() {
    require_binary!("rust-analyzer");

    let fixture = build_rust_fixture();
    let project_root = fixture.path().to_path_buf();

    // Hand-roll a single LspManager + RustAnalyzerAdapter rather than going
    // through lsp::run_pass so we can control timeout and assert against
    // the resolver output directly.
    let mut manager = LspManager::new(project_root.clone());
    let adapters: Vec<Box<dyn LspAdapter>> = vec![Box::new(RustAnalyzerAdapter)];
    let started = manager.start(&adapters).await;

    assert!(
        started.contains(&"rust"),
        "rust-analyzer should register the rust language"
    );
    assert!(manager.is_active(), "manager should be active");

    // Two nodes: the caller in src/lib.rs and the target in src/helper.rs.
    let nodes = vec![main_logic_caller_node(), helper_greet_node()];

    // The unresolved ref points to the call site:
    // src/lib.rs:4: helper::greet("world")
    //                       ^ column 13 (1-based) — middle of "greet"
    let uref = UnresolvedRef {
        from_node_id: "lsp_fixture::main_logic".to_string(),
        reference_name: "greet".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 4,
        column: 13,
        file_path: "src/lib.rs".to_string(),
    };

    let resolver = LspResolver::from_nodes(&manager, &nodes);
    let result = resolver.resolve_all(&[uref]).await.expect("resolve_all");

    assert_eq!(result.total, 1);
    assert_eq!(
        result.resolved_count, 1,
        "rust-analyzer should resolve the cross-file call to greet"
    );
    let resolved = &result.resolved[0];
    assert_eq!(resolved.target_node_id, "lsp_fixture::helper::greet");
    assert_eq!(resolved.resolved_by, "lsp");

    manager.shutdown(Duration::from_secs(3)).await;
}

#[tokio::test]
async fn run_pass_returns_lsp_edges_when_rust_analyzer_present() {
    require_binary!("rust-analyzer");

    let fixture = build_rust_fixture();
    let project_root = fixture.path().to_path_buf();

    let nodes = vec![main_logic_caller_node(), helper_greet_node()];
    let uref = UnresolvedRef {
        from_node_id: "lsp_fixture::main_logic".to_string(),
        reference_name: "greet".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 4,
        column: 13,
        file_path: "src/lib.rs".to_string(),
    };

    let pass = lsp::run_pass(project_root, &nodes, &[uref])
        .await
        .expect("run_pass");

    assert!(
        pass.started_languages.contains(&"rust"),
        "rust language should appear in started_languages"
    );
    assert_eq!(
        pass.lsp_edges.len(),
        1,
        "expected one LSP-resolved edge for the cross-file call"
    );
    let edge = &pass.lsp_edges[0];
    assert_eq!(edge.source, "lsp_fixture::main_logic");
    assert_eq!(edge.target, "lsp_fixture::helper::greet");
    assert!(matches!(edge.kind, EdgeKind::Calls));
    assert!(pass.remaining_unresolved.is_empty());
}

#[tokio::test]
async fn run_pass_kill_switch_skips_lsp() {
    // Even if rust-analyzer is installed, TOKENSAVE_LSP=0 must short-circuit.
    let prev = std::env::var("TOKENSAVE_LSP").ok();
    // SAFETY: env mutation is unsafe in newer Rust. Test is single-threaded
    // by tokio::test; restore the prior value at the end.
    unsafe {
        std::env::set_var("TOKENSAVE_LSP", "0");
    }

    let fixture = build_rust_fixture();
    let project_root = fixture.path().to_path_buf();

    let nodes = vec![main_logic_caller_node(), helper_greet_node()];
    let uref = UnresolvedRef {
        from_node_id: "lsp_fixture::main_logic".to_string(),
        reference_name: "greet".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 4,
        column: 13,
        file_path: "src/lib.rs".to_string(),
    };

    let pass = lsp::run_pass(project_root, &nodes, &[uref])
        .await
        .expect("run_pass");

    assert!(
        pass.started_languages.is_empty(),
        "kill switch should prevent any server from starting"
    );
    assert!(
        pass.lsp_edges.is_empty(),
        "kill switch should produce zero LSP edges"
    );
    assert_eq!(
        pass.remaining_unresolved.len(),
        1,
        "every ref passes through"
    );

    match prev {
        Some(v) => unsafe { std::env::set_var("TOKENSAVE_LSP", v) },
        None => unsafe { std::env::remove_var("TOKENSAVE_LSP") },
    }
}

#[tokio::test]
async fn manager_skips_rust_analyzer_without_cargo_toml() {
    // No Cargo.toml at the project root. Manager should refuse to start
    // rust-analyzer regardless of whether the binary is on PATH.
    let dir = TempDir::new().unwrap();
    // Touch a single .rs file so the directory isn't pathologically empty.
    std::fs::write(dir.path().join("scratch.rs"), "fn main() {}").unwrap();

    let mut manager = LspManager::new(dir.path().to_path_buf());
    let adapters: Vec<Box<dyn LspAdapter>> = vec![Box::new(RustAnalyzerAdapter)];
    let started = manager.start(&adapters).await;

    assert!(
        started.is_empty(),
        "rust-analyzer must not start without Cargo.toml; got {started:?}"
    );
    assert!(!manager.is_active());
}

#[allow(dead_code)]
fn _project_root_helper(p: &Path) -> &Path {
    p
}
