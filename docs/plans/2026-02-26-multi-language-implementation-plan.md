# Multi-Language Support (Go + Java) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Go and Java language support to codegraph with deep extraction, using a trait-based abstraction layer.

**Architecture:** Introduce a `LanguageExtractor` trait and `LanguageRegistry` that dispatches to per-language extractors based on file extension. Each extractor uses tree-sitter with a language-specific grammar. The existing `RustExtractor` is retrofitted to implement the trait.

**Tech Stack:** tree-sitter, tree-sitter-go, tree-sitter-java, Rust traits

---

### Task 1: Add tree-sitter dependencies to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add dependencies**

Add `tree-sitter-go` and `tree-sitter-java` to `[dependencies]`:

```toml
tree-sitter-go = "0.23"
tree-sitter-java = "0.23"
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles successfully (new deps are unused but that's OK)

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add tree-sitter-go and tree-sitter-java dependencies"
```

---

### Task 2: Expand NodeKind and EdgeKind enums

**Files:**
- Modify: `src/types.rs`
- Test: `tests/types_test.rs`

**Step 1: Write tests for new NodeKind variants**

Add to `tests/types_test.rs`:

```rust
#[test]
fn test_new_node_kinds_roundtrip() {
    let kinds = vec![
        (NodeKind::Class, "class"),
        (NodeKind::Interface, "interface"),
        (NodeKind::Constructor, "constructor"),
        (NodeKind::Annotation, "annotation"),
        (NodeKind::AnnotationUsage, "annotation_usage"),
        (NodeKind::Package, "package"),
        (NodeKind::InnerClass, "inner_class"),
        (NodeKind::InitBlock, "init_block"),
        (NodeKind::AbstractMethod, "abstract_method"),
        (NodeKind::InterfaceType, "interface_type"),
        (NodeKind::StructMethod, "struct_method"),
        (NodeKind::GoPackage, "go_package"),
        (NodeKind::StructTag, "struct_tag"),
        (NodeKind::GenericParam, "generic_param"),
    ];
    for (kind, expected_str) in kinds {
        assert_eq!(kind.as_str(), expected_str);
        assert_eq!(NodeKind::from_str(expected_str), Some(kind));
    }
}

#[test]
fn test_new_edge_kinds_roundtrip() {
    let kinds = vec![
        (EdgeKind::Extends, "extends"),
        (EdgeKind::Annotates, "annotates"),
        (EdgeKind::Receives, "receives"),
    ];
    for (kind, expected_str) in kinds {
        assert_eq!(kind.as_str(), expected_str);
        assert_eq!(EdgeKind::from_str(expected_str), Some(kind));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test types_test test_new_node_kinds_roundtrip test_new_edge_kinds_roundtrip`
Expected: FAIL — variants don't exist yet

**Step 3: Add NodeKind variants**

In `src/types.rs`, add to `NodeKind` enum after `Use`:

```rust
    // Java-specific
    Class,
    Interface,
    Constructor,
    Annotation,
    AnnotationUsage,
    Package,
    InnerClass,
    InitBlock,
    AbstractMethod,
    // Go-specific
    InterfaceType,
    StructMethod,
    GoPackage,
    StructTag,
    // Shared
    GenericParam,
```

Add corresponding arms to `as_str()`:

```rust
    NodeKind::Class => "class",
    NodeKind::Interface => "interface",
    NodeKind::Constructor => "constructor",
    NodeKind::Annotation => "annotation",
    NodeKind::AnnotationUsage => "annotation_usage",
    NodeKind::Package => "package",
    NodeKind::InnerClass => "inner_class",
    NodeKind::InitBlock => "init_block",
    NodeKind::AbstractMethod => "abstract_method",
    NodeKind::InterfaceType => "interface_type",
    NodeKind::StructMethod => "struct_method",
    NodeKind::GoPackage => "go_package",
    NodeKind::StructTag => "struct_tag",
    NodeKind::GenericParam => "generic_param",
```

