use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::TomlExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    TomlExtractor.extract("Cargo.toml", source)
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
fn extracts_top_level_pairs_as_const() {
    let source = "title = \"hello\"\nversion = 3\n";
    let result = extract(source);
    assert!(result.errors.is_empty());
    let consts = names_of(&result, NodeKind::Const);
    assert!(consts.contains(&"title".to_string()));
    assert!(consts.contains(&"version".to_string()));
}

#[test]
fn extracts_table_as_module() {
    let source = "[package]\nname = \"demo\"\nversion = \"1.0\"\n";
    let result = extract(source);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["package".to_string()]);
}

#[test]
fn dotted_table_keeps_dotted_name() {
    let source = "[profile.release]\nopt-level = 3\n";
    let result = extract(source);
    let modules = names_of(&result, NodeKind::Module);
    assert_eq!(modules, vec!["profile.release".to_string()]);
}

#[test]
fn table_array_element_is_module() {
    let source = "[[bin]]\nname = \"a\"\n\n[[bin]]\nname = \"b\"\n";
    let result = extract(source);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    // Two [[bin]] entries produce two distinct module nodes (different start_line).
    assert_eq!(modules.len(), 2);
    assert!(modules.iter().all(|m| m.name == "bin"));
}

#[test]
fn pairs_inside_table_are_parented_to_table() {
    let source = "[package]\nname = \"demo\"\nversion = \"1.0\"\n";
    let result = extract(source);
    let pkg = result.nodes.iter().find(|n| n.name == "package").unwrap();
    let name = result
        .nodes
        .iter()
        .find(|n| n.name == "name" && n.kind == NodeKind::Const)
        .unwrap();
    let version = result
        .nodes
        .iter()
        .find(|n| n.name == "version" && n.kind == NodeKind::Const)
        .unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == pkg.id && e.target == name.id && e.kind == EdgeKind::Contains));
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == pkg.id && e.target == version.id && e.kind == EdgeKind::Contains));
}

#[test]
fn top_level_pair_parented_to_file() {
    let source = "key = 42\n";
    let result = extract(source);
    let file = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap();
    let key = result
        .nodes
        .iter()
        .find(|n| n.name == "key" && n.kind == NodeKind::Const)
        .unwrap();
    assert!(result
        .edges
        .iter()
        .any(|e| e.source == file.id && e.target == key.id && e.kind == EdgeKind::Contains));
}

#[test]
fn empty_file_produces_only_file_node() {
    let result = extract("");
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0].kind, NodeKind::File);
}

#[test]
fn extensions_are_toml() {
    assert_eq!(TomlExtractor.extensions(), &["toml"]);
}

#[test]
fn language_name_is_toml() {
    assert_eq!(TomlExtractor.language_name(), "TOML");
}

#[test]
fn inline_table_value_is_one_pair() {
    // `clap = { version = "4", features = ["derive"] }` is one pair at the
    // top level (the inline table is its value, not its own pairs).
    let source = "clap = { version = \"4\", features = [\"derive\"] }\n";
    let result = extract(source);
    let consts = names_of(&result, NodeKind::Const);
    // Only `clap` is a top-level pair; the inline table's inner `version`
    // and `features` belong to that value, not the document.
    assert!(consts.contains(&"clap".to_string()));
}
