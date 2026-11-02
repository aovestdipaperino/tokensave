#![cfg(feature = "lang-commonlisp")]

use tokensave::extraction::CommonLispExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

use crate::common::names_of;

fn extract(source: &str) -> ExtractionResult {
    CommonLispExtractor.extract("sample.lisp", source)
}

#[test]
fn file_node_is_emitted() {
    let result = extract("(defun foo (x) x)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["sample.lisp".to_string()]);
}

#[test]
fn extracts_defun_as_function() {
    let result = extract("(defun add (x y)\n  (+ x y))");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["add".to_string()]);
}

#[test]
fn extracts_defmacro_as_function() {
    let result = extract("(defmacro with-thing ((var) &body body)\n  `(let ((,var 1)) ,@body))");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["with-thing".to_string()]);
}

#[test]
fn defvar_and_defparameter_are_static() {
    let result = extract("(defvar *count* 0)\n(defparameter *limit* 10)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let statics = names_of(&result, NodeKind::Static);
    assert_eq!(statics, vec!["*count*".to_string(), "*limit*".to_string()]);
}

#[test]
fn defconstant_is_const() {
    let result = extract("(defconstant +pi+ 3.14159)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["+pi+".to_string()]);
}

#[test]
fn defclass_and_defstruct_are_class() {
    let result =
        extract("(defclass person ()\n  ((name :initarg :name)))\n(defstruct point (x 0) (y 0))");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes = names_of(&result, NodeKind::Class);
    assert_eq!(classes, vec!["person".to_string(), "point".to_string()]);
}

#[test]
fn defpackage_and_in_package_are_modules() {
    let result = extract("(defpackage :my-app\n  (:use :cl))\n(in-package :my-app)");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["my-app".to_string(), "my-app".to_string()]);
}

#[test]
fn defgeneric_is_function_and_defmethod_is_method() {
    let result = extract(
        "(defgeneric describe-thing (thing))\n(defmethod describe-thing ((thing person))\n  (name thing))",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["describe-thing".to_string()]);
    let methods = names_of(&result, NodeKind::Method);
    assert_eq!(methods, vec!["describe-thing".to_string()]);
}

#[test]
fn realistic_sample_emits_each_kind() {
    let source = "(defpackage :shapes\n  (:use :cl))\n\
                  (in-package :shapes)\n\
                  (defvar *sides* 4)\n\
                  (defconstant +unit+ 1)\n\
                  (defclass rectangle ()\n  ((width :initarg :width)))\n\
                  (defun area (r)\n  (* (width r) (width r)))\n\
                  (defmacro with-rect ((r) &body body)\n  `(let ((,r (make-instance 'rectangle))) ,@body))\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(
        names_of(&result, NodeKind::Module),
        vec!["shapes".to_string(), "shapes".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Static),
        vec!["*sides*".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Const),
        vec!["+unit+".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Class),
        vec!["rectangle".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Function),
        vec!["area".to_string(), "with-rect".to_string()]
    );
}

#[test]
fn definitions_are_contained_by_file() {
    let result = extract("(defun foo (x) x)\n(defvar *bar* 1)");
    let file_id = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .map(|n| n.id.clone())
        .unwrap_or_default();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && e.source == file_id)
        .map(|e| e.target.clone())
        .collect();
    assert_eq!(contains.len(), 2);
    for name in ["foo", "*bar*"] {
        let id = result
            .nodes
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.id.clone())
            .unwrap_or_default();
        assert!(contains.contains(&id), "missing Contains edge for {name}");
    }
}

#[test]
fn extensions_are_lisp_variants() {
    let ext = CommonLispExtractor;
    assert_eq!(ext.extensions(), &["lisp", "lsp", "cl", "asd"]);
}

#[test]
fn language_name_is_common_lisp() {
    let ext = CommonLispExtractor;
    assert_eq!(ext.language_name(), "Common Lisp");
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
    let result = extract("(defun broken (x y");
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["sample.lisp".to_string()]);
}
