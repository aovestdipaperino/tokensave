# Future Language Support

## Currently Supported (50+ languages)

| Tier | Languages |
|------|-----------|
| **Lite** (always compiled) | Rust, Go, Java, Scala, TypeScript/JavaScript/TSX/JSX, Python, C, C++, Kotlin, C#, Swift, Svelte, Astro |
| **Medium** (feature flags) | Dart, Pascal, PHP, Ruby, Bash, Protobuf, PowerShell, Nix, VB.NET |
| **Full** (feature flags) | ActionScript, Lua, Zig, Objective-C, Perl, Batch, Fortran, COBOL, MSBASIC2, GW-BASIC, QBasic, QuickBASIC, Dockerfile, GLSL, WGSL, HLSL, Metal, Markdown, R, SQL, Julia, Haskell, OCaml, Clojure, Erlang, Elixir, F#, F*, Quint, TOML, Lean |

## How to add a language

Each language needs 4 things:

| # | What | Where | Pattern to follow |
|---|------|-------|-------------------|
| 1 | Tree-sitter grammar | `tokensave-large-treesitters` crate on crates.io | Add dep + register in `all_languages()` |
| 2 | Extractor | `src/extraction/{lang}_extractor.rs` (~400-700 lines) | Implement `LanguageExtractor` trait |
| 3 | Wiring | `Cargo.toml` + `src/extraction/mod.rs` | Feature flag, `mod` decl, registry push |
| 4 | Tests | `tests/fixtures/sample.{ext}` + `tests/{lang}_extraction_test.rs` | Sample file + extraction assertions |

### The `LanguageExtractor` trait

```rust
pub trait LanguageExtractor: Send + Sync {
    fn extensions(&self) -> &[&str];        // e.g. &["svelte"]
    fn language_name(&self) -> &str;        // e.g. "Svelte"
    fn extract(&self, file_path: &str, source: &str) -> ExtractionResult;
}
```

### Grammar sourcing

- **Crate on crates.io:** Add as a dependency to `tokensave-large-treesitters` and register in `all_languages()`. This is the standard path.
- **Vendor from C source:** If no Rust crate exists, compile the grammar's C source via `build.rs` (same pattern as `protobuf` and `cobol` in the bundled crate).
- **No grammar at all:** Either write a regex-based extractor (skip tree-sitter) or wait for a community grammar.

---

## Proposed languages by tier

