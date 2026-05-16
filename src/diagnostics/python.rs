// Rust guideline compliant 2025-10-17
//! `pyright --outputjson` driver.
//!
//! pyright emits a structured JSON document with a `generalDiagnostics`
//! array. Each entry has `file`, `severity`, `message`, optional `rule`
//! (the reason code, e.g. `reportMissingImports`), and a `range` whose
//! `start.line` is 0-based. The driver normalises the line to 1-based and
//! the rule name into the `code` slot.
//!
//! Detection probes for `pyrightconfig.json` or `pyproject.toml`. Either
//! one is enough — pyright resolves Python sources from there. We do not
//! probe loose `.py` trees because pyright's defaults in that case skip
//! cross-file resolution and the diagnostics it emits are mostly
//! "missing imports" noise rather than real errors.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;

use serde::Deserialize;

use crate::diagnostics::{Diagnostic, Driver, Scope};
use crate::errors::Result;

pub struct PyrightDriver;

impl Driver for PyrightDriver {
    fn name(&self) -> &'static str {
        "python"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("pyrightconfig.json").exists()
            || project_root.join("pyproject.toml").exists()
    }

    fn run<'a>(
        &'a self,
        project_root: &'a Path,
        _scope: &'a Scope,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Diagnostic>>> + Send + 'a>> {
        Box::pin(async move {
            let mut cmd = tokio::process::Command::new("pyright");
            cmd.arg("--outputjson")
                .current_dir(project_root)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true);

            // Spawn errors (pyright not installed) fall through to an empty
            // diagnostic list rather than killing the broader diagnostics
            // call. The detect() probe gates on pyproject.toml / pyrightconfig
            // which can be present on projects that don't actually want
            // pyright to run; punishing them with a hard error is wrong.
            let output = match cmd.output().await {
                Ok(o) => o,
                Err(_) => return Ok(Vec::new()),
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(parse_pyright_output(&stdout, project_root))
        })
    }
}

/// Parse a pyright `--outputjson` document into a flat diagnostic list.
/// Returns an empty Vec for unparseable input rather than erroring — a
/// pyright crash shouldn't take down a sync.
pub fn parse_pyright_output(stdout: &str, project_root: &Path) -> Vec<Diagnostic> {
    let parsed: PyrightReport = match serde_json::from_str(stdout) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    parsed
        .general_diagnostics
        .into_iter()
        .filter(|d| matches_severity(&d.severity))
        .map(|d| {
            let file = canonicalise_file(&d.file, project_root);
            // pyright lines are 0-based; normalise to 1-based.
            let line_start = d.range.start.line.saturating_add(1);
            let line_end = d.range.end.line.saturating_add(1);
            Diagnostic {
                file,
                line_start,
                line_end,
                level: d.severity,
                code: d.rule.unwrap_or_default(),
                message: d.message,
                driver: "python",
            }
        })
        .collect()
}

fn matches_severity(severity: &str) -> bool {
    matches!(severity, "error" | "warning")
}

fn canonicalise_file(file_name: &str, project_root: &Path) -> String {
    let abs = if Path::new(file_name).is_absolute() {
        std::path::PathBuf::from(file_name)
    } else {
        project_root.join(file_name)
    };
    if let Ok(rel) = abs.strip_prefix(project_root) {
        return rel.to_string_lossy().to_string();
    }
    file_name.to_string()
}

#[derive(Debug, Deserialize)]
struct PyrightReport {
    #[serde(rename = "generalDiagnostics", default)]
    general_diagnostics: Vec<PyrightDiag>,
}

#[derive(Debug, Deserialize)]
struct PyrightDiag {
    file: String,
    severity: String,
    message: String,
    #[serde(default)]
    rule: Option<String>,
    range: PyrightRange,
}

#[derive(Debug, Deserialize)]
struct PyrightRange {
    start: PyrightPosition,
    end: PyrightPosition,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct PyrightPosition {
    line: u32,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_basic_diagnostic_doc() {
        let stdout = r#"{
  "version": "1.1.350",
  "time": "now",
  "generalDiagnostics": [
    {
      "file": "/tmp/proj/src/foo.py",
      "severity": "error",
      "message": "Import \"missing\" could not be resolved",
      "rule": "reportMissingImports",
      "range": {
        "start": { "line": 0, "character": 7 },
        "end":   { "line": 0, "character": 14 }
      }
    }
  ],
  "summary": {}
}"#;
        let diags = parse_pyright_output(stdout, Path::new("/tmp/proj"));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "src/foo.py");
        assert_eq!(diags[0].line_start, 1, "0-based line should become 1");
        assert_eq!(diags[0].level, "error");
        assert_eq!(diags[0].code, "reportMissingImports");
        assert_eq!(diags[0].driver, "python");
    }

    #[test]
    fn parse_drops_information_severity() {
        let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "/tmp/proj/src/foo.py",
      "severity": "information",
      "message": "Type narrowing applied",
      "range": { "start": { "line": 5 }, "end": { "line": 5 } }
    },
    {
      "file": "/tmp/proj/src/foo.py",
      "severity": "warning",
      "message": "Unused variable",
      "range": { "start": { "line": 6 }, "end": { "line": 6 } }
    }
  ]
}"#;
        let diags = parse_pyright_output(stdout, Path::new("/tmp/proj"));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, "warning");
    }

    #[test]
    fn parse_returns_empty_on_unparseable_input() {
        assert!(parse_pyright_output("not-json", Path::new("/")).is_empty());
        assert!(parse_pyright_output("", Path::new("/")).is_empty());
    }

    #[test]
    fn parse_handles_missing_rule_field() {
        let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "/tmp/proj/x.py",
      "severity": "error",
      "message": "Generic error",
      "range": { "start": { "line": 0 }, "end": { "line": 0 } }
    }
  ]
}"#;
        let diags = parse_pyright_output(stdout, Path::new("/tmp/proj"));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "");
    }
}
