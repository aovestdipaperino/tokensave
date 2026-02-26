use codegraph::types::*;

#[test]
fn node_kind_as_str_roundtrip() {
    let kinds = vec![
        NodeKind::File,
        NodeKind::Module,
        NodeKind::Struct,
        NodeKind::Enum,
        NodeKind::EnumVariant,
        NodeKind::Trait,
        NodeKind::Function,
        NodeKind::Method,
        NodeKind::Impl,
        NodeKind::Const,
        NodeKind::Static,
        NodeKind::TypeAlias,
        NodeKind::Field,
        NodeKind::Macro,
        NodeKind::Use,
    ];

    for kind in kinds {
        let s = kind.as_str();
        let parsed =
            NodeKind::from_str(s).unwrap_or_else(|| panic!("failed to parse NodeKind from '{}'", s));
        assert_eq!(kind, parsed, "roundtrip failed for NodeKind::{}", s);
    }
}

#[test]
fn node_kind_from_str_unknown_returns_none() {
    assert!(NodeKind::from_str("unknown_kind").is_none());
    assert!(NodeKind::from_str("").is_none());
}

#[test]
fn edge_kind_as_str_roundtrip() {
    let kinds = vec![
        EdgeKind::Contains,
        EdgeKind::Calls,
        EdgeKind::Uses,
        EdgeKind::Implements,
        EdgeKind::TypeOf,
        EdgeKind::Returns,
        EdgeKind::DerivesMacro,
    ];

    for kind in kinds {
        let s = kind.as_str();
        let parsed =
            EdgeKind::from_str(s).unwrap_or_else(|| panic!("failed to parse EdgeKind from '{}'", s));
        assert_eq!(kind, parsed, "roundtrip failed for EdgeKind::{}", s);
    }
}

#[test]
fn edge_kind_from_str_unknown_returns_none() {
    assert!(EdgeKind::from_str("unknown_edge").is_none());
    assert!(EdgeKind::from_str("").is_none());
}

#[test]
fn visibility_default_is_private() {
    let vis: Visibility = Visibility::default();
    assert_eq!(vis, Visibility::Private);
}

#[test]
fn generate_node_id_is_deterministic() {
    let id1 = generate_node_id("src/main.rs", &NodeKind::Function, "main", 1);
    let id2 = generate_node_id("src/main.rs", &NodeKind::Function, "main", 1);
    assert_eq!(id1, id2, "same inputs must produce same ID");
}

#[test]
fn generate_node_id_format() {
    let id = generate_node_id("src/lib.rs", &NodeKind::Struct, "MyStruct", 10);

    // Format should be "kind:32hexchars"
    let parts: Vec<&str> = id.splitn(2, ':').collect();
    assert_eq!(parts.len(), 2, "ID should have exactly one colon separator");
    assert_eq!(parts[0], "struct", "prefix should be the node kind");
    assert_eq!(parts[1].len(), 32, "hex portion should be 32 characters");

    // Verify the hex portion contains only hex characters
    assert!(
        parts[1].chars().all(|c| c.is_ascii_hexdigit()),
        "hex portion should contain only hex digits"
    );
}

#[test]
fn generate_node_id_different_inputs_produce_different_ids() {
    let id1 = generate_node_id("src/main.rs", &NodeKind::Function, "main", 1);
    let id2 = generate_node_id("src/main.rs", &NodeKind::Function, "other", 1);
    let id3 = generate_node_id("src/main.rs", &NodeKind::Function, "main", 2);
    let id4 = generate_node_id("src/lib.rs", &NodeKind::Function, "main", 1);
    let id5 = generate_node_id("src/main.rs", &NodeKind::Struct, "main", 1);

    assert_ne!(id1, id2, "different names should produce different IDs");
    assert_ne!(id1, id3, "different lines should produce different IDs");
    assert_ne!(
        id1, id4,
        "different file paths should produce different IDs"
    );
    assert_ne!(id1, id5, "different kinds should produce different IDs");
}

