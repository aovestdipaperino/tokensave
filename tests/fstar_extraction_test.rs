use tokensave::extraction::FStarExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    FStarExtractor.extract("Demo.fst", source)
}

fn names_of(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    let mut v: Vec<String> = result
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.name.clone())
        .collect();
    v.sort();
    v
}

fn find<'a>(result: &'a ExtractionResult, name: &str) -> &'a Node {
    result
        .nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("node {name} not found; have: {:?}", names_of_all(result)))
}

fn names_of_all(result: &ExtractionResult) -> Vec<String> {
    result.nodes.iter().map(|n| n.name.clone()).collect()
}

fn contains_edge(result: &ExtractionResult, src: &str, tgt: &str) -> bool {
    let s = find(result, src);
    let t = find(result, tgt);
    result
        .edges
        .iter()
        .any(|e| e.source == s.id && e.target == t.id && e.kind == EdgeKind::Contains)
}

// ---------------------------------------------------------------------------
// Basic metadata
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
    let result = extract("");
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
    assert!(result.errors.is_empty());
}

// ---------------------------------------------------------------------------
// Module, open, include, module abbreviations
// ---------------------------------------------------------------------------

#[test]
fn module_declaration_is_module_and_scopes_children() {
    let source = "module FStar.Demo\n\nlet x = 1\n";
    let result = extract(source);
    let m = find(&result, "FStar.Demo");
    assert_eq!(m.kind, NodeKind::Module);
    assert_eq!(m.qualified_name, "Demo.fst::FStar.Demo");
    // `x` is parented to the module, not the file.
    assert!(contains_edge(&result, "FStar.Demo", "x"));
    let x = find(&result, "x");
    assert_eq!(x.qualified_name, "Demo.fst::FStar.Demo::x");
}

#[test]
fn open_emits_uses_edge() {
    let source = "module M\nopen FStar.List.Tot\nlet x = 1\n";
    let result = extract(source);
    let uses: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses.len(), 1);
}

#[test]
fn open_with_restriction_emits_single_uses_edge() {
    let source = "module M\nopen FStar.Fin { fin }\nlet x = 1\n";
    let result = extract(source);
    let uses = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .count();
    assert_eq!(uses, 1);
}

#[test]
fn module_abbreviation_emits_uses_edge_not_module_node() {
    let source = "module M\nmodule CE = FStar.Algebra.CommMonoid.Equiv\nlet x = 1\n";
    let result = extract(source);
    // Only one Module node (the file module M).
    assert_eq!(names_of(&result, NodeKind::Module), vec!["M".to_string()]);
    let uses = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .count();
    assert_eq!(uses, 1);
}

#[test]
fn include_and_friend_emit_uses_edges() {
    let source = "module M\ninclude FStar.A\nfriend FStar.B\nlet x = 1\n";
    let result = extract(source);
    let uses = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .count();
    assert_eq!(uses, 2);
}

// ---------------------------------------------------------------------------
// let / val / and
// ---------------------------------------------------------------------------

#[test]
fn let_is_function() {
    let source = "module M\nlet square (n:int) : int = n * n\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"square".to_string()));
}

#[test]
fn val_is_function() {
    let source = "module M\nval foo (a b : int) : Lemma (a + b == b + a)\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"foo".to_string()));
}

#[test]
fn let_rec_skips_rec_keyword() {
    let source = "module M\nlet rec fact (n:nat) : nat = if n = 0 then 1 else n * fact (n - 1)\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert!(funcs.contains(&"fact".to_string()));
    assert!(!funcs.contains(&"rec".to_string()));
}

#[test]
fn operator_let_uses_operator_symbol_as_name() {
    let source = "module M\nlet ( ++ ) x y = x + y\nlet ( =~ ) a b = a == b\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert!(funcs.contains(&"++".to_string()), "have: {funcs:?}");
    assert!(funcs.contains(&"=~".to_string()), "have: {funcs:?}");
}

#[test]
fn mutually_recursive_and_creates_two_functions() {
    let source = "module M\n\
                  let rec even (n:nat) : bool = if n = 0 then true else odd (n - 1)\n\
                  and odd (n:nat) : bool = if n = 0 then false else even (n - 1)\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert!(funcs.contains(&"even".to_string()));
    assert!(funcs.contains(&"odd".to_string()));
}

