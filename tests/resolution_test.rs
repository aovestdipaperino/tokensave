use tempfile::TempDir;
use tokensave::db::Database;
use tokensave::resolution::ReferenceResolver;
use tokensave::types::*;

/// Sets up a temporary database pre-populated with two nodes: a `helper`
/// function in `src/utils.rs` and a `main` function in `src/main.rs`.
async fn setup_db_with_nodes() -> (TempDir, Database) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let callee = Node {
        id: generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
        kind: NodeKind::Function,
        name: "helper".to_string(),
        qualified_name: "src/utils.rs::helper".to_string(),
        file_path: "src/utils.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn helper() -> i32".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    let caller = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    db.insert_node(&callee)
        .await
        .expect("failed to insert callee");
    db.insert_node(&caller)
        .await
        .expect("failed to insert caller");
    (dir, db)
}

#[tokio::test]
async fn test_resolve_exact_name_match() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let uref = UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve the helper reference");
    let resolved = result.unwrap();
    assert!(
        resolved.confidence >= 0.7,
        "confidence should be at least 0.7, got {}",
        resolved.confidence
    );
    assert_eq!(
        resolved.target_node_id,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
    );
}

#[tokio::test]
async fn test_resolve_qualified_name_match() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let uref = UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "src/utils.rs::helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve via qualified name match");
    let resolved = result.unwrap();
    assert!(
        (resolved.confidence - 0.95).abs() < f64::EPSILON,
        "qualified match should have confidence 0.95, got {}",
        resolved.confidence
    );
    assert_eq!(resolved.resolved_by, "qualified-match");
}

#[tokio::test]
async fn test_resolve_all() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let refs = vec![UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    }];

    let result = resolver.resolve_all(&refs);
    assert_eq!(result.total, 1);
    assert_eq!(result.resolved_count, 1);
    assert_eq!(result.resolved.len(), 1);
    assert!(result.unresolved.is_empty());
}

#[tokio::test]
async fn test_unresolvable_reference() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let uref = UnresolvedRef {
        from_node_id: "function:caller".to_string(),
        reference_name: "nonexistent".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 5,
        column: 8,
        file_path: "src/main.rs".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "nonexistent reference should not resolve"
    );
}

#[tokio::test]
async fn test_unresolvable_in_resolve_all() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let refs = vec![
        UnresolvedRef {
            from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
            reference_name: "helper".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3,
            column: 12,
            file_path: "src/main.rs".to_string(),
        },
        UnresolvedRef {
            from_node_id: "function:caller".to_string(),
            reference_name: "nonexistent".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 5,
            column: 8,
            file_path: "src/main.rs".to_string(),
        },
    ];

    let result = resolver.resolve_all(&refs);
    assert_eq!(result.total, 2);
    assert_eq!(result.resolved_count, 1);
    assert_eq!(result.unresolved.len(), 1);
    assert_eq!(
        refs[result.unresolved[0] as usize].reference_name, "nonexistent",
        "`unresolved` indexes the slice passed in (#483)"
    );
}

#[tokio::test]
async fn test_creates_edges_from_resolved() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let resolved = ResolvedRef {
        original: UnresolvedRef {
            from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
            reference_name: "helper".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3,
            column: 12,
            file_path: "src/main.rs".to_string(),
        },
        target_node_id: generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
        confidence: 0.9,
        resolved_by: "exact-match".to_string(),
    };

    let edges = resolver.create_edges(&[resolved]);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, EdgeKind::Calls);
    assert_eq!(edges[0].line, Some(3));
    assert_eq!(
        edges[0].source,
        generate_node_id("src/main.rs", &NodeKind::Function, "main", 1)
    );
    assert_eq!(
        edges[0].target,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1)
    );
}

#[tokio::test]
async fn test_multiple_candidates_best_match_scoring() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    // Two nodes with the same name "process" in different files.
    let same_file_node = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "process", 10),
        kind: NodeKind::Function,
        name: "process".to_string(),
        qualified_name: "src/main.rs::process".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 10,
        attrs_start_line: 10,
        end_line: 15,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    let other_file_node = Node {
        id: generate_node_id("src/other.rs", &NodeKind::Function, "process", 1),
        kind: NodeKind::Function,
        name: "process".to_string(),
        qualified_name: "src/other.rs::process".to_string(),
        file_path: "src/other.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process()".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    let caller = Node {
        id: generate_node_id("src/main.rs", &NodeKind::Function, "run", 1),
        kind: NodeKind::Function,
        name: "run".to_string(),
        qualified_name: "src/main.rs::run".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn run()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    };

    db.insert_node(&same_file_node)
        .await
        .expect("failed to insert same_file_node");
    db.insert_node(&other_file_node)
        .await
        .expect("failed to insert other_file_node");
    db.insert_node(&caller)
        .await
        .expect("failed to insert caller");

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    // Reference from src/main.rs should prefer the same-file candidate.
    let uref = UnresolvedRef {
        from_node_id: caller.id.clone(),
        reference_name: "process".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 4,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve with multiple candidates");
    let resolved = result.unwrap();
    assert_eq!(
        resolved.target_node_id, same_file_node.id,
        "should prefer the same-file candidate"
    );
    assert!(
        (resolved.confidence - 0.7).abs() < f64::EPSILON,
        "multiple-match confidence should be 0.7, got {}",
        resolved.confidence
    );
}

#[tokio::test]
async fn test_create_edges_empty_input() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let edges = resolver.create_edges(&[]);
    assert!(edges.is_empty());
}

#[tokio::test]
async fn test_resolve_all_empty_input() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let result = resolver.resolve_all(&[]);
    assert_eq!(result.total, 0);
    assert_eq!(result.resolved_count, 0);
    assert!(result.resolved.is_empty());
    assert!(result.unresolved.is_empty());
}

