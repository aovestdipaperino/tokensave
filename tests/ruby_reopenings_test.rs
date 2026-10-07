#![cfg(feature = "lang-ruby")]

use std::path::Path;

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;
use tokensave::types::{Edge, EdgeKind, Node, NodeKind};

fn write_project(root: &Path, first: &str) {
    std::fs::write(root.join("a.rb"), first).unwrap();
    std::fs::write(
        root.join("b.rb"),
        "class Atlas::Entry\n  def second; end\nend\n",
    )
    .unwrap();
    std::fs::write(
        root.join("c.rb"),
        "module Atlas\n  class Entry\n    def third; end\n  end\nend\n",
    )
    .unwrap();
    std::fs::write(root.join("d.rb"), "module Atlas::Entry\nend\n").unwrap();
    std::fs::write(
        root.join("e.rb"),
        "class ::Atlas::Entry\n  def absolute; end\nend\n",
    )
    .unwrap();
}

async fn declarations_and_reopenings(graph: &TokenSave) -> (Vec<Node>, Vec<Edge>) {
    let nodes = graph.db().get_all_nodes().await.unwrap();
    let mut edges = graph
        .db()
        .get_edges_by_kind(EdgeKind::Reopens)
        .await
        .unwrap();
    edges.sort_by(|a, b| (&a.source, &a.target, a.line).cmp(&(&b.source, &b.target, b.line)));
    (nodes, edges)
}

#[tokio::test]
async fn ruby_reopenings_follow_canonical_declarations_after_incremental_edits() {
    let root = tempdir().unwrap();
    write_project(
        root.path(),
        "module Atlas\n  class Entry\n    def first; end\n  end\nend\n",
    );
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();

    let (nodes, edges) = declarations_and_reopenings(&graph).await;
    let class_in = |file: &str| {
        nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.file_path == file)
            .unwrap()
    };
    let first = class_in("a.rb");
    let second = class_in("b.rb");
    let third = class_in("c.rb");
    let absolute = class_in("e.rb");
    assert!(first.qualified_name.ends_with("::Atlas::Entry"));
    assert!(second.qualified_name.ends_with("::Atlas::Entry"));
    assert!(third.qualified_name.ends_with("::Atlas::Entry"));
    assert_eq!(edges.len(), 4);
    assert!(edges
        .iter()
        .any(|e| e.source == second.id && e.target == first.id));
    assert!(edges
        .iter()
        .any(|e| e.source == third.id && e.target == first.id));
    assert!(edges
        .iter()
        .any(|e| e.source == absolute.id && e.target == first.id));

    std::fs::write(
        root.path().join("a.rb"),
        "module Atlas\n  class Other\n  end\nend\n",
    )
    .unwrap();
    graph.sync().await.unwrap();
    let (nodes, edges) = declarations_and_reopenings(&graph).await;
    let second = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.file_path == "b.rb")
        .unwrap();
    let third = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.file_path == "c.rb")
        .unwrap();
    let absolute = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.file_path == "e.rb")
        .unwrap();
    assert_eq!(edges.len(), 3);
    assert!(edges
        .iter()
        .any(|e| e.source == third.id && e.target == second.id));
    assert!(edges
        .iter()
        .any(|e| e.source == absolute.id && e.target == second.id));

    let fresh_root = tempdir().unwrap();
    write_project(
        fresh_root.path(),
        "module Atlas\n  class Other\n  end\nend\n",
    );
    let fresh = TokenSave::init(fresh_root.path()).await.unwrap();
    fresh.sync().await.unwrap();
    let (_, fresh_edges) = declarations_and_reopenings(&fresh).await;
    assert_eq!(edges, fresh_edges);

    std::fs::remove_file(root.path().join("b.rb")).unwrap();
    graph.sync().await.unwrap();
    let (nodes, edges) = declarations_and_reopenings(&graph).await;
    let third = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.file_path == "c.rb")
        .unwrap();
    let absolute = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.file_path == "e.rb")
        .unwrap();
    assert_eq!(edges.len(), 2);
    assert!(!edges.iter().any(|e| e.source == third.id));
    assert!(edges
        .iter()
        .any(|e| e.source == absolute.id && e.target == third.id));

    let deleted_root = tempdir().unwrap();
    write_project(
        deleted_root.path(),
        "module Atlas\n  class Other\n  end\nend\n",
    );
    std::fs::remove_file(deleted_root.path().join("b.rb")).unwrap();
    let deleted_fresh = TokenSave::init(deleted_root.path()).await.unwrap();
    deleted_fresh.sync().await.unwrap();
    let (_, deleted_fresh_edges) = declarations_and_reopenings(&deleted_fresh).await;
    assert_eq!(edges, deleted_fresh_edges);
}

