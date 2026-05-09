// Rust guideline compliant 2025-10-17
//! Lua LSP adapter (`lua-language-server`, sometimes packaged as `luals`).
//!
//! Detection probes:
//!
//! 1. `lua-language-server` — the canonical binary name from the upstream
//!    release tarballs and most package managers
//! 2. `luals` — a shorter alias some Linux distros ship
//!
//! Manifest: none required. The server scans the workspace directory at
//! startup and resolves cross-file `require` calls without any
//! configuration.
//!
//! Initialization: ~3s on a small workspace. Larger projects (Neovim
//! plugin trees) can stretch to 10s; the per-request timeout catches
//! genuinely stuck requests.

use std::time::Duration;

use super::LspAdapter;

pub struct LuaLsAdapter;

impl LspAdapter for LuaLsAdapter {
    fn languages(&self) -> &[&'static str] {
        &["lua"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["lua-language-server", "luals"]
    }

    fn requires_manifest(&self) -> Option<&'static str> {
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
    fn languages_returns_lua() {
        assert_eq!(LuaLsAdapter.languages(), &["lua"]);
    }

    #[test]
    fn server_binaries_lists_lua_language_server_first() {
        let bins = LuaLsAdapter.server_binaries();
        assert_eq!(bins[0], "lua-language-server");
        assert!(bins.contains(&"luals"));
    }

    #[test]
    fn no_manifest_required() {
        assert_eq!(LuaLsAdapter.requires_manifest(), None);
    }
}
