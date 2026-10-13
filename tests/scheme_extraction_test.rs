#![cfg(feature = "lang-scheme")]

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::SchemeExtractor;
use tokensave::types::*;

use crate::common::names_of;

fn extract(source: &str) -> ExtractionResult {
    SchemeExtractor.extract("main.scm", source)
}

#[test]
fn file_node_is_emitted() {
    let result = extract("(define x 1)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["main.scm".to_string()]);
}

#[test]
fn define_function_form_is_function() {
    let result = extract("(define (add x y) (+ x y))");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["add".to_string()]);
}

#[test]
fn define_variable_form_is_const() {
    let result = extract("(define pi 3.14)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["pi".to_string()]);
}

#[test]
fn define_syntax_is_macro() {
    let source =
        "(define-syntax my-if\n  (syntax-rules ()\n    ((_ c t e) (cond (c t) (else e)))))";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let macros = names_of(&result, NodeKind::Macro);
    assert_eq!(macros, vec!["my-if".to_string()]);
}

#[test]
fn define_library_is_module_with_contained_definitions() {
    let source = "(define-library (my lib)\n  (export square)\n  (begin\n    (define pi 3)\n    (define (square x) (* x x))))";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["my.lib".to_string()]);
    assert_eq!(names_of(&result, NodeKind::Const), vec!["pi".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::Function),
        vec!["square".to_string()]
    );

    let m = result.nodes.iter().find(|n| n.name == "my.lib").unwrap();
    let pi = result.nodes.iter().find(|n| n.name == "pi").unwrap();
    let square = result.nodes.iter().find(|n| n.name == "square").unwrap();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && e.source == m.id)
        .collect();
    assert!(contains.iter().any(|e| e.target == pi.id));
    assert!(contains.iter().any(|e| e.target == square.id));
}

#[test]
fn realistic_sample_emits_each_kind() {
    let source = "(define-library (geometry)\n  (export area)\n  (begin\n    (define pi 3.14159)\n    (define (area r) (* pi r r))\n    (define-syntax unless\n      (syntax-rules ()\n        ((_ c body ...) (if (not c) (begin body ...)))))))";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(
        names_of(&result, NodeKind::Module),
        vec!["geometry".to_string()]
    );
    assert_eq!(names_of(&result, NodeKind::Const), vec!["pi".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::Function),
        vec!["area".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Macro),
        vec!["unless".to_string()]
    );
}

#[test]
fn quoted_define_is_ignored() {
    let result = extract("'(define phantom 1)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(names_of(&result, NodeKind::Const).is_empty());
    assert!(names_of(&result, NodeKind::Function).is_empty());
}

#[test]
fn commented_out_define_is_ignored() {
    let result = extract("#;(define ghost 1)\n; (define specter 2)\n(define real 3)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(names_of(&result, NodeKind::Const), vec!["real".to_string()]);
}

#[test]
fn malformed_input_does_not_panic() {
    let result = extract("(define");
    assert_eq!(
        names_of(&result, NodeKind::File),
        vec!["main.scm".to_string()]
    );
    let result = extract("(define (broken");
    assert_eq!(
        names_of(&result, NodeKind::File),
        vec!["main.scm".to_string()]
    );
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = extract("");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
}

#[test]
fn function_body_records_call_refs() {
    let result = extract("(define (f x) (g x))");
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(calls.iter().any(|r| r.reference_name == "g"));
}

#[test]
fn extensions_are_scm_and_ss() {
    let ext = SchemeExtractor;
    assert_eq!(ext.extensions(), &["scm", "ss"]);
}

#[test]
fn language_name_is_scheme() {
    let ext = SchemeExtractor;
    assert_eq!(ext.language_name(), "Scheme");
}