Add corresponding arms to `from_str()`:

```rust
    "class" => Some(NodeKind::Class),
    "interface" => Some(NodeKind::Interface),
    "constructor" => Some(NodeKind::Constructor),
    "annotation" => Some(NodeKind::Annotation),
    "annotation_usage" => Some(NodeKind::AnnotationUsage),
    "package" => Some(NodeKind::Package),
    "inner_class" => Some(NodeKind::InnerClass),
    "init_block" => Some(NodeKind::InitBlock),
    "abstract_method" => Some(NodeKind::AbstractMethod),
    "interface_type" => Some(NodeKind::InterfaceType),
    "struct_method" => Some(NodeKind::StructMethod),
    "go_package" => Some(NodeKind::GoPackage),
    "struct_tag" => Some(NodeKind::StructTag),
    "generic_param" => Some(NodeKind::GenericParam),
```

**Step 4: Add EdgeKind variants**

In `src/types.rs`, add to `EdgeKind` enum after `DerivesMacro`:

```rust
    Extends,
    Annotates,
    Receives,
```

Add to `EdgeKind::as_str()`:

```rust
    EdgeKind::Extends => "extends",
    EdgeKind::Annotates => "annotates",
    EdgeKind::Receives => "receives",
```

Add to `EdgeKind::from_str()`:

```rust
    "extends" => Some(EdgeKind::Extends),
    "annotates" => Some(EdgeKind::Annotates),
    "receives" => Some(EdgeKind::Receives),
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --test types_test`
Expected: PASS

**Step 6: Run full test suite to check no regressions**

Run: `cargo test`
Expected: PASS — existing code doesn't break since we only added new variants

**Step 7: Commit**

```bash
git add src/types.rs tests/types_test.rs
git commit -m "feat: expand NodeKind and EdgeKind enums for Go and Java support"
```

---

### Task 3: Create LanguageExtractor trait and LanguageRegistry

**Files:**
- Modify: `src/extraction/mod.rs`
- Test: `tests/extraction_test.rs`

**Step 1: Write test for language registry**

Add to `tests/extraction_test.rs`:

```rust
use codegraph::extraction::LanguageRegistry;

#[test]
fn test_language_registry_finds_rust_extractor() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("src/main.rs").is_some());
    assert!(registry.extractor_for_file("lib.rs").is_some());
}

#[test]
fn test_language_registry_finds_go_extractor() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("main.go").is_some());
    assert!(registry.extractor_for_file("pkg/server.go").is_some());
}

#[test]
fn test_language_registry_finds_java_extractor() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("Main.java").is_some());
    assert!(registry.extractor_for_file("src/com/example/App.java").is_some());
}

#[test]
fn test_language_registry_returns_none_for_unknown() {
    let registry = LanguageRegistry::new();
    assert!(registry.extractor_for_file("script.py").is_none());
    assert!(registry.extractor_for_file("style.css").is_none());
    assert!(registry.extractor_for_file("README.md").is_none());
}

#[test]
fn test_language_registry_supported_extensions() {
    let registry = LanguageRegistry::new();
    let exts = registry.supported_extensions();
    assert!(exts.contains(&"rs"));
    assert!(exts.contains(&"go"));
    assert!(exts.contains(&"java"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test extraction_test test_language_registry`
Expected: FAIL — `LanguageRegistry` doesn't exist

**Step 3: Define trait and registry in `src/extraction/mod.rs`**

Replace the contents of `src/extraction/mod.rs` with:

