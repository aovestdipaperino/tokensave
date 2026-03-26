use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::ProtoExtractor;
use tokensave::types::*;

fn extract_sample() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.proto")
        .expect("failed to read sample.proto");
    let extractor = ProtoExtractor;
    extractor.extract("sample.proto", &source)
}

#[test]
fn test_proto_no_errors() {
    let result = extract_sample();
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn test_proto_file_node() {
    let result = extract_sample();
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
}

#[test]
fn test_proto_package() {
    let result = extract_sample();
    let pkgs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Package)
        .collect();
    assert_eq!(pkgs.len(), 1, "expected 1 package, got {}", pkgs.len());
    assert_eq!(pkgs[0].name, "networking");
}

#[test]
fn test_proto_imports() {
    let result = extract_sample();
    let imports: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(imports.len(), 2, "expected 2 imports, got {}", imports.len());
    assert!(imports
        .iter()
        .any(|n| n.name == "google/protobuf/timestamp.proto"));
    assert!(imports
        .iter()
        .any(|n| n.name == "google/protobuf/empty.proto"));
}

#[test]
fn test_proto_messages() {
    let result = extract_sample();
    let msgs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ProtoMessage)
        .collect();
    // Endpoint, ConnectionConfig, AuthConfig (nested), ConnectionStatus, DisconnectRequest, HealthCheckRequest, HealthCheckResponse
    assert!(
        msgs.len() >= 7,
        "expected >= 7 messages, got {} : {:?}",
        msgs.len(),
        msgs.iter().map(|m| &m.name).collect::<Vec<_>>()
    );
    assert!(msgs.iter().any(|m| m.name == "Endpoint"));
    assert!(msgs.iter().any(|m| m.name == "ConnectionConfig"));
    assert!(msgs.iter().any(|m| m.name == "ConnectionStatus"));
    assert!(msgs.iter().any(|m| m.name == "DisconnectRequest"));
    assert!(msgs.iter().any(|m| m.name == "HealthCheckRequest"));
    assert!(msgs.iter().any(|m| m.name == "HealthCheckResponse"));
}

#[test]
fn test_proto_nested_message() {
    let result = extract_sample();
    let auth = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::ProtoMessage && n.name == "AuthConfig");
    assert!(auth.is_some(), "nested AuthConfig message not found");

    // AuthConfig should be contained within ConnectionConfig via an edge.
    let conn_config = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::ProtoMessage && n.name == "ConnectionConfig")
        .unwrap();
    let auth_config = auth.unwrap();
    assert!(
        result.edges.iter().any(|e| e.source == conn_config.id
            && e.target == auth_config.id
            && e.kind == EdgeKind::Contains),
        "expected Contains edge from ConnectionConfig to AuthConfig"
    );
}

#[test]
fn test_proto_enum() {
    let result = extract_sample();
    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "LogLevel");
    assert!(
        enums[0].docstring.is_some(),
        "LogLevel should have docstring"
    );
    assert!(
        enums[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("log level"),
        "docstring: {:?}",
        enums[0].docstring
    );
}

#[test]
fn test_proto_enum_variants() {
    let result = extract_sample();
    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(
        variants.len(),
        5,
        "expected 5 enum variants, got {} : {:?}",
        variants.len(),
        variants.iter().map(|v| &v.name).collect::<Vec<_>>()
    );
    assert!(variants.iter().any(|v| v.name == "LOG_LEVEL_UNSPECIFIED"));
    assert!(variants.iter().any(|v| v.name == "LOG_LEVEL_DEBUG"));
    assert!(variants.iter().any(|v| v.name == "LOG_LEVEL_INFO"));
    assert!(variants.iter().any(|v| v.name == "LOG_LEVEL_WARNING"));
    assert!(variants.iter().any(|v| v.name == "LOG_LEVEL_ERROR"));
}

#[test]
fn test_proto_service() {
    let result = extract_sample();
    let services: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ProtoService)
        .collect();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].name, "ConnectionService");
    assert!(
        services[0].docstring.is_some(),
        "ConnectionService should have docstring"
    );
}

#[test]
fn test_proto_rpcs() {
    let result = extract_sample();
    let rpcs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ProtoRpc)
        .collect();
    assert_eq!(
        rpcs.len(),
        3,
        "expected 3 rpcs, got {} : {:?}",
        rpcs.len(),
        rpcs.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
    assert!(rpcs.iter().any(|r| r.name == "Connect"));
    assert!(rpcs.iter().any(|r| r.name == "Disconnect"));
    assert!(rpcs.iter().any(|r| r.name == "HealthCheck"));

    // Check docstrings on rpcs
    let connect = rpcs.iter().find(|r| r.name == "Connect").unwrap();
    assert!(
        connect.docstring.is_some(),
        "Connect rpc should have docstring"
    );
}

#[test]
fn test_proto_fields() {
    let result = extract_sample();
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    // Endpoint (3) + ConnectionConfig (4 + auth + 2 oneof) + AuthConfig (2) + ConnectionStatus (2) + DisconnectRequest (1) + HealthCheckRequest (1) + HealthCheckResponse (2) = 17
    assert!(
        fields.len() >= 15,
        "expected >= 15 fields, got {}",
        fields.len()
    );
    assert!(fields.iter().any(|f| f.name == "host"));
    assert!(fields.iter().any(|f| f.name == "port"));
    assert!(fields.iter().any(|f| f.name == "tls"));
    assert!(fields.iter().any(|f| f.name == "connection_id"));

    // Check field signatures contain type and number
    let host = fields.iter().find(|f| f.name == "host").unwrap();
    assert!(
        host.signature.as_ref().unwrap().contains("string"),
        "host signature should contain type"
    );
    assert!(
        host.signature.as_ref().unwrap().contains("1"),
        "host signature should contain field number"
    );
}

#[test]
fn test_proto_oneof_fields() {
    let result = extract_sample();
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    // oneof fields: round_robin, least_connections
    assert!(fields.iter().any(|f| f.name == "round_robin"));
    assert!(fields.iter().any(|f| f.name == "least_connections"));
}

#[test]
fn test_proto_docstrings() {
    let result = extract_sample();

    // Endpoint message should have a docstring
    let endpoint = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::ProtoMessage && n.name == "Endpoint")
        .unwrap();
    assert!(endpoint.docstring.is_some(), "Endpoint should have docstring");
    assert!(
        endpoint.docstring.as_ref().unwrap().contains("network endpoint"),
        "docstring: {:?}",
        endpoint.docstring
    );
}

#[test]
fn test_proto_contains_edges() {
    let result = extract_sample();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // Should have many Contains edges: file->package, file->imports, file->messages, etc.
    assert!(
        contains.len() >= 10,
        "expected >= 10 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_proto_service_contains_rpcs() {
    let result = extract_sample();
    let service = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::ProtoService && n.name == "ConnectionService")
        .unwrap();
    let rpcs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ProtoRpc)
        .collect();
    for rpc in &rpcs {
        assert!(
            result.edges.iter().any(|e| e.source == service.id
                && e.target == rpc.id
                && e.kind == EdgeKind::Contains),
            "expected Contains edge from service to rpc '{}'",
            rpc.name
        );
    }
}

#[test]
fn test_proto_message_docstring_connection_config() {
    let result = extract_sample();
    let conn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::ProtoMessage && n.name == "ConnectionConfig")
        .unwrap();
    assert!(
        conn.docstring.is_some(),
        "ConnectionConfig should have docstring"
    );
    assert!(
        conn.docstring
            .as_ref()
            .unwrap()
            .contains("Connection configuration"),
        "docstring: {:?}",
        conn.docstring
    );
}