#[test]
fn multiline_val_then_let_are_distinct() {
    let source = "module M\n\
                  val eq_mult_one (a b:int) : Lemma\n\
                  \x20 (requires a * b = 1)\n\
                  \x20 (ensures (a = 1 /\\ b = 1) \\/ (a = -1 /\\ b = -1))\n\
                  let eq_mult_one a b = ()\n";
    let result = extract(source);
    // Two functions named eq_mult_one (the val and the let), no phantom nodes
    // from the indented requires/ensures lines.
    let funcs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function && n.name == "eq_mult_one")
        .collect();
    assert_eq!(funcs.len(), 2, "have: {:?}", names_of_all(&result));
}

// ---------------------------------------------------------------------------
// Records, variants, type aliases
// ---------------------------------------------------------------------------

#[test]
fn record_type_is_struct_with_fields() {
    let source = "module M\n\
                  type point = {\n\
                  \x20 x : int;\n\
                  \x20 y : int;\n\
                  }\n";
    let result = extract(source);
    assert_eq!(names_of(&result, NodeKind::Struct), vec!["point".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::Field),
        vec!["x".to_string(), "y".to_string()]
    );
    assert!(contains_edge(&result, "point", "x"));
    assert!(contains_edge(&result, "point", "y"));
}

#[test]
fn record_with_params_and_function_fields() {
    let source = "module M\n\
                  noeq\n\
                  type bijection (a b : Type) = {\n\
                  \x20 right : a -> GTot b;\n\
                  \x20 left  : b -> GTot a;\n\
                  \x20 left_right : x:a -> squash (left (right x) == x);\n\
                  }\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::Struct),
        vec!["bijection".to_string()]
    );
    let fields = names_of(&result, NodeKind::Field);
    assert_eq!(
        fields,
        vec![
            "left".to_string(),
            "left_right".to_string(),
            "right".to_string()
        ]
    );
}

#[test]
fn record_body_brace_on_next_line() {
    let source = "module M\n\
                  type t =\n\
                  {\n\
                  \x20 a : int;\n\
                  \x20 b : bool;\n\
                  }\n";
    let result = extract(source);
    assert_eq!(names_of(&result, NodeKind::Struct), vec!["t".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::Field),
        vec!["a".to_string(), "b".to_string()]
    );
}

#[test]
fn variant_type_is_enum_with_constructors() {
    let source = "module M\n\
                  type color =\n\
                  \x20 | Red\n\
                  \x20 | Green\n\
                  \x20 | Blue\n";
    let result = extract(source);
    assert_eq!(names_of(&result, NodeKind::Enum), vec!["color".to_string()]);
    assert_eq!(
        names_of(&result, NodeKind::EnumVariant),
        vec!["Blue".to_string(), "Green".to_string(), "Red".to_string()]
    );
    assert!(contains_edge(&result, "color", "Red"));
}

#[test]
fn variant_with_constructor_args() {
    let source = "module M\n\
                  type tree =\n\
                  \x20 | Leaf\n\
                  \x20 | Node of tree * int * tree\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::EnumVariant),
        vec!["Leaf".to_string(), "Node".to_string()]
    );
}

#[test]
fn type_alias_is_type_alias() {
    let source = "module M\ntype nat32 = x:int{x >= 0 /\\ x < 4_294_967_296}\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::TypeAlias),
        vec!["nat32".to_string()]
    );
    // A refinement brace must not be mistaken for a record.
    assert!(names_of(&result, NodeKind::Struct).is_empty());
    assert!(names_of(&result, NodeKind::Field).is_empty());
}

#[test]
fn mutually_recursive_types_with_and() {
    let source = "module M\n\
                  type even = | EZ | ES of odd\n\
                  and odd = | OS of even\n";
    let result = extract(source);
    let enums = names_of(&result, NodeKind::Enum);
    assert!(enums.contains(&"even".to_string()));
    assert!(enums.contains(&"odd".to_string()));
}

// ---------------------------------------------------------------------------
// Typeclasses and instances
// ---------------------------------------------------------------------------

#[test]
fn class_is_trait_with_method_fields() {
    let source = "module M\n\
                  class additive a = {\n\
                  \x20 zero : a;\n\
                  \x20 plus : a -> a -> a;\n\
                  }\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::Trait),
        vec!["additive".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Field),
        vec!["plus".to_string(), "zero".to_string()]
    );
    assert!(contains_edge(&result, "additive", "zero"));
}

#[test]
fn instance_is_impl() {
    let source = "module M\n\
                  instance add_int : additive int = {\n\
                  \x20 zero = 0;\n\
                  \x20 plus = (+);\n\
                  }\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::Impl),
        vec!["add_int".to_string()]
    );
    // The instance body assignments must not become fields.
    assert!(names_of(&result, NodeKind::Field).is_empty());
}

