use tokensave::diagnostics::python::parse_pyright_output;
use tokensave::diagnostics::typescript::{parse_tsc_line, parse_tsc_output};

// ---------------------------------------------------------------------------
// Pyright (Python) diagnostics parser tests
// ---------------------------------------------------------------------------

#[test]
fn pyright_empty_diagnostics_array() {
    let stdout = r#"{"generalDiagnostics": []}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert!(diags.is_empty());
}

#[test]
fn pyright_missing_general_diagnostics_field() {
    let stdout = r#"{"version": "1.1.350"}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert!(diags.is_empty());
}

#[test]
fn pyright_multiple_diagnostics_preserved() {
    let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "/tmp/proj/src/a.py",
      "severity": "error",
      "message": "First",
      "rule": "reportGeneralTypeIssues",
      "range": { "start": { "line": 0 }, "end": { "line": 0 } }
    },
    {
      "file": "/tmp/proj/src/b.py",
      "severity": "warning",
      "message": "Second",
      "rule": "reportUnusedVariable",
      "range": { "start": { "line": 10 }, "end": { "line": 10 } }
    }
  ]
}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].file, "src/a.py");
    assert_eq!(diags[1].file, "src/b.py");
    assert_eq!(diags[0].level, "error");
    assert_eq!(diags[1].level, "warning");
    assert_eq!(
        diags[1].line_start, 11,
        "0-based line 10 should become 1-based 11"
    );
}

#[test]
fn pyright_relative_path_passes_through() {
    let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "relative/path/file.py",
      "severity": "error",
      "message": "Missing import",
      "rule": "reportMissingImports",
      "range": { "start": { "line": 5 }, "end": { "line": 5 } }
    }
  ]
}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].file, "relative/path/file.py");
}

#[test]
fn pyright_path_outside_project_root() {
    let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "/other/project/file.py",
      "severity": "warning",
      "message": "Unused import",
      "rule": "reportUnusedImport",
      "range": { "start": { "line": 3 }, "end": { "line": 3 } }
    }
  ]
}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].file, "/other/project/file.py");
}

#[test]
fn pyright_filters_error_and_warning_only() {
    let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "/tmp/proj/x.py",
      "severity": "error",
      "message": "Err",
      "range": { "start": { "line": 1 }, "end": { "line": 1 } }
    },
    {
      "file": "/tmp/proj/x.py",
      "severity": "warning",
      "message": "Warn",
      "range": { "start": { "line": 2 }, "end": { "line": 2 } }
    },
    {
      "file": "/tmp/proj/x.py",
      "severity": "information",
      "message": "Info",
      "range": { "start": { "line": 3 }, "end": { "line": 3 } }
    },
    {
      "file": "/tmp/proj/x.py",
      "severity": "hint",
      "message": "Hint",
      "range": { "start": { "line": 4 }, "end": { "line": 4 } }
    }
  ]
}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert_eq!(diags.len(), 2, "only error and warning should be kept");
    assert_eq!(diags[0].level, "error");
    assert_eq!(diags[1].level, "warning");
}

#[test]
fn pyright_driver_label_is_python() {
    let stdout = r#"{
  "generalDiagnostics": [
    {
      "file": "/tmp/proj/x.py",
      "severity": "error",
      "message": "Boom",
      "range": { "start": { "line": 0 }, "end": { "line": 0 } }
    }
  ]
}"#;
    let diags = parse_pyright_output(stdout, std::path::Path::new("/tmp/proj"));
    assert_eq!(diags[0].driver, "python");
}

// ---------------------------------------------------------------------------
// TypeScript (tsc) diagnostics parser tests — line-level
// ---------------------------------------------------------------------------

#[test]
fn tsc_line_error_with_code() {
    let line = "src/lib.ts(4,15): error TS2322: Type 'string' is not assignable to type 'number'.";
    let d = parse_tsc_line(line).expect("should parse error");
    assert_eq!(d.file, "src/lib.ts");
    assert_eq!(d.line_start, 4);
    assert_eq!(d.level, "error");
    assert_eq!(d.code, "TS2322");
    assert!(d.message.contains("not assignable"));
    assert_eq!(d.driver, "typescript");
}

#[test]
fn tsc_line_warning_with_code() {
    let line = "src/foo.ts(10,1): warning TS6133: 'x' is declared but its value is never read.";
    let d = parse_tsc_line(line).expect("should parse warning");
    assert_eq!(d.level, "warning");
    assert_eq!(d.code, "TS6133");
    assert_eq!(d.line_start, 10);
}

#[test]
fn tsc_line_without_code_number() {
    // Some tsc output lines may have different code formats
    let line =
        "src/app.ts(1,8): error TS18003: No inputs were found in config file 'tsconfig.json'.";
    let d = parse_tsc_line(line).expect("should parse TS code with 5 digits");
    assert_eq!(d.code, "TS18003");
    assert_eq!(d.line_start, 1);
}

