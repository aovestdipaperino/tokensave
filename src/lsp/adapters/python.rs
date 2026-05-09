// Rust guideline compliant 2025-10-17
//! Python LSP adapter.
//!
//! Detection probes, in priority order:
//!
//! 1. `pyright-langserver` — Microsoft's TypeScript-based server. Best
//!    broken-code tolerance of any Python LSP. Distributed via npm; once
//!    installed it acts like a regular CLI binary.
//! 2. `pylsp` — pure-Python alternative (`python-lsp-server`). Slower and
//!    less tolerant of partial code, but doesn't require Node.js.
//! 3. `python-lsp-server` — alias some package managers expose.
//!
//! No project manifest is required: both servers handle plain `.py` trees
//! out of the box. Pyright benefits from a `pyrightconfig.json` or
//! `pyproject.toml`, but works without either.
//!
//! Pyright needs `--stdio` to speak the LSP framing tokensave expects;
//! pylsp uses stdio by default.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::process::Command;

use super::LspAdapter;

pub struct PythonAdapter;

impl LspAdapter for PythonAdapter {
    fn languages(&self) -> &[&'static str] {
        &["python"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["pyright-langserver", "pylsp", "python-lsp-server"]
    }

    fn spawn_command(&self, binary: &Path) -> Command {
        let mut cmd = Command::new(binary);
        // Pyright requires --stdio; pylsp accepts it as a no-op (stdio is
        // its default). Passing the flag unconditionally is safe.
        if binary
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.contains("pyright"))
            .unwrap_or(false)
        {
            cmd.arg("--stdio");
        }
        cmd
    }

    fn init_options(&self, _project_root: &Path) -> Option<Value> {
        // Reasonable defaults that pyright understands and pylsp ignores.
        Some(json!({
            "python": {
                "analysis": {
                    "autoSearchPaths": true,
                    "useLibraryCodeForTypes": true,
                    "diagnosticMode": "openFilesOnly"
                }
            }
        }))
    }

    fn requires_manifest(&self) -> Option<&'static str> {
        // Python tooling works without a manifest; pyrightconfig is a hint
        // but not a hard requirement for definition resolution.
        None
    }

    fn index_grace_period(&self) -> Duration {
        // Pyright initializes faster than rust-analyzer because it doesn't
        // build anything; 5s comfortably covers `openFilesOnly` indexing.
        Duration::from_secs(5)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn languages_returns_python() {
        assert_eq!(PythonAdapter.languages(), &["python"]);
    }

    #[test]
    fn server_binaries_lists_pyright_first() {
        let bins = PythonAdapter.server_binaries();
        assert_eq!(bins[0], "pyright-langserver");
        assert!(bins.contains(&"pylsp"));
    }

    #[test]
    fn no_manifest_required() {
        assert_eq!(PythonAdapter.requires_manifest(), None);
    }

    #[test]
    fn init_options_have_python_section() {
        let opts = PythonAdapter
            .init_options(std::path::Path::new("/"))
            .unwrap();
        assert!(opts["python"]["analysis"].is_object());
    }
}
