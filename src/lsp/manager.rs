// Rust guideline compliant 2025-10-17
//! Lifecycle owner for one or more `LspClient`s, keyed by language.
//!
//! `LspManager` is the integration point between the rest of tokensave and
//! per-language LSP servers. It is intentionally adapter-free in this 5.0
//! slice — concrete spawn logic for `rust-analyzer`, `gopls`, `clangd`, etc.
//! lands in the follow-up commit. What this file establishes:
//!
//! - the contract `LspManager::client_for_language` that the resolver layer
//!   queries to find an active client for a given file's language
//! - graceful degradation when no client is running for a language: the
//!   resolver's pass-through behaviour falls back to the heuristic resolver
//! - explicit shutdown that the sync caller invokes at the end of a pass
//!
//! Adapters will register themselves via `register_client` in commit 3. The
//! manager itself stays adapter-free so future Phase 2 / Phase 3 work can
//! plug in without touching this module.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::client::LspClient;

/// Default grace period for `shutdown` to wait per server before SIGKILL.
const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// Lifecycle owner for active LSP clients.
///
/// One client per language id (`"rust"`, `"go"`, `"c"`, `"cpp"`, `"objc"`,
/// `"zig"`, `"lua"`). Multiple file extensions can share the same client
/// (e.g. clangd serves `c`, `cpp`, and `objc`); the registration layer is
/// responsible for de-duplicating spawns.
pub struct LspManager {
    clients: HashMap<String, Arc<LspClient>>,
    project_root: PathBuf,
}

impl LspManager {
    /// Create an empty manager rooted at `project_root`. No servers are
    /// spawned; callers register clients via `register_client`.
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            clients: HashMap::new(),
            project_root,
        }
    }

    /// Register `client` as the LSP for `language_id`. Replaces any prior
    /// client for the same language; the prior client is dropped, which
    /// cancels its background tasks (the writer task exits when the channel
    /// closes; the reader exits when stdout is closed).
    pub fn register_client(&mut self, language_id: impl Into<String>, client: LspClient) {
        self.clients.insert(language_id.into(), Arc::new(client));
    }

    /// Look up the client serving `language_id`, if any. The Arc lets the
    /// resolver hold the client across `await` points without locking the
    /// whole manager.
    pub fn client_for_language(&self, language_id: &str) -> Option<Arc<LspClient>> {
        self.clients.get(language_id).cloned()
    }

    /// Best-known language id for a file path, derived from its extension.
    /// Returns `None` for extensions tokensave doesn't have a Phase 1
    /// adapter for; the resolver treats that as "skip LSP, use heuristic".
    pub fn language_for_path(path: &std::path::Path) -> Option<&'static str> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "rs" => "rust",
            "go" => "go",
            "c" | "h" => "c",
            "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
            "m" | "mm" => "objc",
            "zig" => "zig",
            "lua" => "lua",
            _ => return None,
        })
    }

    /// Project root the manager was constructed with. Adapters use this for
    /// `rootUri` in `initialize` and for path-relative URI computation.
    pub fn project_root(&self) -> &std::path::Path {
        &self.project_root
    }

    /// True when at least one client is registered. Used by the sync caller
    /// to skip the LSP pass entirely on projects whose languages have no
    /// installed servers.
    pub fn is_active(&self) -> bool {
        !self.clients.is_empty()
    }

    /// Send `shutdown` + `exit` to every registered client and wait up to
    /// `grace` per server for a clean exit. Servers that don't quit get
    /// killed. Idempotent: calling twice is safe.
    pub async fn shutdown(&self, grace: Duration) {
        for (_, client) in self.clients.iter() {
            let _ = client.shutdown(grace).await;
        }
    }

    /// Convenience for the most common shutdown configuration.
    pub async fn shutdown_default(&self) {
        self.shutdown(DEFAULT_SHUTDOWN_GRACE).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_manager_is_inactive() {
        let m = LspManager::new(PathBuf::from("/tmp/proj"));
        assert!(!m.is_active());
        assert!(m.client_for_language("rust").is_none());
    }

    #[test]
    fn language_for_path_known_extensions() {
        let cases: &[(&str, &str)] = &[
            ("foo.rs", "rust"),
            ("foo.go", "go"),
            ("foo.c", "c"),
            ("foo.h", "c"),
            ("foo.cpp", "cpp"),
            ("foo.hpp", "cpp"),
            ("foo.cc", "cpp"),
            ("foo.m", "objc"),
            ("foo.zig", "zig"),
            ("foo.lua", "lua"),
        ];
        for (name, want) in cases {
            assert_eq!(
                LspManager::language_for_path(std::path::Path::new(name)),
                Some(*want),
                "language for {name} should be {want}"
            );
        }
    }

    #[test]
    fn language_for_path_unknown_extension_is_none() {
        assert!(LspManager::language_for_path(std::path::Path::new("foo.py")).is_none());
        assert!(LspManager::language_for_path(std::path::Path::new("README.md")).is_none());
        assert!(LspManager::language_for_path(std::path::Path::new("Makefile")).is_none());
    }

    #[test]
    fn language_for_path_is_case_insensitive() {
        assert_eq!(
            LspManager::language_for_path(std::path::Path::new("Foo.RS")),
            Some("rust")
        );
    }

    #[test]
    fn project_root_is_returned() {
        let m = LspManager::new(PathBuf::from("/tmp/proj"));
        assert_eq!(m.project_root(), std::path::Path::new("/tmp/proj"));
    }
}
