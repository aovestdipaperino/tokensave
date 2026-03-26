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

fn extract_flake() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample-flake.nix")
        .expect("failed to read sample-flake.nix");
    let extractor = NixExtractor;
    extractor.extract("flake.nix", &source)
}

// -------------------------------------------------------------------
// Enhancement 2: Import path resolution
// -------------------------------------------------------------------

#[test]
fn test_nix_import_path_resolution() {
    let result = extract_sample();

    // Should have a Use node for `import ./utils.nix`
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use && n.name == "./utils.nix")
        .collect();
    assert!(
        !uses.is_empty(),
        "should have Use node for import ./utils.nix, got uses: {:?}",
        result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Use)
            .map(|n| &n.name)
            .collect::<Vec<_>>()
    );

    // Should have an unresolved Uses ref for ./utils.nix
    let uses_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses && r.reference_name == "./utils.nix")
        .collect();
    assert!(
        !uses_refs.is_empty(),
        "should have unresolved Uses ref for ./utils.nix, got: {:?}",
        result
            .unresolved_refs
            .iter()
            .map(|r| (&r.reference_kind, &r.reference_name))
            .collect::<Vec<_>>()
    );
}

// -------------------------------------------------------------------
// Enhancement 1: Derivation field extraction
// -------------------------------------------------------------------

#[test]
fn test_nix_derivation_fields() {
    let result = extract_flake();
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();

    // mkDerivation calls should produce Field nodes for pname, version, buildInputs, etc.
    let field_names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();

    assert!(
        field_names.contains(&"pname"),
        "should have pname Field, got: {:?}",
        field_names
    );
    assert!(
        field_names.contains(&"version"),
        "should have version Field, got: {:?}",
        field_names
    );
    assert!(
        field_names.contains(&"src"),
        "should have src Field, got: {:?}",
        field_names
    );
    assert!(
        field_names.contains(&"buildInputs"),
        "should have buildInputs Field, got: {:?}",
        field_names
    );
    assert!(
        field_names.contains(&"nativeBuildInputs"),
        "should have nativeBuildInputs Field, got: {:?}",
        field_names
    );

    // Fields should have signatures (first line of the binding text)
    let pname = fields.iter().find(|f| f.name == "pname").unwrap();
    assert!(
        pname.signature.is_some(),
        "pname field should have a signature"
    );
    assert!(
        pname.signature.as_ref().unwrap().contains("pname"),
        "pname signature should contain 'pname', got: {}",
        pname.signature.as_ref().unwrap()
    );
}

// -------------------------------------------------------------------
// Enhancement 3: Flake output schema awareness
// -------------------------------------------------------------------

#[test]
fn test_nix_flake_output_modules() {
    let result = extract_flake();
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    let module_names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();

    // packages, devShells, apps, checks should all be Module nodes
    assert!(
        module_names.contains(&"packages"),
        "packages should be Module, got modules: {:?}",
        module_names
    );
    assert!(
        module_names.contains(&"devShells"),
        "devShells should be Module, got modules: {:?}",
        module_names
    );
    assert!(
        module_names.contains(&"apps"),
        "apps should be Module, got modules: {:?}",
        module_names
    );
    assert!(
        module_names.contains(&"checks"),
        "checks should be Module, got modules: {:?}",
        module_names
    );

    // Verify that these are nested under outputs
    let packages = modules.iter().find(|m| m.name == "packages").unwrap();
    assert!(
        packages.qualified_name.contains("outputs"),
        "packages should be qualified under outputs, got: {}",
        packages.qualified_name
    );
}

// -------------------------------------------------------------------
// Enhancement 1+3: mkShell field extraction
// -------------------------------------------------------------------

#[test]
fn test_nix_mkshell_fields() {
    let result = extract_flake();
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // devShells is forced to Module and its value is a mkShell call
    // The mkShell attrset should produce Field nodes
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Field
                && n.qualified_name.contains("devShells")
        })
        .collect();
    let field_names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();

    assert!(
        field_names.contains(&"buildInputs"),
        "mkShell should have buildInputs Field, got: {:?}",
        field_names
    );
    assert!(
        field_names.contains(&"shellHook"),
        "mkShell should have shellHook Field, got: {:?}",
        field_names
    );
}
