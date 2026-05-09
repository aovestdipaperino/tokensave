// Rust guideline compliant 2025-10-17
//! `tsc --pretty false --noEmit` driver.
//!
//! tsc emits diagnostics as one-per-line text:
//!
//! ```text
//! src/lib.ts(4,15): error TS2322: Type 'string' is not assignable to type 'number'.
//! ```
//!
//! The parser extracts file, line, column, level, code, and message from
//! that shape. Multi-line `error: …` continuations are concatenated into
//! the prior diagnostic. We don't follow `tsc --build` references because
//! the resolver only ever asks for definitions on the explicitly opened
//! tsconfig.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;

use crate::diagnostics::{Diagnostic, Driver, Scope};
use crate::errors::Result;

pub struct TscDriver;

impl Driver for TscDriver {
    fn name(&self) -> &'static str {
        "typescript"
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("tsconfig.json").exists()
    }

    fn run<'a>(
        &'a self,
        project_root: &'a Path,
        _scope: &'a Scope,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Diagnostic>>> + Send + 'a>> {
        Box::pin(async move {
            let mut cmd = tokio::process::Command::new("tsc");
            cmd.arg("--noEmit")
                .arg("--pretty")
                .arg("false")
                .current_dir(project_root)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true);

            // Spawn errors (tsc not on PATH) fall through to empty rather
            // than killing the call. Same reasoning as the Python driver:
            // a tsconfig.json's presence doesn't guarantee tsc is installed,
            // and a Rust project with a JS sibling shouldn't be punished.
            let output = match cmd.output().await {
                Ok(o) => o,
                Err(_) => return Ok(Vec::new()),
            };

            // tsc exits with status 1 when there are errors; status 0 means
            // "no diagnostics." Either way, stdout has the diagnostic stream.
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(parse_tsc_output(&stdout))
        })
    }
}

/// Parse the full tsc stdout into a flat diagnostic list. Top-level so it
/// can be unit-tested without spawning tsc.
pub fn parse_tsc_output(stdout: &str) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for line in stdout.lines() {
        if let Some(diag) = parse_tsc_line(line) {
            out.push(diag);
            continue;
        }
        // Continuation line: append to the prior diagnostic if its text
        // doesn't start a new file(line,col): error pattern.
        if let Some(last) = out.last_mut() {
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                last.message.push(' ');
                last.message.push_str(trimmed);
            }
        }
    }
    out
}

/// Parse a single tsc output line into a `Diagnostic`. Returns `None` for
/// non-diagnostic lines (banners, summary lines, blanks).
pub fn parse_tsc_line(line: &str) -> Option<Diagnostic> {
    // file(line,col): level TSnnnn: message
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    let after = &line[close + 1..];
    let after = after.strip_prefix(':')?.trim_start();

    let file = line[..open].to_string();
    let location = &line[open + 1..close];
    let mut parts = location.splitn(2, ',');
    let line_no: u32 = parts.next()?.trim().parse().ok()?;
    let _col: u32 = parts.next()?.trim().parse().unwrap_or(0);

    // after = "error TS2322: Type ..." or "warning TS####: ..."
    let mut tokens = after.splitn(3, ' ');
    let level = tokens.next()?.to_string();
    if !matches!(level.as_str(), "error" | "warning") {
        return None;
    }
    let code_token = tokens.next()?;
    let message = tokens.next()?.trim().to_string();
    let code = code_token.trim_end_matches(':').to_string();

    Some(Diagnostic {
        file,
        line_start: line_no,
        line_end: line_no,
        level,
        code,
        message,
        driver: "typescript",
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_error_line() {
        let line =
            "src/lib.ts(4,15): error TS2322: Type 'string' is not assignable to type 'number'.";
        let d = parse_tsc_line(line).expect("should parse");
        assert_eq!(d.file, "src/lib.ts");
        assert_eq!(d.line_start, 4);
        assert_eq!(d.level, "error");
        assert_eq!(d.code, "TS2322");
        assert!(d.message.contains("not assignable"));
        assert_eq!(d.driver, "typescript");
    }

    #[test]
    fn parse_warning_line() {
        let line = "src/foo.ts(10,1): warning TS6133: 'x' is declared but its value is never read.";
        let d = parse_tsc_line(line).expect("should parse");
        assert_eq!(d.level, "warning");
        assert_eq!(d.code, "TS6133");
    }

    #[test]
    fn parse_returns_none_for_blank_lines() {
        assert!(parse_tsc_line("").is_none());
        assert!(parse_tsc_line("   ").is_none());
    }

    #[test]
    fn parse_returns_none_for_summary_line() {
        // tsc summary lines like "Found 3 errors."
        assert!(parse_tsc_line("Found 3 errors.").is_none());
    }

    #[test]
    fn parse_continuation_appends_to_prior() {
        let stdout = "src/a.ts(1,1): error TS2322: Outer message.\n  Inner detail line.\n";
        let diags = parse_tsc_output(stdout);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Outer message"));
        assert!(diags[0].message.contains("Inner detail"));
    }

    #[test]
    fn parse_multiple_diagnostics() {
        let stdout = "\
src/a.ts(1,1): error TS2322: First.
src/b.ts(2,2): warning TS6133: Second.
";
        let diags = parse_tsc_output(stdout);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].file, "src/a.ts");
        assert_eq!(diags[1].level, "warning");
    }
}
