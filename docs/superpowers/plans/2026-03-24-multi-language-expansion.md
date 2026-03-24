# Multi-Language Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 8 new language extractors (TypeScript/JS, Python, C, C++, Kotlin, Dart, C#, Pascal) to codegraph, following the existing extractor pattern.

**Architecture:** Each language gets its own `*_extractor.rs` file implementing the `LanguageExtractor` trait. The file follows the same `ExtractionState` + AST visitor pattern used by the existing Go/Rust/Java/Scala extractors. New language-specific `NodeKind` variants are added to `types.rs`. Each extractor is registered in `LanguageRegistry::new()`.

**Tech Stack:** tree-sitter grammars (`tree-sitter-typescript`, `tree-sitter-javascript`, `tree-sitter-python`, `tree-sitter-c`, `tree-sitter-cpp`, `tree-sitter-kotlin`, `tree-sitter-dart`, `tree-sitter-c-sharp`, `tree-sitter-pascal`)

---

## File Structure

**New files (one per extractor + test):**
- `src/extraction/typescript_extractor.rs` — TS/JS/TSX/JSX extraction
- `src/extraction/python_extractor.rs` — Python extraction
- `src/extraction/c_extractor.rs` — C extraction
- `src/extraction/cpp_extractor.rs` — C++ extraction
- `src/extraction/kotlin_extractor.rs` — Kotlin extraction
- `src/extraction/dart_extractor.rs` — Dart extraction
- `src/extraction/csharp_extractor.rs` — C# extraction
- `src/extraction/pascal_extractor.rs` — Pascal extraction
- `tests/typescript_extraction_test.rs`
- `tests/python_extraction_test.rs`
- `tests/c_extraction_test.rs`
- `tests/cpp_extraction_test.rs`
- `tests/kotlin_extraction_test.rs`
- `tests/dart_extraction_test.rs`
- `tests/csharp_extraction_test.rs`
- `tests/pascal_extraction_test.rs`

**Modified files:**
- `Cargo.toml` — add tree-sitter grammar dependencies
- `src/types.rs` — add new `NodeKind` variants + `as_str`/`from_str` arms
- `src/extraction/mod.rs` — add module declarations, re-exports, registry entries
- `src/config.rs` — add exclude patterns for language-specific build dirs (e.g. `dist/**`, `__pycache__/**`, `*.pyc`)

## Shared Patterns

Every extractor follows this identical structure (reference: `src/extraction/go_extractor.rs`):

1. **Struct:** `pub struct XExtractor;`
2. **ExtractionState:** private struct with `nodes`, `edges`, `unresolved_refs`, `errors`, `node_stack`, `file_path`, `source`, `timestamp` (plus any language-specific fields like `class_depth`, `inside_trait`)
3. **Entry point:** `pub fn extract_x(file_path: &str, source: &str) -> ExtractionResult` — creates state, parses, creates File root node, walks AST, returns result
4. **parse_source:** creates `Parser`, sets language, parses
5. **visit_children / visit_node:** dispatches on `node.kind()` strings
6. **Visitor methods:** one per construct — creates `Node`, pushes to state, adds `Contains` edge from parent, extracts call sites from bodies
7. **Helpers:** `find_child_by_kind`, `extract_signature`, `extract_docstring`, `extract_call_sites`, `clean_comment`, `build_result`
8. **LanguageExtractor impl:** `extensions()`, `language_name()`, `extract()` delegating to the entry point

## What Each Extractor Should Extract

### TypeScript/JavaScript
- **Extensions:** `ts`, `tsx`, `js`, `jsx`
- **Language name:** `TypeScript` (covers JS as subset)
- **Grammar:** `tree-sitter-typescript` for TS/TSX, `tree-sitter-javascript` for JS/JSX. Use TS grammar for `.ts`/`.tsx`, JS grammar for `.js`/`.jsx`.
- **Constructs:** functions, arrow functions, classes, methods, interfaces, type aliases, enums, imports/exports, decorators, namespaces/modules, async functions, generators
- **NodeKind additions:** `Decorator`, `ArrowFunction`, `Export`, `Namespace`
- **Visibility:** `export` = Pub, otherwise Private
- **Docstring:** JSDoc (`/** ... */`) before declarations
- **Call sites:** `call_expression` nodes in function/method bodies

