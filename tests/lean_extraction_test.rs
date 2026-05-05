use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::LeanExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    LeanExtractor.extract("Demo.lean", source)
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
fn def_is_function() {
    let source = "def square (n : Nat) : Nat := n * n\n";
    let result = extract(source);
    assert!(result.errors.is_empty());
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["square".to_string()]);
}

#[test]
fn theorem_is_function() {
    let source = "theorem foo : 1 + 1 = 2 := by rfl\n";
    let result = extract(source);
    let funcs = names_of(&result, NodeKind::Function);
    assert_eq!(funcs, vec!["foo".to_string()]);
}

#[test]
fn structure_is_struct() {
    let source = "structure Point where\n  x : Nat\n  y : Nat\n";
    let result = extract(source);
    let structs = names_of(&result, NodeKind::Struct);
    assert_eq!(structs, vec!["Point".to_string()]);
}

#[test]
fn axiom_is_const() {
    let source = "axiom myAxiom : 1 + 1 = 2\n";
    let result = extract(source);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["myAxiom".to_string()]);
}

#[test]
fn inductive_is_enum() {
    let source = "inductive Color where\n  | Red\n  | Green\n  | Blue\n";
    let result = extract(source);
    let enums = names_of(&result, NodeKind::Enum);
    assert_eq!(enums, vec!["Color".to_string()]);
}

#[test]
fn namespace_creates_module_and_parents_children() {
    let source = "namespace Foo\n\
                  def bar : Nat := 1\n\
                  end Foo\n";
    let result = extract(source);
    let foo = result.nodes.iter().find(|n| n.name == "Foo").unwrap();
    let bar = result.nodes.iter().find(|n| n.name == "bar").unwrap();
    assert_eq!(foo.kind, NodeKind::Module);
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == foo.id && e.target == bar.id && e.kind == EdgeKind::Contains));
}

#[test]
fn nested_namespaces_parent_correctly() {
    let source = "namespace Outer\n\
                  def a : Nat := 0\n\
                  namespace Inner\n\
                  def b : Nat := 1\n\
                  end Inner\n\
                  def c : Nat := 2\n\
                  end Outer\n";
    let result = extract(source);
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

    // Outer contains Inner, a, c
    assert!(contains
        .iter()
        .any(|e| e.source == outer.id && e.target == inner.id));
    assert!(contains
        .iter()
        .any(|e| e.source == outer.id && e.target == a.id));
    assert!(contains
        .iter()
        .any(|e| e.source == outer.id && e.target == c.id));
    // Inner contains b only
    assert!(contains
        .iter()
        .any(|e| e.source == inner.id && e.target == b.id));
    assert!(!contains
        .iter()
        .any(|e| e.source == outer.id && e.target == b.id));
}

#[test]
fn import_emits_uses_edge() {
    let source = "import Mathlib.Data.Nat.Basic\n\ndef x : Nat := 1\n";
    let result = extract(source);
    let uses: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses.len(), 1);
}

#[test]
fn extensions_are_lean() {
    assert_eq!(LeanExtractor.extensions(), &["lean"]);
}

#[test]
fn language_name_is_lean() {
    assert_eq!(LeanExtractor.language_name(), "Lean");
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = extract("");
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
}

#[test]
fn anonymous_instance_emits_nothing() {
    // `instance : Add Nat where ...` has no name field; we now skip the
    // emit rather than producing an `<anonymous_instance>` Const node.
    let source = "instance : Add Nat where\n  add := Nat.add\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts = names_of(&result, NodeKind::Const);
    assert!(
        consts.is_empty(),
        "anonymous instance should produce no Const node, got: {consts:?}"
    );
}

#[test]
fn named_instance_is_const() {
    let source = "instance addNat : Add Nat where\n  add := Nat.add\n";
    let result = extract(source);
    let consts = names_of(&result, NodeKind::Const);
    assert_eq!(consts, vec!["addNat".to_string()]);
}

#[test]
fn anonymous_section_emits_no_module_but_recurses_body() {
    // `section ... end` (no name) is a scope marker with no graph value.
    // We don't emit a Module, but defs inside still get extracted and
    // parent to the surrounding scope (the file in this case).
    let source = "section\n\
                  def hidden : Nat := 0\n\
                  end\n";
    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let modules = names_of(&result, NodeKind::Module);
    assert!(
        modules.is_empty(),
        "anonymous section should produce no Module, got: {modules:?}"
    );

    let file = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap();
    let hidden = result.nodes.iter().find(|n| n.name == "hidden").unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == file.id && e.target == hidden.id && e.kind == EdgeKind::Contains));
}

#[test]
fn named_section_still_emits_module() {
    let source = "section MySection\n\
                  def x : Nat := 1\n\
                  end MySection\n";
    let result = extract(source);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["MySection".to_string()]);

    let section = result.nodes.iter().find(|n| n.name == "MySection").unwrap();
    let x = result.nodes.iter().find(|n| n.name == "x").unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == section.id && e.target == x.id));
}

#[test]
fn anonymous_section_inside_namespace_parents_defs_to_namespace() {
    // The body of an anonymous section should re-attach to whatever
    // scope contains the section — here, namespace `N`.
    let source = "namespace N\n\
                  section\n\
                  def inSection : Nat := 1\n\
                  end\n\
                  def outsideSection : Nat := 2\n\
                  end N\n";
    let result = extract(source);
    let n = result.nodes.iter().find(|node| node.name == "N").unwrap();
    let in_sec = result
        .nodes
        .iter()
        .find(|node| node.name == "inSection")
        .unwrap();
    let outside = result
        .nodes
        .iter()
        .find(|node| node.name == "outsideSection")
        .unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == n.id && e.target == in_sec.id && e.kind == EdgeKind::Contains));
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == n.id && e.target == outside.id && e.kind == EdgeKind::Contains));
}
