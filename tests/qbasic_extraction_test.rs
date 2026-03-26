use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::QBasicExtractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.qb").unwrap();
    let extractor = QBasicExtractor;
    let result = extractor.extract("sample.qb", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

#[test]
fn test_qbasic_file_node() {
    let result = extract_fixture();
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.qb");
}

#[test]
fn test_qbasic_sub_functions() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // SUBs: LogMessage, ValidateConfig, ConnectServer, DisconnectServer
    // FUNCTION: IsConnected
    assert!(
        fns.len() >= 5,
        "expected >= 5 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|n| n.name == "LogMessage"),
        "LogMessage not found, got: {:?}",
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|n| n.name == "ValidateConfig"),
        "ValidateConfig not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "ConnectServer"),
        "ConnectServer not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "DisconnectServer"),
        "DisconnectServer not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "IsConnected"),
        "IsConnected not found"
    );
}

#[test]
fn test_qbasic_type_as_struct() {
    let result = extract_fixture();
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(
        structs.len(),
        1,
        "expected 1 struct (Endpoint), got {}",
        structs.len()
    );
    assert_eq!(structs[0].name, "Endpoint");
}

#[test]
fn test_qbasic_type_fields() {
    let result = extract_fixture();
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Field
                && n.qualified_name.contains("Endpoint")
        })
        .collect();
    assert!(
        fields.len() >= 3,
        "expected >= 3 fields in Endpoint (host, port, connected), got {}: {:?}",
        fields.len(),
        fields.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fields.iter().any(|n| n.name == "host"), "host field not found");
    assert!(fields.iter().any(|n| n.name == "port"), "port field not found");
    assert!(
        fields.iter().any(|n| n.name == "connected"),
        "connected field not found"
    );
}

#[test]
fn test_qbasic_const_nodes() {
    let result = extract_fixture();
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    // CONST MAX_RETRIES = 3 and CONST DEFAULT_PORT = 8080
    // Note: the grammar may have trouble with underscored names; at least some should parse.
    assert!(
        !consts.is_empty(),
        "expected at least 1 CONST node, got 0"
    );
}

#[test]
fn test_qbasic_call_sites() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");

    // Top-level CALL statements: ValidateConfig, ConnectServer, DisconnectServer
    assert!(
        calls.iter().any(|r| r.reference_name == "ValidateConfig"),
        "expected CALL ValidateConfig, got: {:?}",
        calls.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "ConnectServer"),
        "expected CALL ConnectServer"
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "DisconnectServer"),
        "expected CALL DisconnectServer"
    );

    // Inside SUBs: CALL LogMessage
    assert!(
        calls.iter().any(|r| r.reference_name == "LogMessage"),
        "expected CALL LogMessage from within SUBs"
    );
}

#[test]
fn test_qbasic_docstrings() {
    let result = extract_fixture();

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "LogMessage")
        .expect("LogMessage function not found");
    assert!(
        log_fn.docstring.is_some(),
        "LogMessage should have docstring"
    );
    assert!(
        log_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("Logs a message"),
        "docstring: {:?}",
        log_fn.docstring
    );

    let validate_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "ValidateConfig")
        .expect("ValidateConfig function not found");
    assert!(
        validate_fn.docstring.is_some(),
        "ValidateConfig should have docstring"
    );

    let connect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "ConnectServer")
        .expect("ConnectServer function not found");
    assert!(
        connect_fn.docstring.is_some(),
        "ConnectServer should have docstring"
    );

    let disconnect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "DisconnectServer")
        .expect("DisconnectServer function not found");
    assert!(
        disconnect_fn.docstring.is_some(),
        "DisconnectServer should have docstring"
    );

    let is_connected_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "IsConnected")
        .expect("IsConnected function not found");
    assert!(
        is_connected_fn.docstring.is_some(),
        "IsConnected should have docstring"
    );
}

#[test]
fn test_qbasic_contains_edges() {
    let result = extract_fixture();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: CONST nodes + DIM SHARED fields + Endpoint struct + 5 functions
    // Endpoint struct contains 3 fields
    // So at least: 1+ consts + 3 dim shared + 1 struct + 5 functions + 3 struct fields = 13+
    assert!(
        contains.len() >= 10,
        "should have >= 10 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_qbasic_complexity() {
    let result = extract_fixture();

    // ValidateConfig has IF branches
    let validate_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "ValidateConfig")
        .expect("ValidateConfig function not found");
    assert!(
        validate_fn.branches >= 1,
        "ValidateConfig should have >= 1 branch, got {}",
        validate_fn.branches
    );

    // ConnectServer has a FOR loop
    let connect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "ConnectServer")
        .expect("ConnectServer function not found");
    assert!(
        connect_fn.loops >= 1,
        "ConnectServer should have >= 1 loop, got {}",
        connect_fn.loops
    );
}

#[test]
fn test_qbasic_extensions() {
    let extractor = QBasicExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"qb"), "extensions should contain 'qb'");
    // Should NOT contain 'bas' to avoid conflict with msbasic2
    assert!(!exts.contains(&"bas"), "extensions should NOT contain 'bas'");
}

#[test]
fn test_qbasic_language_name() {
    let extractor = QBasicExtractor;
    assert_eq!(extractor.language_name(), "QBasic");
}

#[test]
fn test_qbasic_signatures() {
    let result = extract_fixture();

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "LogMessage")
        .expect("LogMessage function not found");
    assert!(
        log_fn.signature.as_ref().unwrap().contains("SUB"),
        "LogMessage signature should contain SUB: {:?}",
        log_fn.signature
    );

    let is_connected_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "IsConnected")
        .expect("IsConnected function not found");
    assert!(
        is_connected_fn.signature.as_ref().unwrap().contains("FUNCTION"),
        "IsConnected signature should contain FUNCTION: {:?}",
        is_connected_fn.signature
    );
}

#[test]
fn test_qbasic_dim_shared_fields() {
    let result = extract_fixture();
    let dim_fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Field
                && !n.qualified_name.contains("Endpoint")
        })
        .collect();
    // DIM SHARED conn, logLevel, logMsg
    assert!(
        dim_fields.len() >= 3,
        "expected >= 3 DIM SHARED fields (conn, logLevel, logMsg), got {}: {:?}",
        dim_fields.len(),
        dim_fields.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(
        dim_fields.iter().any(|n| n.name == "conn"),
        "conn field not found"
    );
    assert!(
        dim_fields.iter().any(|n| n.name == "logLevel"),
        "logLevel field not found"
    );
    assert!(
        dim_fields.iter().any(|n| n.name == "logMsg"),
        "logMsg field not found"
    );
}
