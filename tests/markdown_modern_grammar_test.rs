//! Migration smoke tests for `tree-sitter-grammars/tree-sitter-markdown`.
//!
//! Pre-migration these inputs caused the markdown extractor to hang or
//! segfault because the old `ikatyang/tree-sitter-markdown` grammar parsed
//! YAML frontmatter as ambiguous markdown (GLR fork explosion). The new
//! grammar produces an opaque `(minus_metadata)` node so the body's
//! markdown rules never see the YAML.
use std::time::{Duration, Instant};

fn timed_extract(source: String, timeout: Duration) -> Option<(f64, usize, usize)> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let res = tokensave::extraction::MarkdownExtractor::extract_markdown("t.md", &source);
        let _ = tx.send((t0.elapsed().as_secs_f64(), res.nodes.len(), res.edges.len()));
    });
    rx.recv_timeout(timeout).ok()
}

/// 4.4 KB / 113-line YAML-frontmatter-heavy file that hung the old grammar
/// indefinitely. With the new grammar it must parse in well under a second.
#[test]
fn yaml_frontmatter_hang_reproducer() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/markdown_yaml_frontmatter_hang.md"
    );
    let src = std::fs::read_to_string(path).expect("fixture missing");
    match timed_extract(src, Duration::from_secs(5)) {
        Some((t, n, _)) => assert!(t < 1.0, "should parse fast, took {t:.3}s ({n} nodes)"),
        None => panic!("hang reproducer still hung > 5s"),
    }
}

#[test]
fn extracts_headings_and_links() {
    let src =
        "# Top\n\nSee [main](src/main.rs) for details.\n\n## Sub\n\nAlso [util](src/util.rs).\n";
    let res = tokensave::extraction::MarkdownExtractor::extract_markdown("doc.md", src);
    let modules: Vec<&str> = res
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, tokensave::types::NodeKind::Module))
        .map(|n| n.name.as_str())
        .collect();
    assert!(modules.contains(&"Top"), "got modules {modules:?}");
    assert!(modules.contains(&"Sub"), "got modules {modules:?}");
    let uses_count = res
        .edges
        .iter()
        .filter(|e| matches!(e.kind, tokensave::types::EdgeKind::Uses))
        .count();
    assert_eq!(uses_count, 2, "expected 2 Uses edges, got {uses_count}");
}

#[test]
fn frontmatter_is_opaque() {
    // YAML frontmatter content that would otherwise look like markdown
    // (a `# heading`-like line, a `- list item`) must NOT produce Module
    // nodes — it's metadata, not document structure.
    let src = "---\ntitle: My Doc\n# this is yaml comment style\n- bogus\n---\n\n# Real Heading\n";
    let res = tokensave::extraction::MarkdownExtractor::extract_markdown("doc.md", src);
    let module_names: Vec<&str> = res
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, tokensave::types::NodeKind::Module))
        .map(|n| n.name.as_str())
        .collect();
    assert_eq!(
        module_names,
        vec!["Real Heading"],
        "frontmatter content should not produce Module nodes"
    );
}