### Python
- **Extensions:** `py`
- **Language name:** `Python`
- **Grammar:** `tree-sitter-python`
- **Constructs:** functions, async functions, classes, methods, decorators, imports (import/from-import), module-level assignments (constants), lambda (skip — anonymous)
- **NodeKind additions:** `Decorator` (shared with TS)
- **Visibility:** `_name` = Private, `__name` = Private, else Pub
- **Docstring:** first string literal in function/class body (triple-quoted)
- **Call sites:** `call` nodes in function/method bodies

### C
- **Extensions:** `c`, `h`
- **Language name:** `C`
- **Grammar:** `tree-sitter-c`
- **Constructs:** functions, structs, unions, enums, enum constants, typedefs, macros (preproc_def), includes, global variables, fields
- **NodeKind additions:** `Union`, `Typedef`, `Include`, `PreprocessorDef`
- **Visibility:** `static` = Private, else Pub
- **Docstring:** `/* ... */` or `//` comments before declarations
- **Call sites:** `call_expression` nodes in function bodies

### C++
- **Extensions:** `cpp`, `hpp`, `cc`, `cxx`, `hh`
- **Language name:** `C++`
- **Grammar:** `tree-sitter-cpp`
- **Constructs:** everything from C plus: classes, methods, constructors, destructors, namespaces, templates, virtual/abstract methods, access specifiers (public/private/protected), operator overloads, using declarations
- **NodeKind additions:** `Namespace`, `Template` (plus reuses `Union`, `Typedef`, `Include`, `PreprocessorDef` from C)
- **Visibility:** access specifiers (`public:`, `private:`, `protected:`) determine visibility. Default: Private for classes, Pub for structs
- **Docstring:** `/* ... */`, `//`, `///` comments before declarations
- **Call sites:** `call_expression` nodes in function/method bodies

### Kotlin
- **Extensions:** `kt`, `kts`
- **Language name:** `Kotlin`
- **Grammar:** `tree-sitter-kotlin`
- **Constructs:** functions, classes, objects, interfaces, enums, enum entries, data classes, sealed classes, annotations, properties (val/var), constructors, companion objects, imports, packages, extension functions
- **NodeKind additions:** `DataClass`, `SealedClass`, `CompanionObject`, `KotlinObject`, `KotlinPackage`, `Property`
- **Visibility:** `public`/default = Pub, `private` = Private, `internal` = PubCrate, `protected` = PubSuper
- **Docstring:** KDoc (`/** ... */`) before declarations
- **Call sites:** `call_expression` nodes in function/method bodies

### Dart
- **Extensions:** `dart`
- **Language name:** `Dart`
- **Grammar:** `tree-sitter-dart`
- **Constructs:** functions, classes, mixins, enums, extensions, methods, constructors, fields, imports, libraries, typedefs, abstract classes
- **NodeKind additions:** `Mixin`, `Extension`, `Library`
- **Visibility:** `_name` = Private, else Pub (Dart convention)
- **Docstring:** `///` doc comments before declarations
- **Call sites:** `call_expression` or equivalent in function/method bodies

### C#
- **Extensions:** `cs`
- **Language name:** `C#`
- **Grammar:** `tree-sitter-c-sharp`
- **Constructs:** classes, structs, interfaces, enums, methods, constructors, properties, fields, namespaces, delegates, events, records, using directives, attributes
- **NodeKind additions:** `Delegate`, `Event`, `Record`, `CSharpProperty` (to distinguish from Kotlin `Property`)
- **Visibility:** `public` = Pub, `private` = Private, `internal` = PubCrate, `protected` = PubSuper
- **Docstring:** XML doc comments (`/// <summary>...`) before declarations
- **Call sites:** `invocation_expression` nodes in method bodies

