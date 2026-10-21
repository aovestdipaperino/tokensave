#![cfg(feature = "lang-ruby")]

use tokensave::extraction::LanguageRegistry;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

#[test]
fn rake_uses_ruby_extractor() {
    let registry = LanguageRegistry::new();
    let extractor = registry
        .extractor_for_file("lib/tasks/sample.rake")
        .unwrap();
    assert_eq!(extractor.language_name(), "Ruby");
    assert!(registry.supported_extensions().contains(&"rake"));
    let result = extractor.extract(
        "lib/tasks/sample.rake",
        include_str!("fixtures/sample.rake"),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result
        .nodes
        .iter()
        .any(|node| node.kind == NodeKind::Module && node.name == "TaskHelpers"));
    assert!(result
        .unresolved_refs
        .iter()
        .any(|reference| reference.reference_kind == EdgeKind::Calls
            && reference.reference_name == "TaskHelpers.prepare"));
}

#[tokio::test]
async fn rake_indexing_resolves_ruby_relationships() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lib").join("tasks")).unwrap();
    std::fs::write(
        dir.path().join("lib").join("tasks").join("sample.rake"),
        include_str!("fixtures/sample.rake"),
    )
    .unwrap();
    std::fs::write(dir.path().join("helpers.rb"), "module SharedHelpers\nend\n").unwrap();
    let graph = TokenSave::init(dir.path()).await.unwrap();
    graph.index_all().await.unwrap();
    let files = graph.get_all_files().await.unwrap();
    assert!(files
        .iter()
        .any(|file| file.path == "lib/tasks/sample.rake"));
    let stats = graph.get_stats().await.unwrap();
    assert_eq!(stats.files_by_language.get("Ruby"), Some(&2));
    let nodes = graph.get_all_nodes().await.unwrap();
    let edges = graph.get_all_edges().await.unwrap();
    let runner = nodes.iter().find(|node| node.name == "TaskRunner").unwrap();
    let helpers = nodes
        .iter()
        .find(|node| node.name == "SharedHelpers")
        .unwrap();
    let run = nodes.iter().find(|node| node.name == "run").unwrap();
    let prepare = nodes.iter().find(|node| node.name == "prepare").unwrap();
    assert!(edges.iter().any(|edge| edge.kind == EdgeKind::Implements
        && edge.source == runner.id
        && edge.target == helpers.id));
    assert!(edges.iter().any(|edge| edge.kind == EdgeKind::Calls
        && edge.source == run.id
        && edge.target == prepare.id));
}
