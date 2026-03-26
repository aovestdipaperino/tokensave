use tokensave::extraction::CobolExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.cob").unwrap();
    let extractor = CobolExtractor;
    let result = extractor.extract("sample.cob", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

#[test]
fn test_cobol_file_root() {
    let result = extract_fixture();
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
}

#[test]
fn test_cobol_program_id_as_module() {
    let result = extract_fixture();
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(
        modules.len(),
        1,
        "expected 1 module, got {}: {:?}",
        modules.len(),
        modules.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert_eq!(modules[0].name, "NETWORKING");
}

#[test]
fn test_cobol_paragraphs_as_functions() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // Paragraphs: MAIN-PROGRAM, VALIDATE-CONFIG, LOG-MESSAGE, CONNECT-SERVER, DISCONNECT-SERVER
    assert_eq!(
        fns.len(),
        5,
        "expected 5 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|n| n.name == "MAIN-PROGRAM"),
        "MAIN-PROGRAM not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "VALIDATE-CONFIG"),
        "VALIDATE-CONFIG not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "LOG-MESSAGE"),
        "LOG-MESSAGE not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "CONNECT-SERVER"),
        "CONNECT-SERVER not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "DISCONNECT-SERVER"),
        "DISCONNECT-SERVER not found"
    );
}

#[test]
fn test_cobol_data_items_as_fields_and_consts() {
    let result = extract_fixture();
    // Items with VALUE clause -> Const: WS-MAX-RETRIES, WS-DEFAULT-PORT, WS-CONNECTED, WS-RETRY-COUNT
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert!(
        consts.iter().any(|n| n.name == "WS-MAX-RETRIES"),
        "WS-MAX-RETRIES const not found"
    );
    assert!(
        consts.iter().any(|n| n.name == "WS-DEFAULT-PORT"),
        "WS-DEFAULT-PORT const not found"
    );

    // Items without VALUE clause -> Field: WS-HOST, WS-PORT, WS-LOG-LEVEL, WS-LOG-MESSAGE
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert!(
        fields.iter().any(|n| n.name == "WS-HOST"),
        "WS-HOST field not found"
    );
    assert!(
        fields.iter().any(|n| n.name == "WS-PORT"),
        "WS-PORT field not found"
    );
    assert!(
        fields.iter().any(|n| n.name == "WS-LOG-LEVEL"),
        "WS-LOG-LEVEL field not found"
    );
    assert!(
        fields.iter().any(|n| n.name == "WS-LOG-MESSAGE"),
        "WS-LOG-MESSAGE field not found"
    );
}

#[test]
fn test_cobol_perform_calls() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");
    // MAIN-PROGRAM performs VALIDATE-CONFIG, CONNECT-SERVER, DISCONNECT-SERVER
    assert!(
        calls.iter().any(|r| r.reference_name == "VALIDATE-CONFIG"),
        "expected call to VALIDATE-CONFIG, got: {:?}",
        calls.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "CONNECT-SERVER"),
        "expected call to CONNECT-SERVER"
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "DISCONNECT-SERVER"),
        "expected call to DISCONNECT-SERVER"
    );
    // VALIDATE-CONFIG and CONNECT-SERVER call LOG-MESSAGE
    assert!(
        calls.iter().any(|r| r.reference_name == "LOG-MESSAGE"),
        "expected call to LOG-MESSAGE"
    );
}

#[test]
fn test_cobol_docstrings() {
    let result = extract_fixture();
    // VALIDATE-CONFIG, LOG-MESSAGE, CONNECT-SERVER, DISCONNECT-SERVER should have docstrings.
    let validate = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "VALIDATE-CONFIG");
    assert!(validate.is_some(), "VALIDATE-CONFIG not found");
    assert!(
        validate.unwrap().docstring.is_some(),
        "VALIDATE-CONFIG should have docstring"
    );

    let log_msg = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "LOG-MESSAGE");
    assert!(log_msg.is_some(), "LOG-MESSAGE not found");
    assert!(
        log_msg.unwrap().docstring.is_some(),
        "LOG-MESSAGE should have docstring"
    );

    let connect = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "CONNECT-SERVER");
    assert!(connect.is_some(), "CONNECT-SERVER not found");
    assert!(
        connect.unwrap().docstring.is_some(),
        "CONNECT-SERVER should have docstring"
    );

    let disconnect = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "DISCONNECT-SERVER");
    assert!(disconnect.is_some(), "DISCONNECT-SERVER not found");
    assert!(
        disconnect.unwrap().docstring.is_some(),
        "DISCONNECT-SERVER should have docstring"
    );

    // Data items should also have docstrings.
    let max_retries = result
        .nodes
        .iter()
        .find(|n| n.name == "WS-MAX-RETRIES");
    assert!(max_retries.is_some(), "WS-MAX-RETRIES not found");
    assert!(
        max_retries.unwrap().docstring.is_some(),
        "WS-MAX-RETRIES should have docstring"
    );
}

#[test]
fn test_cobol_contains_edges() {
    let result = extract_fixture();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(
        !contains.is_empty(),
        "expected Contains edges"
    );
    // File -> Module, Module -> data items + paragraphs
    // 1 module + 8 data items + 5 paragraphs = 14 Contains edges
    assert!(
        contains.len() >= 10,
        "expected >= 10 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_cobol_qualified_names() {
    let result = extract_fixture();
    let validate = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "VALIDATE-CONFIG")
        .unwrap();
    assert!(
        validate.qualified_name.contains("NETWORKING"),
        "VALIDATE-CONFIG qualified name should contain 'NETWORKING', got: {}",
        validate.qualified_name
    );
}

#[test]
fn test_cobol_extensions() {
    let extractor = CobolExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"cob"));
    assert!(exts.contains(&"cbl"));
    assert!(exts.contains(&"cpy"));
}

#[test]
fn test_cobol_language_name() {
    let extractor = CobolExtractor;
    assert_eq!(extractor.language_name(), "COBOL");
}