### Pascal
- **Extensions:** `pas`, `pp`, `dpr`
- **Language name:** `Pascal`
- **Grammar:** `tree-sitter-pascal`
- **Constructs:** programs, units, functions, procedures, classes, records, interfaces, types, constants, variables, uses clauses, methods, constructors, destructors, properties
- **NodeKind additions:** `Procedure`, `PascalUnit`, `PascalProgram`, `PascalRecord`
- **Visibility:** `public` = Pub, `private` = Private, `protected` = PubSuper, default in implementation section = Private
- **Docstring:** `{ ... }` or `(* ... *)` comments before declarations
- **Call sites:** function/procedure call nodes in bodies

---

## Task 0: Dependencies and Type System Setup

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/types.rs`
- Modify: `src/extraction/mod.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Add tree-sitter grammar dependencies to Cargo.toml**

Add to `[dependencies]`:
```toml
tree-sitter-typescript = "0.23"
tree-sitter-javascript = "0.25"
tree-sitter-python = "0.25"
tree-sitter-c = "0.24"
tree-sitter-cpp = "0.23"
tree-sitter-kotlin = "0.3"
tree-sitter-dart = "0.1"
tree-sitter-c-sharp = "0.23"
tree-sitter-pascal = "0.10"
```

- [ ] **Step 2: Run `cargo check` to verify dependencies resolve**

Run: `cargo check`
Expected: compiles successfully (no usage yet, just deps)

- [ ] **Step 3: Add new NodeKind variants to `src/types.rs`**

Add these variants to the `NodeKind` enum, with corresponding `as_str()` and `from_str()` arms:

```rust
// TypeScript/JavaScript-specific
ArrowFunction,   // "arrow_function"
Decorator,       // "decorator" (shared: TS, Python, Kotlin)
Export,          // "export"
Namespace,       // "namespace" (shared: TS, C++)

// C/C++-specific
Union,           // "union"
Typedef,         // "typedef"
Include,         // "include"
PreprocessorDef, // "preprocessor_def"
Template,        // "template" (C++ only)

// Kotlin-specific
DataClass,       // "data_class"
SealedClass,     // "sealed_class"
CompanionObject, // "companion_object"
KotlinObject,    // "kotlin_object"
KotlinPackage,   // "kotlin_package"
Property,        // "property"

// Dart-specific
Mixin,           // "mixin"
Extension,       // "extension"
Library,         // "library"

// C#-specific
Delegate,        // "delegate"
Event,           // "event"
Record,          // "record"
CSharpProperty,  // "csharp_property"

// Pascal-specific
Procedure,       // "procedure"
PascalUnit,      // "pascal_unit"
PascalProgram,   // "pascal_program"
PascalRecord,    // "pascal_record"
```

- [ ] **Step 4: Add module stubs to `src/extraction/mod.rs`**

Add module declarations, pub use, and registry entries for all 8 new extractors (initially as empty files so the project compiles):

```rust
mod typescript_extractor;
mod python_extractor;
mod c_extractor;
mod cpp_extractor;
mod kotlin_extractor;
mod dart_extractor;
mod csharp_extractor;
mod pascal_extractor;

pub use typescript_extractor::TypeScriptExtractor;
pub use python_extractor::PythonExtractor;
pub use c_extractor::CExtractor;
pub use cpp_extractor::CppExtractor;
pub use kotlin_extractor::KotlinExtractor;
pub use dart_extractor::DartExtractor;
pub use csharp_extractor::CSharpExtractor;
pub use pascal_extractor::PascalExtractor;
```

Registry:
```rust
Box::new(TypeScriptExtractor),
Box::new(PythonExtractor),
Box::new(CExtractor),
Box::new(CppExtractor),
Box::new(KotlinExtractor),
Box::new(DartExtractor),
Box::new(CSharpExtractor),
Box::new(PascalExtractor),
```

- [ ] **Step 5: Create stub extractor files**

Create each of the 8 new `*_extractor.rs` files with a minimal struct + `LanguageExtractor` impl that returns an empty `ExtractionResult` with an error "X extraction not yet implemented". This lets the project compile.