#[test]
fn tsc_line_message_with_colons() {
    let line = "src/utils.ts(5,10): error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.";
    let d = parse_tsc_line(line).expect("should parse message with colons");
    assert_eq!(d.code, "TS2345");
    assert!(d.message.contains("Argument of type"));
    assert!(d.message.contains("parameter of type"));
}

#[test]
fn tsc_line_path_with_parentheses() {
    // Edge case: file path itself contains parentheses (unusual but possible)
    // The parser uses the first '(' for the position, so this will misparse.
    // Documenting the current behavior.
    let line = "src/lib.ts(4,15): error TS2322: ok";
    let d = parse_tsc_line(line).expect("normal case should work");
    assert_eq!(d.file, "src/lib.ts");
    assert_eq!(d.line_start, 4);
}

#[test]
fn tsc_line_blank_returns_none() {
    assert!(parse_tsc_line("").is_none());
    assert!(parse_tsc_line("   ").is_none());
    assert!(parse_tsc_line("\t").is_none());
}

#[test]
fn tsc_line_non_diagnostic_returns_none() {
    assert!(parse_tsc_line("Found 3 errors.").is_none());
    assert!(parse_tsc_line("src/lib.ts:42:1 - error TS2322: Type 'string'").is_none());
    assert!(parse_tsc_line("This is just some text").is_none());
}

#[test]
fn tsc_line_unexpected_level_returns_none() {
    // Levels other than "error" or "warning" should be rejected
    assert!(parse_tsc_line("src/x.ts(1,1): info TS9999: Something happened").is_none());
    assert!(parse_tsc_line("src/x.ts(1,1): note TS0000: Just a note").is_none());
}

// ---------------------------------------------------------------------------
// TypeScript (tsc) diagnostics parser tests — full output
// ---------------------------------------------------------------------------

#[test]
fn tsc_output_empty_string() {
    assert!(parse_tsc_output("").is_empty());
}

#[test]
fn tsc_output_single_error() {
    let stdout = "src/a.ts(1,1): error TS2322: First.\n";
    let diags = parse_tsc_output(stdout);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].file, "src/a.ts");
    assert_eq!(diags[0].code, "TS2322");
}

#[test]
fn tsc_output_multiple_errors_and_warnings() {
    let stdout = "\
src/a.ts(1,1): error TS2322: First error.
src/b.ts(2,2): warning TS6133: First warning.
src/c.ts(3,3): error TS2345: Second error.
";
    let diags = parse_tsc_output(stdout);
    assert_eq!(diags.len(), 3);
    assert_eq!(diags[0].file, "src/a.ts");
    assert_eq!(diags[0].level, "error");
    assert_eq!(diags[1].file, "src/b.ts");
    assert_eq!(diags[1].level, "warning");
    assert_eq!(diags[2].file, "src/c.ts");
    assert_eq!(diags[2].level, "error");
}

#[test]
fn tsc_output_continuation_lines() {
    let stdout = "\
src/a.ts(1,5): error TS2322: Type 'string' is not assignable.
  'string' is the expected type here.
  The type originates from this expression.
src/b.ts(2,2): warning TS6133: 'x' is unused.
";
    let diags = parse_tsc_output(stdout);
    assert_eq!(diags.len(), 2);
    assert!(diags[0].message.contains("Type 'string' is not assignable"));
    assert!(diags[0].message.contains("the expected type here"));
    assert!(diags[0].message.contains("this expression"));
    assert_eq!(diags[1].message, "'x' is unused.");
}

#[test]
fn tsc_output_continuation_with_empty_lines() {
    let stdout = "\
src/a.ts(1,5): error TS2322: First message.

  Continuation after blank line.
";
    let diags = parse_tsc_output(stdout);
    assert_eq!(diags.len(), 1);
    // The blank line is not appended (it's empty after trim)
    assert!(diags[0].message.contains("Continuation after blank line"));
}

#[test]
fn tsc_output_all_drivers_labeled_typescript() {
    let stdout = "\
src/a.ts(1,1): error TS2322: A.
src/b.ts(2,2): warning TS6133: B.
";
    let diags = parse_tsc_output(stdout);
    for d in &diags {
        assert_eq!(d.driver, "typescript");
    }
}

#[test]
fn tsc_output_line_end_equals_line_start() {
    let stdout = "src/a.ts(42,10): error TS2322: Single-line error.\n";
    let diags = parse_tsc_output(stdout);
    assert_eq!(diags[0].line_start, 42);
    assert_eq!(diags[0].line_end, 42);
}

#[test]
fn tsc_output_summary_and_banner_lines_ignored() {
    let stdout = "\
Version 5.4.0
src/a.ts(1,1): error TS2322: The error.
Found 1 error in src/a.ts:1
";
    let diags = parse_tsc_output(stdout);
    assert_eq!(diags.len(), 1, "only the diagnostic line should be parsed");
    assert_eq!(diags[0].file, "src/a.ts");
}
