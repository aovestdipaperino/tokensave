// Rust guideline compliant 2025-10-17
//! `clangd` adapter for C, C++, and Objective-C.
//!
//! One adapter, three languages: clangd's request handling is uniform
//! across the three so a single registered client backs `c`, `cpp`, and
//! `objc` files. The `LspManager` shares the resulting `Arc<LspClient>`
//! across all three language ids.
//!
//! Manifest: `compile_commands.json` is preferred (clangd uses it for
//! exact compile-command resolution) but optional — without it clangd
//! falls back to heuristics that work for header-only and small tree
//! navigation. We do not gate on a manifest so single-file C/C++ tools
//! still get LSP-quality definitions.
//!
//! Detection: probes `clangd` on `$PATH`. Most distros and the LLVM
//! release tarballs ship the binary directly.
//!
//! `--background-index` is on by default in modern clangd, which means
//! cross-file resolution works without any explicit indexing call.

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use super::LspAdapter;

pub struct ClangdAdapter;

impl LspAdapter for ClangdAdapter {
    fn languages(&self) -> &[&'static str] {
        &["c", "cpp", "objc"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["clangd"]
    }

    fn spawn_command(&self, binary: &Path) -> Command {
        let mut cmd = Command::new(binary);
        // Background indexing makes cross-file resolution reliable on
        // first hit; --header-insertion=never avoids surprise edits when
        // we only ever ask for definitions.
        cmd.arg("--background-index")
            .arg("--header-insertion=never");
        cmd
    }

    fn requires_manifest(&self) -> Option<&'static str> {
        // compile_commands.json improves accuracy but is not required.
        // Returning None means manifest_present always passes.
        None
    }

    fn index_grace_period(&self) -> Duration {
        Duration::from_secs(3)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn languages_returns_c_cpp_objc() {
        assert_eq!(ClangdAdapter.languages(), &["c", "cpp", "objc"]);
    }

    #[test]
    fn server_binaries_lists_clangd() {
        assert_eq!(ClangdAdapter.server_binaries(), &["clangd"]);
    }

    #[test]
    fn no_manifest_required() {
        assert_eq!(ClangdAdapter.requires_manifest(), None);
    }
}
