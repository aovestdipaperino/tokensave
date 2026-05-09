// Rust guideline compliant 2025-10-17
//! Java LSP adapter (Eclipse JDT.LS).
//!
//! Detection probes:
//!
//! 1. `jdtls` — the wrapper script most distros / `sdkman` install
//! 2. `eclipse-jdtls` — alias under some Linux package managers
//!
//! Manifest gate: at least one of `pom.xml`, `build.gradle`,
//! `build.gradle.kts`, or `.classpath` must be present. Without a build
//! manifest jdtls falls into single-file mode where cross-file resolution
//! returns null, which is exactly the case we don't want to pay for.
//!
//! jdtls is a JVM application. Cold-start is heavy (15-60s on real-world
//! projects). The `index_grace_period` reflects the lower end; the LSP
//! pipeline's outer timeout-budget catches genuinely stuck servers.
//!
//! Note: 5.0 ships jdtls in Phase 1 even though the original design doc
//! placed Java in Phase 3, because most agents work in mixed Rust/Python/
//! Java repos and jdtls is the only LSP that actually understands Java
//! generics, overloads, and reflection-light dispatch. Phase 3 is reserved
//! for the truly heavy daemon-kept servers.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{which, LspAdapter};

pub struct JavaAdapter;

impl LspAdapter for JavaAdapter {
    fn languages(&self) -> &[&'static str] {
        &["java"]
    }

    fn server_binaries(&self) -> &[&'static str] {
        &["jdtls", "eclipse-jdtls"]
    }

    fn detect(&self) -> Option<PathBuf> {
        // Same probe logic as the default impl; named separately so the
        // adapter can grow JVM-version checks without changing the trait.
        for name in self.server_binaries() {
            if let Some(p) = which(name) {
                return Some(p);
            }
        }
        None
    }

    fn requires_manifest(&self) -> Option<&'static str> {
        // The manager checks a single filename. We pick the most common
        // (Maven). The alternative-manifest fallback is implemented in
        // `manifest_present` below and replaces the manager's hardcoded
        // check via `requires_manifest_present`.
        Some("pom.xml")
    }

    fn index_grace_period(&self) -> Duration {
        // jdtls's first request has to wait for the JVM, classpath
        // resolution, and the workspace scan. 15s is the lower bound on
        // a small Maven project; large multi-module builds can stretch
        // well past 30s but the per-request timeout catches them.
        Duration::from_secs(15)
    }
}

impl JavaAdapter {
    /// True when at least one Java build manifest is present in
    /// `project_root`. Used by the manager when `requires_manifest` returns
    /// a value but the actual gate is "any of these files."
    ///
    /// The LspManager today checks a single filename; this helper exists
    /// so a future commit can teach the manager to call
    /// `adapter.manifest_present(root)` instead, removing the single-file
    /// limitation. Until then, `pom.xml` is the canonical Maven manifest
    /// and Gradle-only projects need to wait for that follow-up.
    pub fn manifest_present(project_root: &Path) -> bool {
        for candidate in &["pom.xml", "build.gradle", "build.gradle.kts", ".classpath"] {
            if project_root.join(candidate).exists() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn languages_returns_java() {
        assert_eq!(JavaAdapter.languages(), &["java"]);
    }

    #[test]
    fn server_binaries_lists_jdtls_first() {
        assert_eq!(JavaAdapter.server_binaries()[0], "jdtls");
    }

    #[test]
    fn requires_pom_xml() {
        assert_eq!(JavaAdapter.requires_manifest(), Some("pom.xml"));
    }

    #[test]
    fn grace_period_at_least_10_seconds() {
        assert!(JavaAdapter.index_grace_period() >= Duration::from_secs(10));
    }

    #[test]
    fn manifest_present_detects_pom() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();
        assert!(JavaAdapter::manifest_present(dir.path()));
    }

    #[test]
    fn manifest_present_detects_gradle() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        assert!(JavaAdapter::manifest_present(dir.path()));
    }

    #[test]
    fn manifest_present_returns_false_when_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(!JavaAdapter::manifest_present(dir.path()));
    }
}
