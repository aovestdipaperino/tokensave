#![cfg(feature = "lang-awk")]

use tokensave::extraction::AwkExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

use crate::common::names_of;

fn extract(source: &str) -> ExtractionResult {
    AwkExtractor.extract("script.awk", source)
}

#[test]
fn file_node_is_emitted() {
    let result = extract("BEGIN { print \"hi\" }");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["script.awk".to_string()]);
}

#[test]
fn extracts_function_definition() {
    let source = "function add(a, b) {\n  return a + b\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["add".to_string()]);
}

#[test]
fn extracts_begin_and_end_blocks() {
    let source = "BEGIN { count = 0 }\nEND { print count }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["BEGIN".to_string(), "END".to_string()]);
}

#[test]
fn extracts_beginfile_and_endfile_blocks() {
    let source = "BEGINFILE { print FILENAME }\nENDFILE { print \"done\" }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["BEGINFILE".to_string(), "ENDFILE".to_string()]);
}

#[test]
fn pattern_action_rule_is_named_after_pattern() {
    let source = "NR == 1 { print $0 }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["NR == 1".to_string()]);
}

#[test]
fn action_only_rule_is_main() {
    let source = "{ print $0 }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["main".to_string()]);
}

#[test]
fn range_pattern_rule_uses_full_pattern_text() {
    let source = "NR == 1, NR == 10 { print $0 }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["NR == 1, NR == 10".to_string()]);
}

#[test]
fn pattern_only_rule_is_emitted() {
    let source = "NR > 5\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["NR > 5".to_string()]);
}

#[test]
fn function_call_emits_unresolved_calls_ref() {
    let source = "function twice(x) {\n  return double(x)\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].reference_name, "double");
    let owner = result.nodes.iter().find(|n| n.name == "twice").unwrap();
    assert_eq!(calls[0].from_node_id, owner.id);
}

#[test]
fn builtin_calls_are_skipped() {
    let source = "{\n  print length($0), substr($0, 1, 3)\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(calls.is_empty(), "unexpected calls: {calls:?}");
}

#[test]
fn realistic_sample() {
    let source = "# Sum the first column.\n\
                  function total(sum, n) {\n\
                  return sum / n\n\
                  }\n\
                  BEGIN { FS = \",\" }\n\
                  NR > 1 { sum += $1; count++ }\n\
                  END { print total(sum, count) }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(
        funcs,
        vec![
            "total".to_string(),
            "BEGIN".to_string(),
            "NR > 1".to_string(),
            "END".to_string()
        ]
    );
    // The END block calls the user function.
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].reference_name, "total");
}

#[test]
fn definitions_are_contained_by_file() {
    let source = "function f() { return 1 }\nBEGIN { print f() }\n";
    let result = extract(source);
    let file = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap();
    let targets: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.source == file.id && e.kind == EdgeKind::Contains)
        .map(|e| e.target.clone())
        .collect();
    assert_eq!(targets.len(), 2);
}

#[test]
fn extensions_are_awk() {
    let ext = AwkExtractor;
    assert_eq!(ext.extensions(), &["awk"]);
}

#[test]
fn language_name_is_awk() {
    let ext = AwkExtractor;
    assert_eq!(ext.language_name(), "AWK");
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = extract("");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
}

#[test]
fn malformed_input_does_not_panic() {
    let result = extract("function foo( { { {");
    assert!(!result.errors.is_empty(), "expected parse errors");
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
}