- [ ] **Step 6: Add exclude patterns to `src/config.rs`**

Add to the default excludes:
```rust
"dist/**".to_string(),
"__pycache__/**".to_string(),
"*.pyc".to_string(),
".tox/**".to_string(),
"venv/**".to_string(),
".venv/**".to_string(),
```

- [ ] **Step 7: Update package description in Cargo.toml**

Update the `description` field to reflect all supported languages.

- [ ] **Step 8: Run `cargo check` and `cargo test`**

Run: `cargo check && cargo test`
Expected: compiles, all existing tests pass, new extractors return "not yet implemented" errors

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: scaffold 8 new language extractors with type system additions"
```

---

## Task 1: TypeScript/JavaScript Extractor

**Files:**
- Create: `src/extraction/typescript_extractor.rs`
- Create: `tests/typescript_extraction_test.rs`

This extractor handles `.ts`, `.tsx` (via `tree-sitter-typescript` TS/TSX grammars) and `.js`, `.jsx` (via `tree-sitter-javascript`).

- [ ] **Step 1: Write failing tests for basic constructs**

Create `tests/typescript_extraction_test.rs` with tests for:
- File node is root
- Function declaration extraction
- Arrow function extraction
- Class with methods
- Interface extraction
- Enum extraction
- Import/export extraction
- Async function detection
- Decorator extraction
- Namespace extraction
- JSDoc docstring extraction
- Call site tracking
- Visibility (export vs non-export)
- `.js` file uses JS grammar

Each test follows the pattern from `tests/go_extraction_test.rs` — create source string, call extractor, assert on nodes/edges.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test typescript`
Expected: all tests FAIL (stub returns empty result)

- [ ] **Step 3: Implement TypeScriptExtractor**

Replace the stub in `src/extraction/typescript_extractor.rs` with full implementation following the Go extractor pattern:
- `ExtractionState` with standard fields
- `parse_source` that picks grammar based on extension (TS for .ts/.tsx, JS for .js/.jsx)
- `visit_node` dispatching on: `function_declaration`, `arrow_function`, `class_declaration`, `method_definition`, `interface_declaration`, `enum_declaration`, `type_alias_declaration`, `import_statement`, `export_statement`, `lexical_declaration` (const/let), `decorator`
- Visitor methods for each construct
- `extract_call_sites` recursing into `call_expression`
- JSDoc extraction from `comment` nodes starting with `/**`
- Visibility: `export` keyword = Pub, otherwise Private

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test typescript`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/typescript_extractor.rs tests/typescript_extraction_test.rs
git commit -m "feat: add TypeScript/JavaScript language support"
```

---

## Task 2: Python Extractor

**Files:**
- Create: `src/extraction/python_extractor.rs`
- Create: `tests/python_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- File node is root
- Function declaration extraction
- Async function detection
- Class extraction
- Method extraction (inside class)
- Decorator extraction
- Import extraction (import X, from X import Y)
- Docstring extraction (triple-quoted first string in body)
- Visibility (`_private`, `__dunder__` public, `__mangled` private)
- Module-level constants (uppercase assignments)
- Call site tracking
- Nested class extraction

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test python`
Expected: all tests FAIL

- [ ] **Step 3: Implement PythonExtractor**

- `parse_source` using `tree_sitter_python::LANGUAGE`
- `visit_node` dispatching on: `function_definition`, `class_definition`, `decorated_definition`, `import_statement`, `import_from_statement`, `assignment` (for module constants), `expression_statement` (for docstrings)
- Python docstring: first `expression_statement` containing a `string` node in a function/class body
- Python visibility: names starting with `_` are private (except `__x__` dunder which are public)
- `is_async`: check if `async` keyword precedes function definition

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test python`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/python_extractor.rs tests/python_extraction_test.rs
git commit -m "feat: add Python language support"
```

---

## Task 3: C Extractor