#[test]
fn instance_records_implements_reference() {
    let source = "module M\ninstance add_int : additive int = { zero = 0; plus = (+); }\n";
    let result = extract(source);
    let impl_node = find(&result, "add_int");
    assert!(result
        .unresolved_refs
        .iter()
        .any(|r| r.from_node_id == impl_node.id
            && r.reference_kind == EdgeKind::Implements
            && r.reference_name == "additive"));
}

// ---------------------------------------------------------------------------
// exception / assume / effect
// ---------------------------------------------------------------------------

#[test]
fn exception_is_struct() {
    let source = "module M\nexception NotFound\nexception Bad of string\n";
    let result = extract(source);
    let structs = names_of(&result, NodeKind::Struct);
    assert!(structs.contains(&"NotFound".to_string()));
    assert!(structs.contains(&"Bad".to_string()));
}

#[test]
fn assume_val_is_function() {
    let source = "module M\nassume val opaque_fn (x:int) : int\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"opaque_fn".to_string()));
}

#[test]
fn assume_declaration_is_const() {
    let source = "module M\nassume Axiom1 : 1 + 1 == 2\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Const).contains(&"Axiom1".to_string()));
}

#[test]
fn effect_abbreviation_is_type_alias() {
    let source = "module M\neffect St (a:Type) = ST a (fun _ -> True) (fun _ _ _ -> True)\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::TypeAlias).contains(&"St".to_string()));
}

// ---------------------------------------------------------------------------
// Qualifiers and attributes
// ---------------------------------------------------------------------------

#[test]
fn private_qualifier_sets_visibility() {
    let source = "module M\nprivate let helper x = x + 1\n";
    let result = extract(source);
    let helper = find(&result, "helper");
    assert_eq!(helper.visibility, Visibility::Private);
}

#[test]
fn attribute_block_above_decl_is_attrs_start() {
    let source = "module M\n\
                  [@@erasable]\n\
                  noeq\n\
                  type t = { a : int; }\n";
    let result = extract(source);
    let t = find(&result, "t");
    assert_eq!(t.kind, NodeKind::Struct);
    // attrs_start_line points at the `[@@erasable]` line (index 1).
    assert_eq!(t.attrs_start_line, 1);
    assert!(t.start_line > t.attrs_start_line);
}

#[test]
fn inline_for_extraction_qualifier_does_not_break_name() {
    let source = "module M\ninline_for_extraction\nlet f x = x\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"f".to_string()));
}

#[test]
fn doc_comment_is_captured() {
    let source = "module M\n/// This is a helper.\n/// Second line.\nlet helper x = x\n";
    let result = extract(source);
    let helper = find(&result, "helper");
    assert_eq!(
        helper.docstring.as_deref(),
        Some("This is a helper.\nSecond line.")
    );
}

// ---------------------------------------------------------------------------
// Pragmas / directives are ignored
// ---------------------------------------------------------------------------

#[test]
fn set_and_push_options_pragmas_are_ignored() {
    let source = "module M\n\
                  #set-options \"--max_fuel 1 --z3rlimit 50\"\n\
                  #push-options \"--split_queries no\"\n\
                  let lemma_x () : Lemma (True) = ()\n\
                  #pop-options\n\
                  #restart-solver\n";
    let result = extract(source);
    assert!(result.errors.is_empty());
    // The pragma strings (which contain no decl keywords) yield no nodes.
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["lemma_x".to_string()]);
}

// ---------------------------------------------------------------------------
// Term-level constructs inside bodies must not produce phantom nodes
// ---------------------------------------------------------------------------

#[test]
fn calc_block_inside_let_yields_only_outer_function() {
    let source = "module M\n\
                  let proof (x:int) : Lemma (x + 0 == x) =\n\
                  \x20 calc (==) {\n\
                  \x20   x + 0;\n\
                  \x20   == { () }\n\
                  \x20   x;\n\
                  \x20 }\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["proof".to_string()]);
    // The `{ ... }` of the calc block must not be parsed as a record.
    assert!(names_of(&result, NodeKind::Field).is_empty());
}

#[test]
fn eliminate_introduce_inside_let_yields_only_outer_function() {
    let source = "module M\n\
                  let divides_transitive a b c =\n\
                  \x20 eliminate exists q1. b == q1 * a\n\
                  \x20 returns a `divides` c\n\
                  \x20 with _pf.\n\
                  \x20   introduce exists q. c == q * a\n\
                  \x20   with (q1 * q2)\n\
                  \x20   and ()\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["divides_transitive".to_string()]);
}

#[test]
fn local_let_in_is_not_a_top_level_function() {
    let source = "module M\n\
                  let outer x =\n\
                  \x20 let y = x + 1 in\n\
                  \x20 let z = y * 2 in\n\
                  \x20 z\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["outer".to_string()]);
    assert!(!funcs.contains(&"y".to_string()));
    assert!(!funcs.contains(&"z".to_string()));
}

