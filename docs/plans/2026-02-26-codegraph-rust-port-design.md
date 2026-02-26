# CodeGraph Rust Port — Design Document

**Date:** 2026-02-26
**Goal:** Replace the TypeScript CodeGraph implementation with a Rust-native version
**Status:** Design approved

## Motivation

The Rust version will become the canonical CodeGraph implementation, replacing the TypeScript version entirely. Benefits include single-binary distribution (no Node.js dependency), better performance for large codebases, and lower memory usage.

## Scope

### In Scope
- Tree-sitter AST extraction (Rust language only)
- SQLite graph database with FTS5 full-text search
- Graph queries (callers, callees, impact radius, call graph, dead code, type hierarchy)
- Vector embeddings via `ort` (ONNX Runtime) for semantic search
- MCP server for Claude Code integration (stdio transport)
- CLI interface

### Out of Scope
- Multi-language support (Rust only for now — can be added later)
- Framework-specific resolvers (React, Express, Laravel, etc.)
- Interactive installer (unnecessary for a single binary)

## Architecture

Single crate, module-based structure:

```
code-graph/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library root
│   ├── config.rs            # Configuration
│   ├── types.rs             # Core types (Node, Edge, etc.)
│   ├── db/                  # SQLite layer
│   │   ├── mod.rs
│   │   ├── connection.rs
│   │   ├── queries.rs
│   │   └── schema.sql
│   ├── extraction/          # Tree-sitter parsing
│   │   ├── mod.rs
│   │   └── rust.rs          # Rust-specific extraction
│   ├── resolution/          # Reference resolution
│   │   ├── mod.rs
│   │   ├── imports.rs
│   │   └── names.rs
│   ├── graph/               # Graph traversal & queries
│   │   ├── mod.rs
│   │   ├── traversal.rs
│   │   └── queries.rs
│   ├── vectors/             # Embeddings
│   │   ├── mod.rs
│   │   ├── embedder.rs
│   │   └── search.rs
│   ├── context/             # Context building
│   │   ├── mod.rs
│   │   └── formatter.rs
│   ├── sync.rs              # Incremental updates
│   └── mcp/                 # MCP server
│       ├── mod.rs
│       ├── server.rs
│       └── tools.rs
```

## Core Types

### Node Kinds

```rust
enum NodeKind {
    File,
    Module,
    Struct,
    Enum,
    EnumVariant,
    Trait,
    Function,
    Method,
    Impl,
    Const,
    Static,
    TypeAlias,
    Field,
    Macro,
    Use,
}
```

### Node

```rust
struct Node {
    id: String,              // deterministic: "file_path::symbol_path"
    kind: NodeKind,
    name: String,
    file_path: String,
    start_line: u32,
    end_line: u32,
    signature: Option<String>,
    docstring: Option<String>,
    visibility: Visibility,  // Pub, PubCrate, Private
    body_hash: Option<String>,
}
```

### Edge Kinds

```rust
enum EdgeKind {
    Contains,       // file/module contains items
    Calls,          // function calls function
    Uses,           // references a symbol (use statement)
    Implements,     // impl Trait for Struct
    TypeOf,         // field/variable type references
    Returns,        // function return type
    DerivesMacro,   // #[derive(Debug, Clone)]
}
```

### Edge

```rust
struct Edge {
    source_id: String,
    target_id: String,
    kind: EdgeKind,
    line: Option<u32>,
}
```

## SQLite Schema

Tables:
- `nodes` — all extracted symbols with metadata
- `edges` — relationships between nodes
- `files` — tracked files with content hashes (for incremental sync)
- `nodes_fts` — FTS5 virtual table on node names and signatures
- `vectors` — embeddings stored as BLOBs with node_id foreign key

## Extraction Pipeline

Uses `tree-sitter` + `tree-sitter-rust` (native bindings, not WASM).

### What We Extract

