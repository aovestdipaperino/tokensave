// Rust guideline compliant 2025-10-17
//! `cargo check --message-format=json` driver.
//!
//! Each `compiler-message` line in the JSON stream produces zero or more
//! `Diagnostic` rows: zero when the message has no spans (rare; usually a
//! cross-cutting note), one per `spans[]` entry otherwise.
//!
//! The cargo target dir is forced to `.tokensave/target/` so concurrent
//! IDE / user `cargo check` runs don't fight us for `target/`'s lockfile.
//! That doubles disk usage on the project but is the only safe option
//! without coordination.
//!
//! Per-package and per-file scopes drop to `cargo check -p <pkg>`; cargo
//! has no native single-file mode, so the `File` scope falls back to
//! `Workspace` and the MCP layer post-filters the results.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;

use serde::Deserialize;

use crate::diagnostics::{Diagnostic, Driver, Scope};
use crate::errors::{Result, TokenSaveError};

/// Driver for Rust projects. Probes for `Cargo.toml` at the project root.
pub struct CargoDriver;

impl Driver for CargoDriver {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("Cargo.toml").exists()
    }

    fn run<'a>(
        &'a self,
        project_root: &'a Path,
        scope: &'a Scope,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Diagnostic>>> + Send + 'a>> {
        Box::pin(async move {
            let target_dir = target_dir_for(project_root);

            let mut cmd = tokio::process::Command::new("cargo");
            cmd.arg("check")
                .arg("--message-format=json")
                .arg("--target-dir")
                .arg(&target_dir)
                .current_dir(project_root)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true);

            if let Scope::Package { name } = scope {
                cmd.arg("-p").arg(name);
            }

            let output = cmd.output().await.map_err(|e| TokenSaveError::Config {
                message: format!("failed to spawn cargo: {e}"),
            })?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut diagnostics = Vec::new();
            for line in stdout.lines() {
                if line.is_empty() {
                    continue;
                }
                let parsed: CargoLine = match serde_json::from_str(line) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if parsed.reason != "compiler-message" {
                    continue;
                }
                let Some(msg) = parsed.message else { continue };
                if !is_diagnostic_level(&msg.level) {
                    continue;
                }
                if msg.spans.is_empty() {
                    continue;
                }

                let code = msg
                    .code
                    .as_ref()
                    .map(|c| c.code.clone())
                    .unwrap_or_default();

                for span in &msg.spans {
                    if !span.is_primary {
                        continue;
                    }
                    let rel_file = canonicalise_file(&span.file_name, project_root);
                    diagnostics.push(Diagnostic {
                        file: rel_file,
                        line_start: span.line_start,
                        line_end: span.line_end,
                        level: msg.level.clone(),
                        code: code.clone(),
                        message: msg.message.clone(),
                        driver: "rust",
                    });
                }
            }

            Ok(diagnostics)
        })
    }
}

/// `.tokensave/target/` is our private cargo target dir — set so we don't
/// race with the user's IDE or interactive `cargo check`. Created lazily
/// by cargo on first run.
fn target_dir_for(project_root: &Path) -> PathBuf {
    project_root.join(".tokensave").join("target")
}

/// Convert cargo's reported `file_name` (project-relative or absolute) into
/// the project-relative form the rest of tokensave uses. Cargo emits paths
/// relative to the manifest dir; we strip a leading project_root prefix
/// when present in case the path is absolute.
fn canonicalise_file(file_name: &str, project_root: &Path) -> String {
    let abs = if Path::new(file_name).is_absolute() {
        PathBuf::from(file_name)
    } else {
        project_root.join(file_name)
    };
    if let Ok(rel) = abs.strip_prefix(project_root) {
        return rel.to_string_lossy().to_string();
    }
    file_name.to_string()
}

/// Cargo emits messages of many levels — "warning" and "error" produce
/// diagnostics; "note", "help", "failure-note" are advisory and would
/// double-count if we surfaced them.
fn is_diagnostic_level(level: &str) -> bool {
    matches!(level, "error" | "warning")
}

#[derive(Debug, Deserialize)]
struct CargoLine {
    reason: String,
    message: Option<CargoMessage>,
}

#[derive(Debug, Deserialize)]
struct CargoMessage {
    level: String,
    message: String,
    code: Option<CargoCode>,
    spans: Vec<CargoSpan>,
}

#[derive(Debug, Deserialize)]
struct CargoCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct CargoSpan {
    file_name: String,
    line_start: u32,
    line_end: u32,
    is_primary: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn canonicalise_file_relative_passes_through() {
        let root = Path::new("/tmp/proj");
        assert_eq!(canonicalise_file("src/lib.rs", root), "src/lib.rs");
    }

    #[test]
    fn canonicalise_file_absolute_strips_root() {
        let root = Path::new("/tmp/proj");
        assert_eq!(
            canonicalise_file("/tmp/proj/src/lib.rs", root),
            "src/lib.rs"
        );
    }

    #[test]
    fn canonicalise_file_outside_project_passes_through() {
        let root = Path::new("/tmp/proj");
        assert_eq!(canonicalise_file("/etc/passwd", root), "/etc/passwd");
    }

    #[test]
    fn is_diagnostic_level_filters_advisory() {
        assert!(is_diagnostic_level("error"));
        assert!(is_diagnostic_level("warning"));
        assert!(!is_diagnostic_level("note"));
        assert!(!is_diagnostic_level("help"));
        assert!(!is_diagnostic_level("failure-note"));
    }

    #[test]
    fn target_dir_under_dot_tokensave() {
        let p = target_dir_for(Path::new("/tmp/proj"));
        assert_eq!(p, Path::new("/tmp/proj/.tokensave/target"));
    }
}
