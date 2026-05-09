// Rust guideline compliant 2025-10-17
//! `rust-analyzer` adapter.
//!
//! Detection: looks for `rust-analyzer` on `$PATH`. Falls back to
//! `rust-analyzer-proc-macro-srv` is *not* probed — that's the proc-macro
//! sidecar binary, not the LSP server.
//!
//! Manifest: `Cargo.toml` is required for cross-file resolution. Without one,
//! rust-analyzer falls into single-file mode (basic syntax only) and
//! `textDocument/definition` for cross-crate calls returns null. The manager
//! skips registration in that case.
//!
//! Initialization: rust-analyzer needs ~10 seconds of background indexing
//! after `initialized` before definition responses are accurate. The
//! `index_grace_period` reflects that.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use super::LspAdapter;

pub struct RustAnalyzerAdapter;

impl LspAdapter for RustAnalyzerAdapter {
    fn languages(&self) -> &[&'static str] {
        &["rust"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["rust-analyzer"]
    }

    fn requires_manifest(&self) -> Option<&'static str> {
        Some("Cargo.toml")
    }

    /// rust-analyzer benefits from being told to skip building proc macros
    /// in resolution-only mode and to enable definition-from-derive support.
    /// These are best-effort hints — the server ignores unknown options.
    fn init_options(&self, _project_root: &Path) -> Option<Value> {
        Some(json!({
            "checkOnSave": false,
            "cargo": {
                "buildScripts": {
                    "enable": false
                },
                "loadOutDirsFromCheck": false
            },
            "procMacro": {
                "enable": true
            }
        }))
    }

    fn index_grace_period(&self) -> Duration {
        Duration::from_secs(10)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn languages_returns_rust() {
        assert_eq!(RustAnalyzerAdapter.languages(), &["rust"]);
    }

    #[test]
    fn server_binaries_lists_rust_analyzer() {
        assert_eq!(RustAnalyzerAdapter.server_binaries(), &["rust-analyzer"]);
    }

    #[test]
    fn requires_cargo_manifest() {
        assert_eq!(RustAnalyzerAdapter.requires_manifest(), Some("Cargo.toml"));
    }

    #[test]
    fn init_options_include_check_on_save_off() {
        let opts = RustAnalyzerAdapter
            .init_options(std::path::Path::new("/"))
            .unwrap();
        assert_eq!(opts["checkOnSave"], false);
    }

    #[test]
    fn grace_period_is_at_least_5_seconds() {
        assert!(RustAnalyzerAdapter.index_grace_period() >= Duration::from_secs(5));
    }
}
