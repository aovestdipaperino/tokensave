use tokensave::extraction::JuliaExtractor;
use tokensave::types::{EdgeKind, NodeKind};

fn extract(source: &str) -> tokensave::types::ExtractionResult {
    JuliaExtractor::extract_julia("test.jl", source)
}

#[test]
fn test_julia_extracts_module_struct_and_function() {
    let result = extract(include_str!("fixtures/julia_module.jl"));
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Module && n.name == "HelloMod"));
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Class && n.name == "Point"));
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Function && n.name == "distance"));

    let contains_edges = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .count();
    assert!(contains_edges >= 3);

    assert!(result
        .unresolved_refs
        .iter()
        .any(|r| { r.reference_kind == EdgeKind::Calls && r.reference_name == "sqrt" }));
}
