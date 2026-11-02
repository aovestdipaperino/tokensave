#![cfg(feature = "lang-wolfram")]

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::WolframExtractor;
use tokensave::types::*;

use crate::common::names_of;

fn extract(source: &str) -> ExtractionResult {
    WolframExtractor.extract("sample.wl", source)
}

#[test]
fn file_node_is_emitted() {
    let result = extract("f[x_] := x + 1");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["sample.wl".to_string()]);
}

#[test]
fn setdelayed_call_is_function() {
    let result = extract("f[x_] := x + 1");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["f".to_string()]);
    assert!(names_of(&result, NodeKind::Const).is_empty());
}

#[test]
fn set_call_is_function() {
    let result = extract("f[x_] = x + 1");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["f".to_string()]);
}

#[test]
fn plain_assignment_is_const() {
    let result = extract("answer = 42");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["answer".to_string()]);
    assert!(names_of(&result, NodeKind::Function).is_empty());
}

#[test]
fn delayed_symbol_assignment_is_const() {
    let result = extract("x := 42");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["x".to_string()]);
}

#[test]
fn multiarg_function_keeps_head_name_and_signature() {
    let result = extract("add[x_, y_] := x + y");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["add".to_string()]);
    let node = result
        .nodes
        .iter()
        .find(|n| n.name == "add")
        .expect("add node");
    assert_eq!(node.signature.as_deref(), Some("add[x_, y_]"));
}

#[test]
fn realistic_sample() {
    let source = "(* Simple statistics helpers. *)\n\
                  mean[xs_List] := Total[xs]/Length[xs]\n\
                  variance[xs_List] := mean[(# - mean[xs])^2 & /@ xs]\n\
                  defaultTrials = 1000\n\
                  runAll[n_] := Table[variance[RandomReal[1, n]], {defaultTrials}]\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(
        funcs,
        vec![
            "mean".to_string(),
            "variance".to_string(),
            "runAll".to_string()
        ]
    );
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["defaultTrials".to_string()]);
}

#[test]
fn body_calls_are_recorded() {
    let result = extract("f[x_] := g[x] + h[2]");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mut calls: Vec<String> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.clone())
        .collect();
    calls.sort();
    assert_eq!(calls, vec!["g".to_string(), "h".to_string()]);
}

#[test]
fn defined_head_is_not_a_call() {
    let result = extract("f[x_] := x");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(result.unresolved_refs.is_empty());
}

#[test]
fn part_assignment_defines_nothing() {
    let result = extract("a[[1]] = 5");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(names_of(&result, NodeKind::Function).is_empty());
    assert!(names_of(&result, NodeKind::Const).is_empty());
}

#[test]
fn leading_comment_becomes_docstring() {
    let result = extract("(* Computes f. *)\nf[x_] := x");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let node = result.nodes.iter().find(|n| n.name == "f").expect("f node");
    assert!(
        node.docstring
            .as_deref()
            .unwrap_or("")
            .contains("Computes f."),
        "docstring: {:?}",
        node.docstring
    );
}

#[test]
fn definitions_are_contained_by_file() {
    let result = extract("f[x_] := x\nanswer = 42\n");
    let file = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .expect("file node");
    let f = result.nodes.iter().find(|n| n.name == "f").expect("f node");
    let answer = result
        .nodes
        .iter()
        .find(|n| n.name == "answer")
        .expect("answer node");
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(contains
        .iter()
        .any(|e| e.source == file.id && e.target == f.id));
    assert!(contains
        .iter()
        .any(|e| e.source == file.id && e.target == answer.id));
}

#[test]
fn extensions_are_wl_wls_wlt() {
    let ext = WolframExtractor;
    assert_eq!(ext.extensions(), &["wl", "wls", "wlt"]);
}

#[test]
fn language_name_is_wolfram() {
    let ext = WolframExtractor;
    assert_eq!(ext.language_name(), "Wolfram");
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
    let result = extract("f[x_ := ");
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["sample.wl".to_string()]);
}