/// #141 regression: `resolve_all`'s pre-filter must not drop a qualified
/// `Self::helper` (or `Type::helper`) ref just because the literal string
/// isn't a known name — its trailing simple name is, and `resolve_one`
/// strips the prefix and matches it. Previously these were silently lost.
#[tokio::test]
async fn test_resolve_all_self_qualified_call_not_dropped() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let refs = vec![UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "Self::helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    }];

    let result = resolver.resolve_all(&refs);
    assert_eq!(
        result.resolved_count, 1,
        "Self::helper should resolve via the simple-name fallback, not be pre-filtered as hopeless"
    );
    assert_eq!(
        result.resolved[0].target_node_id,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
    );
}

/// #141 cross-language: Python/TS extractors emit the full dotted callee
/// (`obj.helper`) with no bare-name ref. The resolver must fall back to the
/// trailing method name so the call edge still forms.
#[tokio::test]
async fn test_resolve_all_dotted_method_call() {
    let (_dir, db) = setup_db_with_nodes().await;
    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);

    let refs = vec![UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", &NodeKind::Function, "main", 1),
        reference_name: "obj.helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    }];

    let result = resolver.resolve_all(&refs);
    assert_eq!(
        result.resolved_count, 1,
        "obj.helper should resolve to `helper` via the dotted-call fallback"
    );
    assert_eq!(
        result.resolved[0].target_node_id,
        generate_node_id("src/utils.rs", &NodeKind::Function, "helper", 1),
    );
}

#[tokio::test]
async fn test_ruby_receiver_calls_do_not_fall_back_to_bare_names() {
    let (_dir, db) = setup_db_with_nodes().await;
    let mut ruby_helper = db
        .get_all_nodes()
        .await
        .unwrap()
        .into_iter()
        .find(|node| node.name == "helper")
        .unwrap();
    ruby_helper.id = generate_node_id("app/helper.rb", &NodeKind::Method, "helper", 1);
    ruby_helper.kind = NodeKind::Method;
    ruby_helper.qualified_name = "app/helper.rb::Helper::helper".to_string();
    ruby_helper.file_path = "app/helper.rb".to_string();
    ruby_helper.signature = Some("def helper".to_string());
    db.insert_node(&ruby_helper).await.unwrap();

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let from_node_id = generate_node_id("app/service.rb", &NodeKind::Method, "run", 1);

    let names = [
        "worker.helper",
        "self.helper",
        "Worker.helper",
        "Namespace::Worker.helper",
        "worker&.helper",
        "worker::helper",
        "helper",
    ];
    let refs: Vec<_> = names
        .iter()
        .map(|name| UnresolvedRef {
            from_node_id: from_node_id.clone(),
            reference_name: (*name).to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3,
            column: 4,
            file_path: "app/service.rb".to_string(),
        })
        .collect();

    let result = resolver.resolve_all(&refs);
    assert_eq!(
        result.resolved_count, 1,
        "only the bare Ruby call should resolve"
    );
    assert_eq!(result.resolved[0].original.reference_name, "helper");

    let unresolved_names: Vec<_> = result
        .unresolved
        .iter()
        .map(|i| refs[*i as usize].reference_name.as_str())
        .collect();
    for receiver_call in &names[..names.len() - 1] {
        assert!(
            unresolved_names.contains(receiver_call),
            "receiver-qualified Ruby call {receiver_call:?} must remain unresolved"
        );
    }
}

