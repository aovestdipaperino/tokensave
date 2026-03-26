use tokensave::extraction::BatchExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_batch_extract_labels_as_functions() {
    let source = std::fs::read_to_string("tests/fixtures/sample.bat").unwrap();
    let extractor = BatchExtractor;
    let result = extractor.extract("sample.bat", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(
        fns.len(),
        5,
        "expected 5 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fns.iter().any(|n| n.name == "Log"));
    assert!(fns.iter().any(|n| n.name == "ValidateConfig"));
    assert!(fns.iter().any(|n| n.name == "Connect"));
    assert!(fns.iter().any(|n| n.name == "Disconnect"));
    assert!(fns.iter().any(|n| n.name == "Main"));
}

#[test]
fn test_batch_extract_set_consts() {
    let source = std::fs::read_to_string("tests/fixtures/sample.bat").unwrap();
    let extractor = BatchExtractor;
    let result = extractor.extract("sample.bat", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

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
    assert!(consts.iter().any(|n| n.name == "MAX_RETRIES"));
    assert!(consts.iter().any(|n| n.name == "DEFAULT_PORT"));
}

#[test]
fn test_batch_call_sites() {
    let source = std::fs::read_to_string("tests/fixtures/sample.bat").unwrap();
    let extractor = BatchExtractor;
    let result = extractor.extract("sample.bat", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
    // ValidateConfig calls Log
    assert!(
        call_refs.iter().any(|r| r.reference_name == "Log"),
        "should find Log call"
    );
    // Main calls ValidateConfig
    assert!(
        call_refs.iter().any(|r| r.reference_name == "ValidateConfig"),
        "should find ValidateConfig call"
    );
    // Main calls Connect
    assert!(
        call_refs.iter().any(|r| r.reference_name == "Connect"),
        "should find Connect call"
    );
    // Main calls Disconnect
    assert!(
        call_refs.iter().any(|r| r.reference_name == "Disconnect"),
        "should find Disconnect call"
    );
}

#[test]
fn test_batch_docstrings() {
    let source = std::fs::read_to_string("tests/fixtures/sample.bat").unwrap();
    let extractor = BatchExtractor;
    let result = extractor.extract("sample.bat", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Log should have a docstring from the preceding REM comment.
    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "Log")
        .expect("Log function not found");
    assert!(
        log_fn.docstring.is_some(),
        "Log should have docstring"
    );
    assert!(
        log_fn.docstring.as_ref().unwrap().contains("Logs a message"),
        "docstring: {:?}",
        log_fn.docstring
    );

    // ValidateConfig should have a docstring.
    let vc_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "ValidateConfig")
        .expect("ValidateConfig function not found");
    assert!(
        vc_fn.docstring.as_ref().unwrap().contains("Validates the configuration"),
        "docstring: {:?}",
        vc_fn.docstring
    );

    // Main should have a docstring.
    let main_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "Main")
        .expect("Main function not found");
    assert!(
        main_fn.docstring.as_ref().unwrap().contains("Main entry point"),
        "docstring: {:?}",
        main_fn.docstring
    );
}

#[test]
fn test_batch_file_node() {
    let source = std::fs::read_to_string("tests/fixtures/sample.bat").unwrap();
    let extractor = BatchExtractor;
    let result = extractor.extract("sample.bat", &source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.bat");
}

#[test]
fn test_batch_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.bat").unwrap();
    let extractor = BatchExtractor;
    let result = extractor.extract("sample.bat", &source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 5 functions + 2 consts = 7 Contains edges
    assert!(
        contains.len() >= 7,
        "should have >= 7 Contains edges, got {}",
        contains.len()
    );
}