#[test]
fn record_literal_in_let_body_is_not_a_record_type() {
    // A `let` whose value is a record literal (with its closing brace in
    // column 0) must not create a Struct or Field nodes.
    let source = "module M\n\
                  let mk : point =\n\
                  {\n\
                  \x20 x = 1;\n\
                  \x20 y = 2;\n\
                  }\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Struct).is_empty());
    assert!(names_of(&result, NodeKind::Field).is_empty());
    assert!(names_of(&result, NodeKind::Function).contains(&"mk".to_string()));
}

#[test]
fn comments_do_not_confuse_parser() {
    let source = "module M\n\
                  (* type fake = { a : int; } *)\n\
                  let real x = x // let alsofake = 1\n\
                  (* nested (* comment *) still comment *)\n\
                  let real2 x = x\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["real".to_string(), "real2".to_string()]);
    assert!(names_of(&result, NodeKind::Struct).is_empty());
}

#[test]
fn string_with_braces_does_not_confuse_record_detection() {
    let source = "module M\nlet msg = \"a record { x : int }\"\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"msg".to_string()));
    assert!(names_of(&result, NodeKind::Struct).is_empty());
}

// ---------------------------------------------------------------------------
// A larger, realistic file
// ---------------------------------------------------------------------------

#[test]
fn realistic_file_extracts_expected_symbols() {
    let source = "module FStar.Demo\n\
                  \n\
                  open FStar.List.Tot\n\
                  module L = FStar.List.Tot.Base\n\
                  \n\
                  /// A typeclass for addition.\n\
                  class additive a = {\n\
                  \x20 zero : a;\n\
                  \x20 plus : a -> a -> a;\n\
                  }\n\
                  \n\
                  instance add_int : additive int = {\n\
                  \x20 zero = 0;\n\
                  \x20 plus = (+);\n\
                  }\n\
                  \n\
                  type color = | Red | Green | Blue\n\
                  \n\
                  noeq\n\
                  type pair (a b : Type) = {\n\
                  \x20 fst : a;\n\
                  \x20 snd : b;\n\
                  }\n\
                  \n\
                  #set-options \"--z3rlimit 20\"\n\
                  \n\
                  val add_comm (a b : int) : Lemma (a + b == b + a)\n\
                  let add_comm a b = ()\n\
                  \n\
                  let rec sum (l : list int) : int =\n\
                  \x20 match l with\n\
                  \x20 | [] -> 0\n\
                  \x20 | hd :: tl -> hd + sum tl\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    assert_eq!(
        names_of(&result, NodeKind::Module),
        vec!["FStar.Demo".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Trait),
        vec!["additive".to_string()]
    );
    assert_eq!(
        names_of(&result, NodeKind::Impl),
        vec!["add_int".to_string()]
    );
    assert_eq!(names_of(&result, NodeKind::Enum), vec!["color".to_string()]);
    assert_eq!(names_of(&result, NodeKind::Struct), vec!["pair".to_string()]);

    let funcs = names_of(&result, NodeKind::Function);
    assert!(funcs.contains(&"add_comm".to_string()));
    assert!(funcs.contains(&"sum".to_string()));

    // Two Uses edges: open + module abbreviation.
    let uses = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .count();
    assert_eq!(uses, 2);

    // Everything is parented under the module.
    assert!(contains_edge(&result, "FStar.Demo", "additive"));
    assert!(contains_edge(&result, "FStar.Demo", "sum"));
}

// ---------------------------------------------------------------------------
// Signatures must capture the spec (requires / ensures / decreases / refinements)
// ---------------------------------------------------------------------------

#[test]
fn val_signature_includes_requires_and_ensures() {
    let source = "module M\n\
                  val eq_mult_one (a b:int) : Lemma\n\
                  \x20 (requires a * b = 1)\n\
                  \x20 (ensures (a = 1 /\\ b = 1) \\/ (a = -1 /\\ b = -1))\n";
    let result = extract(source);
    let f = find(&result, "eq_mult_one");
    let sig = f.signature.as_deref().unwrap_or("");
    assert!(sig.contains("requires a * b = 1"), "signature was: {sig}");
    assert!(sig.contains("ensures"), "signature was: {sig}");
    assert!(sig.contains("Lemma"), "signature was: {sig}");
}

