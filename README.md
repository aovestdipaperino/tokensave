# codegraph-rs

A Rust port of [CodeGraph](https://github.com/colbymchenry/codegraph) — a local-first code intelligence system that builds a semantic knowledge graph from any codebase.

## Origin

This project is a Rust port of the original [TypeScript implementation](https://github.com/colbymchenry/codegraph) by [@colbymchenry](https://github.com/colbymchenry). The original TypeScript source is included as a git submodule under `codegraph/` for reference.

The port maintains the same architecture and MCP tool interface while leveraging Rust for performance and native tree-sitter bindings.

## Features

- Tree-sitter AST parsing for Rust, Go, and Java
- SQLite-backed knowledge graph with FTS5 search
- MCP server (JSON-RPC 2.0 over stdio) for AI assistant integration
- Graph traversal: callers, callees, impact radius
- Incremental sync for fast re-indexing
- Vector embeddings for semantic search

## Usage

```bash
# Initialize in a project
codegraph init [path]

# Full index
codegraph index [path]

# Incremental update
codegraph sync [path]

# Show statistics
codegraph status [path]

# Search symbols
codegraph query <search> [path]

# Start MCP server
codegraph serve
```

## Building

```bash
cargo build --release
cargo test
cargo clippy --all
```