#[tokio::test]
async fn ruby_reopenings_link_rake_declarations() {
    let root = tempdir().unwrap();
    std::fs::write(
        root.path().join("a.rb"),
        "class Deploy\n  def run; end\nend\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join("lib/tasks")).unwrap();
    std::fs::write(
        root.path().join("lib/tasks/deploy.rake"),
        "class Deploy\n  def task_helper; end\nend\n",
    )
    .unwrap();
    let graph = TokenSave::init(root.path()).await.unwrap();
    graph.sync().await.unwrap();

    let (nodes, edges) = declarations_and_reopenings(&graph).await;
    let class_in = |file: &str| {
        nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.file_path == file)
            .unwrap()
    };
    let rb = class_in("a.rb");
    let rake = class_in("lib/tasks/deploy.rake");
    assert_eq!(edges.len(), 1);
    assert!(edges
        .iter()
        .any(|e| e.source == rake.id && e.target == rb.id));
}

#[tokio::test]
async fn ruby_reopenings_follow_template_declarations_and_edits() {
    for (file, original, changed) in [
        (
            "view.html.erb",
            "<% class Entry; end %>",
            "<% class Other; end %>",
        ),
        (
            "view.html.slim",
            "ruby:\n  class Entry; end\n",
            "ruby:\n  class Other; end\n",
        ),
    ] {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("a.rb"), "class Entry; end\n").unwrap();
        std::fs::write(root.path().join(file), original).unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.sync().await.unwrap();
        let (nodes, edges) = declarations_and_reopenings(&graph).await;
        let template = nodes
            .iter()
            .find(|node| node.kind == NodeKind::Class && node.file_path == file)
            .unwrap_or_else(|| panic!("{file}: missing declaration"));
        let canonical = nodes
            .iter()
            .find(|node| node.kind == NodeKind::Class && node.file_path == "a.rb")
            .unwrap();
        assert_eq!(edges.len(), 1, "{file}");
        assert_eq!(edges[0].source, template.id);
        assert_eq!(edges[0].target, canonical.id);

        std::fs::write(root.path().join(file), changed).unwrap();
        graph.sync().await.unwrap();
        let (_, edges) = declarations_and_reopenings(&graph).await;
        assert!(edges.is_empty(), "{file}: stale reopening after edit");

        std::fs::write(root.path().join(file), original).unwrap();
        graph.sync().await.unwrap();
        let (_, edges) = declarations_and_reopenings(&graph).await;
        assert_eq!(edges.len(), 1, "{file}: missing reopening after edit");
    }
}

#[tokio::test]
async fn ruby_reopenings_rebuild_when_a_canonical_template_declaration_goes_away() {
    // The template sorts first, so it holds the canonical declaration both
    // Ruby files reopen. Dropping it, by edit or by deletion, must promote
    // b.rb, which needs the rebuild even though no `.rb` file changed.
    let edit: fn(&Path) =
        |root| std::fs::write(root.join("a.html.erb"), "<%= helper() %>").unwrap();
    let delete: fn(&Path) = |root| std::fs::remove_file(root.join("a.html.erb")).unwrap();
    for remove in [edit, delete] {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("a.html.erb"), "<% class Entry; end %>").unwrap();
        std::fs::write(root.path().join("b.rb"), "class Entry; end\n").unwrap();
        std::fs::write(root.path().join("c.rb"), "class Entry; end\n").unwrap();
        let graph = TokenSave::init(root.path()).await.unwrap();
        graph.sync().await.unwrap();
        let (nodes, edges) = declarations_and_reopenings(&graph).await;
        let template = nodes
            .iter()
            .find(|n| n.kind == NodeKind::Class && n.file_path == "a.html.erb")
            .unwrap();
        assert_eq!(edges.len(), 2, "{edges:?}");
        assert!(edges.iter().all(|e| e.target == template.id));

        remove(root.path());
        graph.sync().await.unwrap();
        let (nodes, edges) = declarations_and_reopenings(&graph).await;
        let class_in = |file: &str| {
            nodes
                .iter()
                .find(|n| n.kind == NodeKind::Class && n.file_path == file)
                .unwrap()
                .id
                .clone()
        };
        assert_eq!(edges.len(), 1, "{edges:?}");
        assert_eq!(edges[0].source, class_in("c.rb"));
        assert_eq!(edges[0].target, class_in("b.rb"));
    }
}
