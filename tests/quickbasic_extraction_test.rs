#[cfg(feature = "lang-qbasic")]
mod quickbasic_tests {

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::QuickBasicExtractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.bi").unwrap();
    let extractor = QuickBasicExtractor;
    let result = extractor.extract("sample.bi", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

#[test]
fn test_quickbasic_extensions() {
    let extractor = QuickBasicExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"bi"), "should handle .bi files");
    assert!(exts.contains(&"bm"), "should handle .bm files");
    assert!(!exts.contains(&"qb"), "should NOT overlap with QBasic .qb");
}

#[test]
fn test_quickbasic_language_name() {
    let extractor = QuickBasicExtractor;
    assert_eq!(extractor.language_name(), "QuickBASIC");
}

#[test]
fn test_quickbasic_file_node() {
    let result = extract_fixture();
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.bi");
}

#[test]
fn test_quickbasic_sub_functions() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // SUBs: InitSystem, Shutdown, LogInit
    // FUNCTION: GetStatus
    assert!(
        fns.len() >= 4,
        "expected >= 4 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fns.iter().any(|n| n.name == "InitSystem"));
    assert!(fns.iter().any(|n| n.name == "Shutdown"));
    assert!(fns.iter().any(|n| n.name == "GetStatus"));
    assert!(fns.iter().any(|n| n.name == "LogInit"));
}

#[test]
fn test_quickbasic_type_as_struct() {
    let result = extract_fixture();
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Config");
}

#[test]
fn test_quickbasic_type_fields() {
    let result = extract_fixture();
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field && n.qualified_name.contains("Config"))
        .collect();
    assert!(
        fields.len() >= 3,
        "expected >= 3 fields in Config, got {}: {:?}",
        fields.len(),
        fields.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fields.iter().any(|n| n.name == "name"));
    assert!(fields.iter().any(|n| n.name == "value"));
    assert!(fields.iter().any(|n| n.name == "active"));
}

#[test]
fn test_quickbasic_const_nodes() {
    let result = extract_fixture();
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert!(
        !consts.is_empty(),
        "expected at least 1 CONST node"
    );
}

#[test]
fn test_quickbasic_call_sites() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");
    assert!(
        calls.iter().any(|r| r.reference_name == "LogInit"),
        "expected CALL LogInit from InitSystem"
    );
}

#[test]
fn test_quickbasic_complexity() {
    let result = extract_fixture();
    let get_status = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "GetStatus")
        .expect("GetStatus function not found");
    assert!(
        get_status.branches >= 1,
        "GetStatus should have >= 1 branch (IF), got {}",
        get_status.branches
    );
}

#[test]
fn test_quickbasic_docstrings() {
    let result = extract_fixture();
    let init_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "InitSystem")
        .expect("InitSystem not found");
    assert!(
        init_fn.docstring.is_some(),
        "InitSystem should have a docstring"
    );
    assert!(
        init_fn.docstring.as_ref().unwrap().contains("Initializes"),
        "docstring: {:?}",
        init_fn.docstring
    );
}

#[test]
fn test_quickbasic_contains_edges() {
    let result = extract_fixture();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(
        contains.len() >= 8,
        "should have >= 8 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_quickbasic_parses_redim_and_sleep() {
    // Verify that QB4.5-specific statements (REDIM, SLEEP, ERASE) don't cause parse errors
    let source = r#"
SUB Test
    REDIM arr(1 TO 10) AS INTEGER
    SLEEP 1
    ERASE arr
END SUB
"#;
    let extractor = QuickBasicExtractor;
    let result = extractor.extract("test.bi", source);
    assert!(
        result.errors.is_empty(),
        "QB4.5 statements should parse without errors: {:?}",
        result.errors
    );
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "Test");
}

} // mod quickbasic_tests
