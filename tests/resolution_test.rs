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
    assert_eq!(result.unresolved[0].reference_name, "nonexistent");
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
        .map(|reference| reference.reference_name.as_str())
        .collect();
    for receiver_call in &names[..names.len() - 1] {
        assert!(
            unresolved_names.contains(receiver_call),
            "receiver-qualified Ruby call {receiver_call:?} must remain unresolved"
        );
    }
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
