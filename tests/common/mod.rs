//! Shared helpers for integration tests.
//!
//! Each integration test binary compiles this module independently, so not
//! every helper is used by every test.
#![allow(dead_code)]

use std::path::Path;

use tokensave::agents::{expected_tool_perms, InstallContext, InstallScope};
use tokensave::types::{ExtractionResult, NodeKind};

/// Names of all extracted nodes of the given kind, in extraction order.
pub fn names_of(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    result
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.name.clone())
        .collect()
}

/// A global-scope install context rooted at `home` with the default tool
/// permissions.
pub fn make_install_ctx(home: &Path) -> InstallContext {
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: "/usr/local/bin/tokensave".to_string(),
        tool_permissions: expected_tool_perms(),
        scope: InstallScope::Global,
    }
}

/// Like [`make_install_ctx`], but creates a fake executable tokensave binary
/// under `home/bin` so healthcheck binary-exists checks pass.
pub fn make_install_ctx_with_real_bin(home: &Path) -> InstallContext {
    let bin_dir = home.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let bin_path = bin_dir.join("tokensave");
    std::fs::write(&bin_path, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    InstallContext {
        home: home.to_path_buf(),
        tokensave_bin: bin_path.to_string_lossy().to_string(),
        tool_permissions: expected_tool_perms(),
        scope: InstallScope::Global,
    }
}

/// Reads and parses a JSON file, panicking on I/O or parse errors.
pub fn read_json(path: &Path) -> serde_json::Value {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}
