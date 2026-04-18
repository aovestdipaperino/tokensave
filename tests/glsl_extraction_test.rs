#![cfg(feature = "lang-glsl")]

use tokensave::extraction::GlslExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_glsl_file_node_is_root() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.glsl");
}

#[test]
fn test_glsl_extract_functions() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    let fn_names: Vec<_> = fns.iter().map(|n| n.name.as_str()).collect();
    assert!(fn_names.contains(&"main"), "functions: {fn_names:?}");
    assert!(
        fn_names.contains(&"fresnelSchlick"),
        "functions: {fn_names:?}"
    );
    assert!(
        fn_names.contains(&"distributionGGX"),
        "functions: {fn_names:?}"
    );
    assert!(
        fn_names.contains(&"geometrySchlickGGX"),
        "functions: {fn_names:?}"
    );
    assert!(
        fn_names.contains(&"calculatePointLight"),
        "functions: {fn_names:?}"
    );
}

#[test]
fn test_glsl_extract_structs() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    let struct_names: Vec<_> = structs.iter().map(|n| n.name.as_str()).collect();
    assert!(
        struct_names.contains(&"PointLight"),
        "structs: {struct_names:?}"
    );
    assert!(
        struct_names.contains(&"Material"),
        "structs: {struct_names:?}"
    );
}

#[test]
fn test_glsl_extract_struct_fields() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    let field_names: Vec<_> = fields.iter().map(|n| n.name.as_str()).collect();
    // PointLight fields
    assert!(field_names.contains(&"position"), "fields: {field_names:?}");
    assert!(field_names.contains(&"color"), "fields: {field_names:?}");
    assert!(
        field_names.contains(&"intensity"),
        "fields: {field_names:?}"
    );
    assert!(field_names.contains(&"radius"), "fields: {field_names:?}");
    // Material fields
    assert!(field_names.contains(&"albedo"), "fields: {field_names:?}");
    assert!(field_names.contains(&"metallic"), "fields: {field_names:?}");
    assert!(
        field_names.contains(&"roughness"),
        "fields: {field_names:?}"
    );
}

#[test]
fn test_glsl_extract_uniforms() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    let const_names: Vec<_> = consts.iter().map(|n| n.name.as_str()).collect();
    // uniform declarations become Const nodes
    assert!(
        const_names.contains(&"uModelMatrix"),
        "consts: {const_names:?}"
    );
    assert!(
        const_names.contains(&"uViewMatrix"),
        "consts: {const_names:?}"
    );
    assert!(
        const_names.contains(&"uProjectionMatrix"),
        "consts: {const_names:?}"
    );
    assert!(const_names.contains(&"uTime"), "consts: {const_names:?}");
}

#[test]
fn test_glsl_extract_in_out_declarations() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    let field_names: Vec<_> = fields.iter().map(|n| n.name.as_str()).collect();
    // in/out declarations become Field nodes
    assert!(
        field_names.contains(&"aPosition"),
        "fields: {field_names:?}"
    );
    assert!(field_names.contains(&"aNormal"), "fields: {field_names:?}");
    assert!(
        field_names.contains(&"aTexCoord"),
        "fields: {field_names:?}"
    );
    assert!(
        field_names.contains(&"vWorldPos"),
        "fields: {field_names:?}"
    );
    assert!(field_names.contains(&"vNormal"), "fields: {field_names:?}");
    assert!(
        field_names.contains(&"vTexCoord"),
        "fields: {field_names:?}"
    );
}

#[test]
fn test_glsl_extract_preproc_defines() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    let const_names: Vec<_> = consts.iter().map(|n| n.name.as_str()).collect();
    assert!(
        const_names.contains(&"MAX_LIGHTS"),
        "consts: {const_names:?}"
    );
}

#[test]
fn test_glsl_extract_const_globals() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    let const_names: Vec<_> = consts.iter().map(|n| n.name.as_str()).collect();
    assert!(const_names.contains(&"PI"), "consts: {const_names:?}");
}

#[test]
fn test_glsl_function_docstrings() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fresnel = result
        .nodes
        .iter()
        .find(|n| n.name == "fresnelSchlick")
        .unwrap();
    assert!(
        fresnel.docstring.is_some(),
        "fresnelSchlick should have a docstring"
    );
    assert!(
        fresnel
            .docstring
            .as_ref()
            .unwrap()
            .contains("Fresnel-Schlick"),
        "docstring: {:?}",
        fresnel.docstring
    );
}

#[test]
fn test_glsl_function_signatures() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let dist = result
        .nodes
        .iter()
        .find(|n| n.name == "distributionGGX")
        .unwrap();
    let sig = dist.signature.as_ref().unwrap();
    assert!(
        sig.contains("float"),
        "signature should include return type: {sig}"
    );
    assert!(
        sig.contains("vec3 N"),
        "signature should include params: {sig}"
    );
}

#[test]
fn test_glsl_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(!contains.is_empty(), "should have Contains edges");
}

#[test]
fn test_glsl_call_sites() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    let call_names: Vec<_> = calls.iter().map(|r| r.reference_name.as_str()).collect();
    // calculatePointLight calls fresnelSchlick, distributionGGX, geometrySchlickGGX
    assert!(
        call_names.contains(&"fresnelSchlick"),
        "calls: {call_names:?}"
    );
    assert!(
        call_names.contains(&"distributionGGX"),
        "calls: {call_names:?}"
    );
    assert!(
        call_names.contains(&"geometrySchlickGGX"),
        "calls: {call_names:?}"
    );
    // main calls calculatePointLight
    assert!(
        call_names.contains(&"calculatePointLight"),
        "calls: {call_names:?}"
    );
}

#[test]
fn test_glsl_extensions() {
    let ext = GlslExtractor;
    let extensions = ext.extensions();
    assert!(extensions.contains(&"glsl"));
    assert!(extensions.contains(&"vert"));
    assert!(extensions.contains(&"frag"));
    assert!(extensions.contains(&"geom"));
    assert!(extensions.contains(&"comp"));
}

#[test]
fn test_glsl_complexity_metrics() {
    let source = std::fs::read_to_string("tests/fixtures/sample.glsl").unwrap();
    let result = GlslExtractor.extract("sample.glsl", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // calculatePointLight has an if statement and main has a for loop
    let calc = result
        .nodes
        .iter()
        .find(|n| n.name == "calculatePointLight")
        .unwrap();
    assert!(
        calc.branches > 0,
        "calculatePointLight should have branch complexity"
    );
    let main_fn = result.nodes.iter().find(|n| n.name == "main").unwrap();
    assert!(main_fn.loops > 0, "main should have loop complexity");
}
