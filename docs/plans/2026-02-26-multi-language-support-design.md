# Multi-Language Support Design: Go & Java

**Date:** 2026-02-26
**Status:** Approved

## Overview

Add Go and Java language support to codegraph, which currently only supports Rust. This requires:
1. An abstraction layer (trait + registry) to make language support pluggable
2. Expanded NodeKind/EdgeKind enums for language-specific constructs
3. Two new extractors: Go and Java (deep extraction)
4. Config and integration changes

## 1. Extractor Trait & Language Registry

### Trait Definition (`src/extraction/mod.rs`)

```rust
pub trait LanguageExtractor: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn language_name(&self) -> &str;
    fn extract(&self, file_path: &str, source: &str) -> Result<ExtractionResult>;
}
```

### Language Registry

```rust
pub struct LanguageRegistry {
    extractors: Vec<Box<dyn LanguageExtractor>>,
}

impl LanguageRegistry {
    pub fn new() -> Self { /* register all extractors */ }
    pub fn extractor_for_file(&self, path: &str) -> Option<&dyn LanguageExtractor>;
    pub fn supported_extensions(&self) -> Vec<&str>;
}
```

- `CodeGraph` owns a `LanguageRegistry`
- `scan_files()` uses `registry.supported_extensions()` for include patterns
- `index_all()`/`sync()` call `registry.extractor_for_file(path)`
- `RustExtractor` implements `LanguageExtractor`

## 2. Expanded NodeKind Enum

### New Java variants:
- `Class` — class declarations
- `Interface` — interface declarations
- `Constructor` — constructor methods
- `Annotation` — annotation types (`@interface`)
- `AnnotationUsage` — annotation applications (`@Override`)
- `Package` — package declarations
- `InnerClass` — nested/inner classes
- `InitBlock` — static/instance initializer blocks
- `AbstractMethod` — abstract method declarations

### New Go variants:
- `InterfaceType` — Go interface type definitions
- `StructMethod` — methods with receivers
- `GoPackage` — Go package declaration
- `StructTag` — struct field tags

### Shared:
- `GenericParam` — type parameters

### New EdgeKind variants:
- `Extends` — Java class inheritance, Go interface embedding
- `Annotates` — annotation → target
- `Receives` — Go method receiver type link

## 3. Go Extractor (`src/extraction/go_extractor.rs`)

Uses `tree-sitter-go`.

### Declarations:
- `package_clause` → `GoPackage`
- `type_declaration` → `struct_type` → `Struct` + `Field` (with `StructTag`)
- `type_declaration` → `interface_type` → `InterfaceType` + method specs
- `function_declaration` → `Function`
- `method_declaration` → `StructMethod` (with `Receives` edge to receiver type)
- `const_declaration` / `var_declaration` → `Const` / `Static`
- `type_alias` → `TypeAlias`
- `import_declaration` → `Use` nodes

### Edges:
- `Contains` — package → types → methods/fields
- `Calls` — scan bodies for `call_expression`, `selector_expression`
- `Receives` — method → receiver type
- `Uses` — import references
- `Extends` — interface embedding

### Deep features:
- Generic type params → `GenericParam` nodes
- Doc comments (`//` preceding declarations)
- Visibility: uppercase = `Pub`, lowercase = `Private`
- Signatures: full function signature text
- Init functions: `func init()` detected

## 4. Java Extractor (`src/extraction/java_extractor.rs`)

Uses `tree-sitter-java`.

### Declarations:
- `package_declaration` → `Package`
- `class_declaration` → `Class` or `InnerClass` (when nested)
- `interface_declaration` → `Interface`
- `enum_declaration` → `Enum` + `EnumVariant`
- `annotation_type_declaration` → `Annotation`
- `constructor_declaration` → `Constructor`
- `method_declaration` → `Method` or `AbstractMethod`
- `field_declaration` → `Field`
- `import_declaration` → `Use` (static imports flagged)
- `static_initializer` / `instance_initializer` → `InitBlock`

### Edges:
- `Contains` — package → class → method/field, class → inner class
- `Calls` — scan bodies for `method_invocation`, `object_creation_expression`
- `Implements` — from `implements` clause
- `Extends` — from `extends` clause
- `Annotates` — `AnnotationUsage` → annotated element
- `Uses` — import references

### Deep features:
- `GenericParam` — type parameters on classes/methods
- Annotations: `marker_annotation`, `annotation` → `AnnotationUsage`
- Visibility: `public` → `Pub`, `protected` → `PubCrate`, `private` → `Private`
- Doc comments: Javadoc `/** */`
- Signatures: method/constructor signature text
- Modifiers: `static`, `final`, `abstract`, `synchronized` captured in signature

## 5. Integration & Config Changes

### Config (`config.rs`):
- Default `include`: `["**/*.rs", "**/*.go", "**/*.java"]`
- Default `exclude` adds: `["vendor/**", "bin/**", "build/**", "out/**", ".gradle/**"]`

### `codegraph.rs`:
- `CodeGraph::new()` creates `LanguageRegistry`
- `scan_files()` uses registry extensions
- `index_all()`/`sync()` use `registry.extractor_for_file()`

### `Cargo.toml`:
- Add `tree-sitter-go` and `tree-sitter-java` dependencies

### Resolver:
- Mostly language-agnostic already
- Qualified name separator stays `::` across all languages

### DB Schema:
- No changes needed — NodeKind/EdgeKind stored as strings
