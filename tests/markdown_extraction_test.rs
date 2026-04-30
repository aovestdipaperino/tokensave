use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::MarkdownExtractor;
use tokensave::types::*;

#[test]
fn test_markdown_file_node_is_root() {
    let source = "# Hello\n\nSome content.";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "README.md");
}

#[test]
fn test_markdown_extracts_headers() {
    let source = "# Title\n\n## Section\n\n### Subsection";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 3);
    assert_eq!(modules[0].name, "Title");
    assert_eq!(modules[1].name, "Section");
    assert_eq!(modules[2].name, "Subsection");
}

#[test]
fn test_markdown_header_hierarchy() {
    let source = "# Top\n\n## Section1\n\n### Deep\n\n## Section2";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Section1 and Section2 should be children of Top
    // Deep should be child of Section1
    let top = result.nodes.iter().find(|n| n.name == "Top").unwrap();
    let section1 = result.nodes.iter().find(|n| n.name == "Section1").unwrap();
    let section2 = result.nodes.iter().find(|n| n.name == "Section2").unwrap();
    let deep = result.nodes.iter().find(|n| n.name == "Deep").unwrap();

    let contains_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();

    // Check Top contains Section1 and Section2
    let top_contains: Vec<_> = contains_edges
        .iter()
        .filter(|e| e.source == top.id)
        .collect();
    assert!(top_contains.iter().any(|e| e.target == section1.id));
    assert!(top_contains.iter().any(|e| e.target == section2.id));

    // Check Section1 contains Deep
    let section1_contains: Vec<_> = contains_edges
        .iter()
        .filter(|e| e.source == section1.id)
        .collect();
    assert!(section1_contains.iter().any(|e| e.target == deep.id));
}

#[test]
fn test_markdown_extracts_code_links() {
    let source = "See [main.rs](src/main.rs) for details.";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert!(
        !uses_edges.is_empty(),
        "should have Uses edge for code link"
    );
}

#[test]
fn test_markdown_skips_external_links() {
    let source = "Check [Google](https://google.com) for more.";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert!(
        uses_edges.is_empty(),
        "should not create Uses edge for external links"
    );
}

#[test]
fn test_markdown_skips_non_code_links() {
    let source = "See [image](docs/image.png) for diagram.";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // .png is not a code extension, so no Uses edge
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert!(
        uses_edges.is_empty(),
        "should not create Uses edge for non-code links"
    );
}

#[test]
fn test_markdown_handles_code_blocks() {
    // Code blocks should not be treated as headers
    let source = "# Title\n\n```rust\nfn main() {}\n```\n\n## Next";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0].name, "Title");
    assert_eq!(modules[1].name, "Next");
}

#[test]
fn test_markdown_handles_empty_file() {
    let source = "";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Should still have a File node
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_markdown_handles_no_headers() {
    let source = "Just some plain text without any headers.";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert!(modules.is_empty(), "should have no Module nodes");
}

#[test]
fn test_markdown_extensions() {
    let ext = MarkdownExtractor;
    let extensions = ext.extensions();
    assert!(extensions.contains(&"md"));
    assert!(extensions.contains(&"markdown"));
}

#[test]
fn test_markdown_language_name() {
    let ext = MarkdownExtractor;
    assert_eq!(ext.language_name(), "Markdown");
}

#[test]
fn test_markdown_multiple_links_same_line() {
    let source = "See [main](src/main.rs) and [lib](src/lib.rs).";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert_eq!(uses_edges.len(), 2);
}

#[test]
fn test_markdown_links_with_fragments() {
    let source = "See [section](#section) for details.";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Fragment links don't have file extensions that match code patterns
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Uses)
        .collect();
    assert!(
        uses_edges.is_empty(),
        "fragment links should not create Uses edges"
    );
}

#[test]
fn test_markdown_handles_header_with_punctuation() {
    let source = "# Hello, World! (2024)";
    let result = MarkdownExtractor.extract("README.md", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "Hello, World! (2024)");
}