fn ruby_node(
    file_path: &str,
    kind: NodeKind,
    name: &str,
    line: u32,
    signature: &str,
    parent: Option<&Node>,
) -> Node {
    let qualified_name = parent.map_or_else(
        || format!("{file_path}::{name}"),
        |owner| format!("{}::{name}", owner.qualified_name),
    );
    Node {
        id: generate_node_id(file_path, &kind, name, line),
        kind,
        name: name.to_string(),
        qualified_name,
        file_path: file_path.to_string(),
        start_line: line,
        attrs_start_line: line,
        end_line: line,
        start_column: 0,
        end_column: 0,
        signature: Some(signature.to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: parent.map(|owner| owner.id.clone()),
    }
}

#[tokio::test]
async fn test_ruby_receiver_calls_resolve_only_with_explicit_singleton_evidence() {
    let (_dir, db) = setup_db_with_nodes().await;

    let service = ruby_node(
        "app/services/report.rb",
        NodeKind::Class,
        "Report",
        1,
        "class Report",
        None,
    );
    let singleton_caller = ruby_node(
        "app/services/report.rb",
        NodeKind::SingletonMethod,
        "run",
        2,
        "def self.run",
        Some(&service),
    );
    let instance_caller = ruby_node(
        "app/services/report.rb",
        NodeKind::Method,
        "instance_run",
        3,
        "def instance_run",
        Some(&service),
    );
    let singleton_target = ruby_node(
        "app/services/report.rb",
        NodeKind::SingletonMethod,
        "publish",
        4,
        "def self.publish; end",
        Some(&service),
    );
    let same_named_instance_target = ruby_node(
        "app/services/report.rb",
        NodeKind::Method,
        "publish",
        5,
        "def publish",
        Some(&service),
    );
    let instance_target = ruby_node(
        "app/services/report.rb",
        NodeKind::Method,
        "archive",
        6,
        "def archive",
        Some(&service),
    );

    let capture = ruby_node(
        "app/services/payments/capture.rb",
        NodeKind::Class,
        "Payments::Capture",
        1,
        "class Payments::Capture",
        None,
    );
    let capture_call = ruby_node(
        "app/services/payments/capture.rb",
        NodeKind::SingletonMethod,
        "call",
        2,
        "def self.call(value)",
        Some(&capture),
    );

    let instance_only = ruby_node(
        "app/models/instance_only.rb",
        NodeKind::Class,
        "InstanceOnly",
        1,
        "class InstanceOnly",
        None,
    );
    let instance_perform = ruby_node(
        "app/models/instance_only.rb",
        NodeKind::Method,
        "perform",
        2,
        "def perform",
        Some(&instance_only),
    );

    let ledger = ruby_node(
        "app/models/ledger.rb",
        NodeKind::Class,
        "Ledger",
        1,
        "class Ledger",
        None,
    );
    let singleton_class_total = ruby_node(
        "app/models/ledger.rb",
        NodeKind::SingletonMethod,
        "total",
        3,
        "def total",
        Some(&ledger),
    );
    let unrelated = ruby_node(
        "app/services/unrelated.rb",
        NodeKind::Class,
        "Unrelated",
        1,
        "class Unrelated",
        None,
    );
    let unrelated_publish = ruby_node(
        "app/services/unrelated.rb",
        NodeKind::SingletonMethod,
        "publish",
        2,
        "def self.publish",
        Some(&unrelated),
    );
    let announcer = ruby_node(
        "app/services/announcer.rb",
        NodeKind::Module,
        "Announcer",
        1,
        "module Announcer",
        None,
    );
    let announcer_publish = ruby_node(
        "app/services/announcer.rb",
        NodeKind::SingletonMethod,
        "publish",
        2,
        "def self.publish",
        Some(&announcer),
    );

    for node in [
        &service,
        &singleton_caller,
        &instance_caller,
        &singleton_target,
        &same_named_instance_target,
        &instance_target,
        &capture,
        &capture_call,
        &instance_only,
        &instance_perform,
        &ledger,
        &singleton_class_total,
        &unrelated,
        &unrelated_publish,
        &announcer,
        &announcer_publish,
    ] {
        db.insert_node(node).await.unwrap();
    }

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let reference = |from_node_id: &str, name: &str| UnresolvedRef {
        from_node_id: from_node_id.to_string(),
        reference_name: name.to_string(),
        reference_kind: EdgeKind::Calls,
        line: 10,
        column: 4,
        file_path: "app/services/report.rb".to_string(),
    };

    for name in [
        "Payments::Capture.call",
        "Payments::Capture&.call",
        "Payments::Capture::call",
    ] {
        let resolved = resolver
            .resolve_one(&reference(&singleton_caller.id, name))
            .unwrap();
        assert_eq!(resolved.target_node_id, capture_call.id);
        assert_eq!(resolved.resolved_by, "ruby-constant-receiver");
        assert_eq!(resolved.confidence, 0.95);
    }

    let self_call = resolver
        .resolve_one(&reference(&singleton_caller.id, "self.publish"))
        .unwrap();
    assert_eq!(self_call.target_node_id, singleton_target.id);
    assert_eq!(self_call.resolved_by, "ruby-self-receiver");

    let class_body_self_call = resolver
        .resolve_one(&reference(&service.id, "self.publish"))
        .unwrap();
    assert_eq!(class_body_self_call.target_node_id, singleton_target.id);

    let module_body_self_call = resolver
        .resolve_one(&reference(&announcer.id, "self.publish"))
        .unwrap();
    assert_eq!(module_body_self_call.target_node_id, announcer_publish.id);

    let singleton_class_call = resolver
        .resolve_one(&reference(&singleton_caller.id, "Ledger.total"))
        .unwrap();
    assert_eq!(
        singleton_class_call.target_node_id,
        singleton_class_total.id
    );
    assert_eq!(singleton_class_call.resolved_by, "ruby-constant-receiver");

    for (caller, name) in [
        (&singleton_caller.id, "InstanceOnly.perform"),
        (&instance_caller.id, "self.publish"),
        (&singleton_caller.id, "self.archive"),
        (&singleton_caller.id, "worker.publish"),
        (&singleton_caller.id, "@worker.publish"),
        (&singleton_caller.id, "account.owner.publish"),
        (&singleton_caller.id, "\"Report\".publish"),
        (&singleton_caller.id, "Unknown.publish"),
    ] {
        assert!(
            resolver.resolve_one(&reference(caller, name)).is_none(),
            "unsupported Ruby receiver call {name:?} must remain unresolved"
        );
    }
}

#[tokio::test]
async fn test_ruby_receiver_calls_reject_ambiguous_owners_and_targets() {
    let (_dir, db) = setup_db_with_nodes().await;

    let top_user = ruby_node(
        "app/models/user.rb",
        NodeKind::Class,
        "User",
        1,
        "class User",
        None,
    );
    let top_find = ruby_node(
        "app/models/user.rb",
        NodeKind::SingletonMethod,
        "find",
        2,
        "def self.find",
        Some(&top_user),
    );
    let admin_user = ruby_node(
        "app/models/admin/user.rb",
        NodeKind::Class,
        "Admin::User",
        1,
        "class Admin::User",
        None,
    );
    let capture_a = ruby_node(
        "app/services/capture_a.rb",
        NodeKind::Class,
        "Payments::Capture",
        1,
        "class Payments::Capture",
        None,
    );
    let call_a = ruby_node(
        "app/services/capture_a.rb",
        NodeKind::SingletonMethod,
        "call",
        2,
        "def self.call",
        Some(&capture_a),
    );
    let capture_b = ruby_node(
        "app/services/capture_b.rb",
        NodeKind::Class,
        "Payments::Capture",
        1,
        "class Payments::Capture",
        None,
    );
    let call_b = ruby_node(
        "app/services/capture_b.rb",
        NodeKind::SingletonMethod,
        "call",
        2,
        "def self.call",
        Some(&capture_b),
    );

    for node in [
        &top_user,
        &top_find,
        &admin_user,
        &capture_a,
        &call_a,
        &capture_b,
        &call_b,
    ] {
        db.insert_node(node).await.unwrap();
    }

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let reference = |name: &str| UnresolvedRef {
        from_node_id: "caller".to_string(),
        reference_name: name.to_string(),
        reference_kind: EdgeKind::Calls,
        line: 1,
        column: 0,
        file_path: "app/services/caller.rb".to_string(),
    };

    assert!(resolver.resolve_one(&reference("User.find")).is_none());
    let absolute = resolver.resolve_one(&reference("::User.find")).unwrap();
    assert_eq!(absolute.target_node_id, top_find.id);
    assert!(resolver
        .resolve_one(&reference("Payments::Capture.call"))
        .is_none());
}

#[tokio::test]
async fn test_ruby_receiver_calls_respect_lexical_constant_shadowing() {
    let (_dir, db) = setup_db_with_nodes().await;

    let service = ruby_node(
        "app/service.rb",
        NodeKind::Class,
        "Service",
        1,
        "class Service",
        None,
    );
    let top_run = ruby_node(
        "app/service.rb",
        NodeKind::SingletonMethod,
        "run",
        2,
        "def self.run",
        Some(&service),
    );
    let admin = ruby_node(
        "app/admin.rb",
        NodeKind::Module,
        "Admin",
        1,
        "module Admin",
        None,
    );
    let shadow = ruby_node(
        "app/admin.rb",
        NodeKind::Const,
        "Service",
        2,
        "Service = Object.new",
        Some(&admin),
    );
    let payments = ruby_node(
        "app/payments/capture.rb",
        NodeKind::Module,
        "Payments",
        1,
        "module Payments",
        None,
    );
    let capture = ruby_node(
        "app/payments/capture.rb",
        NodeKind::Class,
        "Capture",
        2,
        "class Capture",
        Some(&payments),
    );
    let capture_call = ruby_node(
        "app/payments/capture.rb",
        NodeKind::SingletonMethod,
        "call",
        3,
        "def self.call",
        Some(&capture),
    );
    let payments_shadow = ruby_node(
        "app/admin.rb",
        NodeKind::Const,
        "Payments",
        3,
        "Payments = Object.new",
        Some(&admin),
    );

    for node in [
        &service,
        &top_run,
        &admin,
        &shadow,
        &payments,
        &capture,
        &capture_call,
        &payments_shadow,
    ] {
        db.insert_node(node).await.unwrap();
    }

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let reference = |name: &str| UnresolvedRef {
        from_node_id: admin.id.clone(),
        reference_name: name.to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 2,
        file_path: admin.file_path.clone(),
    };

    assert!(resolver.resolve_one(&reference("Service.run")).is_none());
    assert!(resolver
        .resolve_one(&reference("Payments::Capture.call"))
        .is_none());
    let absolute = resolver.resolve_one(&reference("::Service.run")).unwrap();
    assert_eq!(absolute.target_node_id, top_run.id);
    let absolute_qualified = resolver
        .resolve_one(&reference("::Payments::Capture.call"))
        .unwrap();
    assert_eq!(absolute_qualified.target_node_id, capture_call.id);
}

#[tokio::test]
async fn test_ruby_receiver_calls_choose_the_innermost_lexical_constant() {
    let (_dir, db) = setup_db_with_nodes().await;

    let top_service = ruby_node(
        "app/service.rb",
        NodeKind::Class,
        "Service",
        1,
        "class Service",
        None,
    );
    let top_run = ruby_node(
        "app/service.rb",
        NodeKind::SingletonMethod,
        "run",
        2,
        "def self.run",
        Some(&top_service),
    );
    let admin = ruby_node(
        "app/admin.rb",
        NodeKind::Module,
        "Admin",
        1,
        "module Admin",
        None,
    );
    let lexical_service = ruby_node(
        "app/admin.rb",
        NodeKind::Class,
        "Service",
        2,
        "class Service",
        Some(&admin),
    );
    let lexical_run = ruby_node(
        "app/admin.rb",
        NodeKind::SingletonMethod,
        "run",
        3,
        "def self.run",
        Some(&lexical_service),
    );
    let runner = ruby_node(
        "app/admin.rb",
        NodeKind::Class,
        "Runner",
        4,
        "class Runner",
        Some(&admin),
    );
    let caller = ruby_node(
        "app/admin.rb",
        NodeKind::SingletonMethod,
        "execute",
        5,
        "def self.execute",
        Some(&runner),
    );
    let other = ruby_node(
        "app/other.rb",
        NodeKind::Module,
        "Other",
        1,
        "module Other",
        None,
    );
    let unrelated_job = ruby_node(
        "app/other.rb",
        NodeKind::Class,
        "Job",
        2,
        "class Job",
        Some(&other),
    );
    let unrelated_run = ruby_node(
        "app/other.rb",
        NodeKind::SingletonMethod,
        "run",
        3,
        "def self.run",
        Some(&unrelated_job),
    );

    for node in [
        &top_service,
        &top_run,
        &admin,
        &lexical_service,
        &lexical_run,
        &runner,
        &caller,
        &other,
        &unrelated_job,
        &unrelated_run,
    ] {
        db.insert_node(node).await.unwrap();
    }

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let reference = |name: &str| UnresolvedRef {
        from_node_id: caller.id.clone(),
        reference_name: name.to_string(),
        reference_kind: EdgeKind::Calls,
        line: 6,
        column: 4,
        file_path: caller.file_path.clone(),
    };

    let lexical = resolver.resolve_one(&reference("Service.run")).unwrap();
    assert_eq!(lexical.target_node_id, lexical_run.id);
    let absolute = resolver.resolve_one(&reference("::Service.run")).unwrap();
    assert_eq!(absolute.target_node_id, top_run.id);
    assert!(resolver.resolve_one(&reference("Job.run")).is_none());
}

// ---------------------------------------------------------------------------
// Ruby mixins: `kind_compatible` resolves a Ruby `Implements` ref
// exclusively to a `NodeKind::Module` target, and only when the ref comes
// from a Ruby file. The tests below lock the language guard from both
// directions, then lock the exclusivity (no Class/Extends leakage).
// ---------------------------------------------------------------------------

/// `include Comparable` in a `.rb` file must resolve to a `NodeKind::Module`
/// node — this fails before the `kind_compatible` change, since `Module`
/// wasn't in the allowed target-kind list for `Implements` refs at all.
#[tokio::test]
async fn test_ruby_module_target_resolves_for_ruby_implements_ref() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let module_node = variant_node(
        &generate_node_id(
            "app/models/concerns/comparable.rb",
            &NodeKind::Module,
            "Comparable",
            1,
        ),
        NodeKind::Module,
        "Comparable",
        "app/models/concerns/comparable.rb::Comparable",
        "app/models/concerns/comparable.rb",
    );

    let resolver = ReferenceResolver::from_nodes(&db, std::slice::from_ref(&module_node));

    let uref = UnresolvedRef {
        from_node_id: "class:c".to_string(),
        reference_name: "Comparable".to_string(),
        reference_kind: EdgeKind::Implements,
        line: 2,
        column: 2,
        file_path: "app/models/c.rb".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(
        result.is_some(),
        "a Ruby Implements ref should resolve to a Module target"
    );
    assert_eq!(result.unwrap().target_node_id, module_node.id);
}

/// Regression guard: the same `NodeKind::Module` target must NOT resolve an
/// Implements ref coming from a non-Ruby (`.rs`) file. If someone later
/// widens the `kind_compatible` allowance to every language instead of
/// gating it on `lang_from_path(&uref.file_path) == "ruby"`, this test
/// fails.
#[tokio::test]
async fn test_ruby_module_target_does_not_resolve_for_non_ruby_implements_ref() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let module_node = variant_node(
        &generate_node_id("src/comparable.rs", &NodeKind::Module, "comparable", 1),
        NodeKind::Module,
        "comparable",
        "src/comparable.rs::comparable",
        "src/comparable.rs",
    );

    let resolver = ReferenceResolver::from_nodes(&db, std::slice::from_ref(&module_node));

    let uref = UnresolvedRef {
        from_node_id: "struct:c".to_string(),
        reference_name: "comparable".to_string(),
        reference_kind: EdgeKind::Implements,
        line: 2,
        column: 2,
        file_path: "src/c.rs".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "a non-Ruby Implements ref must not resolve to a Module target"
    );
}

