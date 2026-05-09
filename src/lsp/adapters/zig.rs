// Rust guideline compliant 2025-10-17
//! Zig LSP adapter (`zls`).
//!
//! Detection: probes `zls` on `$PATH`. zls ships as a single binary and
//! Zig's official release page links to a recent build.
//!
//! Manifest: none required. zls works on individual `.zig` files and the
//! incremental compilation model means cross-file resolution works for
//! anything reachable through `@import`.
//!
//! Initialization: zls's startup is the cheapest of the Phase 1 servers
//! because it doesn't compile anything until the first definition request.

use std::time::Duration;

use super::LspAdapter;

pub struct ZlsAdapter;

impl LspAdapter for ZlsAdapter {
    fn languages(&self) -> &[&'static str] {
        &["zig"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["zls"]
    }

    fn requires_manifest(&self) -> Option<&'static str> {
        None
    }

    fn index_grace_period(&self) -> Duration {
        Duration::from_secs(2)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn languages_returns_zig() {
        assert_eq!(ZlsAdapter.languages(), &["zig"]);
    }

    #[test]
    fn server_binaries_lists_zls() {
        assert_eq!(ZlsAdapter.server_binaries(), &["zls"]);
    }

    #[test]
    fn grace_period_under_5_seconds() {
        assert!(ZlsAdapter.index_grace_period() < Duration::from_secs(5));
    }
}
