use tokensave::extraction::AstroExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_astro_file_node() {
    let source = "---\nconst x = 1;\n---\n<h1>hi</h1>";
    let result = AstroExtractor.extract("page.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "page.astro");
}

#[test]
fn test_astro_frontmatter_function_extracted() {
    let source = r#"---
export function formatTitle(t: string): string {
    return t.toUpperCase();
}
---
<html></html>"#;
    let result = AstroExtractor.extract("page.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fmt = result
        .nodes
        .iter()
        .find(|n| n.name == "formatTitle")
        .unwrap();
    assert_eq!(fmt.visibility, Visibility::Pub);
}

#[test]
fn test_astro_line_numbers_are_original_file_positions() {
    // `greet` is on line 2 (0-indexed) in the full .astro file.
    let source = "---\n\nexport function greet(): void {}\n---\n<p>hi</p>";
    let result = AstroExtractor.extract("greet.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let greet = result.nodes.iter().find(|n| n.name == "greet").unwrap();
    assert_eq!(
        greet.start_line, 2,
        "expected greet on line 2, got {}",
        greet.start_line
    );
}

#[test]
fn test_astro_interface_props_extracted() {
    let source = r#"---
interface Props {
    title: string;
    description?: string;
}
const { title } = Astro.props;
---
<html></html>"#;
    let result = AstroExtractor.extract("Layout.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let iface = result.nodes.iter().find(|n| n.name == "Props");
    assert!(
        iface.is_some(),
        "expected Props interface in {:?}",
        result.nodes
    );
}

#[test]
fn test_astro_no_frontmatter_returns_file_node_only() {
    let source = "<html><body><h1>Static</h1></body></html>";
    let result = AstroExtractor.extract("static.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let non_file: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind != NodeKind::File)
        .collect();
    assert!(non_file.is_empty(), "unexpected nodes: {:?}", non_file);
}

#[test]
fn test_astro_template_markup_does_not_produce_symbols() {
    // HTML after the closing `---` must not leak TypeScript symbols.
    let source = "---\nconst greeting = 'hello';\n---\n<p class=\"text-lg\">{greeting}</p>\n<script>window.foo = 1;</script>";
    let result = AstroExtractor.extract("page.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        fns.is_empty(),
        "unexpected functions from template: {:?}",
        fns
    );
}

#[test]
fn test_astro_fixture() {
    let source = include_str!("fixtures/sample.astro");
    let result = AstroExtractor.extract("sample.astro", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let names: Vec<_> = result.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"formatTitle"),
        "expected formatTitle in {:?}",
        names
    );
    assert!(
        names.contains(&"Props"),
        "expected Props interface in {:?}",
        names
    );
    assert!(
        names.contains(&"greeting"),
        "expected greeting const in {:?}",
        names
    );
}