| Rust Construct | Node Kind | Edges Emitted |
|---|---|---|
| `fn foo()` | Function | Contains (from parent), Calls (to callees), Returns |
| `struct Foo` | Struct | Contains (fields) |
| `enum Bar` | Enum | Contains (variants) |
| `impl Trait for S` | Impl | Implements (trait → struct) |
| `impl S` | Impl | Contains (methods) |
| `use crate::x` | Use | Uses (resolved target) |
| `#[derive(..)]` | — | DerivesMacro edges |
| `mod foo` | Module | Contains |
| `const`/`static` | Const/Static | TypeOf |

### Processing Flow

```
file path → read source → tree-sitter parse → walk AST → emit Nodes + Edges
    → resolve use statements → resolve call targets → store in SQLite
```

## Reference Resolution

Two strategies (no framework-specific resolvers):

1. **Use-statement resolution:** Follow `use` paths to find target symbols. Handles `use crate::`, `use super::`, `use self::`, and external crate references.

2. **Name-based matching:** For method calls (`.foo()`), match by method name against all known methods. Ranking: same module > same crate > external.

Type-informed matching (narrowing method resolution by receiver type) is a stretch goal — treat as best-effort.

## Graph Queries

| Query | Description | Implementation |
|---|---|---|
| `callers(node_id)` | What calls this function? | edges WHERE target = ? AND kind = calls |
| `callees(node_id)` | What does this call? | edges WHERE source = ? AND kind = calls |
| `impact(node_id, depth)` | Transitive callers to N levels | BFS over caller edges |
| `call_graph(node_id)` | Bidirectional call relationships | BFS both directions |
| `dead_code()` | Unreferenced symbols | Zero in-degree, excluding main/#[test]/pub |
| `type_hierarchy(node_id)` | Trait implementation chain | Follow Implements edges |
| `search(query)` | Full-text symbol search | FTS5 on nodes_fts |
| `semantic_search(query, k)` | Vector similarity | Cosine similarity on embeddings |

## MCP Server

Stdio transport, JSON-RPC protocol. Tools exposed to Claude Code:

- `codegraph_search` — find symbols by name (FTS5)
- `codegraph_context` — build context for a task (semantic search + graph expansion)
- `codegraph_callers` / `codegraph_callees` — call relationships
- `codegraph_impact` — impact radius analysis
- `codegraph_node` — get full symbol details
- `codegraph_status` — index stats and health

Implementation: `tokio` async I/O, read/write JSON-RPC over stdin/stdout.

## CLI

```
codegraph init [path]        # Create .codegraph/ config
codegraph index [path]       # Full index
codegraph sync [path]        # Incremental update
codegraph status [path]      # Show stats
codegraph query <search>     # Search symbols
codegraph context <task>     # Build context
codegraph serve              # Start MCP server (stdio)
```

## Dependencies

| Crate | Purpose |
|---|---|
| `rusqlite` (bundled) | SQLite database |
| `tree-sitter` | AST parsing framework |
| `tree-sitter-rust` | Rust grammar |
| `ort` | ONNX Runtime for embeddings |
| `clap` | CLI argument parsing |
| `serde` / `serde_json` | Serialization |
| `tokio` | Async runtime (MCP server) |
| `thiserror` | Error types |
| `tracing` | Structured logging |
| `sha2` | Content hashing for sync |

## Configuration

Per-project `.codegraph/config.json`:

```json
{
  "version": 1,
  "root_dir": ".",
  "include": ["**/*.rs"],
  "exclude": ["target/**", "tests/**"],
  "max_file_size": 1048576,
  "extract_docstrings": true,
  "track_call_sites": true,
  "enable_embeddings": false
}
```

## Implementation Order

1. **Types & config** — Core types, configuration, error handling
2. **SQLite layer** — Schema, connection, CRUD operations, FTS5
3. **Tree-sitter extraction** — Parse Rust files, emit nodes and edges
4. **Reference resolution** — Use-statement and name-based resolution
5. **Graph queries** — Callers, callees, impact, dead code, etc.
6. **CLI** — Init, index, sync, status, query commands
7. **Context builder** — Semantic search + graph expansion for context
8. **Vector embeddings** — ONNX runtime integration, embedding storage
9. **MCP server** — Stdio JSON-RPC transport, tool handlers
10. **Incremental sync** — Content hashing, dirty detection, partial re-index