#[test]
fn node_serde_roundtrip() {
    let node = Node {
        id: "function:abcdef01234567890abcdef012345678".to_string(),
        kind: NodeKind::Function,
        name: "my_function".to_string(),
        qualified_name: "crate::module::my_function".to_string(),
        file_path: "src/module.rs".to_string(),
        start_line: 10,
        end_line: 20,
        start_column: 0,
        end_column: 1,
        signature: Some("fn my_function(x: i32) -> bool".to_string()),
        docstring: Some("Does something useful.".to_string()),
        visibility: Visibility::Pub,
        is_async: true,
        updated_at: 1700000000,
    };

    let json = serde_json::to_string(&node).expect("failed to serialize Node");
    let deserialized: Node = serde_json::from_str(&json).expect("failed to deserialize Node");

    assert_eq!(node.id, deserialized.id);
    assert_eq!(node.kind, deserialized.kind);
    assert_eq!(node.name, deserialized.name);
    assert_eq!(node.qualified_name, deserialized.qualified_name);
    assert_eq!(node.file_path, deserialized.file_path);
    assert_eq!(node.start_line, deserialized.start_line);
    assert_eq!(node.end_line, deserialized.end_line);
    assert_eq!(node.start_column, deserialized.start_column);
    assert_eq!(node.end_column, deserialized.end_column);
    assert_eq!(node.signature, deserialized.signature);
    assert_eq!(node.docstring, deserialized.docstring);
    assert_eq!(node.visibility, deserialized.visibility);
    assert_eq!(node.is_async, deserialized.is_async);
    assert_eq!(node.updated_at, deserialized.updated_at);
}

#[test]
fn edge_serde_roundtrip() {
    let edge = Edge {
        source: "function:aaaa".to_string(),
        target: "function:bbbb".to_string(),
        kind: EdgeKind::Calls,
        line: Some(15),
    };

    let json = serde_json::to_string(&edge).expect("failed to serialize Edge");
    let deserialized: Edge = serde_json::from_str(&json).expect("failed to deserialize Edge");

    assert_eq!(edge.source, deserialized.source);
    assert_eq!(edge.target, deserialized.target);
    assert_eq!(edge.kind, deserialized.kind);
    assert_eq!(edge.line, deserialized.line);
}

#[test]
fn traversal_options_default() {
    let opts = TraversalOptions::default();
    assert_eq!(opts.max_depth, 3);
    assert_eq!(opts.limit, 100);
    assert!(opts.include_start);
    assert_eq!(opts.direction, TraversalDirection::Outgoing);
    assert!(opts.edge_kinds.is_none());
    assert!(opts.node_kinds.is_none());
}

#[test]
fn build_context_options_default() {
    let opts = BuildContextOptions::default();
    assert_eq!(opts.max_nodes, 20);
    assert_eq!(opts.max_code_blocks, 5);
    assert_eq!(opts.max_code_block_size, 1500);
    assert!(opts.include_code);
    assert_eq!(opts.format, OutputFormat::Markdown);
    assert_eq!(opts.search_limit, 3);
    assert_eq!(opts.traversal_depth, 1);
    assert!((opts.min_score - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_new_node_kinds_roundtrip() {
    use codegraph::types::NodeKind;
    let kinds = vec![
        (NodeKind::Class, "class"),
        (NodeKind::Interface, "interface"),
        (NodeKind::Constructor, "constructor"),
        (NodeKind::Annotation, "annotation"),
        (NodeKind::AnnotationUsage, "annotation_usage"),
        (NodeKind::Package, "package"),
        (NodeKind::InnerClass, "inner_class"),
        (NodeKind::InitBlock, "init_block"),
        (NodeKind::AbstractMethod, "abstract_method"),
        (NodeKind::InterfaceType, "interface_type"),
        (NodeKind::StructMethod, "struct_method"),
        (NodeKind::GoPackage, "go_package"),
        (NodeKind::StructTag, "struct_tag"),
        (NodeKind::GenericParam, "generic_param"),
    ];
    for (kind, expected_str) in kinds {
        assert_eq!(kind.as_str(), expected_str);
        assert_eq!(NodeKind::from_str(expected_str), Some(kind));
    }
}

#[test]
fn test_new_edge_kinds_roundtrip() {
    use codegraph::types::EdgeKind;
    let kinds = vec![
        (EdgeKind::Extends, "extends"),
        (EdgeKind::Annotates, "annotates"),
        (EdgeKind::Receives, "receives"),
    ];
    for (kind, expected_str) in kinds {
        assert_eq!(kind.as_str(), expected_str);
        assert_eq!(EdgeKind::from_str(expected_str), Some(kind));
    }
}
