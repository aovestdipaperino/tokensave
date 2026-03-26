use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::NixExtractor;
use tokensave::types::*;

fn extract_sample() -> ExtractionResult {
    let source =
        std::fs::read_to_string("tests/fixtures/sample.nix").expect("failed to read sample.nix");
    let extractor = NixExtractor;
    extractor.extract("sample.nix", &source)
}

#[test]
fn test_nix_no_errors() {
    let result = extract_sample();
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn test_nix_file_node() {
    let result = extract_sample();
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.nix");
}

#[test]
fn test_nix_functions() {
    let result = extract_sample();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // log and mkConnection are top-level functions
    assert!(
        fns.iter().any(|f| f.name == "log"),
        "log function not found, got: {:?}",
        fns.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|f| f.name == "mkConnection"),
        "mkConnection function not found, got: {:?}",
        fns.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_nix_consts() {
    let result = extract_sample();
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert!(
        consts.iter().any(|c| c.name == "defaultPort"),
        "defaultPort const not found, got: {:?}",
        consts.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert!(
        consts.iter().any(|c| c.name == "maxRetries"),
        "maxRetries const not found, got: {:?}",
        consts.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_nix_modules() {
    let result = extract_sample();
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert!(
        modules.iter().any(|m| m.name == "networking"),
        "networking module not found, got: {:?}",
        modules.iter().map(|m| &m.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_nix_nested_functions() {
    let result = extract_sample();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // mkPool and validateConfig are nested inside networking module
    assert!(
        fns.iter().any(|f| f.name == "mkPool"),
        "mkPool nested function not found, got: {:?}",
        fns.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    assert!(
        fns.iter().any(|f| f.name == "validateConfig"),
        "validateConfig nested function not found, got: {:?}",
        fns.iter().map(|f| &f.name).collect::<Vec<_>>()
    );

    // Verify mkPool is qualified under networking
    let mk_pool = fns.iter().find(|f| f.name == "mkPool").unwrap();
    assert!(
        mk_pool.qualified_name.contains("networking"),
        "mkPool should be qualified under networking, got: {}",
        mk_pool.qualified_name
    );
}

#[test]
fn test_nix_docstrings() {
    let result = extract_sample();

    // defaultPort should have a docstring
    let dp = result
        .nodes
        .iter()
        .find(|n| n.name == "defaultPort")
        .unwrap();
    assert!(
        dp.docstring.is_some(),
        "defaultPort should have docstring"
    );
    assert!(
        dp.docstring
            .as_ref()
            .unwrap()
            .contains("Default port"),
        "docstring: {:?}",
        dp.docstring
    );

    // log should have a docstring
    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log")
        .unwrap();
    assert!(
        log_fn.docstring.is_some(),
        "log function should have docstring"
    );
    assert!(
        log_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("Formats a log message"),
        "docstring: {:?}",
        log_fn.docstring
    );

    // networking should have a docstring
    let net = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Module && n.name == "networking")
        .unwrap();
    assert!(
        net.docstring.is_some(),
        "networking should have docstring"
    );
    assert!(
        net.docstring
            .as_ref()
            .unwrap()
            .contains("Networking utilities"),
        "docstring: {:?}",
        net.docstring
    );
}

#[test]
fn test_nix_call_sites() {
    let result = extract_sample();
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        !call_refs.is_empty(),
        "should have call site refs"
    );
    // mkConnection should be called (e.g., from mkPool)
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "mkConnection"),
        "should find mkConnection call, got: {:?}",
        call_refs
            .iter()
            .map(|r| &r.reference_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_nix_contains_edges() {
    let result = extract_sample();
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File -> consts, functions, module; module -> nested functions/consts
    assert!(
        contains.len() >= 5,
        "should have >= 5 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_nix_visibility() {
    let result = extract_sample();
    // All Nix definitions should be Pub (Nix has no visibility modifiers)
    for node in &result.nodes {
        assert_eq!(
            node.visibility,
            Visibility::Pub,
            "node {} ({:?}) should be Pub",
            node.name,
            node.kind
        );
    }
}

#[test]
fn test_nix_inherit_use_nodes() {
    let result = extract_sample();
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    // inherit networking; inherit (networking) mkPool validateConfig;
    assert!(
        uses.iter().any(|u| u.name == "networking"),
        "should have Use node for inherit networking, got: {:?}",
        uses.iter().map(|u| &u.name).collect::<Vec<_>>()
    );
    assert!(
        uses.iter().any(|u| u.name == "mkPool"),
        "should have Use node for inherit mkPool, got: {:?}",
        uses.iter().map(|u| &u.name).collect::<Vec<_>>()
    );
    assert!(
        uses.iter().any(|u| u.name == "validateConfig"),
        "should have Use node for inherit validateConfig, got: {:?}",
        uses.iter().map(|u| &u.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_nix_function_signature() {
    let result = extract_sample();
    let mk_conn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "mkConnection")
        .unwrap();
    assert!(
        mk_conn.signature.is_some(),
        "mkConnection should have a signature"
    );
    assert!(
        mk_conn.signature.as_ref().unwrap().contains("mkConnection"),
        "signature should contain mkConnection, got: {}",
        mk_conn.signature.as_ref().unwrap()
    );
}