**Files:**
- Create: `src/extraction/c_extractor.rs`
- Create: `tests/c_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- File node is root
- Function declaration/definition extraction
- Struct with fields
- Union extraction
- Enum with constants
- Typedef extraction
- Preprocessor macro (#define)
- #include extraction
- Global variable extraction
- Static function = Private visibility
- Docstring extraction (C-style comments)
- Call site tracking
- Function pointer typedefs

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test c_extract`
Expected: all tests FAIL

- [ ] **Step 3: Implement CExtractor**

- `parse_source` using `tree_sitter_c::LANGUAGE`
- `visit_node` dispatching on: `function_definition`, `declaration`, `struct_specifier`, `union_specifier`, `enum_specifier`, `type_definition`, `preproc_def`, `preproc_include`
- Visibility: `static` storage class = Private, else Pub
- Handle `.h` files (declarations without bodies)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test c_extract`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/c_extractor.rs tests/c_extraction_test.rs
git commit -m "feat: add C language support"
```

---

## Task 4: C++ Extractor

**Files:**
- Create: `src/extraction/cpp_extractor.rs`
- Create: `tests/cpp_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- All C constructs (function, struct, enum, union, typedef, macro, include)
- Class with methods and fields
- Constructor and destructor
- Namespace extraction
- Template extraction
- Virtual/abstract methods
- Access specifiers (public/private/protected)
- Inheritance (extends edge)
- Using declaration
- Operator overloads
- Docstring extraction
- Call site tracking

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test cpp_extract`
Expected: all tests FAIL

- [ ] **Step 3: Implement CppExtractor**

- `parse_source` using `tree_sitter_cpp::LANGUAGE`
- `ExtractionState` with `access_specifier` field (tracks current visibility in class body)
- `visit_node` dispatching on everything from C plus: `class_specifier`, `namespace_definition`, `template_declaration`, `using_declaration`, `access_specifier`, `constructor_definition`, `destructor_definition`
- Inheritance: `base_class_clause` → `Extends` edge

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test cpp_extract`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/cpp_extractor.rs tests/cpp_extraction_test.rs
git commit -m "feat: add C++ language support"
```

---

## Task 5: Kotlin Extractor

**Files:**
- Create: `src/extraction/kotlin_extractor.rs`
- Create: `tests/kotlin_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- File node is root
- Package extraction
- Import extraction
- Function extraction
- Class extraction
- Data class extraction
- Sealed class extraction
- Object declaration
- Companion object
- Interface extraction
- Enum class with entries
- Property (val/var)
- Constructor extraction
- Annotation extraction
- Extension function
- KDoc docstring extraction
- Visibility modifiers (public/private/internal/protected)
- Call site tracking

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test kotlin`
Expected: all tests FAIL

- [ ] **Step 3: Implement KotlinExtractor**

- `parse_source` using `tree_sitter_kotlin::LANGUAGE` (note: may use `tree_sitter_kotlin::language()` depending on crate version)
- `visit_node` dispatching on: `function_declaration`, `class_declaration`, `object_declaration`, `companion_object`, `interface_declaration`, `enum_class_body`, `property_declaration`, `import_header`, `package_header`, `secondary_constructor`, `annotation`
- Data class / sealed class: check modifiers on `class_declaration`
- Visibility from modifier list: `public`/default = Pub, `private` = Private, `internal` = PubCrate, `protected` = PubSuper

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test kotlin`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/kotlin_extractor.rs tests/kotlin_extraction_test.rs
git commit -m "feat: add Kotlin language support"
```

---

## Task 6: Dart Extractor

