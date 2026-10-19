//! Shared helpers for integration tests.
//!
//! Compiled once into the shared `integration` test binary and once into each
//! isolated `[[test]]` target that declares it, so not every helper is used by
//! every binary.
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
        force_permission_style: false,
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
        force_permission_style: false,
    }
}

/// Reads and parses a JSON file, panicking on I/O or parse errors.
pub fn read_json(path: &Path) -> serde_json::Value {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

/// The libtest name of `test` declared in the module at `module_path`, for a
/// test that re-runs its own binary with `--exact`. libtest names omit the
/// crate, so `integration::sync_test` + `foo` is `sync_test::foo`, while a
/// test at the root of its own binary is just `foo`.
pub fn qualified_test_name(module_path: &str, test: &str) -> String {
    match module_path.split_once("::") {
        Some((_, module)) => format!("{module}::{test}"),
        None => test.to_string(),
    }
}

/// `path` canonicalized and spelled the way tokensave reports a root: without
/// the `\\?\` verbatim prefix that [`Path::canonicalize`] adds on Windows.
/// Identical to `canonicalize` elsewhere (macOS still resolves `/var` to
/// `/private/var`).
pub fn reported_root(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap().to_string_lossy().into_owned();
    if let Some(rest) = canonical.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = canonical.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        canonical
    }
}

/// `value` as it appears inside a JSON string or a JSON-quoted message, without
/// the surrounding quotes. A Windows path's backslashes come out doubled.
pub fn json_escaped(value: &str) -> String {
    let quoted = serde_json::to_string(value).unwrap();
    quoted[1..quoted.len() - 1].to_string()
}

#[allow(dead_code)]
pub mod stability;