/// Ruby forbids mixing in a class (`include SomeClass` raises `TypeError:
/// wrong argument type Class (expected Module)`), so a Ruby `Implements` ref
/// must never resolve to a `NodeKind::Class` target — even though `Class` is
/// in the shared Implements/Extends/DerivesMacro allow-list for every other
/// language. Fails before the fix, since the old rule was additive
/// (shared list `||` Module) rather than exclusive for Ruby.
#[tokio::test]
async fn test_ruby_implements_does_not_resolve_to_class() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let class_node = variant_node(
        &generate_node_id("app/models/foo.rb", &NodeKind::Class, "Foo", 1),
        NodeKind::Class,
        "Foo",
        "app/models/foo.rb::Foo",
        "app/models/foo.rb",
    );

    let resolver = ReferenceResolver::from_nodes(&db, std::slice::from_ref(&class_node));

    let uref = UnresolvedRef {
        from_node_id: "class:c".to_string(),
        reference_name: "Foo".to_string(),
        reference_kind: EdgeKind::Implements,
        line: 2,
        column: 2,
        file_path: "app/models/c.rb".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "a Ruby Implements ref must not resolve to a Class target"
    );
}

/// When a project indexes both a `class Foo` and a `module Foo`, a Ruby
/// `Implements` ref for `Foo` must resolve to the module, not the class.
/// The class's qualified name (`app/models/a_klass.rb::Foo`) sorts before
/// the module's (`app/models/concerns/z_mixin.rb::Foo`) in the
/// lexicographically sorted suffix index, so before the fix `try_qualified_match`
/// deterministically picks the class first.
#[tokio::test]
async fn test_ruby_implements_prefers_module_over_same_named_class() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let class_node = variant_node(
        &generate_node_id("app/models/a_klass.rb", &NodeKind::Class, "Foo", 1),
        NodeKind::Class,
        "Foo",
        "app/models/a_klass.rb::Foo",
        "app/models/a_klass.rb",
    );
    let module_node = variant_node(
        &generate_node_id(
            "app/models/concerns/z_mixin.rb",
            &NodeKind::Module,
            "Foo",
            1,
        ),
        NodeKind::Module,
        "Foo",
        "app/models/concerns/z_mixin.rb::Foo",
        "app/models/concerns/z_mixin.rb",
    );

    let nodes = vec![class_node.clone(), module_node.clone()];
    let resolver = ReferenceResolver::from_nodes(&db, &nodes);

    let uref = UnresolvedRef {
        from_node_id: "class:c".to_string(),
        reference_name: "Foo".to_string(),
        reference_kind: EdgeKind::Implements,
        line: 2,
        column: 2,
        file_path: "app/models/c.rb".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(
        result.is_some(),
        "a Ruby Implements ref for a duplicate name should still resolve"
    );
    assert_eq!(
        result.unwrap().target_node_id,
        module_node.id,
        "the module must win over the same-named class"
    );
}