#[test]
fn let_lemma_signature_has_spec_but_not_proof_body() {
    let source = "module M\n\
                  let lemma_pos (x:int)\n\
                  \x20 : Lemma (requires x > 0) (ensures x >= 0)\n\
                  \x20 = assert (x >= 0)\n";
    let result = extract(source);
    let f = find(&result, "lemma_pos");
    let sig = f.signature.as_deref().unwrap_or("");
    assert!(sig.contains("requires x > 0"), "signature was: {sig}");
    assert!(sig.contains("ensures x >= 0"), "signature was: {sig}");
    // The proof term after `=` must NOT be part of the signature.
    assert!(!sig.contains("assert"), "signature leaked the body: {sig}");
}

#[test]
fn refinement_type_is_preserved_in_signature() {
    let source = "module M\nval to_nat (x:int) : y:int{y >= 0 /\\ y >= x}\n";
    let result = extract(source);
    let f = find(&result, "to_nat");
    let sig = f.signature.as_deref().unwrap_or("");
    assert!(sig.contains("y >= 0"), "signature was: {sig}");
    assert!(sig.contains("{"), "refinement braces dropped: {sig}");
}

#[test]
fn decreases_clause_preserved_in_signature() {
    let source = "module M\n\
                  let rec ackermann (m n:nat) : Tot nat (decreases %[m; n]) =\n\
                  \x20 if m = 0 then n + 1 else 0\n";
    let result = extract(source);
    let f = find(&result, "ackermann");
    let sig = f.signature.as_deref().unwrap_or("");
    assert!(sig.contains("decreases"), "signature was: {sig}");
    assert!(!sig.contains("if m = 0"), "signature leaked the body: {sig}");
}

#[test]
fn tot_effect_with_decreases_is_captured() {
    // A non-lemma recursive function with a `Tot _ (decreases _)` annotation.
    let source = "module M\n\
                  let rec fp_enum_from (p:int{p > 1}) (lo:nat{lo <= p})\n\
                  \x20 : Tot (list (fp p)) (decreases (p - lo))\n\
                  \x20 = if lo = p then [] else mk lo :: fp_enum_from p (lo + 1)\n";
    let result = extract(source);
    let sig = find(&result, "fp_enum_from").signature.clone().unwrap_or_default();
    assert!(sig.contains("Tot (list (fp p))"), "signature was: {sig}");
    assert!(sig.contains("decreases (p - lo)"), "signature was: {sig}");
    assert!(!sig.contains("if lo = p"), "signature leaked the body: {sig}");
}

#[test]
fn pure_effect_with_requires_ensures_is_captured() {
    // A non-lemma function with `Pure _ (requires _) (ensures _)`.
    let source = "module M\n\
                  let fp_inv_member (p:int{is_prime p}) (x: fp p)\n\
                  \x20 : Pure (fp p) (requires x <> 0) (ensures fun y -> y <> 0)\n\
                  \x20 = compute_inverse p x\n";
    let result = extract(source);
    let sig = find(&result, "fp_inv_member").signature.clone().unwrap_or_default();
    assert!(sig.contains("Pure (fp p)"), "signature was: {sig}");
    assert!(sig.contains("requires x <> 0"), "signature was: {sig}");
    assert!(sig.contains("ensures fun y -> y <> 0"), "signature was: {sig}");
    assert!(!sig.contains("compute_inverse"), "signature leaked the body: {sig}");
}

#[test]
fn stateful_effect_signature_is_captured() {
    // Stack/ST style effect with pre/post predicates (no `requires`/`ensures`
    // keywords) is still part of the type and captured verbatim.
    let source = "module M\n\
                  let push (x:int) : Stack unit (fun _ -> True) (fun h0 _ h1 -> modifies !{} h0 h1)\n\
                  \x20 = ()\n";
    let result = extract(source);
    let sig = find(&result, "push").signature.clone().unwrap_or_default();
    assert!(sig.contains("Stack unit"), "signature was: {sig}");
    assert!(sig.contains("modifies"), "signature was: {sig}");
}

#[test]
fn local_let_in_inside_lemma_spec_is_kept_and_cut_correctly() {
    // The `=` inside `let z = x + y` lives inside `Lemma ( ... )` (depth >= 1),
    // so it must NOT be taken as the body separator. The whole spec is kept,
    // the proof term is dropped, and no phantom `z` node appears.
    let source =
        "module M\nlet test (x y: t) : Lemma (let z = x + y in some_prop z) = proof_term\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["test".to_string()], "phantom node? {funcs:?}");
    let sig = find(&result, "test").signature.clone().unwrap_or_default();
    assert!(
        sig.contains("Lemma (let z = x + y in some_prop z)"),
        "signature was: {sig}"
    );
    assert!(!sig.contains("proof_term"), "signature leaked the body: {sig}");
}