```rust
/// Tree-sitter based source code extraction module.
///
/// This module provides extractors that parse source files using tree-sitter
/// and produce structured graph nodes and edges.
mod rust_extractor;
mod go_extractor;
mod java_extractor;

pub use rust_extractor::RustExtractor;
pub use go_extractor::GoExtractor;
pub use java_extractor::JavaExtractor;

use crate::types::ExtractionResult;

/// Trait for language-specific source code extractors.
///
/// Each implementation handles a single programming language,
/// using tree-sitter to parse source and emit graph nodes and edges.
pub trait LanguageExtractor: Send + Sync {
    /// File extensions this extractor handles (without leading dot).
    fn extensions(&self) -> &[&str];

    /// Human-readable language name.
    fn language_name(&self) -> &str;

    /// Extract nodes, edges, and unresolved refs from source code.
    ///
    /// `file_path` is the relative path used for qualified names and node IDs.
    /// `source` is the source code to parse.
    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult;
}

/// Registry of all available language extractors.
///
/// Dispatches to the correct extractor based on file extension.
pub struct LanguageRegistry {
    extractors: Vec<Box<dyn LanguageExtractor>>,
}

impl LanguageRegistry {
    /// Creates a new registry with all built-in language extractors.
    pub fn new() -> Self {
        Self {
            extractors: vec![
                Box::new(RustExtractor),
                Box::new(GoExtractor),
                Box::new(JavaExtractor),
            ],
        }
    }

    /// Returns the extractor for a file path based on its extension.
    pub fn extractor_for_file(&self, path: &str) -> Option<&dyn LanguageExtractor> {
        let ext = path.rsplit('.').next()?;
        self.extractors
            .iter()
            .find(|e| e.extensions().contains(&ext))
            .map(|e| e.as_ref())
    }

    /// Returns all supported file extensions across all extractors.
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.extractors
            .iter()
            .flat_map(|e| e.extensions().iter().copied())
            .collect()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 4: Create stub Go and Java extractors**

Create `src/extraction/go_extractor.rs` with a minimal stub:

```rust
/// Tree-sitter based Go source code extractor.
use crate::extraction::LanguageExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from Go source files.
pub struct GoExtractor;

impl LanguageExtractor for GoExtractor {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn language_name(&self) -> &str {
        "Go"
    }

    fn extract(&self, _file_path: &str, _source: &str) -> ExtractionResult {
        ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: vec!["Go extraction not yet implemented".to_string()],
            duration_ms: 0,
        }
    }
}
```

Create `src/extraction/java_extractor.rs` with a minimal stub:

```rust
/// Tree-sitter based Java source code extractor.
use crate::extraction::LanguageExtractor;
use crate::types::ExtractionResult;

/// Extracts code graph nodes and edges from Java source files.
pub struct JavaExtractor;

impl LanguageExtractor for JavaExtractor {
    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn language_name(&self) -> &str {
        "Java"
    }

    fn extract(&self, _file_path: &str, _source: &str) -> ExtractionResult {
        ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: vec!["Java extraction not yet implemented".to_string()],
            duration_ms: 0,
        }
    }
}
```

**Step 5: Implement LanguageExtractor for RustExtractor**

Add to bottom of `src/extraction/rust_extractor.rs`:

```rust
impl crate::extraction::LanguageExtractor for RustExtractor {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn language_name(&self) -> &str {
        "Rust"
    }

    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult {
        RustExtractor::extract(file_path, source)
    }
}
```

**Step 6: Run tests to verify they pass**

Run: `cargo test --test extraction_test`
Expected: PASS — all existing extraction tests still pass, plus new registry tests

**Step 7: Commit**

```bash
git add src/extraction/mod.rs src/extraction/rust_extractor.rs src/extraction/go_extractor.rs src/extraction/java_extractor.rs tests/extraction_test.rs
git commit -m "feat: add LanguageExtractor trait, LanguageRegistry, and stub extractors"
```

---

### Task 4: Integrate LanguageRegistry into CodeGraph

**Files:**
- Modify: `src/codegraph.rs`
- Modify: `src/config.rs`

**Step 1: Update config defaults**

In `src/config.rs`, change the `Default` impl:

Replace the `include` default:
```rust
include: vec!["**/*.rs".to_string()],
```
with:
```rust
include: vec![
    "**/*.rs".to_string(),
    "**/*.go".to_string(),
    "**/*.java".to_string(),
],
```

Add to the `exclude` default list:
```rust
    "bin/**".to_string(),
    "build/**".to_string(),
    "out/**".to_string(),
    ".gradle/**".to_string(),