/// Ruby's `class Foo < Bar` superclass ref must not resolve to a
/// `NodeKind::Module` target — a superclass must be a class. Guards the
/// second half of the over-permissive guard: it applied to `Extends` too,
/// even though Ruby never emits an `Extends` ref that could plausibly target
/// a module.
#[tokio::test]
async fn test_ruby_extends_does_not_resolve_to_module() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let module_node = variant_node(
        &generate_node_id("app/models/concerns/bar.rb", &NodeKind::Module, "Bar", 1),
        NodeKind::Module,
        "Bar",
        "app/models/concerns/bar.rb::Bar",
        "app/models/concerns/bar.rb",
    );

    let resolver = ReferenceResolver::from_nodes(&db, std::slice::from_ref(&module_node));

    let uref = UnresolvedRef {
        from_node_id: "class:c".to_string(),
        reference_name: "Bar".to_string(),
        reference_kind: EdgeKind::Extends,
        line: 2,
        column: 2,
        file_path: "app/models/c.rb".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "a Ruby Extends ref must not resolve to a Module target"
    );
}

/// Positive control: Ruby superclass resolution (`class Foo < Bar`) must
/// still work for an ordinary class target — proves the narrowing didn't
/// break the one Ruby `Extends` path, which has no other coverage.
#[tokio::test]
async fn test_ruby_extends_resolves_to_class() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let class_node = variant_node(
        &generate_node_id("app/models/bar.rb", &NodeKind::Class, "Bar", 1),
        NodeKind::Class,
        "Bar",
        "app/models/bar.rb::Bar",
        "app/models/bar.rb",
    );

    let resolver = ReferenceResolver::from_nodes(&db, std::slice::from_ref(&class_node));

    let uref = UnresolvedRef {
        from_node_id: "class:c".to_string(),
        reference_name: "Bar".to_string(),
        reference_kind: EdgeKind::Extends,
        line: 2,
        column: 2,
        file_path: "app/models/c.rb".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(
        result.is_some(),
        "a Ruby Extends ref should still resolve to a Class target"
    );
    assert_eq!(result.unwrap().target_node_id, class_node.id);
}

