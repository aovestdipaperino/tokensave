use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::GwBasicExtractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.gw").unwrap();
    let extractor = GwBasicExtractor;
    let result = extractor.extract("sample.gw", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

#[test]
fn test_gwbasic_file_node() {
    let result = extract_fixture();
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.gw");
}

#[test]
fn test_gwbasic_let_constants() {
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
fn test_gwbasic_def_fn() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // DEF FNLOG$ should be extracted as a Function
    assert!(
        fns.iter().any(|n| n.name == "FNLOG"),
        "FNLOG function not found, got: {:?}",
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_gwbasic_subroutine_functions() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // We expect 4 functions: FNLOG (DEF FN) + 3 subroutines
    assert!(
        fns.len() >= 4,
        "expected >= 4 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|n| n.name == "VALIDATE_CONFIGURATION"),
        "VALIDATE_CONFIGURATION not found"
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
fn test_gwbasic_gosub_calls() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");

    // Top-level: GOSUB 1000, GOSUB 2000, GOSUB 3000
    assert!(
        calls.iter().any(|r| r.reference_name == "1000"),
        "expected GOSUB 1000 call, got: {:?}",
        calls.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "2000"),
        "expected GOSUB 2000 call"
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "3000"),
        "expected GOSUB 3000 call"
    );
}

#[test]
fn test_gwbasic_docstrings() {
    let result = extract_fixture();

    let validate_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "VALIDATE_CONFIGURATION")
        .expect("VALIDATE_CONFIGURATION function not found");
    assert!(
        validate_fn.docstring.is_some(),
        "VALIDATE_CONFIGURATION should have docstring"
    );
    assert!(
        validate_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("VALIDATE CONFIGURATION"),
        "docstring: {:?}",
        validate_fn.docstring
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
fn test_gwbasic_contains_edges() {
    let result = extract_fixture();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 2 consts + 4 functions (1 DEF FN + 3 subroutines) = 6 Contains edges
    assert!(
        contains.len() >= 6,
        "should have >= 6 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_gwbasic_subroutine_complexity() {
    let result = extract_fixture();

    // CONNECT_TO_SERVER has a WHILE loop
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

    // VALIDATE_CONFIGURATION has IF branches
    let validate_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "VALIDATE_CONFIGURATION")
        .expect("VALIDATE_CONFIGURATION function not found");
    assert!(
        validate_fn.branches >= 1,
        "VALIDATE_CONFIGURATION should have >= 1 branch, got {}",
        validate_fn.branches
    );
}

#[test]
fn test_gwbasic_extensions() {
    let extractor = GwBasicExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"gw"));
}

#[test]
fn test_gwbasic_language_name() {
    let extractor = GwBasicExtractor;
    assert_eq!(extractor.language_name(), "GW-BASIC");
}

#[test]
fn test_gwbasic_subroutine_signatures() {
    let result = extract_fixture();

    // Subroutine signatures should contain the GOSUB target line number.
    let validate_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "VALIDATE_CONFIGURATION")
        .expect("VALIDATE_CONFIGURATION function not found");
    assert!(
        validate_fn.signature.as_ref().unwrap().contains("GOSUB"),
        "signature should contain GOSUB: {:?}",
        validate_fn.signature
    );
}
