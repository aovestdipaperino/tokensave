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

pub mod client;
pub mod manager;
pub mod protocol;
pub mod resolver;