#[test]
fn multiline_local_let_in_inside_spec_with_col0_equals() {
    // Same, but the spec spans lines and the body `=` sits in column 0 — it is
    // still the definitional `=` (depth 0) and the cut point.
    let source = "module M\n\
                  let test (x y: t) : Lemma (let z = x + y in\n\
                  some_prop z)\n\
                  = proof_term\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["test".to_string()], "phantom node? {funcs:?}");
    let sig = find(&result, "test").signature.clone().unwrap_or_default();
    assert!(sig.contains("let z = x + y in some_prop z"), "signature was: {sig}");
    assert!(!sig.contains("proof_term"), "signature leaked the body: {sig}");
}

#[test]
fn unbalanced_parens_in_inline_comment_within_spec_dont_break_depth() {
    // The comment carries unbalanced `)` and `(` that would corrupt bracket
    // counting if comments weren't blanked before scanning.
    let source = "module M\n\
                  let test (x y:int) : Lemma (* tricky ) ( comment *) (requires x > y) = ()\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::Function),
        vec!["test".to_string()]
    );
    let sig = find(&result, "test").signature.clone().unwrap_or_default();
    assert!(sig.contains("Lemma (requires x > y)"), "signature was: {sig}");
    assert!(!sig.contains("tricky"), "comment leaked into signature: {sig}");
}

#[test]
fn multiline_comment_inside_spec_is_blanked() {
    let source = "module M\n\
                  let test (x:int) : Lemma (requires x > 0)\n\
                  (* this comment spans\n\
                  \x20  several ) ( unbalanced lines *)\n\
                  (ensures x >= 0) = ()\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::Function),
        vec!["test".to_string()]
    );
    let sig = find(&result, "test").signature.clone().unwrap_or_default();
    assert!(sig.contains("requires x > 0"), "signature was: {sig}");
    assert!(sig.contains("ensures x >= 0"), "signature was: {sig}");
    assert!(!sig.contains("unbalanced"), "comment leaked: {sig}");
}

#[test]
fn nested_comment_with_parens_inside_spec() {
    let source = "module M\n\
                  let f (x:int) : Lemma (* a (* nested ) *) c ( *) (ensures x == x) = ()\n";
    let result = extract(source);
    assert_eq!(names_of(&result, NodeKind::Function), vec!["f".to_string()]);
    let sig = find(&result, "f").signature.clone().unwrap_or_default();
    assert!(sig.contains("Lemma (ensures x == x)"), "signature was: {sig}");
    assert!(!sig.contains("nested"), "comment leaked: {sig}");
}

#[test]
fn paren_inside_string_in_spec_does_not_break_depth() {
    // A string literal carrying an unbalanced paren (e.g. in a labelled
    // assertion message) must also be blanked, not counted.
    let source = "module M\n\
                  let test (x:int) : Lemma (requires x > 0) (ensures labeled \")\" (x >= 0)) = ()\n";
    let result = extract(source);
    assert_eq!(
        names_of(&result, NodeKind::Function),
        vec!["test".to_string()]
    );
    let sig = find(&result, "test").signature.clone().unwrap_or_default();
    assert!(sig.contains("requires x > 0"), "signature was: {sig}");
    assert!(sig.contains("ensures"), "signature was: {sig}");
}

#[test]
fn real_calc_eliminate_lemma_is_single_node_with_clean_signature() {
    // Faithful to CuteCAS legacy/AlgebraTypes.fst::unit_product_is_unit_new:
    // `%`-quotations, local operator let-bindings (`let ( = ) = eq in`),
    // `eliminate exists ... returns ... with`, begin/end, and nested calc
    // blocks must yield exactly one Function node and a spec-only signature.
    let source = "module M\n\
let unit_product_is_unit_new #a (#eq: equivalence_relation a)\n\
                             (mul: op_with_congruence eq{is_associative mul})\n\
                             (x y: units_of mul)\n\
  : Lemma (is_unit (mul x y) mul) =\n\
  reveal_opaque (`%is_unit) (is_unit #a #eq);\n\
  let ( * ) = mul in\n\
  let ( = ) = eq in\n\
  eliminate exists (x' y':a). (is_neutral_of (x'*x) mul /\\ is_neutral_of (y'*y) mul)\n\
  returns is_unit (x*y) mul with _.\n\
  begin\n\
    calc (=) {\n\
      (y'*x')*(x*y); = { assoc_lemma_3 mul y' x' (x*y) }\n\
      y'*y;\n\
    };\n\
    ()\n\
  end\n\
let after_it (z:int) : int = z\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let funcs = names_of(&result, NodeKind::Function);
    // Exactly the lemma and the following decl — nothing from the proof sugar.
    assert_eq!(
        funcs,
        vec![
            "after_it".to_string(),
            "unit_product_is_unit_new".to_string()
        ],
        "phantom nodes from proof body: {funcs:?}"
    );
    // The local operator bindings `let ( * )` / `let ( = )` must not leak as nodes.
    assert!(!funcs.iter().any(|f| f == "*" || f == "=" || f == "x'"));

    let lemma = find(&result, "unit_product_is_unit_new");
    let sig = lemma.signature.clone().unwrap_or_default();
    assert!(
        sig.contains("Lemma (is_unit (mul x y) mul)"),
        "signature was: {sig}"
    );
    assert!(!sig.contains("reveal_opaque"), "body leaked: {sig}");
    assert!(!sig.contains("calc"), "body leaked: {sig}");
    assert!(!sig.contains("eliminate"), "body leaked: {sig}");

    // The span must cover the whole proof (through `end`) up to the next decl.
    let after = find(&result, "after_it");
    assert!(lemma.end_line < after.start_line);
    assert!(
        lemma.end_line >= 16,
        "end_line {} should reach the `end` line",
        lemma.end_line
    );
}

