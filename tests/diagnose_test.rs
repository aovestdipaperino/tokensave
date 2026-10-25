use tokensave::diagnose::{parse_cargo_output, Severity};

// ---------------------------------------------------------------------------
// Parse header tests
// ---------------------------------------------------------------------------

#[test]
fn parses_error_with_code() {
    let input = "error[E0308]: mismatched types\n  --> src/lib.rs:42:10\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].code.as_deref(), Some("E0308"));
    assert_eq!(diags[0].message, "mismatched types");
}

#[test]
fn parses_warning_without_code() {
    let input = "warning: unused variable\n  --> src/main.rs:10:5\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert!(diags[0].code.is_none());
    assert_eq!(diags[0].message, "unused variable");
}

#[test]
fn parses_note() {
    let input = "note: variable moved here\n  --> src/a.rs:5:8\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Note);
}

#[test]
fn parses_help() {
    let input = "help: consider borrowing here\n  --> src/a.rs:5:8\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Help);
}

#[test]
fn ignores_unknown_severity() {
    let input = "unknown_severity: something\n  --> src/a.rs:5:8\n";
    let diags = parse_cargo_output(input);
    assert!(diags.is_empty());
}

#[test]
fn clippy_lint_code() {
    let input = "error[clippy::redundant_closure]: redundant closure\n  --> src/lib.rs:10:20\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("clippy::redundant_closure"));
}

// ---------------------------------------------------------------------------
// Parse span tests
// ---------------------------------------------------------------------------

#[test]
fn span_with_drive_letter() {
    // Windows-style path with drive letter and colons
    let input = "error[E0308]: type mismatch\n  --> C:\\Users\\dev\\src\\lib.rs:42:10\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].file, "C:\\Users\\dev\\src\\lib.rs");
    assert_eq!(diags[0].line, 42);
    assert_eq!(diags[0].column, 10);
}

#[test]
fn span_with_path_containing_colons() {
    // File path with colons (Windows drive letter or non-standard paths)
    let input = "warning: deprecated\n  --> D:/projects/rust/src/main.rs:15:3\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].line, 15);
    assert_eq!(diags[0].column, 3);
}

#[test]
fn span_file_with_single_letter_path() {
    let input = "error[E0001]: oops\n  --> a:1:1\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].file, "a");
    assert_eq!(diags[0].line, 1);
    assert_eq!(diags[0].column, 1);
}

// ---------------------------------------------------------------------------
// Multi-diagnostic tests
// ---------------------------------------------------------------------------

#[test]
fn multiple_diagnostics_with_source_context() {
    let input = "\
error[E0382]: borrow of moved value: x
  --> src/a.rs:10:5
   |
10 |     let x = String::from(\"hi\");
   |         - move occurs because x has type String
11 |     println!(\"{}\", x);
   |                     ^ value borrowed here after move
   |
warning: unused variable: y
  --> src/b.rs:20:9
   |
20 |     let y = 42;
   |         ^
   |
";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].code.as_deref(), Some("E0382"));
    assert_eq!(diags[0].file, "src/a.rs");
    assert_eq!(diags[1].severity, Severity::Warning);
    assert_eq!(diags[1].file, "src/b.rs");
}

#[test]
fn diagnostic_without_span_is_dropped() {
    let input = "\
error: could not compile `foo` due to 2 previous errors
note: run with `RUST_BACKTRACE=1` for a backtrace
";
    let diags = parse_cargo_output(input);
    assert!(diags.is_empty());
}

#[test]
fn empty_input() {
    let diags = parse_cargo_output("");
    assert!(diags.is_empty());
}

#[test]
fn only_source_lines_no_header() {
    let input = "\
   |
42 |     let x = 1;
   |         ^
";
    let diags = parse_cargo_output(input);
    assert!(diags.is_empty());
}

#[test]
fn header_far_from_span_is_dropped() {
    // When a header and its span are more than 12 lines apart, the span
    // isn't found and the diagnostic is dropped.
    let mut input = String::from("error[E0001]: far away\n");
    for _ in 0..15 {
        input.push_str("   |\n");
    }
    input.push_str("  --> src/far.rs:1:1\n");
    let diags = parse_cargo_output(&input);
    assert!(
        diags.is_empty(),
        "span too far from header should be dropped"
    );
}

#[test]
fn intermediate_header_breaks_search() {
    // When another header line appears before the span, the search stops.
    let input = "\
error[E0001]: first
warning: second
  --> src/a.rs:1:1
";
    let diags = parse_cargo_output(input);
    // "error[E0001]: first" has no span before the next header, so dropped.
    // "warning: second" DOES have a span on the next line.
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warning);
}

#[test]
fn ansi_escape_in_header() {
    // Cargo with --color=always may emit ANSI escape sequences.
    // The parser strips the leading reset sequence.
    let input = "\u{1b}[0merror[E0308]: mismatched types\n  --> src/lib.rs:42:10\n";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].code.as_deref(), Some("E0308"));
}

#[test]
fn diagnostic_struct_fields() {
    let input = "\
error[E0507]: cannot move out of borrowed content
  --> src/foo.rs:99:15
";
    let diags = parse_cargo_output(input);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.code.as_deref(), Some("E0507"));
    assert_eq!(d.message, "cannot move out of borrowed content");
    assert_eq!(d.file, "src/foo.rs");
    assert_eq!(d.line, 99);
    assert_eq!(d.column, 15);
}