// ---------------------------------------------------------------------------
// The resolver never produces `annotates` edges: `kind_compatible` returns
// `false` for every target kind under `EdgeKind::Annotates`. Extractors emit
// the attachment edge (usage -> decorated item) directly; that is the only
// relation `annotates` names to any consumer, so the resolver has nothing to
// add. These tests pin that an `Annotates` ref stays unresolved against
// every kind of candidate that could otherwise look like a match: a sibling
// usage (self- or cross-node), a real `Annotation` declaration, and a
// `Decorator` node (which is itself a usage-site node, not a declaration —
// emitted at the `@foo(...)` application site, not the `def`/`class` it
// decorates).
// ---------------------------------------------------------------------------

/// Two `@override` usages in one file, each with an `Annotates` ref named
/// "override": neither may resolve to the other (self-edge) or to its
/// sibling (cross-node phantom).
#[tokio::test]
async fn test_annotation_ref_does_not_bind_to_sibling_usage() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let usage_a = variant_node(
        "au:override:a",
        NodeKind::AnnotationUsage,
        "override",
        "lib/a.dart::override",
        "lib/a.dart",
    );
    let usage_b = variant_node(
        "au:override:b",
        NodeKind::AnnotationUsage,
        "override",
        "lib/a.dart::override",
        "lib/a.dart",
    );

    let nodes = vec![usage_a.clone(), usage_b.clone()];
    let resolver = ReferenceResolver::from_nodes(&db, &nodes);

    let uref_a = UnresolvedRef {
        from_node_id: usage_a.id.clone(),
        reference_name: "override".to_string(),
        reference_kind: EdgeKind::Annotates,
        line: 1,
        column: 1,
        file_path: "lib/a.dart".to_string(),
    };
    let uref_b = UnresolvedRef {
        from_node_id: usage_b.id.clone(),
        reference_name: "override".to_string(),
        reference_kind: EdgeKind::Annotates,
        line: 5,
        column: 1,
        file_path: "lib/a.dart".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref_a).is_none(),
        "an Annotates ref must not resolve to a sibling AnnotationUsage (self or cross-node)"
    );
    assert!(
        resolver.resolve_one(&uref_b).is_none(),
        "an Annotates ref must not resolve to a sibling AnnotationUsage (self or cross-node)"
    );
}

/// An `Annotates` ref must not resolve to a real `Annotation` declaration
/// (e.g. Java `@interface`) either: the resolver produces no `annotates`
/// edges at all, since the extractor already emits the attachment edge
/// directly and no consumer reads a resolver-produced usage -> declaration
/// edge under this kind as attachment.
#[tokio::test]
async fn test_annotation_ref_does_not_bind_to_declaration() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let decl_node = variant_node(
        "an:JsonSerializable",
        NodeKind::Annotation,
        "JsonSerializable",
        "lib/model.dart::JsonSerializable",
        "lib/model.dart",
    );
    let usage_node = variant_node(
        "au:JsonSerializable",
        NodeKind::AnnotationUsage,
        "JsonSerializable",
        "lib/a.dart::JsonSerializable",
        "lib/a.dart",
    );

    let nodes = vec![decl_node, usage_node];
    let resolver = ReferenceResolver::from_nodes(&db, &nodes);

    let uref = UnresolvedRef {
        from_node_id: "au:JsonSerializable".to_string(),
        reference_name: "JsonSerializable".to_string(),
        reference_kind: EdgeKind::Annotates,
        line: 1,
        column: 1,
        file_path: "lib/a.dart".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "an Annotates ref must not resolve to an Annotation declaration"
    );
}

/// `NodeKind::Decorator` is a usage-site node (emitted at the `@foo(...)`
/// application site, not the declaration it decorates), so it must not be a
/// valid `Annotates` target either.
#[tokio::test]
async fn test_annotation_ref_does_not_bind_to_decorator() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");

    let decorator_node = variant_node(
        "dec:retry",
        NodeKind::Decorator,
        "retry",
        "lib/decorators.py::retry",
        "lib/decorators.py",
    );

    let resolver = ReferenceResolver::from_nodes(&db, std::slice::from_ref(&decorator_node));

    let uref = UnresolvedRef {
        from_node_id: "fn:call_api".to_string(),
        reference_name: "retry".to_string(),
        reference_kind: EdgeKind::Annotates,
        line: 1,
        column: 1,
        file_path: "lib/api.py".to_string(),
    };

    assert!(
        resolver.resolve_one(&uref).is_none(),
        "an Annotates ref must not resolve to a Decorator usage node"
    );
}

