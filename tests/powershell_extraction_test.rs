use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::PowerShellExtractor;
use tokensave::types::*;

#[test]
fn test_powershell_extract_functions() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
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
    assert!(fns.iter().any(|n| n.name == "Write-Log"));
    assert!(fns.iter().any(|n| n.name == "Test-Config"));
    assert!(fns.iter().any(|n| n.name == "Connect-Server"));
    assert!(fns.iter().any(|n| n.name == "Disconnect-Server"));
    assert!(fns.iter().any(|n| n.name == "Main"));
}

#[test]
fn test_powershell_extract_consts() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
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
    assert!(consts.iter().any(|n| n.name == "MaxRetries"));
    assert!(consts.iter().any(|n| n.name == "DefaultPort"));
}

#[test]
fn test_powershell_extract_imports() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(
        uses.len(),
        2,
        "expected 2 Use nodes, got {}: {:?}",
        uses.len(),
        uses.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(uses.iter().any(|n| n.name == "ActiveDirectory"));
    assert!(uses.iter().any(|n| n.name.contains("Utils.ps1")));
}

#[test]
fn test_powershell_call_sites() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
    // Write-Log calls Write-Host and Get-Date
    assert!(
        call_refs.iter().any(|r| r.reference_name == "Write-Host"),
        "should find Write-Host call"
    );
    // Test-Config calls Write-Log
    assert!(
        call_refs.iter().any(|r| r.reference_name == "Write-Log"),
        "should find Write-Log call"
    );
    // Connect-Server calls Test-Connection
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "Test-Connection"),
        "should find Test-Connection call"
    );
    // Main calls Test-Config
    assert!(
        call_refs.iter().any(|r| r.reference_name == "Test-Config"),
        "should find Test-Config call"
    );
}

#[test]
fn test_powershell_docstrings() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Write-Log should have a block comment docstring.
    let write_log = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "Write-Log")
        .expect("Write-Log function not found");
    assert!(
        write_log
            .docstring
            .as_ref()
            .unwrap()
            .contains("Logs a message"),
        "docstring: {:?}",
        write_log.docstring
    );

    // Test-Config should have a line comment docstring.
    let test_config = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "Test-Config")
        .expect("Test-Config function not found");
    assert!(
        test_config
            .docstring
            .as_ref()
            .unwrap()
            .contains("Validates the configuration"),
        "docstring: {:?}",
        test_config.docstring
    );

    // Main should have a docstring.
    let main_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "Main")
        .expect("Main function not found");
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
fn test_powershell_file_node() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.ps1");
}

#[test]
fn test_powershell_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.ps1").unwrap();
    let extractor = PowerShellExtractor;
    let result = extractor.extract("sample.ps1", &source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 5 functions + 2 consts + 2 Use = 9 Contains edges
    assert!(
        contains.len() >= 9,
        "should have >= 9 Contains edges, got {}",
        contains.len()
    );
}
