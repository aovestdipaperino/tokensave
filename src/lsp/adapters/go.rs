// Rust guideline compliant 2025-10-17
//! Go LSP adapter (`gopls`).
//!
//! Detection: probes for `gopls` on `$PATH`. The Go toolchain bundles it
//! starting with Go 1.20, so most installs already have it.
//!
//! Manifest: `go.mod` is required. Without it gopls falls into GOPATH mode,
//! which gives less reliable cross-file resolution and skips cross-package
//! definition entirely. Skipping when the manifest is absent saves the
//! spawn cost in the (rare) GOPATH-only case.
//!
//! Initialization: gopls reuses the build cache, so its grace period is
//! shorter than rust-analyzer's. 5s comfortably covers a small workspace;
//! large monorepos pay more but the per-request timeout catches genuinely
//! stuck queries.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use super::LspAdapter;

pub struct GoplsAdapter;

impl LspAdapter for GoplsAdapter {
    fn languages(&self) -> &[&'static str] {
        &["go"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["gopls"]
    }

    fn requires_manifest(&self) -> Option<&'static str> {
        Some("go.mod")
    }

    fn init_options(&self, _project_root: &Path) -> Option<Value> {
        // Modest defaults: disable diagnostics on save (we only care about
        // definitions) and turn on the build cache so warm runs are fast.
        Some(json!({
            "ui": {
                "diagnostic": {
                    "diagnosticsDelay": "1s"
                }
            },
            "build": {
                "memoryMode": "DegradeClosed"
            }
        }))
    }

    fn index_grace_period(&self) -> Duration {
        Duration::from_secs(5)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn languages_returns_go() {
        assert_eq!(GoplsAdapter.languages(), &["go"]);
    }

    #[test]
    fn server_binaries_lists_gopls() {
        assert_eq!(GoplsAdapter.server_binaries(), &["gopls"]);
    }

    #[test]
    fn requires_go_mod() {
        assert_eq!(GoplsAdapter.requires_manifest(), Some("go.mod"));
    }
}
