// Rust guideline compliant 2025-10-17
//! Per-language LSP adapter trait and registry.
//!
//! An `LspAdapter` knows how to detect, spawn, and configure its language's
//! LSP server. The `LspManager::start` entry point walks an `&[Box<dyn
//! LspAdapter>]` list, calls `detect()` on each, and skips any whose binary
//! is not on `$PATH`. Detected adapters get spawned, initialized, and
//! registered as clients on the manager.
//!
//! 5.0 ships Phase 1 (standalone-binary servers): `rust-analyzer` lands in
//! commit 3a here; `gopls`, `clangd`, `zls`, `lua-language-server` land in
//! commit 4. Phase 2 (Node.js servers) and Phase 3 (daemon-kept) follow.

pub mod clangd;
pub mod go;
pub mod java;
pub mod lua;
pub mod python;
pub mod rust;
pub mod zig;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

/// Per-language LSP adapter contract.
///
/// Each adapter is responsible for:
///
/// - declaring which language ids it serves (matching `LspManager::language_for_path`)
/// - locating its server binary on `$PATH`
/// - producing a `Command` that spawns the server in stdio mode
/// - optionally providing server-specific `initializationOptions`
/// - declaring whether a manifest (e.g. `Cargo.toml`, `go.mod`) is required
///   for cross-file resolution; manifest-required adapters skip silently when
///   the manifest is absent
pub trait LspAdapter: Send + Sync {
    /// Languages this adapter handles. Each id is what `language_for_path`
    /// returns; e.g. `&["rust"]`, `&["c", "cpp", "objc"]`.
    fn languages(&self) -> &[&'static str];

    /// Binary names to probe on `$PATH`, in priority order.
    fn server_binaries(&self) -> &[&'static str];

    /// First binary from `server_binaries` that exists on `$PATH`.
    /// Default impl uses the local `which` helper; override only if the
    /// adapter needs special detection (version pinning, sdk checks, etc.).
    fn detect(&self) -> Option<PathBuf> {
        for name in self.server_binaries() {
            if let Some(p) = which(name) {
                return Some(p);
            }
        }
        None
    }

    /// Build the spawn command for `binary`. The default impl runs the
    /// binary with no arguments (most LSP servers default to stdio); adapters
    /// that need flags override this.
    fn spawn_command(&self, binary: &Path) -> Command {
        Command::new(binary)
    }

    /// Optional server-specific `initializationOptions`. `None` = no field
    /// emitted in the `initialize` request.
    fn init_options(&self, _project_root: &Path) -> Option<Value> {
        None
    }

    /// Canonical manifest filename this adapter expects in `project_root`.
    /// When set, the manager calls `manifest_present` to gate spawn; the
    /// default impl uses this filename as a single-file check, but adapters
    /// can override `manifest_present` to accept several alternatives
    /// (e.g. Maven *or* Gradle for Java).
    fn requires_manifest(&self) -> Option<&'static str> {
        None
    }

    /// True when `project_root` contains a manifest acceptable to this
    /// adapter. Default impl: when `requires_manifest` returns `Some(name)`,
    /// the file `project_root/name` must exist; otherwise no gate.
    /// Override to accept multiple alternative manifests.
    fn manifest_present(&self, project_root: &Path) -> bool {
        match self.requires_manifest() {
            None => true,
            Some(name) => project_root.join(name).exists(),
        }
    }

    /// Per-server post-initialize grace period. Some servers (rust-analyzer,
    /// gopls) need a few seconds of background indexing before
    /// `textDocument/definition` returns accurate results. The manager waits
    /// this long after `initialized` before registering the client.
    fn index_grace_period(&self) -> Duration {
        Duration::from_secs(3)
    }
}

/// Locate `name` on `$PATH`. Returns the first match. Std-only — we don't
/// pull in the `which` crate just for this. Skips empty PATH entries and
/// nonexistent directories.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        // Windows: try common executable extensions.
        #[cfg(target_os = "windows")]
        for ext in ["exe", "cmd", "bat"] {
            let with_ext = candidate.with_extension(ext);
            if is_executable(&with_ext) {
                return Some(with_ext);
            }
        }
    }
    None
}

/// True when `path` exists and is executable. On Unix that means the user
/// has the executable bit on the file; on Windows it just means the file
/// exists with one of the known executable extensions.
fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        (meta.permissions().mode() & 0o111) != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn which_finds_real_command() {
        // `sh` (or `cmd` on Windows) is present everywhere we run tests.
        #[cfg(unix)]
        let probe = "sh";
        #[cfg(not(unix))]
        let probe = "cmd";
        let found = which(probe);
        assert!(found.is_some(), "{probe} should be discoverable on PATH");
    }

    #[test]
    fn which_returns_none_for_nonexistent_binary() {
        assert!(which("definitely-not-a-real-binary-xyz123").is_none());
    }
}
