use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::QuintExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    QuintExtractor.extract("spec.qnt", source)
}

fn names_of(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    result
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.name.clone())
        .collect()
}

#[test]
fn file_node_is_emitted() {
    let result = extract("module Foo { val x = 1 }");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files = names_of(&result, NodeKind::File);
    assert_eq!(files, vec!["spec.qnt".to_string()]);
}

#[test]
fn extracts_top_level_module() {
    let source = "module Counter {\n  val x = 0\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["Counter".to_string()]);
}

#[test]
fn extracts_def_as_function() {
    let source = "module M {\n  def add(x: int, y: int): int = x + y\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["add".to_string()]);
}

#[test]
fn pure_def_is_function() {
    let source = "module M {\n  pure def f(x: int): int = x\n}\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["f".to_string()]);
}

#[test]
fn val_is_const() {
    let source = "module M {\n  val pi = 3\n}\n";
    let result = extract(source);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["pi".to_string()]);
}

#[test]
fn var_is_static() {
    let source = "module M {\n  var counter: int\n}\n";
    let result = extract(source);
    let statics = names_of(&result, NodeKind::Static);
    assert_eq!(statics, vec!["counter".to_string()]);
}

#[test]
fn type_is_type_alias() {
    let source = "module M {\n  type State = int\n}\n";
    let result = extract(source);
    let types = names_of(&result, NodeKind::TypeAlias);
    assert_eq!(types, vec!["State".to_string()]);
}

#[test]
fn action_temporal_run_are_functions() {
    let source = "module M {\n\
                  action step = x' = x + 1\n\
                  temporal alwaysPositive = always(x > 0)\n\
                  run myRun = init.then(step)\n\
                  }\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(
        funcs,
        vec![
            "step".to_string(),
            "alwaysPositive".to_string(),
            "myRun".to_string()
        ]
    );
}

#[test]
fn definitions_are_contained_by_module() {
    let source = "module M {\n  val x = 1\n  def f = 2\n}\n";
    let result = extract(source);
    let m = result.nodes.iter().find(|n| n.name == "M").unwrap();
    let x = result.nodes.iter().find(|n| n.name == "x").unwrap();
    let f = result.nodes.iter().find(|n| n.name == "f").unwrap();

    let edges_from_m: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.source == m.id && e.kind == EdgeKind::Contains)
        .collect();
    assert!(edges_from_m.iter().any(|e| e.target == x.id));
    assert!(edges_from_m.iter().any(|e| e.target == f.id));
}

#[test]
fn nested_modules_parent_correctly() {
    let source = "module Outer {\n\
                  val a = 1\n\
                  module Inner {\n\
                  val b = 2\n\
                  }\n\
                  val c = 3\n\
                  }\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let outer = result.nodes.iter().find(|n| n.name == "Outer").unwrap();
    let inner = result.nodes.iter().find(|n| n.name == "Inner").unwrap();
    let a = result.nodes.iter().find(|n| n.name == "a").unwrap();
    let b = result.nodes.iter().find(|n| n.name == "b").unwrap();
    let c = result.nodes.iter().find(|n| n.name == "c").unwrap();

    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();

    // Outer contains Inner, a, c (but not b)
    assert!(contains
        .iter()
        .any(|e| e.source == outer.id && e.target == inner.id));
    assert!(contains
        .iter()
        .any(|e| e.source == outer.id && e.target == a.id));
    assert!(contains
        .iter()
        .any(|e| e.source == outer.id && e.target == c.id));
    assert!(!contains
        .iter()
        .any(|e| e.source == outer.id && e.target == b.id));

    // Inner contains b (only)
    assert!(contains
        .iter()
        .any(|e| e.source == inner.id && e.target == b.id));
}

#[test]
fn inner_braces_do_not_pop_module_scope() {
    // Set/record literals use `{}` too — they must not trick the brace
    // counter into popping the module scope.
    let source = "module M {\n\
                  val s = Set(1, 2, 3)\n\
                  val r = { a: 1, b: 2 }\n\
                  val after = 42\n\
                  }\n";
    let result = extract(source);
    let m = result.nodes.iter().find(|n| n.name == "M").unwrap();
    let after = result.nodes.iter().find(|n| n.name == "after").unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == m.id && e.target == after.id && e.kind == EdgeKind::Contains));
}

#[test]
fn comments_do_not_break_pending_kind() {
    let source = "module M {\n\
                  /* doc */ def f(x: int): int = x\n\
                  val // line comment\n\
                  y = 2\n\
                  }\n";
    let result = extract(source);
    assert!(names_of(&result, NodeKind::Function).contains(&"f".to_string()));
    assert!(names_of(&result, NodeKind::Const).contains(&"y".to_string()));
}

#[test]
fn extensions_are_qnt() {
    let ext = QuintExtractor;
    assert_eq!(ext.extensions(), &["qnt"]);
}

#[test]
fn language_name_is_quint() {
    let ext = QuintExtractor;
    assert_eq!(ext.language_name(), "Quint");
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = extract("");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
}

#[test]
fn assume_is_const() {
    let source = "module M {\n  assume nonNeg = x >= 0\n}\n";
    let result = extract(source);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["nonNeg".to_string()]);
}

#[test]
fn const_parameter_is_const() {
    let source = "module M {\n  const N: int\n}\n";
    let result = extract(source);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["N".to_string()]);
}

fn uses_targets(result: &ExtractionResult) -> Vec<String> {
    // Map Uses-edge target ids to their (synthetic) target paths by
    // pulling the path back out of the module name encoded in the edge.
    // We can't reverse the hash, so just count and inspect edges directly.
    result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .map(|e| e.target.clone())
        .collect()
}

#[test]
fn import_emits_uses_edge() {
    let source = "import basicSpells\n\nmodule M {\n  val x = 1\n}\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses = uses_targets(&result);
    assert_eq!(uses.len(), 1, "expected 1 Uses edge, got {uses:?}");
}

#[test]
fn dotted_import_path_is_one_edge() {
    // `import Foo.bar` should produce a single Uses edge with target
    // synthesized from "Foo.bar", not two edges (one per identifier).
    let source = "import Foo.bar\n\nmodule M { }\n";
    let result = extract(source);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses_edges.len(), 1);
}

#[test]
fn import_from_clause_terminates_path() {
    // `import path.* from "spells"` — the `from` keyword should commit
    // the path before reaching the string literal. The `*` is an
    // operator, currently dropped (we only join identifiers and `.`).
    let source = "import basicSpells.* from \"spells/basicSpells\"\n\nmodule M { }\n";
    let result = extract(source);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses_edges.len(), 1);
}

#[test]
fn multiple_imports_produce_multiple_edges() {
    let source = "import a\nimport b.c\nimport d as e\n\nmodule M { }\n";
    let result = extract(source);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses_edges.len(), 3);
}

#[test]
fn import_inside_module_attributes_to_module() {
    // Quint allows imports inside a module body. The Uses edge should
    // come from the enclosing module, not the file.
    let source = "module M {\n  import Helpers\n  val x = 1\n}\n";
    let result = extract(source);
    let m = result.nodes.iter().find(|n| n.name == "M").unwrap();
    let uses_from_module: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses && e.source == m.id)
        .collect();
    assert_eq!(uses_from_module.len(), 1);
}

#[test]
fn bare_import_keyword_with_no_path_emits_nothing() {
    // Defensive: malformed `import` with no following identifier shouldn't
    // panic or emit a phantom edge.
    let source = "import\n\nmodule M { }\n";
    let result = extract(source);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses_edges.len(), 0);
}