> Many earlier proposals have since shipped (Svelte, Astro, Elixir, Haskell, OCaml, Erlang, R, Julia, Clojure, Dockerfile, SQL, TOML, Markdown, WGSL — plus HLSL, Metal, F#, F*, Quint, Lean, and ActionScript which were never listed here). The tables below list what remains open.

Languages are tiered by a combination of: popularity (TIOBE, Stack Overflow, GitHub usage), relevance to tokensave's target users (professional developers using AI coding tools), and implementation complexity.

### High Priority — Web Frameworks

These produce code graphs that are structurally rich and heavily used in
AI-assisted development. They also tend to generate high tool-call counts
in exploration agents because of their component/template structure.

| Language | Extensions | Grammar crate | Complexity | Notes |
|----------|-----------|---------------|------------|-------|
| **Vue** | `.vue` | `tree-sitter-vue3` (0.0.4) | Medium-high | Same embedded-language challenge as Svelte: `<script>`, `<template>`, `<style>` blocks. Delegate `<script setup>` to TS extractor. Extract: components, props, emits, composables. |

**Shared challenge:** All three are "embedded language" formats. The tree-sitter
grammar gives you the document structure, but the `<script>` content needs the
TypeScript extractor. Consider building a shared `EmbeddedScriptExtractor`
helper that parses the script block with the existing `TypeScriptExtractor` and
merges the results.

### High Priority — General Purpose

Popular languages with clear structural semantics and active tree-sitter grammars.

| Language | Extensions | Grammar crate | Complexity | Notes |
|----------|-----------|---------------|------------|-------|

### Medium Priority — Infrastructure & Config

Languages used in CI/CD, infrastructure-as-code, and build systems. Lower
structural complexity but high value for DevOps-focused users.

| Language | Extensions | Grammar crate | Complexity | Notes |
|----------|-----------|---------------|------------|-------|
| **HCL/Terraform** | `.tf`, `.hcl` | `tree-sitter-hcl` (1.1.0) | Low-medium | Resources, data sources, modules, variables, outputs, locals. Graph edges from module refs and resource dependencies. |
| **Makefile** | `Makefile`, `*.mk` | `tree-sitter-make` (1.1.1) | Low | Targets, dependencies, variables. Target→dependency edges. |
| **CMake** | `CMakeLists.txt`, `*.cmake` | `tree-sitter-cmake` (0.7.1) | Low | Functions, macros, targets, `add_subdirectory`. |
| **GraphQL** | `.graphql`, `.gql` | `tree-sitter-graphql` (0.1.0) | Low | Types, queries, mutations, subscriptions, fragments. Clean schema graph. |

### Medium Priority — Emerging / Niche

Languages with growing communities or specific ecosystem value.

| Language | Extensions | Grammar crate | Complexity | Notes |
|----------|-----------|---------------|------------|-------|
| **Gleam** | `.gleam` | `tree-sitter-gleam` (1.0.0) | Low-medium | Functions, types, imports, externals. Clean syntax, good graph potential. BEAM ecosystem. |
| **Odin** | `.odin` | `tree-sitter-odin` (1.3.0) | Medium | Procedures, structs, enums, imports, packages. Systems programming language gaining traction. |
| **GDScript** | `.gd` | `tree-sitter-gdscript` (6.1.0) | Medium | Classes, functions, signals, exports, `extends`. Godot engine scripting — large gamedev community. |
| **Solidity** | `.sol` | `tree-sitter-solidity` (1.2.13) | Medium | Contracts, functions, events, modifiers, inheritance. Contract→contract `extends` edges. Web3 niche but high demand. |
| **Elm** | `.elm` | `tree-sitter-elm` (5.9.0) | Low-medium | Modules, functions, type aliases, custom types, ports, imports. Clean ML-like syntax. |
| **Groovy** | `.groovy`, `.gradle` | `tree-sitter-groovy` (0.1.2) | Medium | Classes, methods, closures. `.gradle` files are Groovy — useful for build graph analysis. |
| **Nim** | `.nim` | No direct crate | Medium | Procs, types, templates, macros, imports. Would need vendored grammar. |

### Low Priority — Template & Markup

Limited structural graph value but sometimes requested.

| Language | Extensions | Grammar crate | Complexity | Notes |
|----------|-----------|---------------|------------|-------|
| **Liquid** | `.liquid` | No crate (vendor from GitHub) | Low-medium | Blocks, includes, assigns, filters. Template language — limited function-level structure. Would need vendored C grammar from [tree-sitter-liquid](https://github.com/nicklockwood/tree-sitter-liquid). |
| **YAML** | `.yml`, `.yaml` | `tree-sitter-yaml` (0.7.2) | Low | Keys, anchors, aliases. Minimal graph value but useful for config file parsing. |
| **CSS/SCSS** | `.css`, `.scss` | `tree-sitter-css` (0.25.0) | Low | Selectors, rules, variables, mixins (SCSS). Limited graph edges. |
| **HTML** | `.html` | `tree-sitter-html` (0.23.2) | Low | Elements, attributes, component references. Mostly useful as an inner parser for Svelte/Vue/Astro. |

### Shader Languages

Very niche but occasionally requested by game/graphics developers.

| Language | Extensions | Grammar crate | Complexity | Notes |
|----------|-----------|---------------|------------|-------|
| ~~**GLSL**~~ | | | | **Implemented** — see Full tier above. |

---

## Implementation notes

### Embedded-language extractors (Svelte, Vue, Astro)

These share a common pattern: the file is a document with embedded script
blocks. The recommended approach:

1. Parse the document with the component grammar (e.g. `tree-sitter-svelte-ng`)
2. Find the `<script>` node and extract its text content
3. Feed that text to the existing `TypeScriptExtractor::extract()`
4. Merge the resulting nodes/edges, adjusting line offsets to account for
   the script block's position in the document
5. Add a top-level component node and edges from script symbols to it

Consider a shared helper:

```rust
fn extract_embedded_script(
    doc_source: &str,
    script_start_line: u32,
    script_text: &str,
    file_path: &str,
) -> ExtractionResult {
    // Delegate to TypeScriptExtractor, then offset all line numbers
    let mut result = TypeScriptExtractor.extract(file_path, script_text);
    for node in &mut result.nodes {
        node.start_line += script_start_line;
        node.end_line += script_start_line;
    }
    result
}
```

### Functional languages (Elixir, Haskell, OCaml, Clojure, Elm)

These need special handling for:
- **Pattern matching:** Multiple function clauses with the same name should
  merge into one node (not N separate nodes)
- **Type classes / protocols / behaviours:** Map to `Trait` + `Implements` edges
- **Pipe operators:** `|>` chains in Elixir, `$` in Haskell — hard to extract
  call edges from without type information
- **Modules as values:** OCaml functors, Elixir `use` macro — generate
  `uses` edges, not `calls`

### Grammar maturity

Some grammar crates are pre-1.0 or have very low version numbers:
- `tree-sitter-sql` (0.0.2) — likely incomplete
- `tree-sitter-clojure` (0.1.0) — may lack edge cases
- `tree-sitter-groovy` (0.1.2) — early stage
- `tree-sitter-liquid` — no crate at all

Test grammars against real-world files before committing to an extractor.
A grammar that can't parse common patterns produces nodes with wrong line
numbers, which breaks the entire graph for that file.