**Files:**
- Create: `src/extraction/dart_extractor.rs`
- Create: `tests/dart_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- File node is root
- Library declaration
- Import extraction
- Function extraction
- Class extraction
- Abstract class
- Mixin extraction
- Extension extraction
- Enum extraction
- Method extraction
- Constructor extraction
- Field extraction
- Typedef extraction
- Dart doc comment (`///`) extraction
- Visibility (`_name` = private)
- Async function detection
- Call site tracking

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test dart`
Expected: all tests FAIL

- [ ] **Step 3: Implement DartExtractor**

- `parse_source` using `tree_sitter_dart::language()` (check crate API — 0.1.0 may use older API)
- `visit_node` dispatching on: `function_signature`, `class_definition`, `mixin_declaration`, `extension_declaration`, `enum_declaration`, `method_signature`, `import_or_export`, `library_name`, `type_alias`
- Visibility: names starting with `_` are private
- Note: tree-sitter-dart 0.1.0 may have limited node types. If the grammar is too old, we may need to use `tree-sitter-dart` from git or fall back to basic extraction.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test dart`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/dart_extractor.rs tests/dart_extraction_test.rs
git commit -m "feat: add Dart language support"
```

---

## Task 7: C# Extractor

**Files:**
- Create: `src/extraction/csharp_extractor.rs`
- Create: `tests/csharp_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- File node is root
- Namespace extraction
- Using directive extraction
- Class extraction
- Struct extraction
- Interface extraction
- Enum extraction
- Method extraction
- Constructor extraction
- Property extraction
- Field extraction
- Record extraction
- Delegate extraction
- Event extraction
- Attribute (annotation) extraction
- Inheritance (extends/implements edges)
- Visibility modifiers (public/private/internal/protected)
- XML doc comment extraction
- Call site tracking
- Async method detection

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test csharp`
Expected: all tests FAIL

- [ ] **Step 3: Implement CSharpExtractor**

- `parse_source` using `tree_sitter_c_sharp::LANGUAGE`
- `ExtractionState` with `class_depth` field
- `visit_node` dispatching on: `namespace_declaration`, `using_directive`, `class_declaration`, `struct_declaration`, `interface_declaration`, `enum_declaration`, `method_declaration`, `constructor_declaration`, `property_declaration`, `field_declaration`, `record_declaration`, `delegate_declaration`, `event_declaration`, `attribute_list`
- Visibility from modifiers: `public` = Pub, `private` = Private, `internal` = PubCrate, `protected` = PubSuper
- Inheritance: `base_list` → `Extends`/`Implements` edges

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test csharp`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/csharp_extractor.rs tests/csharp_extraction_test.rs
git commit -m "feat: add C# language support"
```

---

## Task 8: Pascal Extractor

**Files:**
- Create: `src/extraction/pascal_extractor.rs`
- Create: `tests/pascal_extraction_test.rs`

- [ ] **Step 1: Write failing tests**

Tests for:
- File node is root
- Program declaration
- Unit declaration
- Uses clause extraction
- Function extraction
- Procedure extraction
- Class/object type extraction
- Record type extraction
- Interface extraction
- Method extraction
- Constructor/destructor extraction
- Type declaration extraction
- Constant declaration extraction
- Variable declaration extraction
- Property extraction
- Visibility (public/private/protected sections)
- Comment extraction (`{ }`, `(* *)`, `//`)
- Call site tracking

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test pascal`
Expected: all tests FAIL

- [ ] **Step 3: Implement PascalExtractor**

- `parse_source` using `tree_sitter_pascal::language()`
- `visit_node` dispatching on: `program`, `unit`, `function_declaration`, `procedure_declaration`, `class_declaration`, `record_type`, `interface_declaration`, `type_section`, `const_section`, `var_section`, `uses_clause`, `constructor_declaration`, `destructor_declaration`, `property_declaration`
- Visibility: `public`, `private`, `protected` sections in class body
- Pascal is case-insensitive — normalize names for matching but preserve original casing

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test pascal`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/extraction/pascal_extractor.rs tests/pascal_extraction_test.rs
git commit -m "feat: add Pascal language support"
```

---

## Task 9: Documentation and Changelog

**Files:**
- Create: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `Cargo.toml` (version bump)

- [ ] **Step 1: Create CHANGELOG.md**

Create `CHANGELOG.md` following [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-03-24

### Added
- TypeScript/JavaScript language support (.ts, .tsx, .js, .jsx)
- Python language support (.py)
- C language support (.c, .h)
- C++ language support (.cpp, .hpp, .cc, .cxx, .hh)
- Kotlin language support (.kt, .kts)
- Dart language support (.dart)
- C# language support (.cs)
- Pascal language support (.pas, .pp, .dpr)
- CHANGELOG.md for tracking version history

## [0.6.0]

### Added
- Scala language support (.scala, .sc)

## [0.5.0]

### Added
- Java language support (.java)
- Go language support (.go)

## [0.4.0]

### Added
- Initial Rust language support (.rs)
- libsql-backed knowledge graph with FTS5 search
- MCP server (JSON-RPC 2.0 over stdio)
- Graph traversal: callers, callees, impact radius
- Incremental sync for fast re-indexing
- Vector embeddings for semantic search
```

