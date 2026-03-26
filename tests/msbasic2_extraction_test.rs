use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::MsBasic2Extractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.bas").unwrap();
    let extractor = MsBasic2Extractor;
    let result = extractor.extract("sample.bas", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

#[test]
fn test_msbasic2_file_node() {
    let result = extract_fixture();
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.bas");
}

#[test]
fn test_msbasic2_let_constants() {
    let result = extract_fixture();
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(
        consts.len(),
        2,
        "expected 2 consts, got {}: {:?}",
        consts.len(),
        consts.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(consts.iter().any(|n| n.name == "MR"), "MR const not found");
    assert!(consts.iter().any(|n| n.name == "DP"), "DP const not found");
}

#[test]
fn test_msbasic2_subroutine_functions() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // We expect 3 subroutines: LOG_A_MESSAGE, CONNECT_TO_SERVER, DISCONNECT
    assert_eq!(
        fns.len(),
        3,
        "expected 3 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|n| n.name == "LOG_A_MESSAGE"),
        "LOG_A_MESSAGE not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "CONNECT_TO_SERVER"),
        "CONNECT_TO_SERVER not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "DISCONNECT"),
        "DISCONNECT not found"
    );
}

#[test]
fn test_msbasic2_gosub_calls() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");

    // Top-level: GOSUB 200, GOSUB 300, GOSUB 400
    assert!(
        calls.iter().any(|r| r.reference_name == "200"),
        "expected GOSUB 200 call, got: {:?}",
        calls.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "300"),
        "expected GOSUB 300 call"
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "400"),
        "expected GOSUB 400 call"
    );
}

#[test]
fn test_msbasic2_docstrings() {
    let result = extract_fixture();

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "LOG_A_MESSAGE")
        .expect("LOG_A_MESSAGE function not found");
    assert!(
        log_fn.docstring.is_some(),
        "LOG_A_MESSAGE should have docstring"
    );
    assert!(
        log_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("LOG A MESSAGE"),
        "docstring: {:?}",
        log_fn.docstring
    );

    let connect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "CONNECT_TO_SERVER")
        .expect("CONNECT_TO_SERVER function not found");
    assert!(
        connect_fn.docstring.is_some(),
        "CONNECT_TO_SERVER should have docstring"
    );
    assert!(
        connect_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("CONNECT TO SERVER"),
        "docstring: {:?}",
        connect_fn.docstring
    );

    let disconnect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "DISCONNECT")
        .expect("DISCONNECT function not found");
    assert!(
        disconnect_fn.docstring.is_some(),
        "DISCONNECT should have docstring"
    );
}

#[test]
fn test_msbasic2_contains_edges() {
    let result = extract_fixture();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 2 consts + 3 functions = 5 Contains edges
    assert!(
        contains.len() >= 5,
        "should have >= 5 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_msbasic2_subroutine_complexity() {
    let result = extract_fixture();

    // CONNECT_TO_SERVER has a FOR loop
    let connect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "CONNECT_TO_SERVER")
        .expect("CONNECT_TO_SERVER function not found");
    assert!(
        connect_fn.loops >= 1,
        "CONNECT_TO_SERVER should have >= 1 loop, got {}",
        connect_fn.loops
    );
}

#[test]
fn test_msbasic2_extensions() {
    let extractor = MsBasic2Extractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"bas"));
}

#[test]
fn test_msbasic2_language_name() {
    let extractor = MsBasic2Extractor;
    assert_eq!(extractor.language_name(), "MS BASIC 2.0");
}

#[test]
fn test_msbasic2_subroutine_signatures() {
    let result = extract_fixture();

    // Subroutine signatures should contain the GOSUB target line number.
    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "LOG_A_MESSAGE")
        .expect("LOG_A_MESSAGE function not found");
    assert!(
        log_fn.signature.as_ref().unwrap().contains("GOSUB"),
        "signature should contain GOSUB: {:?}",
        log_fn.signature
    );
}

#[test]
fn test_msbasic2_subroutine_internal_calls() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();

    // CONNECT_TO_SERVER and DISCONNECT both GOSUB 200 (LOG_A_MESSAGE)
    let gosub_200_count = calls.iter().filter(|r| r.reference_name == "200").count();
    assert!(
        gosub_200_count >= 3,
        "expected >= 3 GOSUB 200 calls (top-level + connect + disconnect), got {}",
        gosub_200_count
    );
}
