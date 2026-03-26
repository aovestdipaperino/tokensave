use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::PerlExtractor;
use tokensave::types::*;

#[test]
fn test_perl_extract_functions() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // Top-level functions: log_message, validate_config
    assert_eq!(
        fns.len(),
        2,
        "expected 2 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fns.iter().any(|n| n.name == "log_message"));
    assert!(fns.iter().any(|n| n.name == "validate_config"));
}

#[test]
fn test_perl_extract_packages_as_modules() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    // Packages: Connection, Pool (package main is skipped)
    assert_eq!(
        modules.len(),
        2,
        "expected 2 modules, got {}: {:?}",
        modules.len(),
        modules.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(modules.iter().any(|n| n.name == "Connection"));
    assert!(modules.iter().any(|n| n.name == "Pool"));
}

#[test]
fn test_perl_extract_methods() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    // Methods inside packages: Connection::new, connect, disconnect, is_connected,
    //                          Pool::new, acquire, release
    assert_eq!(
        methods.len(),
        7,
        "expected 7 methods, got {}: {:?}",
        methods.len(),
        methods.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(methods.iter().any(|n| n.name == "new"));
    assert!(methods.iter().any(|n| n.name == "connect"));
    assert!(methods.iter().any(|n| n.name == "disconnect"));
    assert!(methods.iter().any(|n| n.name == "is_connected"));
    assert!(methods.iter().any(|n| n.name == "acquire"));
    assert!(methods.iter().any(|n| n.name == "release"));
}

#[test]
fn test_perl_extract_use_imports() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    // use strict, use warnings, use File::Path, use Carp
    assert_eq!(
        uses.len(),
        4,
        "expected 4 Use nodes, got {}: {:?}",
        uses.len(),
        uses.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(uses.iter().any(|n| n.name == "strict"));
    assert!(uses.iter().any(|n| n.name == "warnings"));
    assert!(uses.iter().any(|n| n.name == "File::Path"));
    assert!(uses.iter().any(|n| n.name == "Carp"));
}

#[test]
fn test_perl_extract_consts() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
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
fn test_perl_call_sites() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");

    // connect method calls main::log_message (qualified call)
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "main::log_message"),
        "should find main::log_message call, got: {:?}",
        call_refs.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );

    // acquire calls Connection->new
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "Connection->new"),
        "should find Connection->new call, got: {:?}",
        call_refs.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );

    // acquire calls $conn->connect
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "$conn->connect"),
        "should find $conn->connect call, got: {:?}",
        call_refs.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );

    // validate_config calls croak
    assert!(
        call_refs.iter().any(|r| r.reference_name == "croak"),
        "should find croak call, got: {:?}",
        call_refs.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );
}

#[test]
fn test_perl_docstrings() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log_message")
        .expect("log_message function not found");
    assert!(
        log_fn.docstring.is_some(),
        "log_message should have docstring"
    );
    let doc = log_fn.docstring.as_ref().unwrap();
    assert!(
        doc.contains("Logs a message"),
        "docstring should contain 'Logs a message', got: {}",
        doc
    );

    // MAX_RETRIES should have docstring
    let max_retries = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Const && n.name == "MAX_RETRIES")
        .expect("MAX_RETRIES not found");
    assert!(
        max_retries
            .docstring
            .as_ref()
            .unwrap()
            .contains("Maximum number of retries"),
        "docstring: {:?}",
        max_retries.docstring
    );

    // connect method should have docstring
    let connect = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "connect")
        .expect("connect method not found");
    assert!(
        connect
            .docstring
            .as_ref()
            .unwrap()
            .contains("Connects to the remote host"),
        "docstring: {:?}",
        connect.docstring
    );
}

#[test]
fn test_perl_file_node() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.pl");
}

#[test]
fn test_perl_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 4 Use + 2 Const + 2 Function + 2 Module = 10
    // Connection module contains: new, connect, disconnect, is_connected = 4
    // Pool module contains: new, acquire, release = 3
    // Total: 10 + 4 + 3 = 17
    assert!(
        contains.len() >= 15,
        "should have >= 15 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_perl_signatures() {
    let source = std::fs::read_to_string("tests/fixtures/sample.pl").unwrap();
    let extractor = PerlExtractor;
    let result = extractor.extract("sample.pl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log_message")
        .expect("log_message function not found");
    let sig = log_fn.signature.as_ref().unwrap();
    assert!(
        sig.contains("sub") && sig.contains("log_message"),
        "log_message signature should contain sub and name, got: {}",
        sig
    );
}