```

**Step 2: Update CodeGraph to use LanguageRegistry**

In `src/codegraph.rs`:

1. Add import: `use crate::extraction::LanguageRegistry;`
2. Remove: `use crate::extraction::RustExtractor;`
3. Add `registry` field to the `CodeGraph` struct:
   ```rust
   pub struct CodeGraph {
       db: Database,
       config: CodeGraphConfig,
       project_root: PathBuf,
       registry: LanguageRegistry,
   }
   ```
4. Add `registry: LanguageRegistry::new()` to both `init()` and `open()` constructors.
5. In `index_all()`, replace line 146:
   ```rust
   let result = RustExtractor::extract(file_path, &source);
   ```
   with:
   ```rust
   let extractor = match self.registry.extractor_for_file(file_path) {
       Some(e) => e,
       None => continue,
   };
   let result = extractor.extract(file_path, &source);
   ```
6. In `sync()`, replace line 227:
   ```rust
   let result = RustExtractor::extract(file_path, &source);
   ```
   with:
   ```rust
   let extractor = match self.registry.extractor_for_file(file_path) {
       Some(e) => e,
       None => continue,
   };
   let result = extractor.extract(file_path, &source);
   ```

**Step 3: Verify compilation and tests**

Run: `cargo test`
Expected: PASS — all existing tests pass, Rust extraction behavior unchanged

**Step 4: Commit**

```bash
git add src/codegraph.rs src/config.rs
git commit -m "feat: integrate LanguageRegistry into CodeGraph for multi-language dispatch"
```

---

### Task 5: Implement Go extractor — types and package

**Files:**
- Modify: `src/extraction/go_extractor.rs`
- Test: `tests/go_extraction_test.rs` (new)

**Step 1: Write tests for Go package and struct extraction**

Create `tests/go_extraction_test.rs`:

```rust
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
    let pkgs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::GoPackage).collect();
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
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 2);
    let add_fn = fns.iter().find(|f| f.name == "Add").unwrap();
    assert_eq!(add_fn.visibility, Visibility::Pub); // uppercase = exported
    assert!(add_fn.docstring.as_ref().unwrap().contains("Add adds two numbers"));
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
    let structs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Struct).collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
    assert_eq!(structs[0].visibility, Visibility::Pub);
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
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
    let tags: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::StructTag).collect();
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
    let ifaces: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::InterfaceType).collect();
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
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::StructMethod).collect();
    assert_eq!(methods.len(), 2);
    // Check Receives edges
    let receives: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Receives).collect();
    assert!(!receives.is_empty(), "should have Receives edges for methods with receivers");
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
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
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
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MaxSize");
    let statics: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Static).collect();
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
    let call_refs: Vec<_> = result.unresolved_refs.iter()
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
    let aliases: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::TypeAlias).collect();
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
        || result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends);
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
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "Map");
    let generics: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::GenericParam).collect();
    assert!(generics.len() >= 2, "should extract generic type params T and U");
}

#[test]
fn test_go_file_node_is_root() {
    let source = r#"package main

func main() {}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("main.go", source);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
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
    let contains: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
    // File contains: GoPackage, Struct, StructMethod; Struct contains: Field
    assert!(contains.len() >= 4, "should have Contains edges: {:?}", contains.len());
}