Backfill prior versions as accurately as possible from git history. The exact entries for older versions are best-effort.

- [ ] **Step 2: Update README.md**

Update the following sections in `README.md`:

1. **Features section** — update the tree-sitter line:
   - From: `Tree-sitter AST parsing for Rust, Go, and Java`
   - To: `Tree-sitter AST parsing for Rust, Go, Java, Scala, TypeScript/JavaScript, Python, C, C++, Kotlin, Dart, C#, and Pascal`

2. **Add a "Supported Languages" section** after Features with a table:

```markdown
## Supported Languages

| Language | Extensions | Since |
|----------|-----------|-------|
| Rust | `.rs` | 0.4.0 |
| Go | `.go` | 0.5.0 |
| Java | `.java` | 0.5.0 |
| Scala | `.scala`, `.sc` | 0.6.0 |
| TypeScript | `.ts`, `.tsx` | 0.7.0 |
| JavaScript | `.js`, `.jsx` | 0.7.0 |
| Python | `.py` | 0.7.0 |
| C | `.c`, `.h` | 0.7.0 |
| C++ | `.cpp`, `.hpp`, `.cc`, `.cxx`, `.hh` | 0.7.0 |
| Kotlin | `.kt`, `.kts` | 0.7.0 |
| Dart | `.dart` | 0.7.0 |
| C# | `.cs` | 0.7.0 |
| Pascal | `.pas`, `.pp`, `.dpr` | 0.7.0 |
```

- [ ] **Step 3: Bump version in Cargo.toml**

Update `version = "0.6.0"` to `version = "0.7.0"` in `Cargo.toml`.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md README.md Cargo.toml
git commit -m "docs: add CHANGELOG.md, update README with all supported languages, bump to 0.7.0"
```

---

## Task 10: Integration Verification

**Files:**
- Modify: `tests/extraction_test.rs` (if needed)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass, including existing Rust/Go/Java/Scala tests

- [ ] **Step 2: Verify `LanguageRegistry` returns all languages**

Add a test (or run manually) that `LanguageRegistry::new().supported_extensions()` includes all expected extensions: `rs`, `go`, `java`, `scala`, `sc`, `ts`, `tsx`, `js`, `jsx`, `py`, `c`, `h`, `cpp`, `hpp`, `cc`, `cxx`, `hh`, `kt`, `kts`, `dart`, `cs`, `pas`, `pp`, `dpr`

- [ ] **Step 3: Test with real files from each language**

Run `codegraph index` against small sample projects/files for each language to verify end-to-end indexing works.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore: integration verification and clippy fixes"
```

---

## Notes

- **tree-sitter-dart 0.1.0** may be incompatible with tree-sitter 0.26. If it doesn't compile, check if there's a newer version on GitHub or use a git dependency. Worst case, skip Dart and add it later when the grammar catches up.
- **tree-sitter-kotlin 0.3.8** uses an older API. May need `tree_sitter_kotlin::language()` function instead of `LANGUAGE` constant.
- **tree-sitter-pascal 0.10.2** — verify the node type names match what tree-sitter-pascal actually emits by checking its `node-types.json`.
- For each extractor, use `node.kind()` strings from the grammar's `node-types.json` to get the exact AST node type names. Run `tree-sitter parse` on sample files to discover the actual node types if the grammar docs are sparse.
- The `.h` extension is shared between C and C++. The C extractor claims it. If users need C++ headers, they should use `.hpp`/`.hh`. This is a reasonable default.
