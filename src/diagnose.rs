//! Parser for `cargo check` / `cargo clippy` stderr output.
//!
//! Extracts structured diagnostics — severity, optional error code, message,
//! and primary source location — from the human-readable text that the
//! `rustc` / `clippy` toolchain emits. Used by the `tokensave_diagnose` MCP
//! tool to map each diagnostic to a graph node and pre-attach the relevant
//! callers/callees.
//!
//! The parser is intentionally lenient: it scans line-by-line and silently
//! skips anything it doesn't recognise. Diagnostics that don't carry a
//! `--> file:line:col` span (e.g. summary errors, "could not compile" tails)
//! are dropped — they have no source location to map.

use serde::{Deserialize, Serialize};

/// Severity of a parsed diagnostic. Matches what `rustc`/`clippy` emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl Severity {
    fn parse(token: &str) -> Option<Severity> {
        match token {
            "error" => Some(Severity::Error),
            "warning" => Some(Severity::Warning),
            "note" => Some(Severity::Note),
            "help" => Some(Severity::Help),
            _ => None,
        }
    }
}

/// One diagnostic with a resolved primary source location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Optional error code, e.g. `E0308`, or a clippy lint name like
    /// `clippy::redundant_closure`. `None` when the diagnostic carried no code.
    pub code: Option<String>,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Parses raw cargo / rustc / clippy stderr text into structured diagnostics.
///
/// Diagnostics without a `--> file:line:col` span are dropped — they cannot
/// be mapped to a graph node and would only add noise. Filtering of which
/// severities to keep is the caller's responsibility.
pub fn parse_cargo_output(text: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some((severity, code, message)) = parse_header(lines[i]) {
            // Look for the next `--> file:line:col` within the diagnostic
            // block. Bound the search to ~12 lines so we don't accidentally
            // attach a span from an unrelated downstream diagnostic.
            let mut span = None;
            let lookahead_end = (i + 12).min(lines.len());
            for line in &lines[i + 1..lookahead_end] {
                if let Some(s) = parse_span(line) {
                    span = Some(s);
                    break;
                }
                if parse_header(line).is_some() {
                    break;
                }
            }
            if let Some((file, line, column)) = span {
                out.push(Diagnostic {
                    severity,
                    code,
                    message,
                    file,
                    line,
                    column,
                });
            }
        }
        i += 1;
    }
    out
}

/// Parses a diagnostic header line:
///   `error[E0308]: mismatched types`
///   `warning: unused variable`
///   `error: useless conversion ...`
fn parse_header(line: &str) -> Option<(Severity, Option<String>, String)> {
    // ANSI escapes can appear when cargo is run with `--color=always`; strip
    // a leading reset sequence if present. We don't bother with full ANSI
    // stripping — the typical input is plain text.
    let line = line.trim_start_matches("\u{1b}[0m");

    // Find the first colon. Severity is everything before it (optionally
    // followed by `[CODE]`). Message is everything after.
    let (head, rest) = line.split_once(": ")?;
    let (sev_token, code) = if let Some(idx) = head.find('[') {
        if !head.ends_with(']') {
            return None;
        }
        let sev = &head[..idx];
        let code = &head[idx + 1..head.len() - 1];
        (sev, Some(code.to_string()))
    } else {
        (head, None)
    };
    let severity = Severity::parse(sev_token.trim())?;
    Some((severity, code, rest.trim().to_string()))
}

/// Parses a primary-span line:
///   `  --> src/foo.rs:42:10`
/// (with arbitrary leading whitespace, sometimes a leading `::: ` for
/// secondary spans which we ignore here).
fn parse_span(line: &str) -> Option<(String, u32, u32)> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("--> ")?;
    // Split on the last two `:`s — the file path itself may contain `:`
    // on Windows (drive letter), so working from the right is safer.
    let (file_and_line, col_str) = rest.rsplit_once(':')?;
    let (file, line_str) = file_and_line.rsplit_once(':')?;
    let line_num: u32 = line_str.parse().ok()?;
    let col_num: u32 = col_str.parse().ok()?;
    Some((file.to_string(), line_num, col_num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rustc_typed_error() {
        let input = "\
error[E0308]: mismatched types
  --> src/lib.rs:42:10
   |
42 |     let x: u32 = \"hello\";
   |          ---   ^^^^^^^ expected `u32`, found `&str`
";
        let diags = parse_cargo_output(input);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.code.as_deref(), Some("E0308"));
        assert_eq!(d.message, "mismatched types");
        assert_eq!(d.file, "src/lib.rs");
        assert_eq!(d.line, 42);
        assert_eq!(d.column, 10);
    }

    #[test]
    fn parses_clippy_warning_without_code() {
        let input = "\
warning: redundant closure
  --> src/main.rs:38:56
   |
";
        let diags = parse_cargo_output(input);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].code.is_none());
        assert_eq!(diags[0].line, 38);
        assert_eq!(diags[0].column, 56);
    }

    #[test]
    fn ignores_unanchored_summary_errors() {
        // Cargo's tail line has no span and must not be reported.
        let input = "error: could not compile `tokensave` (lib) due to 43 previous errors";
        let diags = parse_cargo_output(input);
        assert!(diags.is_empty());
    }

    #[test]
    fn parses_multiple_in_one_block() {
        let input = "\
error[E0382]: borrow of moved value: `x`
  --> src/a.rs:10:5
   |
warning: unused variable: `y`
  --> src/b.rs:20:9
   |
";
        let diags = parse_cargo_output(input);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].file, "src/a.rs");
        assert_eq!(diags[1].severity, Severity::Warning);
        assert_eq!(diags[1].file, "src/b.rs");
    }

    #[test]
    fn header_without_following_span_is_dropped() {
        // Cargo sometimes emits headers without a `-->` (e.g. final summary).
        let input = "\
error: aborting due to previous error
note: For more information about this error, try `rustc --explain E0308`.
";
        let diags = parse_cargo_output(input);
        assert!(diags.is_empty());
    }
}
