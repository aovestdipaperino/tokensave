// Rust guideline compliant 2025-10-17
//! Language Server Protocol integration.
//!
//! Optional LSP pass during sync that upgrades cross-file edge accuracy.
//! Tree-sitter remains the primary extraction engine; LSP servers, when
//! available, replace heuristic name-matching with semantically exact
//! definition resolution. See `docs/LSP-INTEGRATION.md` for the full design.
//!
//! 5.0 ships Phase 1 (standalone-binary servers): `rust-analyzer`, `gopls`,
//! `clangd` (C/C++/Obj-C), `zls`, `lua-language-server`. Phase 2 (Node.js
//! servers) and Phase 3 (daemon-kept servers) follow.
//!
//! This module exposes:
//!
//! - `protocol` — typed LSP message bodies (initialize, didOpen, definition)
//! - `client`   — JSON-RPC 2.0 stdin/stdout transport over a child process
//!
//! Higher layers (`LspManager`, `LspResolver`, per-language adapters) land in
//! follow-up commits.

pub mod adapters;
pub mod client;
pub mod manager;
pub mod protocol;
pub mod resolver;

use std::path::PathBuf;

use crate::errors::Result;
use crate::types::{Edge, Node, UnresolvedRef};

/// Output of an LSP resolution pass. The caller inserts `lsp_edges` via
/// `Database::insert_edges_with_provenance(_, "lsp")` and forwards
/// `remaining_unresolved` to the heuristic resolver.
pub struct LspPassOutput {
    pub lsp_edges: Vec<Edge>,
    pub remaining_unresolved: Vec<UnresolvedRef>,
    /// Language ids whose servers were successfully started this pass. An
    /// empty vec means the pass was a no-op (no LSP for this project, or
    /// the kill switch was set).
    pub started_languages: Vec<&'static str>,
}

/// Run an LSP resolution pass against `unresolved` using all Phase 1
/// adapters (currently rust-analyzer; more land in subsequent commits).
///
/// Honors the `TOKENSAVE_LSP` env var: any of `0`, `false`, `off`, `no`
/// (case-insensitive) skips the pass entirely and returns every ref as
/// still-unresolved. An absent var, or any other value, enables LSP.
///
/// Per-server failures (binary missing, manifest missing, initialize
/// timeout) are non-fatal: the pass returns whatever subset of refs the
/// running servers managed to resolve.
pub async fn run_pass(
    project_root: PathBuf,
    nodes: &[Node],
    unresolved: &[UnresolvedRef],
) -> Result<LspPassOutput> {
    if !lsp_enabled() {
        return Ok(LspPassOutput {
            lsp_edges: Vec::new(),
            remaining_unresolved: unresolved.to_vec(),
            started_languages: Vec::new(),
        });
    }

    let adapters: Vec<Box<dyn adapters::LspAdapter>> = vec![
        Box::new(adapters::rust::RustAnalyzerAdapter),
        Box::new(adapters::go::GoplsAdapter),
        Box::new(adapters::clangd::ClangdAdapter),
        Box::new(adapters::zig::ZlsAdapter),
        Box::new(adapters::lua::LuaLsAdapter),
        Box::new(adapters::python::PythonAdapter),
        Box::new(adapters::java::JavaAdapter),
    ];

    let mut manager = manager::LspManager::new(project_root);
    let started = manager.start(&adapters).await;

    if !manager.is_active() {
        return Ok(LspPassOutput {
            lsp_edges: Vec::new(),
            remaining_unresolved: unresolved.to_vec(),
            started_languages: Vec::new(),
        });
    }

    let lsp_resolver = resolver::LspResolver::from_nodes(&manager, nodes);
    let result = lsp_resolver.resolve_all(unresolved).await?;

    let lsp_edges: Vec<Edge> = result
        .resolved
        .iter()
        .map(|r| Edge {
            source: r.original.from_node_id.clone(),
            target: r.target_node_id.clone(),
            kind: r.original.reference_kind,
            line: Some(r.original.line),
        })
        .collect();

    manager.shutdown_default().await;

    Ok(LspPassOutput {
        lsp_edges,
        remaining_unresolved: result.unresolved,
        started_languages: started,
    })
}

/// True when the `TOKENSAVE_LSP` env var is unset or set to a truthy value.
/// Treats `0`, `false`, `off`, `no` as the kill switch (case-insensitive).
fn lsp_enabled() -> bool {
    match std::env::var("TOKENSAVE_LSP") {
        Err(_) => true,
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
    }
}

// Note: the `lsp_enabled` env-var helper is exercised end-to-end in the
// gated integration test in commit 4b. Unit-testing it would require
// touching shared process env state and serialising against other tests
// that read TOKENSAVE_LSP; not worth the test infrastructure for a 5-line
// helper.
