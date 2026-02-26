use codegraph::extraction::GoExtractor;
use codegraph::extraction::LanguageExtractor;
use codegraph::types::*;

#[test]
fn test_go_extract_package() {
    let source = r#"package main

import "fmt"

func main() {
    fmt.Println("hello")
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let pkgs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::GoPackage)
        .collect();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "main");
}

#[test]
fn test_go_extract_function() {
    let source = r#"package main

// Add adds two numbers.
func Add(a, b int) int {
    return a + b
}

func helper() {}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("math.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2);
    let add_fn = fns.iter().find(|f| f.name == "Add").unwrap();
    assert_eq!(add_fn.visibility, Visibility::Pub); // uppercase = exported
    assert!(add_fn
        .docstring
        .as_ref()
        .unwrap()
        .contains("Add adds two numbers"));
    let helper_fn = fns.iter().find(|f| f.name == "helper").unwrap();
    assert_eq!(helper_fn.visibility, Visibility::Private); // lowercase = unexported
}

#[test]
fn test_go_extract_struct_with_fields() {
    let source = r#"package model

// Point represents a 2D point.
type Point struct {
    X float64
    Y float64
    label string
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("model/point.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
    assert_eq!(structs[0].visibility, Visibility::Pub);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 3);
    // X is exported, label is not
    let x_field = fields.iter().find(|f| f.name == "X").unwrap();
    assert_eq!(x_field.visibility, Visibility::Pub);
    let label_field = fields.iter().find(|f| f.name == "label").unwrap();
    assert_eq!(label_field.visibility, Visibility::Private);
}

#[test]
fn test_go_extract_struct_tags() {
    let source = r#"package model

type Config struct {
    Name string `json:"name" yaml:"name"`
    Port int    `json:"port"`
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("model/config.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let tags: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::StructTag)
        .collect();
    assert!(tags.len() >= 2, "should extract struct tags");
}

#[test]
fn test_go_extract_interface() {
    let source = r#"package io

// Reader is the interface for reading.
type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("io/reader.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ifaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::InterfaceType)
        .collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "Reader");
    assert_eq!(ifaces[0].visibility, Visibility::Pub);
}

#[test]
fn test_go_extract_method_with_receiver() {
    let source = r#"package model

type Circle struct {
    Radius float64
}

// Area calculates the area.
func (c *Circle) Area() float64 {
    return 3.14159 * c.Radius * c.Radius
}

func (c Circle) String() string {
    return "circle"
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("model/circle.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::StructMethod)
        .collect();
    assert_eq!(methods.len(), 2);
    // Check Receives edges
    let receives: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Receives)
        .collect();
    assert!(
        !receives.is_empty(),
        "should have Receives edges for methods with receivers"
    );
}

#[test]
fn test_go_extract_imports() {
    let source = r#"package main

import (
    "fmt"
    "os"
    "github.com/pkg/errors"
)
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 3);
}

#[test]
fn test_go_extract_const_and_var() {
    let source = r#"package main

const MaxSize = 1024

var counter int
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MaxSize");
    let statics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Static)
        .collect();
    assert_eq!(statics.len(), 1);
    assert_eq!(statics[0].name, "counter");
}

#[test]
fn test_go_extract_call_sites() {
    let source = r#"package main

import "fmt"

func greet(name string) {
    fmt.Println("Hello", name)
}

func main() {
    greet("world")
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
}

#[test]
fn test_go_extract_type_alias() {
    let source = r#"package main

type StringSlice = []string
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let aliases: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "StringSlice");
}

#[test]
fn test_go_extract_interface_embedding() {
    let source = r#"package io

type Reader interface {
    Read(p []byte) (int, error)
}

type ReadWriter interface {
    Reader
    Write(p []byte) (int, error)
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("io/io.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Should have an Extends edge or unresolved ref for Reader embedded in ReadWriter
    let has_extends = result.edges.iter().any(|e| e.kind == EdgeKind::Extends)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Extends);
    assert!(has_extends, "should detect interface embedding as Extends");
}

#[test]
fn test_go_extract_generic_function() {
    let source = r#"package main

func Map[T any, U any](s []T, f func(T) U) []U {
    r := make([]U, len(s))
    for i, v := range s {
        r[i] = f(v)
    }
    return r
}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "Map");
    let generics: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::GenericParam)
        .collect();
    assert!(
        generics.len() >= 2,
        "should extract generic type params T and U"
    );
}

#[test]
fn test_go_file_node_is_root() {
    let source = r#"package main

func main() {}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "main.go");
}

#[test]
fn test_go_contains_edges() {
    let source = r#"package main

type Foo struct {
    Bar int
}

func (f Foo) Baz() {}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: GoPackage, Struct, StructMethod; Struct contains: Field
    assert!(
        contains.len() >= 4,
        "should have Contains edges: {:?}",
        contains.len()
    );
}

#[test]
fn test_go_qualified_names() {
    let source = r#"package server

func HandleRequest() {}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("pkg/server/handler.go", source);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].qualified_name.contains("HandleRequest"));
    assert!(fns[0].qualified_name.contains("handler.go"));
}
