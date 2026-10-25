#![cfg(feature = "lang-cuda")]

use tokensave::extraction::CudaExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

fn extract() -> ExtractionResult {
    let source = std::fs::read_to_string("tests/fixtures/sample.cu").unwrap();
    CudaExtractor.extract("sample.cu", &source)
}

#[test]
fn test_cuda_file_node_is_root() {
    let result = extract();
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.cu");
}

#[test]
fn test_cuda_extract_struct_and_fields() {
    let result = extract();
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .map(|n| n.name.as_str())
        .collect();
    assert!(structs.contains(&"ReduceState"), "structs: {structs:?}");

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .map(|n| n.name.as_str())
        .collect();
    assert!(fields.contains(&"sum"), "fields: {fields:?}");
    assert!(fields.contains(&"count"), "fields: {fields:?}");
}

#[test]
fn test_cuda_extract_device_and_global_functions() {
    let result = extract();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .map(|n| n.name.as_str())
        .collect();
    // `__device__`/`__global__` are execution-space qualifiers the C++
    // grammar treats like any other declaration specifier -- the functions
    // themselves must still be extracted.
    assert!(fns.contains(&"square"), "functions: {fns:?}");
    assert!(fns.contains(&"reduce_kernel"), "functions: {fns:?}");
    assert!(fns.contains(&"launch_reduce"), "functions: {fns:?}");
}

#[test]
fn test_cuda_kernel_launch_does_not_block_extraction() {
    // `reduce_kernel<<<blocks, BLOCK_SIZE>>>(...)` is CUDA-only syntax with
    // no C++ equivalent. It must not prevent `launch_reduce` (the enclosing
    // function) or the call to `reduce_kernel` from being extracted.
    let result = extract();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .map(|n| n.name.as_str())
        .collect();
    assert!(
        fns.contains(&"launch_reduce"),
        "launch_reduce must be extracted despite the <<<>>> launch inside it: {fns:?}"
    );

    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        calls.contains(&"reduce_kernel"),
        "expected a call reference to reduce_kernel: {calls:?}"
    );
    assert!(
        calls.contains(&"square"),
        "expected a call reference to square: {calls:?}"
    );
}

#[test]
fn test_cuda_extract_device_constant() {
    let result = extract();
    let names: Vec<_> = result.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"kEpsilon"), "nodes: {names:?}");
}

#[test]
fn test_cuda_struct_docstring() {
    let result = extract();
    let reduce_state = result
        .nodes
        .iter()
        .find(|n| n.name == "ReduceState")
        .unwrap();
    assert!(
        reduce_state.docstring.is_some(),
        "ReduceState should have a docstring"
    );
    assert!(reduce_state
        .docstring
        .as_ref()
        .unwrap()
        .contains("per-block reduction state"));
}

#[test]
fn test_cuda_extensions() {
    let ext = CudaExtractor;
    let extensions = ext.extensions();
    assert!(extensions.contains(&"cu"));
    assert!(extensions.contains(&"cuh"));
}

#[test]
fn test_cuda_registry_dispatch() {
    use tokensave::extraction::LanguageRegistry;
    let registry = LanguageRegistry::new();
    let cu = registry
        .extractor_for_file("kernels/reduce.cu")
        .expect(".cu must be handled");
    assert_eq!(cu.language_name(), "CUDA");
    let cuh = registry
        .extractor_for_file("kernels/reduce_common.cuh")
        .expect(".cuh must be handled");
    assert_eq!(cuh.language_name(), "CUDA");
}
