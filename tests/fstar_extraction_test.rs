//! F* extractor tests.
//!
//! All assertions run against a single fixture, `tests/fixtures/sample.fst`,
//! which exercises every construct the extractor understands. The fixture is
//! embedded at compile time with `include_str!` (no runtime file I/O, resolved
//! relative to this source file).

use tokensave::extraction::{FStarExtractor, LanguageExtractor};
use tokensave::types::*;

/// The comprehensive F* fixture, embedded at compile time.
const SAMPLE: &str = include_str!("fixtures/sample.fst");

fn sample() -> ExtractionResult {
    FStarExtractor.extract("Demo.fst", SAMPLE)
}

mod common;

/// Sorted names of extracted nodes of the given kind.
fn names_of(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    let mut v = common::names_of(result, kind);
    v.sort();
    v
}

fn names_of_all(result: &ExtractionResult) -> Vec<String> {
    result.nodes.iter().map(|n| n.name.clone()).collect()
}

fn find<'a>(result: &'a ExtractionResult, name: &str) -> &'a Node {
    result
        .nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("node {name} not found; have: {:?}", names_of_all(result)))
}

/// A node looked up by both name and kind (names like `add`/`x` are reused
/// across kinds, e.g. a field vs a function).
fn find_kind<'a>(result: &'a ExtractionResult, name: &str, kind: NodeKind) -> &'a Node {
    result
        .nodes
        .iter()
        .find(|n| n.name == name && n.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "{kind:?} {name} not found; have: {:?}",
                names_of_all(result)
            )
        })
}

fn sig(result: &ExtractionResult, name: &str) -> String {
    find(result, name).signature.clone().unwrap_or_default()
}

fn contains_edge(result: &ExtractionResult, src: &str, tgt: &str) -> bool {
    let s = find(result, src);
    let t = find(result, tgt);
    result
        .edges
        .iter()
        .any(|e| e.source == s.id && e.target == t.id && e.kind == EdgeKind::Contains)
}

fn implements(result: &ExtractionResult, impl_name: &str, class: &str) -> bool {
    let n = find(result, impl_name);
    result.unresolved_refs.iter().any(|r| {
        r.from_node_id == n.id
            && r.reference_kind == EdgeKind::Implements
            && r.reference_name == class
    })
}

// ---------------------------------------------------------------------------
// Extractor metadata (no source needed)
// ---------------------------------------------------------------------------

#[test]
fn extensions_are_fst_and_fsti() {
    assert_eq!(FStarExtractor.extensions(), &["fst", "fsti"]);
}

#[test]
fn language_name_is_fstar() {
    assert_eq!(FStarExtractor.language_name(), "F*");
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = FStarExtractor.extract("Demo.fst", "");
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
    assert!(result.errors.is_empty());
}

// ---------------------------------------------------------------------------
// The fixture parses cleanly and yields exactly the expected symbol sets.
// These two tests are the backstop against phantom nodes from proof-body sugar.
// ---------------------------------------------------------------------------

#[test]
fn fixture_extracts_without_errors() {
    assert!(sample().errors.is_empty(), "errors: {:?}", sample().errors);
}

#[test]
fn function_set_is_exact_no_phantoms() {
    let r = sample();
    let mut expected = vec![
        "sum_with",
        "origin",
        "is_zero",
        "abs",
        "negate",
        "twice",
        "magic",
        "bump",
        "fact_pos",
        "divmod",
        "countdown",
        "+^",
        "let?",
        "even",
        "odd",
        "banner",
        "comm_proof",
        "spec_let",
        "heavy",
        "excluded_middle",
        "try_add",
    ];
    expected.sort_unstable();
    assert_eq!(names_of(&r, NodeKind::Function), expected);
}

