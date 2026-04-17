use tokensave::extraction::BashExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_bash_extract_functions() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
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
    assert!(fns.iter().any(|n| n.name == "log"));
    assert!(fns.iter().any(|n| n.name == "validate_config"));
    assert!(fns.iter().any(|n| n.name == "connect"));
    assert!(fns.iter().any(|n| n.name == "disconnect"));
    assert!(fns.iter().any(|n| n.name == "main"));
}

#[test]
fn test_bash_extract_readonly_consts() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
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
fn test_bash_extract_source_import() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 1, "expected 1 Use node, got {}", uses.len());
    assert_eq!(uses[0].name, "./utils.sh");
}

#[test]
fn test_bash_call_sites() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
    // The log function calls echo and date
    assert!(
        call_refs.iter().any(|r| r.reference_name == "echo"),
        "should find echo call"
    );
    // validate_config calls log
    assert!(
        call_refs.iter().any(|r| r.reference_name == "log"),
        "should find log call"
    );
    // connect calls curl
    assert!(
        call_refs.iter().any(|r| r.reference_name == "curl"),
        "should find curl call"
    );
    // main calls validate_config
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "validate_config"),
        "should find validate_config call"
    );
}

#[test]
fn test_bash_docstrings() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log")
        .expect("log function not found");
    assert!(
        log_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("Logs a message"),
        "docstring: {:?}",
        log_fn.docstring
    );

    let connect_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "connect")
        .expect("connect function not found");
    assert!(
        connect_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("Connects to the remote server"),
        "docstring: {:?}",
        connect_fn.docstring
    );

    let main_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "main")
        .expect("main function not found");
    assert!(
        main_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("Main entry point"),
        "docstring: {:?}",
        main_fn.docstring
    );
}

#[test]
fn test_bash_file_node() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.sh");
}

#[test]
fn test_bash_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.sh").unwrap();
    let extractor = BashExtractor;
    let result = extractor.extract("sample.sh", &source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 5 functions + 2 consts + 1 Use = 8 Contains edges
    assert!(
        contains.len() >= 8,
        "should have >= 8 Contains edges, got {}",
        contains.len()
    );
}