#[test]
fn introduce_exists_block_in_body_is_not_a_decl() {
    // `introduce exists ... with ... and ...` (F* manual's existential intro).
    let source = "module M\n\
let pick (n:nat) : Lemma (exists m. m > n) =\n\
  introduce exists m. m > n\n\
  with (n + 1)\n\
  and ()\n\
let next () : unit = ()\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(
        funcs,
        vec!["next".to_string(), "pick".to_string()],
        "phantom nodes: {funcs:?}"
    );
    let sig = find(&result, "pick").signature.clone().unwrap_or_default();
    assert!(sig.contains("Lemma (exists m. m > n)"), "signature was: {sig}");
    assert!(!sig.contains("introduce"), "body leaked: {sig}");
}

#[test]
fn plain_let_signature_excludes_body() {
    let source = "module M\nlet add (a b : int) : int = a + b\n";
    let result = extract(source);
    let f = find(&result, "add");
    let sig = f.signature.as_deref().unwrap_or("");
    assert!(sig.contains("add (a b : int) : int"), "signature was: {sig}");
    assert!(!sig.contains("a + b"), "signature leaked the body: {sig}");
}

#[test]
fn node_line_span_covers_full_spec_for_source_retrieval() {
    // The node's [start_line, end_line] must cover the whole multi-line spec so
    // tools that fetch source by line range return requires/ensures too.
    let source = "module M\n\
                  val big_lemma (a b : int) : Lemma\n\
                  \x20 (requires a > b)\n\
                  \x20 (ensures a - b > 0)\n\
                  \x20 [SMTPat (a - b)]\n";
    let result = extract(source);
    let f = find(&result, "big_lemma");
    // val starts at line 1; the [SMTPat ...] line is line 4.
    assert_eq!(f.start_line, 1);
    assert!(f.end_line >= 4, "end_line {} should cover the spec", f.end_line);
}

// ---------------------------------------------------------------------------
// Indentation is NOT mandatory in F*: bodies may sit in column 0
// ---------------------------------------------------------------------------

#[test]
fn unindented_if_body_is_absorbed_not_a_new_decl() {
    // `if ... then ... else ...` at column 0 is the body of `choice`, not a
    // new declaration. `choice` and `next` must both be functions; nothing else.
    let source = "module M\n\
                  let choice (b:bool) x y =\n\
                  if b then x else y\n\
                  let next = 0\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["choice".to_string(), "next".to_string()]);
    // `choice`'s span must extend over the unindented body line.
    let choice = find(&result, "choice");
    assert!(choice.end_line >= 2, "choice end_line was {}", choice.end_line);
}

#[test]
fn unindented_local_let_in_is_not_a_top_level_function() {
    // The crux: a column-0 local `let y = ... in` is the body of `foo`, not a
    // top-level `let`. Only `foo` and `bar` are functions.
    let source = "module M\n\
                  let foo x =\n\
                  let y = x + 1 in\n\
                  y\n\
                  let bar = 2\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["bar".to_string(), "foo".to_string()]);
    assert!(!funcs.contains(&"y".to_string()));
}

#[test]
fn unindented_nested_let_in_chain() {
    let source = "module M\n\
                  let compute x =\n\
                  let a = x + 1 in\n\
                  let b = a * 2 in\n\
                  let c = b - 3 in\n\
                  c\n\
                  let other y = y\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["compute".to_string(), "other".to_string()]);
}

