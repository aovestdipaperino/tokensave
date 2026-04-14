#![cfg(feature = "lang-dockerfile")]

use tokensave::extraction::DockerfileExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_dockerfile_file_node_is_root() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.dockerfile");
}

#[test]
fn test_dockerfile_extract_from_stages() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // FROM instructions with AS create named stages -- map to Module nodes
    let modules: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Module).collect();
    assert!(modules.iter().any(|n| n.name == "builder"), "should have 'builder' stage");
    assert!(modules.iter().any(|n| n.name == "runtime"), "should have 'runtime' stage");
}

#[test]
fn test_dockerfile_extract_env_vars() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    // ENV instructions -> Const nodes
    assert!(consts.iter().any(|n| n.name == "CARGO_HOME"), "consts: {:?}", consts.iter().map(|n| &n.name).collect::<Vec<_>>());
    assert!(consts.iter().any(|n| n.name == "APP_PORT"));
    assert!(consts.iter().any(|n| n.name == "LOG_LEVEL"));
}

#[test]
fn test_dockerfile_extract_arg_vars() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert!(consts.iter().any(|n| n.name == "APP_VERSION"), "ARG should be extracted as Const");
}

#[test]
fn test_dockerfile_extract_expose_ports() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // EXPOSE -> Field node (port declaration)
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(fields.iter().any(|n| n.name == "8080"), "EXPOSE 8080 should create a Field node");
}

#[test]
fn test_dockerfile_extract_labels() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert!(fields.iter().any(|n| n.name == "maintainer"), "LABEL maintainer should be Field node, got: {:?}", fields.iter().map(|n| &n.name).collect::<Vec<_>>());
}

#[test]
fn test_dockerfile_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
    assert!(!contains.is_empty(), "should have Contains edges from file/stage to children");
}

#[test]
fn test_dockerfile_copy_from_creates_uses_edge() {
    let source = std::fs::read_to_string("tests/fixtures/sample.dockerfile").unwrap();
    let result = DockerfileExtractor.extract("sample.dockerfile", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // COPY --from=builder creates a Uses edge referencing the builder stage
    let uses_edges: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Uses).collect();
    assert!(!uses_edges.is_empty(), "COPY --from=builder should create a Uses edge");
}

#[test]
fn test_dockerfile_extensions() {
    let ext = DockerfileExtractor;
    let extensions = ext.extensions();
    assert!(extensions.contains(&"dockerfile"));
    assert!(extensions.contains(&"Dockerfile"));
}
