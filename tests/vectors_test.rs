use tokensave::db::Database;
use tokensave::types::*;
use tokensave::vectors::*;
use tempfile::TempDir;

#[test]
fn test_cosine_similarity_identical() {
    let a = vec![1.0f32, 0.0, 0.0];
    let b = vec![1.0f32, 0.0, 0.0];
    assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = vec![1.0f32, 0.0];
    let b = vec![0.0f32, 1.0];
    assert!(cosine_similarity(&a, &b).abs() < 1e-6);
}

#[test]
fn test_cosine_similarity_zero_vector() {
    let a = vec![0.0f32, 0.0];
    let b = vec![1.0f32, 0.0];
    assert_eq!(cosine_similarity(&a, &b), 0.0);
}

#[tokio::test]
async fn test_store_and_retrieve_vector() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).await.unwrap();

    // Must have a node to reference (FK constraint)
    let node = Node {
        id: "function:test_fn".to_string(),
        kind: NodeKind::Function,
        name: "test_fn".to_string(),
        qualified_name: "test_fn".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: None,
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
        updated_at: 0,
    };
    db.insert_node(&node).await.unwrap();

    let embedding = vec![0.1f32, 0.2, 0.3, 0.4, 0.5];
    store_vector(&db, "function:test_fn", &embedding, "test-model").await.unwrap();

    let retrieved = get_vector(&db, "function:test_fn").await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.len(), 5);
    assert!((retrieved[0] - 0.1).abs() < 1e-6);
}

#[tokio::test]
async fn test_brute_force_search() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).await.unwrap();

    for i in 0..5u32 {
        let node = Node {
            id: format!("function:fn_{}", i),
            kind: NodeKind::Function,
            name: format!("fn_{}", i),
            qualified_name: format!("fn_{}", i),
            file_path: "src/lib.rs".to_string(),
            start_line: i + 1,
            end_line: i + 5,
            start_column: 0,
            end_column: 1,
            signature: None,
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
            updated_at: 0,
        };
        db.insert_node(&node).await.unwrap();

        let mut embedding = vec![0.0f32; 5];
        embedding[i as usize] = 1.0;
        store_vector(&db, &format!("function:fn_{}", i), &embedding, "test").await.unwrap();
    }

    let query = vec![0.0f32, 0.0, 0.9, 0.1, 0.0];
    let results = brute_force_search(&db, &query, 3).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, "function:fn_2");
}

#[test]
fn test_create_node_text() {
    let node = Node {
        id: "function:test".to_string(),
        kind: NodeKind::Function,
        name: "process_data".to_string(),
        qualified_name: "src/lib.rs::process_data".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 10,
        start_column: 0,
        end_column: 1,
        signature: Some("fn process_data(input: &str) -> Result<Data>".to_string()),
        docstring: Some("Processes raw data input".to_string()),
        visibility: Visibility::Pub,
        is_async: false,
        branches: 0,
        loops: 0,
        returns: 0,
        max_nesting: 0,
        unsafe_blocks: 0,
        unchecked_calls: 0,
        assertions: 0,
        updated_at: 0,
    };
    let text = create_node_text(&node);
    assert!(text.contains("process_data"));
    assert!(text.contains("function"));
    assert!(text.contains("Processes raw data"));
}

#[tokio::test]
async fn test_vector_count() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).await.unwrap();
    assert_eq!(vector_count(&db).await.unwrap(), 0);

    let node = Node {
        id: "function:count_test".to_string(),
        kind: NodeKind::Function,
        name: "count_test".to_string(),
        qualified_name: "count_test".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: None,
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
        updated_at: 0,
    };
    db.insert_node(&node).await.unwrap();
    store_vector(&db, "function:count_test", &[1.0, 2.0, 3.0], "test").await.unwrap();
    assert_eq!(vector_count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn test_delete_vector() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).await.unwrap();
    let node = Node {
        id: "function:del".to_string(),
        kind: NodeKind::Function,
        name: "del".to_string(),
        qualified_name: "del".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: None,
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
        updated_at: 0,
    };
    db.insert_node(&node).await.unwrap();
    store_vector(&db, "function:del", &[1.0, 2.0], "test").await.unwrap();
    assert!(get_vector(&db, "function:del").await.unwrap().is_some());
    delete_vector(&db, "function:del").await.unwrap();
    assert!(get_vector(&db, "function:del").await.unwrap().is_none());
}

#[tokio::test]
async fn test_clear_vectors() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).await.unwrap();

    for i in 0..3u32 {
        let node = Node {
            id: format!("function:clear_{}", i),
            kind: NodeKind::Function,
            name: format!("clear_{}", i),
            qualified_name: format!("clear_{}", i),
            file_path: "src/lib.rs".to_string(),
            start_line: i + 1,
            end_line: i + 5,
            start_column: 0,
            end_column: 1,
            signature: None,
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
            updated_at: 0,
        };
        db.insert_node(&node).await.unwrap();
        store_vector(&db, &format!("function:clear_{}", i), &[1.0, 2.0], "test").await.unwrap();
    }

    assert_eq!(vector_count(&db).await.unwrap(), 3);
    clear_vectors(&db).await.unwrap();
    assert_eq!(vector_count(&db).await.unwrap(), 0);
}

#[tokio::test]
async fn test_get_vector_not_found() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).await.unwrap();
    let result = get_vector(&db, "nonexistent:id").await.unwrap();
    assert!(result.is_none());
}

#[test]
fn test_create_node_text_without_optional_fields() {
    let node = Node {
        id: "function:bare".to_string(),
        kind: NodeKind::Function,
        name: "bare_fn".to_string(),
        qualified_name: "bare_fn".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 3,
        start_column: 0,
        end_column: 1,
        signature: None,
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
        updated_at: 0,
    };
    let text = create_node_text(&node);
    assert!(text.contains("kind: function"));
    assert!(text.contains("name: bare_fn"));
    assert!(!text.contains("signature:"));
    assert!(!text.contains("docstring:"));
}