#[test]
fn test_go_qualified_names() {
    let source = r#"package server

func HandleRequest() {}
"#;
    let extractor = GoExtractor;
    let result = extractor.extract("pkg/server/handler.go", source);
    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].qualified_name.contains("HandleRequest"));
    assert!(fns[0].qualified_name.contains("handler.go"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test go_extraction_test`
Expected: FAIL — Go extractor is a stub

**Step 3: Implement the full Go extractor**

Replace `src/extraction/go_extractor.rs` with the full implementation. The implementation follows the same `ExtractionState` pattern as `RustExtractor`:

- `parse_source()` uses `tree_sitter_go::LANGUAGE`
- `visit_node()` dispatches on tree-sitter Go node kinds:
  - `package_clause` → `GoPackage`
  - `function_declaration` → `Function`
  - `method_declaration` → `StructMethod` + `Receives` edge
  - `type_declaration` → dispatches on child type spec:
    - `struct_type` → `Struct` with `Field` children (with `StructTag`)
    - `interface_type` → `InterfaceType` with embedded interface `Extends` edges
    - type alias (has `=`) → `TypeAlias`
  - `import_declaration` → `Use` nodes (one per import spec)
  - `const_declaration` → `Const` nodes
  - `var_declaration` → `Static` nodes
- Visibility: first character uppercase → `Pub`, lowercase → `Private`
- Doc comments: collect `comment` nodes preceding declarations
- Signatures: text from start to `{`
- Call sites: scan for `call_expression` and `selector_expression` calls
- Generics: `type_parameter_list` → `GenericParam` nodes

**Step 4: Run tests to verify they pass**

Run: `cargo test --test go_extraction_test`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: PASS — no regressions

**Step 6: Commit**

```bash
git add src/extraction/go_extractor.rs tests/go_extraction_test.rs
git commit -m "feat: implement Go extractor with deep extraction support"
```

---

### Task 6: Implement Java extractor

**Files:**
- Modify: `src/extraction/java_extractor.rs`
- Test: `tests/java_extraction_test.rs` (new)

**Step 1: Write tests for Java extraction**

Create `tests/java_extraction_test.rs`:

```rust
use codegraph::extraction::JavaExtractor;
use codegraph::extraction::LanguageExtractor;
use codegraph::types::*;

#[test]
fn test_java_extract_package() {
    let source = r#"package com.example.app;

public class Main {
    public static void main(String[] args) {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("src/Main.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let pkgs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Package).collect();
    assert_eq!(pkgs.len(), 1);
    assert_eq!(pkgs[0].name, "com.example.app");
}

#[test]
fn test_java_extract_class() {
    let source = r#"package com.example;

/**
 * A simple calculator.
 */
public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Calculator.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Calculator");
    assert_eq!(classes[0].visibility, Visibility::Pub);
    assert!(classes[0].docstring.as_ref().unwrap().contains("simple calculator"));
}

#[test]
fn test_java_extract_methods() {
    let source = r#"
public class Foo {
    public void doSomething() {}
    private int compute(int x) { return x * 2; }
    protected String getName() { return "foo"; }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 3);
    let do_something = methods.iter().find(|m| m.name == "doSomething").unwrap();
    assert_eq!(do_something.visibility, Visibility::Pub);
    let compute = methods.iter().find(|m| m.name == "compute").unwrap();
    assert_eq!(compute.visibility, Visibility::Private);
    let get_name = methods.iter().find(|m| m.name == "getName").unwrap();
    assert_eq!(get_name.visibility, Visibility::PubCrate); // protected maps to PubCrate
}

#[test]
fn test_java_extract_constructor() {
    let source = r#"
public class Person {
    private String name;
    public Person(String name) {
        this.name = name;
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Person.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let constructors: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Constructor).collect();
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].name, "Person");
}

#[test]
fn test_java_extract_interface() {
    let source = r#"
public interface Drawable {
    void draw();
    double area();
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Drawable.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ifaces: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Interface).collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "Drawable");
    let methods: Vec<_> = result.nodes.iter()
        .filter(|n| n.kind == NodeKind::Method || n.kind == NodeKind::AbstractMethod)
        .collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_java_extract_enum() {
    let source = r#"
public enum Color {
    RED,
    GREEN,
    BLUE
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Color.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Enum).collect();
    assert_eq!(enums.len(), 1);
    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert_eq!(variants.len(), 3);
}

#[test]
fn test_java_extract_fields() {
    let source = r#"
public class Config {
    public static final int MAX_SIZE = 1024;
    private String name;
    protected int port;
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Config.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert_eq!(fields.len(), 3);
    let max_size = fields.iter().find(|f| f.name == "MAX_SIZE").unwrap();
    assert_eq!(max_size.visibility, Visibility::Pub);
}

#[test]
fn test_java_extract_imports() {
    let source = r#"
import java.util.List;
import java.util.Map;
import static java.lang.Math.PI;

public class Foo {}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(uses.len(), 3);
}

#[test]
fn test_java_extract_extends_implements() {
    let source = r#"
interface Runnable { void run(); }
class Base {}
class Worker extends Base implements Runnable {
    public void run() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Worker.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let has_extends = result.edges.iter().any(|e| e.kind == EdgeKind::Extends)
        || result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Extends);
    assert!(has_extends, "should detect extends");
    let has_implements = result.edges.iter().any(|e| e.kind == EdgeKind::Implements)
        || result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Implements);
    assert!(has_implements, "should detect implements");
}

#[test]
fn test_java_extract_annotations() {
    let source = r#"
import java.lang.Override;

public class Foo {
    @Override
    public String toString() {
        return "Foo";
    }

    @Deprecated
    public void oldMethod() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let annots: Vec<_> = result.nodes.iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert!(annots.len() >= 2, "should extract annotation usages");
    let has_annotates = result.edges.iter().any(|e| e.kind == EdgeKind::Annotates)
        || result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Annotates);
    assert!(has_annotates, "should have Annotates edges");
}

#[test]
fn test_java_extract_inner_class() {
    let source = r#"
public class Outer {
    public class Inner {
        public void innerMethod() {}
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Outer.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let inners: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::InnerClass).collect();
    assert_eq!(inners.len(), 1);
    assert_eq!(inners[0].name, "Inner");
}

#[test]
fn test_java_extract_static_init_block() {
    let source = r#"
public class Registry {
    private static Map<String, Object> cache;
    static {
        cache = new HashMap<>();
    }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Registry.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let init_blocks: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::InitBlock).collect();
    assert_eq!(init_blocks.len(), 1);
}

#[test]
fn test_java_extract_abstract_method() {
    let source = r#"
public abstract class Shape {
    public abstract double area();
    public void describe() { System.out.println("shape"); }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Shape.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let abstract_methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::AbstractMethod).collect();
    assert_eq!(abstract_methods.len(), 1);
    assert_eq!(abstract_methods[0].name, "area");
}

#[test]
fn test_java_extract_generics() {
    let source = r#"
public class Box<T> {
    private T value;
    public T getValue() { return value; }
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Box.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let generics: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::GenericParam).collect();
    assert!(generics.len() >= 1, "should extract generic type param T");
}

#[test]
fn test_java_extract_call_sites() {
    let source = r#"
public class App {
    public void run() {
        System.out.println("hello");
        helper();
        new ArrayList<>();
    }
    private void helper() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("App.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result.unresolved_refs.iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
}

#[test]
fn test_java_extract_annotation_type() {
    let source = r#"
public @interface MyAnnotation {
    String value();
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("MyAnnotation.java", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let annots: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Annotation).collect();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].name, "MyAnnotation");
}

#[test]
fn test_java_file_node_is_root() {
    let source = "public class Main {}";
    let extractor = JavaExtractor;
    let result = extractor.extract("src/Main.java", source);
    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "src/Main.java");
}

#[test]
fn test_java_contains_edges() {
    let source = r#"
public class Foo {
    private int x;
    public void bar() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("Foo.java", source);
    let contains: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
    // File contains: Class; Class contains: Field, Method
    assert!(contains.len() >= 3, "should have Contains edges: {}", contains.len());
}

#[test]
fn test_java_qualified_names() {
    let source = r#"
package com.example;

public class App {
    public void run() {}
}
"#;
    let extractor = JavaExtractor;
    let result = extractor.extract("src/App.java", source);
    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 1);
    assert!(methods[0].qualified_name.contains("App"));
    assert!(methods[0].qualified_name.contains("run"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test java_extraction_test`
Expected: FAIL — Java extractor is a stub

**Step 3: Implement the full Java extractor**

Replace `src/extraction/java_extractor.rs` with the full implementation. Same `ExtractionState` pattern:

- `parse_source()` uses `tree_sitter_java::LANGUAGE`
- `visit_node()` dispatches on tree-sitter Java node kinds:
  - `package_declaration` → `Package`
  - `class_declaration` → `Class` (or `InnerClass` when nested inside another class)
  - `interface_declaration` → `Interface`
  - `enum_declaration` → `Enum` + `EnumVariant` children
  - `annotation_type_declaration` → `Annotation`
  - `constructor_declaration` → `Constructor`
  - `method_declaration` → `Method` or `AbstractMethod` (detect `abstract` modifier)
  - `field_declaration` → `Field` (one per variable declarator)
  - `import_declaration` → `Use` (detect `static` keyword)
  - `static_initializer` → `InitBlock`
  - `marker_annotation` / `annotation` → `AnnotationUsage` + `Annotates` edge
- Visibility: scan for `modifiers` child → `public`=`Pub`, `protected`=`PubCrate`, `private`=`Private`, none=`Private`
- Doc comments: `block_comment` starting with `/**` preceding declarations
- Signatures: text from declaration start to `{`
- Call sites: `method_invocation`, `object_creation_expression`
- `extends`/`implements` from `superclass`/`interfaces` fields → `Extends`/`Implements` edges
- Generics: `type_parameters` → `GenericParam` nodes

**Step 4: Run tests to verify they pass**

Run: `cargo test --test java_extraction_test`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: PASS — no regressions

**Step 6: Commit**

```bash
git add src/extraction/java_extractor.rs tests/java_extraction_test.rs
git commit -m "feat: implement Java extractor with deep extraction support"
```

---

### Task 7: Update resolver for new callable kinds

**Files:**
- Modify: `src/resolution/resolver.rs`
- Test: `tests/resolution_test.rs`

**Step 1: Update the `find_best_match` scoring**

In `src/resolution/resolver.rs`, in `find_best_match()`, the callable kind bonus check (line ~206) currently only checks `Function` and `Method`. Add the new callable kinds:

Replace:
```rust
if uref.reference_kind == EdgeKind::Calls
    && (node.kind == NodeKind::Function || node.kind == NodeKind::Method)
{
    score += 25;
}
```

With:
```rust
if uref.reference_kind == EdgeKind::Calls
    && matches!(
        node.kind,
        NodeKind::Function
            | NodeKind::Method
            | NodeKind::StructMethod
            | NodeKind::Constructor
            | NodeKind::AbstractMethod
    )
{
    score += 25;
}
```

**Step 2: Run tests**

Run: `cargo test`
Expected: PASS — all tests pass including resolution tests

**Step 3: Commit**

```bash
git add src/resolution/resolver.rs
git commit -m "feat: update resolver scoring for Go/Java callable kinds"
```

---

### Task 8: Run clippy and final verification

**Step 1: Run clippy**

Run: `cargo clippy --all`
Expected: No warnings (fix any that appear)

**Step 2: Run fmt**

Run: `cargo fmt --all`

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore: clippy and fmt fixes"
```

---

## Summary

| Task | Description | Key Files |
|------|-------------|-----------|
| 1 | Add tree-sitter deps | `Cargo.toml` |
| 2 | Expand NodeKind/EdgeKind | `src/types.rs` |
| 3 | Create trait + registry + stubs | `src/extraction/mod.rs`, `*_extractor.rs` |
| 4 | Integrate registry into CodeGraph | `src/codegraph.rs`, `src/config.rs` |
| 5 | Implement Go extractor (full) | `src/extraction/go_extractor.rs` |
| 6 | Implement Java extractor (full) | `src/extraction/java_extractor.rs` |
| 7 | Update resolver scoring | `src/resolution/resolver.rs` |
| 8 | Clippy + fmt + final verification | All files |