#[test]
fn type_like_sets_are_exact() {
    let r = sample();
    assert_eq!(
        names_of(&r, NodeKind::Struct),
        vec!["Out_of_bounds".to_string(), "point".to_string()]
    );
    assert_eq!(
        names_of(&r, NodeKind::Enum),
        vec![
            "forest".to_string(),
            "shape".to_string(),
            "tree".to_string()
        ]
    );
    assert_eq!(
        names_of(&r, NodeKind::TypeAlias),
        vec!["Id".to_string(), "predicate".to_string()]
    );
    assert_eq!(names_of(&r, NodeKind::Trait), vec!["addable".to_string()]);
    assert_eq!(
        names_of(&r, NodeKind::Impl),
        vec!["addable_int".to_string()]
    );
    assert_eq!(
        names_of(&r, NodeKind::Module),
        vec!["Sample.Comprehensive".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Module, open / include / friend, module abbreviations
// ---------------------------------------------------------------------------

#[test]
fn module_is_file_scope_and_parents_children() {
    let r = sample();
    let m = find(&r, "Sample.Comprehensive");
    assert_eq!(m.kind, NodeKind::Module);
    assert!(contains_edge(&r, "Sample.Comprehensive", "point"));
    assert!(contains_edge(&r, "Sample.Comprehensive", "fact_pos"));
}

#[test]
fn open_include_and_abbrev_emit_three_uses_edges() {
    // `open FStar.Mul`, `include FStar.Tactics.V2`, `module L = FStar...`.
    let r = sample();
    let uses = r.edges.iter().filter(|e| e.kind == EdgeKind::Uses).count();
    assert_eq!(uses, 3, "expected open + include + abbrev");
    // The abbreviation does not create a Module node.
    assert!(!r.nodes.iter().any(|n| n.name == "L"));
}

// ---------------------------------------------------------------------------
// Types: records, inductives, mutual recursion, abbreviations
// ---------------------------------------------------------------------------

#[test]
fn record_type_is_struct_with_fields() {
    let r = sample();
    assert_eq!(find(&r, "point").kind, NodeKind::Struct);
    assert!(contains_edge(&r, "point", "x"));
    assert!(contains_edge(&r, "point", "y"));
}

#[test]
fn inductive_type_is_enum_with_constructors() {
    let r = sample();
    assert_eq!(find(&r, "shape").kind, NodeKind::Enum);
    for v in ["Circle", "Rect", "Origin"] {
        assert_eq!(
            find_kind(&r, v, NodeKind::EnumVariant).kind,
            NodeKind::EnumVariant
        );
        assert!(contains_edge(&r, "shape", v));
    }
}

#[test]
fn mutually_recursive_types_via_and() {
    let r = sample();
    assert_eq!(find(&r, "tree").kind, NodeKind::Enum);
    assert_eq!(find(&r, "forest").kind, NodeKind::Enum); // introduced by `and`
}

#[test]
fn type_abbreviation_is_type_alias() {
    assert_eq!(find(&sample(), "predicate").kind, NodeKind::TypeAlias);
}

// ---------------------------------------------------------------------------
// Typeclasses and instances
// ---------------------------------------------------------------------------

#[test]
fn class_is_trait_with_published_and_no_method_fields() {
    let r = sample();
    assert_eq!(find(&r, "addable").kind, NodeKind::Trait);
    // Both the `[@@@no_method]` field and the published method are extracted.
    assert!(contains_edge(&r, "addable", "zero"));
    assert!(contains_edge(&r, "addable", "add"));
}

#[test]
fn instance_is_impl_with_resolved_implements_edge() {
    let r = sample();
    assert_eq!(find(&r, "addable_int").kind, NodeKind::Impl);
    assert!(implements(&r, "addable_int", "addable"));
}

#[test]
fn definition_requiring_typeclass_binder() {
    let r = sample();
    assert_eq!(find(&r, "sum_with").kind, NodeKind::Function);
    assert!(
        sig(&r, "sum_with").contains("{| d : addable a |}"),
        "{}",
        sig(&r, "sum_with")
    );
}

// ---------------------------------------------------------------------------
// let / val / qualifiers / record value
// ---------------------------------------------------------------------------

#[test]
fn record_value_let_is_a_function_not_a_type() {
    let r = sample();
    let origin = find(&r, "origin");
    assert_eq!(origin.kind, NodeKind::Function);
    // Signature stops at `=`; the `{ x = 0; y = 0 }` literal is the body.
    assert_eq!(sig(&r, "origin"), "let origin : point");
}

#[test]
fn val_declaration_is_a_function() {
    let r = sample();
    assert_eq!(find(&r, "is_zero").kind, NodeKind::Function);
    assert_eq!(sig(&r, "is_zero"), "val is_zero : int -> bool");
}

#[test]
fn refinement_type_preserved_in_val_signature() {
    let s = sig(&sample(), "abs");
    assert!(s.contains("y:int{y >= 0"), "{s}");
}

#[test]
fn qualifiers_are_stripped_and_private_sets_visibility() {
    let r = sample();
    // unfold / irreducible / inline_for_extraction are stripped; still Functions.
    assert_eq!(find(&r, "twice").kind, NodeKind::Function);
    assert_eq!(find(&r, "magic").kind, NodeKind::Function);
    // `bump` is `inline_for_extraction private` -> Private.
    assert_eq!(find(&r, "bump").visibility, Visibility::Private);
    assert_eq!(find(&r, "twice").visibility, Visibility::Pub);
}

#[test]
fn plain_let_signature_excludes_body() {
    assert_eq!(sig(&sample(), "negate"), "let negate (x:int) : int");
}

// ---------------------------------------------------------------------------
// Specifications: Lemma / Pure / Tot carry requires / ensures / decreases
// ---------------------------------------------------------------------------

#[test]
fn lemma_signature_has_requires_ensures_decreases() {
    let s = sig(&sample(), "fact_pos");
    assert!(s.contains("Lemma"), "{s}");
    assert!(s.contains("requires n >= 0"), "{s}");
    assert!(s.contains("ensures factorial n >= 1"), "{s}");
    assert!(s.contains("decreases n"), "{s}");
    assert!(!s.contains("if n = 0"), "body leaked: {s}");
}

#[test]
fn pure_signature_has_requires_ensures_decreases() {
    let s = sig(&sample(), "divmod");
    assert!(s.contains("Pure nat"), "{s}");
    assert!(s.contains("requires x >= 0"), "{s}");
    assert!(s.contains("ensures fun r -> r >= 0"), "{s}");
    assert!(s.contains("decreases x"), "{s}");
    assert!(!s.contains("if x < y"), "body leaked: {s}");
}

#[test]
fn tot_signature_has_decreases() {
    let s = sig(&sample(), "countdown");
    assert!(s.contains("Tot nat (decreases n)"), "{s}");
    assert!(!s.contains("countdown (n - 1)"), "body leaked: {s}");
}

// ---------------------------------------------------------------------------
// Operators, binding operators, mutual recursion
// ---------------------------------------------------------------------------

#[test]
fn symbolic_operator_uses_symbol_as_name() {
    assert_eq!(find(&sample(), "+^").kind, NodeKind::Function);
}

#[test]
fn binding_operator_is_a_function() {
    // `( let? )` is a single operator token, not the `let` keyword.
    assert_eq!(find(&sample(), "let?").kind, NodeKind::Function);
}

#[test]
fn mutually_recursive_functions_via_and() {
    let r = sample();
    assert_eq!(find(&r, "even").kind, NodeKind::Function);
    assert_eq!(find(&r, "odd").kind, NodeKind::Function); // introduced by `and`
}

// ---------------------------------------------------------------------------
// exception, effect, assume val
// ---------------------------------------------------------------------------

#[test]
fn exception_is_struct() {
    assert_eq!(find(&sample(), "Out_of_bounds").kind, NodeKind::Struct);
}

#[test]
fn effect_is_type_alias() {
    assert_eq!(find(&sample(), "Id").kind, NodeKind::TypeAlias);
}

#[test]
fn assume_val_is_a_function() {
    let r = sample();
    assert_eq!(find(&r, "excluded_middle").kind, NodeKind::Function);
    assert!(sig(&r, "excluded_middle").starts_with("assume val excluded_middle"));
}

// ---------------------------------------------------------------------------
// Robustness: strings, comments, nested let-in, proof-body sugar, pragmas
// ---------------------------------------------------------------------------

#[test]
fn string_with_braces_is_not_a_record_and_does_not_swallow_next_decl() {
    let r = sample();
    let banner = find(&r, "banner");
    assert_eq!(banner.kind, NodeKind::Function);
    assert_eq!(sig(&r, "banner"), "let banner : string");
    // Regression: a string-valued RHS must not absorb the following decl.
    assert!(r.nodes.iter().any(|n| n.name == "comm_proof"));
}

#[test]
fn proof_body_with_calc_introduce_eliminate_has_no_phantom_nodes() {
    let r = sample();
    let n = find(&r, "comm_proof");
    assert_eq!(n.kind, NodeKind::Function);
    // The proof spans through the calc block.
    assert!(n.end_line > n.start_line);
    // No phantom function from the local `let z` or `introduce` witness `w`.
    let funcs = names_of(&r, NodeKind::Function);
    assert!(!funcs.contains(&"z".to_string()));
    assert!(!funcs.contains(&"w".to_string()));
    // Signature is the spec, not the proof term.
    assert_eq!(
        sig(&r, "comm_proof"),
        "let comm_proof (x y : int) : Lemma (x + y == y + x)"
    );
}

#[test]
fn local_let_in_and_inline_comment_inside_spec() {
    let s = sig(&sample(), "spec_let");
    assert!(s.contains("requires (let p = x > 0 in p)"), "{s}");
    assert!(s.contains("ensures x >= 0"), "{s}");
    assert!(
        !s.contains("mismatched"),
        "comment leaked into signature: {s}"
    );
}

#[test]
fn push_pop_options_pragmas_are_ignored() {
    let r = sample();
    // The decl between #push-options and #pop-options is extracted normally.
    assert_eq!(find(&r, "heavy").kind, NodeKind::Function);
    assert!(sig(&r, "heavy").contains("requires a > 0"));
    // No phantom nodes from any #...-options pragma.
    assert!(!r.nodes.iter().any(|n| n.name.contains("options")));
}

#[test]
fn monadic_let_in_body_is_not_a_top_level_decl() {
    let r = sample();
    let n = find(&r, "try_add");
    assert_eq!(n.kind, NodeKind::Function);
    assert_eq!(
        sig(&r, "try_add"),
        "let try_add (a b : option int) : option int"
    );
}

#[test]
fn doc_comment_is_captured_as_docstring() {
    let d = find(&sample(), "point")
        .docstring
        .clone()
        .unwrap_or_default();
    assert!(d.contains("record type"), "docstring was: {d:?}");
}

// ---------------------------------------------------------------------------
// Span correctness for source retrieval
// ---------------------------------------------------------------------------

#[test]
fn multiline_decl_span_covers_full_spec() {
    let r = sample();
    // `fact_pos` spans its keyword line through the multi-line spec/body.
    let n = find(&r, "fact_pos");
    assert!(
        n.end_line > n.start_line,
        "span did not cover the spec/body"
    );
}