// ---------------------------------------------------------------------------
// #141 Option 2: build-variant call-edge propagation
// ---------------------------------------------------------------------------

fn variant_node(id: &str, kind: NodeKind, name: &str, qn: &str, file: &str) -> Node {
    Node {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        qualified_name: qn.to_string(),
        file_path: file.to_string(),
        start_line: 1,
        attrs_start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some(format!("fn {name}()")),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        cognitive_complexity: 0,
        distinct_operators: 0,
        distinct_operands: 0,
        total_operators: 0,
        total_operands: 0,
        updated_at: 0,
        parent_id: None,
    }
}

fn calls_edge(from: &str, to: &str) -> Edge {
    Edge {
        source: from.to_string(),
        target: to.to_string(),
        kind: EdgeKind::Calls,
        line: Some(1),
    }
}

/// Rust `#[cfg]` twins (same qualified_name, both cfg-gated): a call landing on
/// one variant must propagate to the other so neither looks dead.
#[test]
fn test_variant_fanout_rust_cfg() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "main",
            "src/main.rs::main",
            "src/main.rs",
        ),
        variant_node(
            "fn:macos",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        variant_node(
            "fn:other",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        variant_node(
            "au:1",
            NodeKind::AnnotationUsage,
            "cfg",
            "src/c.rs::cfg",
            "src/c.rs",
        ),
        variant_node(
            "au:2",
            NodeKind::AnnotationUsage,
            "cfg",
            "src/c.rs::cfg",
            "src/c.rs",
        ),
    ];
    let edges = vec![
        Edge {
            source: "au:1".into(),
            target: "fn:macos".into(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        Edge {
            source: "au:2".into(),
            target: "fn:other".into(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        calls_edge("fn:caller", "fn:macos"),
    ];
    let extra = tokensave::resolution::propagate_variant_edges(&nodes, &edges);
    assert!(
        extra.iter().any(|e| e.source == "fn:caller"
            && e.target == "fn:other"
            && e.kind == EdgeKind::Calls),
        "call should propagate to the cfg sibling, got: {extra:?}"
    );
}

/// Go platform files (`foo_linux.go` / `foo_windows.go`): same package
/// directory + function name across different files = build variants.
#[test]
fn test_variant_fanout_go_platform_files() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "Main",
            "pkg/main.go::Main",
            "pkg/main.go",
        ),
        variant_node(
            "fn:linux",
            NodeKind::Function,
            "Do",
            "pkg/foo_linux.go::Do",
            "pkg/foo_linux.go",
        ),
        variant_node(
            "fn:win",
            NodeKind::Function,
            "Do",
            "pkg/foo_windows.go::Do",
            "pkg/foo_windows.go",
        ),
    ];
    let edges = vec![calls_edge("fn:caller", "fn:linux")];
    let extra = tokensave::resolution::propagate_variant_edges(&nodes, &edges);
    assert!(
        extra
            .iter()
            .any(|e| e.source == "fn:caller" && e.target == "fn:win"),
        "call should propagate to the windows platform-file sibling, got: {extra:?}"
    );
}

/// Negative: two functions sharing a qualified_name but NOT cfg-gated (e.g.
/// distinct trait impls) must NOT be fused — that would invent false edges.
#[test]
fn test_no_fanout_without_cfg() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "main",
            "src/main.rs::main",
            "src/main.rs",
        ),
        variant_node(
            "m:a",
            NodeKind::Method,
            "from",
            "src/t.rs::T::from",
            "src/t.rs",
        ),
        variant_node(
            "m:b",
            NodeKind::Method,
            "from",
            "src/t.rs::T::from",
            "src/t.rs",
        ),
    ];
    let edges = vec![calls_edge("fn:caller", "m:a")];
    let extra = tokensave::resolution::propagate_variant_edges(&nodes, &edges);
    assert!(
        extra.is_empty(),
        "non-cfg same-qualified-name nodes must not fan out, got: {extra:?}"
    );
}

