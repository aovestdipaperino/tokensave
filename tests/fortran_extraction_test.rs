use tokensave::extraction::FortranExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract_fixture() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.f90").unwrap();
    let extractor = FortranExtractor;
    let result = extractor.extract("sample.f90", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    result
}

#[test]
fn test_fortran_file_root() {
    let result = extract_fixture();
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
}

#[test]
fn test_fortran_module() {
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
    assert_eq!(modules[0].name, "networking");
}

#[test]
fn test_fortran_program() {
    let result = extract_fixture();
    let prog = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "main");
    assert!(prog.is_some(), "program main not found as Function node");
}

#[test]
fn test_fortran_subroutines() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // Subroutines: log_message, connect_endpoint, disconnect_endpoint
    assert!(
        fns.iter().any(|n| n.name == "log_message"),
        "log_message subroutine not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "connect_endpoint"),
        "connect_endpoint subroutine not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "disconnect_endpoint"),
        "disconnect_endpoint subroutine not found"
    );
}

#[test]
fn test_fortran_functions() {
    let result = extract_fixture();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // Functions: create_endpoint, is_connected
    assert!(
        fns.iter().any(|n| n.name == "create_endpoint"),
        "create_endpoint function not found"
    );
    assert!(
        fns.iter().any(|n| n.name == "is_connected"),
        "is_connected function not found"
    );
}

#[test]
fn test_fortran_derived_types() {
    let result = extract_fixture();
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert!(
        structs.iter().any(|n| n.name == "Endpoint"),
        "Endpoint derived type not found"
    );
    assert!(
        structs.iter().any(|n| n.name == "PooledEndpoint"),
        "PooledEndpoint derived type not found"
    );
}

#[test]
fn test_fortran_type_extension() {
    let result = extract_fixture();
    // PooledEndpoint extends Endpoint
    let extends_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(
        extends_refs.iter().any(|r| r.reference_name == "Endpoint"),
        "expected Extends ref for PooledEndpoint -> Endpoint, got: {:?}",
        extends_refs
            .iter()
            .map(|r| &r.reference_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_fortran_interface() {
    let result = extract_fixture();
    let interfaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(
        interfaces.len(),
        1,
        "expected 1 interface, got {}: {:?}",
        interfaces.len(),
        interfaces.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert_eq!(interfaces[0].name, "Connectable");
}

#[test]
fn test_fortran_constants() {
    let result = extract_fixture();
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert!(
        consts.iter().any(|n| n.name == "MAX_RETRIES"),
        "MAX_RETRIES constant not found"
    );
    assert!(
        consts.iter().any(|n| n.name == "DEFAULT_PORT"),
        "DEFAULT_PORT constant not found"
    );
}

#[test]
fn test_fortran_use_imports() {
    let result = extract_fixture();
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert!(
        uses.iter().any(|n| n.name == "networking"),
        "use networking not found, got: {:?}",
        uses.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_fortran_fields() {
    let result = extract_fixture();
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    // Endpoint has: host, port, connected
    assert!(
        fields.iter().any(|n| n.name == "host"),
        "host field not found"
    );
    assert!(
        fields.iter().any(|n| n.name == "port"),
        "port field not found"
    );
    assert!(
        fields.iter().any(|n| n.name == "connected"),
        "connected field not found"
    );
    // PooledEndpoint has: pool_size
    assert!(
        fields.iter().any(|n| n.name == "pool_size"),
        "pool_size field not found"
    );
}

#[test]
fn test_fortran_call_sites() {
    let result = extract_fixture();
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");
    // connect_endpoint calls log_message
    assert!(
        calls.iter().any(|r| r.reference_name == "log_message"),
        "expected call to log_message, got: {:?}",
        calls
            .iter()
            .map(|r| &r.reference_name)
            .collect::<Vec<_>>()
    );
    // program calls create_endpoint, connect_endpoint, disconnect_endpoint
    assert!(
        calls.iter().any(|r| r.reference_name == "create_endpoint"),
        "expected call to create_endpoint"
    );
    assert!(
        calls.iter().any(|r| r.reference_name == "connect_endpoint"),
        "expected call to connect_endpoint"
    );
    assert!(
        calls
            .iter()
            .any(|r| r.reference_name == "disconnect_endpoint"),
        "expected call to disconnect_endpoint"
    );
}

#[test]
fn test_fortran_docstrings() {
    let result = extract_fixture();
    // Check subroutines which have comments inside the module.
    let log_msg = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log_message");
    assert!(log_msg.is_some(), "log_message not found");
    assert!(
        log_msg.unwrap().docstring.is_some(),
        "log_message should have docstring"
    );

    let create_ep = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "create_endpoint");
    assert!(create_ep.is_some(), "create_endpoint not found");
    assert!(
        create_ep.unwrap().docstring.is_some(),
        "create_endpoint should have docstring"
    );

    // Endpoint type should have docstring.
    let ep = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Struct && n.name == "Endpoint");
    assert!(ep.is_some(), "Endpoint not found");
    assert!(
        ep.unwrap().docstring.is_some(),
        "Endpoint should have docstring"
    );
}

#[test]
fn test_fortran_contains_edges() {
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
    // Module should contain subroutines, functions, types, constants, interface
    assert!(
        contains.len() >= 5,
        "expected >= 5 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_fortran_qualified_names() {
    let result = extract_fixture();
    let log_msg = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log_message")
        .unwrap();
    assert!(
        log_msg.qualified_name.contains("networking"),
        "log_message qualified name should contain 'networking', got: {}",
        log_msg.qualified_name
    );
}

#[test]
fn test_fortran_extensions() {
    let extractor = FortranExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"f90"));
    assert!(exts.contains(&"f95"));
    assert!(exts.contains(&"f03"));
    assert!(exts.contains(&"f08"));
    assert!(exts.contains(&"f18"));
    assert!(exts.contains(&"f"));
    assert!(exts.contains(&"for"));
}

#[test]
fn test_fortran_language_name() {
    let extractor = FortranExtractor;
    assert_eq!(extractor.language_name(), "Fortran");
}
