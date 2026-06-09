use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::SvelteExtractor;
use tokensave::types::*;

#[test]
fn test_svelte_file_node() {
    let source = r#"<script lang="ts">
export function greet(): void {}
</script>
<h1>Hello</h1>"#;
    let result = SvelteExtractor.extract("Page.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "Page.svelte");
}

#[test]
fn test_svelte_exported_function_is_pub() {
    let source = r#"<script lang="ts">
export function increment(n: number): number {
    return n + 1;
}

function internal(): void {}
</script>"#;
    let result = SvelteExtractor.extract("Counter.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2);
    let inc = fns.iter().find(|f| f.name == "increment").unwrap();
    assert_eq!(inc.visibility, Visibility::Pub);
    let internal = fns.iter().find(|f| f.name == "internal").unwrap();
    assert_eq!(internal.visibility, Visibility::Private);
}

#[test]
fn test_svelte_line_numbers_are_original_file_positions() {
    // `greet` is on line 2 (0-indexed) in the full .svelte file.
    let source = "<script lang=\"ts\">\n\nexport function greet(): void {}\n</script>\n<h1>hi</h1>";
    let result = SvelteExtractor.extract("greet.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let greet = result.nodes.iter().find(|n| n.name == "greet").unwrap();
    assert_eq!(
        greet.start_line, 2,
        "expected greet on line 2, got {}",
        greet.start_line
    );
}

#[test]
fn test_svelte_module_script_symbols_extracted() {
    let source = r#"<script module>
export const prerender = true;
</script>

<script lang="ts">
export function render(): string { return ""; }
</script>"#;
    let result = SvelteExtractor.extract("Layout.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let names: Vec<_> = result.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"prerender"),
        "expected prerender in {:?}",
        names
    );
    assert!(names.contains(&"render"), "expected render in {:?}", names);
}

#[test]
fn test_svelte_no_script_block_returns_file_node_only() {
    let source = "<h1>Hello</h1>\n<p>World</p>";
    let result = SvelteExtractor.extract("Static.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Only the File node — no symbols to extract.
    let non_file: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind != NodeKind::File)
        .collect();
    assert!(non_file.is_empty(), "unexpected nodes: {:?}", non_file);
}

#[test]
fn test_svelte_interface_extracted() {
    let source = r#"<script lang="ts">
interface Props {
    title: string;
    count?: number;
}
</script>"#;
    let result = SvelteExtractor.extract("Props.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let iface = result.nodes.iter().find(|n| n.name == "Props");
    assert!(
        iface.is_some(),
        "expected Props interface in {:?}",
        result.nodes
    );
}

#[test]
fn test_svelte_fixture() {
    let source = include_str!("fixtures/sample.svelte");
    let result = SvelteExtractor.extract("sample.svelte", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let names: Vec<_> = result.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"increment"),
        "expected increment in {:?}",
        names
    );
    assert!(
        names.contains(&"fetchData"),
        "expected fetchData in {:?}",
        names
    );
    assert!(
        names.contains(&"prerender"),
        "expected prerender const in {:?}",
        names
    );
    // Props interface
    assert!(
        names.contains(&"Props"),
        "expected Props interface in {:?}",
        names
    );
}