// ---------------------------------------------------------------------------
// Typeclass-constraint binders {| ... |}
// ---------------------------------------------------------------------------

#[test]
fn let_with_named_tc_constraint_binder() {
    let source = "module M\n\
                  let cmp (#a:Type) {| d: eq a |} (x:a) (y:a) : bool = d.eqb x y\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"cmp".to_string()));
}

#[test]
fn val_operator_with_anonymous_tc_constraint_binder() {
    // From FStar.Class.Add: `val (++) : #a:_ -> {| additive a |} -> a -> a -> a`
    let source = "module M\n\
                  val ( ++ ) : #a:_ -> {| additive a |} -> a -> a -> a\n\
                  let ( ++ ) = plus\n";
    let result = extract(source);
    let funcs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function && n.name == "++")
        .collect();
    assert_eq!(funcs.len(), 2, "have: {:?}", names_of_all(&result));
}

#[test]
fn instance_with_anonymous_tc_binder_resolves_class_after_binder() {
    let source = "module M\ninstance foo {| eq a |} : ord a = mk_ord ()\n";
    let result = extract(source);
    let impl_node = find(&result, "foo");
    assert_eq!(impl_node.kind, NodeKind::Impl);
    // The implemented class is `ord` (after the binder), not `eq` (inside it).
    assert!(result.unresolved_refs.iter().any(|r| r.from_node_id
        == impl_node.id
        && r.reference_kind == EdgeKind::Implements
        && r.reference_name == "ord"));
    assert!(!result
        .unresolved_refs
        .iter()
        .any(|r| r.reference_name == "eq"));
}

#[test]
fn instance_with_named_tc_binder_skips_binder_colon() {
    // From CuteCAS: the `:` inside `{| g: gcd_domain t |}` must be skipped; the
    // implemented class is `integral_domain`.
    let source = "module M\n\
                  instance id_of_gcd t {| g: gcd_domain t |} : integral_domain t = g.gcd_id\n";
    let result = extract(source);
    let impl_node = find(&result, "id_of_gcd");
    assert_eq!(impl_node.kind, NodeKind::Impl);
    assert!(result.unresolved_refs.iter().any(|r| r.from_node_id
        == impl_node.id
        && r.reference_kind == EdgeKind::Implements
        && r.reference_name == "integral_domain"));
    assert!(!result
        .unresolved_refs
        .iter()
        .any(|r| r.reference_name == "gcd_domain"));
}

// ---------------------------------------------------------------------------
// Monadic let / "let operators" (let?, and?, ( let:: ), ( let* ))
// ---------------------------------------------------------------------------

#[test]
fn let_operator_definitions_are_functions() {
    // From examples/misc/MonadicLetBindings.fst
    let source = "module M\n\
                  let (let?) (x: option 'a) (f: 'a -> option 'b) : option 'b = bind x f\n\
                  let (and?) (x: option 'a) (y: option 'b) : option ('a & 'b) = pair x y\n\
                  let ( let:: ) (l: list 'a) (f: 'a -> list 'b) : list 'b = concatMap f l\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert!(funcs.contains(&"let?".to_string()), "have: {funcs:?}");
    assert!(funcs.contains(&"and?".to_string()), "have: {funcs:?}");
    assert!(funcs.contains(&"let::".to_string()), "have: {funcs:?}");
}

#[test]
fn operator_name_argument_does_not_become_the_decl_name() {
    // `let sugared1 (let*) (and*) ex ey ez f = ...` — the operators are
    // parameters; the function is named `sugared1`.
    let source = "module M\nlet sugared1 (let*) (and*) ex ey ez f = f ex ey ez\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert!(funcs.contains(&"sugared1".to_string()), "have: {funcs:?}");
    assert!(!funcs.contains(&"let*".to_string()));
}

#[test]
fn monadic_let_bang_in_body_is_not_a_top_level_decl() {
    let source = "module M\n\
                  let option_example (a b: list (int & int)) =\n\
                  \x20 let? haL, haR = head a\n\
                  \x20 and? hbL, hbR = head b in\n\
                  \x20 Some (haL + hbR)\n\
                  let next = 0\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(
        funcs,
        vec!["next".to_string(), "option_example".to_string()]
    );
}

#[test]
fn unindented_monadic_let_bang_in_body() {
    // Same as above but with the monadic binds in column 0.
    let source = "module M\n\
                  let option_example a b =\n\
                  let? haL = head a\n\
                  and? hbL = head b in\n\
                  Some (haL + hbL)\n\
                  let next = 0\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(
        funcs,
        vec!["next".to_string(), "option_example".to_string()]
    );
}