/// #418: the sync paths pass `propagate_variant_edges` only the `Annotates` and
/// `Calls` edges, because those are the only kinds it reads. This pins that
/// equivalence, so a change that starts reading a third kind fails here rather
/// than silently losing variant edges during an incremental sync.
///
/// The noise edges are shaped so that reading any one of them *would* change
/// the result: each runs from a `cfg` annotation node to a third same-named
/// function, so a kind treated as gating would pull `fn:third` into the variant
/// group and produce an extra propagated edge. Noise that could not matter
/// would make this test pass against a function reading anything at all.
#[test]
fn propagate_variant_edges_ignores_every_kind_but_annotates_and_calls() {
    let nodes = vec![
        variant_node(
            "fn:caller",
            NodeKind::Function,
            "main",
            "src/main.rs::main",
            "src/main.rs",
        ),
        variant_node(
            "fn:macos",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        variant_node(
            "fn:other",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        // Same qualified_name as the two variants, but gated by nothing.
        variant_node(
            "fn:third",
            NodeKind::Function,
            "copy",
            "src/c.rs::copy",
            "src/c.rs",
        ),
        variant_node(
            "au:1",
            NodeKind::AnnotationUsage,
            "cfg",
            "src/c.rs::cfg",
            "src/c.rs",
        ),
        variant_node(
            "au:2",
            NodeKind::AnnotationUsage,
            "cfg",
            "src/c.rs::cfg",
            "src/c.rs",
        ),
    ];
    let needed = vec![
        Edge {
            source: "au:1".into(),
            target: "fn:macos".into(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        Edge {
            source: "au:2".into(),
            target: "fn:other".into(),
            kind: EdgeKind::Annotates,
            line: Some(1),
        },
        calls_edge("fn:caller", "fn:macos"),
    ];

    // Every other kind the graph can hold, each from a `cfg` node to `fn:third`
    // so that reading it as gating would visibly change the output.
    let noise: Vec<Edge> = [
        EdgeKind::Contains,
        EdgeKind::Uses,
        EdgeKind::Implements,
        EdgeKind::TypeOf,
        EdgeKind::Returns,
        EdgeKind::DerivesMacro,
        EdgeKind::Extends,
        EdgeKind::Receives,
        EdgeKind::Documents,
        EdgeKind::Instantiates,
    ]
    .into_iter()
    .map(|kind| Edge {
        source: "au:1".into(),
        target: "fn:third".into(),
        kind,
        line: Some(7),
    })
    .collect();

    let mut with_noise = needed.clone();
    with_noise.extend(noise);

    let key = |v: &[Edge]| {
        let mut k: Vec<(String, String)> = v
            .iter()
            .map(|e| (e.source.clone(), e.target.clone()))
            .collect();
        k.sort_unstable();
        k
    };
    let filtered = tokensave::resolution::propagate_variant_edges(&nodes, &needed);
    let whole_table = tokensave::resolution::propagate_variant_edges(&nodes, &with_noise);
    assert_eq!(
        key(&filtered),
        key(&whole_table),
        "the kind-filtered slice must give the same result as the whole table"
    );

    // Controls. The fixture must actually propagate, or the comparison above is
    // between two empty vectors; and `fn:third` must stay out of it, or the
    // noise was never capable of changing anything.
    assert!(
        filtered
            .iter()
            .any(|e| e.source == "fn:caller" && e.target == "fn:other"),
        "fixture must propagate to the cfg sibling, got: {filtered:?}"
    );
    assert!(
        !filtered.iter().any(|e| e.target == "fn:third"),
        "an ungated same-named function must not join the variant group"
    );
}

/// Ext tags differ (`c` vs `cpp`), so a cross-language penalty here lands under the floor.
#[tokio::test]
async fn test_cpp_call_resolves_to_header_declaration() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");
    let decl = variant_node(
        "fn:cell_to_world",
        NodeKind::Function,
        "CellToWorld",
        "grid.h::CellToWorld",
        "include/grid.h",
    );
    let caller = variant_node(
        "fn:step",
        NodeKind::Function,
        "Step",
        "mover.cpp::Step",
        "src/mover.cpp",
    );
    db.insert_node(&decl).await.expect("insert decl");
    db.insert_node(&caller).await.expect("insert caller");

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let uref = UnresolvedRef {
        from_node_id: "fn:step".to_string(),
        reference_name: "CellToWorld".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 12,
        column: 8,
        file_path: "src/mover.cpp".to_string(),
    };

    let resolved = resolver.resolve_one(&uref).expect("header decl resolves");
    assert_eq!(resolved.target_node_id, "fn:cell_to_world");
    assert!(
        resolved.confidence >= 0.6,
        "confidence must clear the resolve_all floor, got {}",
        resolved.confidence
    );
}

/// `.inl`, `.ipp` and `.tcc` are headers to `is_header_path`; unmapped in `lang_from_path` they
/// would score as `unknown`, which #346 measured as exempt from both cross-language guards and so
/// an advantage over the correct same-language candidate.
#[tokio::test]
async fn test_cpp_call_resolves_into_a_template_implementation_header() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");
    let decl = variant_node(
        "fn:apply",
        NodeKind::Function,
        "Apply",
        "grid.inl::Apply",
        "include/grid.inl",
    );
    let caller = variant_node(
        "fn:step",
        NodeKind::Function,
        "Step",
        "mover.cpp::Step",
        "src/mover.cpp",
    );
    db.insert_node(&decl).await.expect("insert decl");
    db.insert_node(&caller).await.expect("insert caller");

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let uref = UnresolvedRef {
        from_node_id: "fn:step".to_string(),
        reference_name: "Apply".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 12,
        column: 8,
        file_path: "src/mover.cpp".to_string(),
    };

    let resolved = resolver
        .resolve_one(&uref)
        .expect("the .inl definition resolves");
    assert_eq!(resolved.target_node_id, "fn:apply");
    assert!(
        resolved.confidence >= 0.6,
        "confidence must clear the resolve_all floor, got {}",
        resolved.confidence
    );
}

/// The family is C/C++ only - a genuinely foreign single candidate still pays.
#[tokio::test]
async fn test_cross_language_single_candidate_still_penalised() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let (db, _) = Database::initialize(&dir.path().join("test.db"))
        .await
        .expect("failed to init db");
    let decl = variant_node(
        "fn:py_helper",
        NodeKind::Function,
        "helper",
        "tools/gen.py::helper",
        "tools/gen.py",
    );
    let caller = variant_node(
        "fn:step",
        NodeKind::Function,
        "Step",
        "mover.cpp::Step",
        "src/mover.cpp",
    );
    db.insert_node(&decl).await.expect("insert decl");
    db.insert_node(&caller).await.expect("insert caller");

    let all_nodes = db.get_all_nodes().await.unwrap();
    let resolver = ReferenceResolver::from_nodes(&db, &all_nodes);
    let uref = UnresolvedRef {
        from_node_id: "fn:step".to_string(),
        reference_name: "helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 12,
        column: 8,
        file_path: "src/mover.cpp".to_string(),
    };

    let resolved = resolver.resolve_one(&uref).expect("a candidate is found");
    assert!(
        resolved.confidence < 0.6,
        "a python callee for a cpp call must stay under the floor, got {}",
        resolved.confidence
    );
}
