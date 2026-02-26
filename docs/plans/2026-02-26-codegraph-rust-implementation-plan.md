# CodeGraph Rust Port — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Port CodeGraph from TypeScript to Rust as a single-binary code intelligence tool for Rust codebases.

**Architecture:** Single crate with module-based structure. SQLite for storage (rusqlite), tree-sitter-rust for AST parsing, ort for ONNX embeddings, clap for CLI, tokio for async MCP server. All data flows through a central `CodeGraph` orchestrator.

**Tech Stack:** Rust 2021, rusqlite (bundled), tree-sitter + tree-sitter-rust, ort, clap, serde/serde_json, tokio, thiserror, tracing, sha2

---

## Task 1: Project Scaffold & Core Types

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/main.rs`
- Create: `src/types.rs`
- Create: `src/errors.rs`
- Test: `tests/types_test.rs`

**Step 1: Initialize Cargo project**

```bash
cd /Users/enzolombardi/Code/code-graph
cargo init --name codegraph
```

**Step 2: Set up Cargo.toml with all dependencies**

```toml
[package]
name = "codegraph"
version = "0.1.0"
edition = "2021"
description = "Code intelligence tool that builds a semantic knowledge graph from Rust codebases"

[dependencies]
rusqlite = { version = "0.31", features = ["bundled", "vtab"] }
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
ort = { version = "2", features = ["load-dynamic"] }
ndarray = "0.16"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
sha2 = "0.10"
glob = "0.3"
walkdir = "2"

[dev-dependencies]
tempfile = "3"
```

**Step 3: Write the failing test for core types**

Create `tests/types_test.rs`:

```rust
use codegraph::types::*;

#[test]
fn test_node_kind_display() {
    assert_eq!(NodeKind::Function.as_str(), "function");
    assert_eq!(NodeKind::Struct.as_str(), "struct");
    assert_eq!(NodeKind::Impl.as_str(), "impl");
    assert_eq!(NodeKind::Use.as_str(), "use");
}

#[test]
fn test_edge_kind_display() {
    assert_eq!(EdgeKind::Contains.as_str(), "contains");
    assert_eq!(EdgeKind::Calls.as_str(), "calls");
    assert_eq!(EdgeKind::Implements.as_str(), "implements");
}

#[test]
fn test_node_kind_from_str() {
    assert_eq!(NodeKind::from_str("function"), Some(NodeKind::Function));
    assert_eq!(NodeKind::from_str("struct"), Some(NodeKind::Struct));
    assert_eq!(NodeKind::from_str("bogus"), None);
}

#[test]
fn test_edge_kind_from_str() {
    assert_eq!(EdgeKind::from_str("calls"), Some(EdgeKind::Calls));
    assert_eq!(EdgeKind::from_str("contains"), Some(EdgeKind::Contains));
    assert_eq!(EdgeKind::from_str("bogus"), None);
}

#[test]
fn test_visibility_default() {
    assert_eq!(Visibility::default(), Visibility::Private);
}

#[test]
fn test_node_id_generation_is_deterministic() {
    let id1 = generate_node_id("src/main.rs", NodeKind::Function, "main", 1);
    let id2 = generate_node_id("src/main.rs", NodeKind::Function, "main", 1);
    assert_eq!(id1, id2);

    let id3 = generate_node_id("src/main.rs", NodeKind::Function, "other", 1);
    assert_ne!(id1, id3);
}

#[test]
fn test_node_id_format() {
    let id = generate_node_id("src/main.rs", NodeKind::Function, "main", 1);
    assert!(id.starts_with("function:"));
    assert_eq!(id.len(), "function:".len() + 32); // kind: + 32-char hash
}

#[test]
fn test_node_serde_roundtrip() {
    let node = Node {
        id: "function:abc123".to_string(),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 0,
    };

    let json = serde_json::to_string(&node).unwrap();
    let deserialized: Node = serde_json::from_str(&json).unwrap();
    assert_eq!(node.id, deserialized.id);
    assert_eq!(node.kind, deserialized.kind);
    assert_eq!(node.name, deserialized.name);
}

#[test]
fn test_edge_serde_roundtrip() {
    let edge = Edge {
        source: "function:abc".to_string(),
        target: "function:def".to_string(),
        kind: EdgeKind::Calls,
        line: Some(10),
    };

    let json = serde_json::to_string(&edge).unwrap();
    let deserialized: Edge = serde_json::from_str(&json).unwrap();
    assert_eq!(edge.source, deserialized.source);
    assert_eq!(edge.kind, deserialized.kind);
}
```

**Step 4: Run test to verify it fails**

```bash
cargo test --test types_test
```

Expected: FAIL — module `codegraph::types` not found.

**Step 5: Implement types.rs**

Create `src/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The kind of code symbol a node represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
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

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::EnumVariant => "enum_variant",
            Self::Trait => "trait",
            Self::Function => "function",
            Self::Method => "method",
            Self::Impl => "impl",
            Self::Const => "constant",
            Self::Static => "static",
            Self::TypeAlias => "type_alias",
            Self::Field => "field",
            Self::Macro => "macro",
            Self::Use => "use",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "file" => Some(Self::File),
            "module" => Some(Self::Module),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "enum_variant" => Some(Self::EnumVariant),
            "trait" => Some(Self::Trait),
            "function" => Some(Self::Function),
            "method" => Some(Self::Method),
            "impl" => Some(Self::Impl),
            "constant" => Some(Self::Const),
            "static" => Some(Self::Static),
            "type_alias" => Some(Self::TypeAlias),
            "field" => Some(Self::Field),
            "macro" => Some(Self::Macro),
            "use" => Some(Self::Use),
            _ => None,
        }
    }
}

/// The kind of relationship between two nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Calls,
    Uses,
    Implements,
    TypeOf,
    Returns,
    DerivesMacro,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Calls => "calls",
            Self::Uses => "uses",
            Self::Implements => "implements",
            Self::TypeOf => "type_of",
            Self::Returns => "returns",
            Self::DerivesMacro => "derives_macro",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "contains" => Some(Self::Contains),
            "calls" => Some(Self::Calls),
            "uses" => Some(Self::Uses),
            "implements" => Some(Self::Implements),
            "type_of" => Some(Self::TypeOf),
            "returns" => Some(Self::Returns),
            "derives_macro" => Some(Self::DerivesMacro),
            _ => None,
        }
    }
}

/// Visibility of a code symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Pub,
    PubCrate,
    PubSuper,
    #[default]
    Private,
}

/// A code symbol extracted from the AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub visibility: Visibility,
    pub is_async: bool,
    pub updated_at: i64,
}

/// A relationship between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub line: Option<u32>,
}

/// A tracked file in the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub content_hash: String,
    pub size: u64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub node_count: u32,
}

/// An unresolved reference found during extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub from_node_id: String,
    pub reference_name: String,
    pub reference_kind: EdgeKind,
    pub line: u32,
    pub column: u32,
    pub file_path: String,
}

/// Result of extracting symbols from a single file.
#[derive(Debug, Clone, Default)]
pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// A subset of the graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Subgraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub roots: Vec<String>,
}

/// A search result with relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
}

/// Options for graph traversal.
#[derive(Debug, Clone)]
pub struct TraversalOptions {
    pub max_depth: usize,
    pub edge_kinds: Vec<EdgeKind>,
    pub node_kinds: Vec<NodeKind>,
    pub direction: TraversalDirection,
    pub limit: usize,
    pub include_start: bool,
}

impl Default for TraversalOptions {
    fn default() -> Self {
        Self {
            max_depth: usize::MAX,
            edge_kinds: Vec::new(),
            node_kinds: Vec::new(),
            direction: TraversalDirection::Outgoing,
            limit: usize::MAX,
            include_start: true,
        }
    }
}

/// Direction of graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    Outgoing,
    Incoming,
    Both,
}

/// Statistics about the graph database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
    pub nodes_by_kind: Vec<(String, usize)>,
    pub edges_by_kind: Vec<(String, usize)>,
    pub db_size_bytes: u64,
    pub last_updated: i64,
}

/// Options for building task context.
#[derive(Debug, Clone)]
pub struct BuildContextOptions {
    pub max_nodes: usize,
    pub max_code_blocks: usize,
    pub max_code_block_size: usize,
    pub include_code: bool,
    pub format: OutputFormat,
    pub search_limit: usize,
    pub traversal_depth: usize,
    pub min_score: f64,
}

impl Default for BuildContextOptions {
    fn default() -> Self {
        Self {
            max_nodes: 20,
            max_code_blocks: 5,
            max_code_block_size: 1500,
            include_code: true,
            format: OutputFormat::Markdown,
            search_limit: 3,
            traversal_depth: 1,
            min_score: 0.3,
        }
    }
}

/// Output format for context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Json,
}

/// Task context built for an AI query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub query: String,
    pub summary: String,
    pub subgraph: Subgraph,
    pub entry_points: Vec<Node>,
    pub code_blocks: Vec<CodeBlock>,
    pub related_files: Vec<String>,
}

/// A block of source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub content: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub node_id: Option<String>,
}

/// Generates a deterministic node ID from its identifying properties.
pub fn generate_node_id(file_path: &str, kind: NodeKind, name: &str, line: u32) -> String {
    let input = format!("{}:{}:{}:{}", file_path, kind.as_str(), name, line);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    let hex = hex::encode(hash);
    format!("{}:{}", kind.as_str(), &hex[..32])
}

/// Result of reference resolution.
#[derive(Debug, Clone, Default)]
pub struct ResolutionResult {
    pub resolved: Vec<ResolvedRef>,
    pub unresolved: Vec<UnresolvedRef>,
    pub total: usize,
    pub resolved_count: usize,
}

/// A resolved reference.
#[derive(Debug, Clone)]
pub struct ResolvedRef {
    pub original: UnresolvedRef,
    pub target_node_id: String,
    pub confidence: f64,
    pub resolved_by: String,
}
```

**Step 6: Implement errors.rs**

Create `src/errors.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodeGraphError {
    #[error("file error: {message} (path: {path})")]
    File { message: String, path: String },

    #[error("parse error: {message} (path: {path}, line: {line:?})")]
    Parse {
        message: String,
        path: String,
        line: Option<u32>,
    },

    #[error("database error: {message} (operation: {operation})")]
    Database { message: String, operation: String },

    #[error("search error: {message} (query: {query})")]
    Search { message: String, query: String },

    #[error("config error: {message}")]
    Config { message: String },

    #[error("vector error: {message}")]
    Vector { message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CodeGraphError>;
```

**Step 7: Wire up lib.rs**

Create `src/lib.rs`:

```rust
pub mod errors;
pub mod types;
```

Add `hex` dependency to `Cargo.toml` under `[dependencies]`:

```toml
hex = "0.4"
```

**Step 8: Create minimal main.rs**

Create `src/main.rs`:

```rust
fn main() {
    println!("codegraph - code intelligence for Rust");
}
```

**Step 9: Run tests to verify they pass**

```bash
cargo test --test types_test
```

Expected: All tests PASS.

**Step 10: Commit**

```bash
git init
git add Cargo.toml src/ tests/
git commit -m "feat: scaffold project with core types and error handling"
```

---

## Task 2: Configuration Module

**Files:**
- Create: `src/config.rs`
- Modify: `src/lib.rs`
- Test: `tests/config_test.rs`

**Step 1: Write the failing test**

Create `tests/config_test.rs`:

```rust
use codegraph::config::*;
use tempfile::TempDir;

#[test]
fn test_default_config_has_rust_patterns() {
    let config = CodeGraphConfig::default();
    assert!(config.include.iter().any(|p| p == "**/*.rs"));
    assert!(config.exclude.iter().any(|p| p == "target/**"));
}

#[test]
fn test_save_and_load_config() {
    let dir = TempDir::new().unwrap();
    let config = CodeGraphConfig::default();
    save_config(dir.path(), &config).unwrap();
    let loaded = load_config(dir.path()).unwrap();
    assert_eq!(config.version, loaded.version);
    assert_eq!(config.include, loaded.include);
}

#[test]
fn test_should_include_file() {
    let config = CodeGraphConfig::default();
    assert!(should_include_file("src/main.rs", &config));
    assert!(!should_include_file("target/debug/foo", &config));
    assert!(!should_include_file("node_modules/foo.rs", &config));
}

#[test]
fn test_codegraph_dir_creation() {
    let dir = TempDir::new().unwrap();
    let cg_dir = get_codegraph_dir(dir.path());
    assert!(cg_dir.ends_with(".codegraph"));
}

#[test]
fn test_config_serde_roundtrip() {
    let config = CodeGraphConfig::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: CodeGraphConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.version, deserialized.version);
    assert_eq!(config.max_file_size, deserialized.max_file_size);
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --test config_test
```

Expected: FAIL — module `codegraph::config` not found.

**Step 3: Implement config.rs**

Create `src/config.rs`:

```rust
use crate::errors::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "config.json";
const CODEGRAPH_DIR: &str = ".codegraph";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraphConfig {
    pub version: u32,
    pub root_dir: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_file_size: u64,
    pub extract_docstrings: bool,
    pub track_call_sites: bool,
    pub enable_embeddings: bool,
}

impl Default for CodeGraphConfig {
    fn default() -> Self {
        Self {
            version: 1,
            root_dir: ".".to_string(),
            include: vec!["**/*.rs".to_string()],
            exclude: vec![
                "target/**".to_string(),
                ".git/**".to_string(),
                ".codegraph/**".to_string(),
                "node_modules/**".to_string(),
                "vendor/**".to_string(),
                "**/*.min.*".to_string(),
            ],
            max_file_size: 1_048_576,
            extract_docstrings: true,
            track_call_sites: true,
            enable_embeddings: false,
        }
    }
}

/// Get the path to the .codegraph directory for a project.
pub fn get_codegraph_dir(project_root: &Path) -> PathBuf {
    project_root.join(CODEGRAPH_DIR)
}

/// Get the path to the config file.
pub fn get_config_path(project_root: &Path) -> PathBuf {
    get_codegraph_dir(project_root).join(CONFIG_FILENAME)
}

/// Load configuration from disk, falling back to defaults.
pub fn load_config(project_root: &Path) -> Result<CodeGraphConfig> {
    let config_path = get_config_path(project_root);
    if !config_path.exists() {
        return Ok(CodeGraphConfig::default());
    }
    let contents = std::fs::read_to_string(&config_path).map_err(|e| CodeGraphError::Config {
        message: format!("failed to read config: {}", e),
    })?;
    let config: CodeGraphConfig =
        serde_json::from_str(&contents).map_err(|e| CodeGraphError::Config {
            message: format!("failed to parse config: {}", e),
        })?;
    Ok(config)
}

/// Save configuration to disk atomically.
pub fn save_config(project_root: &Path, config: &CodeGraphConfig) -> Result<()> {
    let cg_dir = get_codegraph_dir(project_root);
    std::fs::create_dir_all(&cg_dir)?;

    let config_path = get_config_path(project_root);
    let tmp_path = config_path.with_extension("json.tmp");

    let json =
        serde_json::to_string_pretty(config).map_err(|e| CodeGraphError::Config {
            message: format!("failed to serialize config: {}", e),
        })?;

    std::fs::write(&tmp_path, &json)?;
    std::fs::rename(&tmp_path, &config_path)?;
    Ok(())
}

/// Check if a file path should be included based on config patterns.
pub fn should_include_file(file_path: &str, config: &CodeGraphConfig) -> bool {
    let path = Path::new(file_path);

    // Check excludes first
    for pattern in &config.exclude {
        if glob_match(pattern, file_path) {
            return false;
        }
    }

    // Check includes
    for pattern in &config.include {
        if glob_match(pattern, file_path) {
            return true;
        }
    }

    false
}

/// Simple glob matching supporting ** and * patterns.
fn glob_match(pattern: &str, path: &str) -> bool {
    let glob = glob::Pattern::new(pattern);
    match glob {
        Ok(g) => g.matches_with(
            path,
            glob::MatchOptions {
                case_sensitive: true,
                require_literal_separator: false,
                require_literal_leading_dot: false,
            },
        ),
        Err(_) => false,
    }
}
```

**Step 4: Add to lib.rs**

```rust
pub mod config;
pub mod errors;
pub mod types;
```

**Step 5: Run tests to verify they pass**

```bash
cargo test --test config_test
```

Expected: All PASS.

**Step 6: Commit**

```bash
git add src/config.rs src/lib.rs tests/config_test.rs
git commit -m "feat: add configuration module with glob-based file filtering"
```

---

## Task 3: SQLite Database Layer

**Files:**
- Create: `src/db/mod.rs`
- Create: `src/db/connection.rs`
- Create: `src/db/queries.rs`
- Create: `src/db/schema.sql`
- Modify: `src/lib.rs`
- Test: `tests/db_test.rs`

**Step 1: Write the failing test**

Create `tests/db_test.rs`:

```rust
use codegraph::db::*;
use codegraph::types::*;
use tempfile::TempDir;

#[test]
fn test_initialize_creates_database() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let conn = Database::initialize(&db_path).unwrap();
    assert!(db_path.exists());
    conn.close();
}

#[test]
fn test_insert_and_get_node() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let node = Node {
        id: "function:abc123def456abc123def456abc12345".to_string(),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 5,
        start_column: 0,
        end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false,
        updated_at: 1000,
    };

    db.insert_node(&node).unwrap();

    let fetched = db.get_node_by_id(&node.id).unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.name, "main");
    assert_eq!(fetched.kind, NodeKind::Function);
    assert_eq!(fetched.signature, Some("fn main()".to_string()));

    db.close();
}

#[test]
fn test_insert_and_get_edge() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let node1 = Node {
        id: "function:aaa".to_string(),
        kind: NodeKind::Function,
        name: "caller".to_string(),
        qualified_name: "caller".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1, end_line: 5,
        start_column: 0, end_column: 1,
        signature: None, docstring: None,
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };
    let node2 = Node {
        id: "function:bbb".to_string(),
        kind: NodeKind::Function,
        name: "callee".to_string(),
        qualified_name: "callee".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 10, end_line: 15,
        start_column: 0, end_column: 1,
        signature: None, docstring: None,
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };
    db.insert_node(&node1).unwrap();
    db.insert_node(&node2).unwrap();

    let edge = Edge {
        source: "function:aaa".to_string(),
        target: "function:bbb".to_string(),
        kind: EdgeKind::Calls,
        line: Some(3),
    };
    db.insert_edge(&edge).unwrap();

    let outgoing = db.get_outgoing_edges("function:aaa", &[]).unwrap();
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].kind, EdgeKind::Calls);

    let incoming = db.get_incoming_edges("function:bbb", &[]).unwrap();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].source, "function:aaa");

    db.close();
}

#[test]
fn test_upsert_file() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let file = FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "abc123".to_string(),
        size: 1024,
        modified_at: 1000,
        indexed_at: 1001,
        node_count: 5,
    };
    db.upsert_file(&file).unwrap();

    let fetched = db.get_file("src/main.rs").unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().content_hash, "abc123");

    db.close();
}

#[test]
fn test_fts_search() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let node = Node {
        id: "function:search_test".to_string(),
        kind: NodeKind::Function,
        name: "process_request".to_string(),
        qualified_name: "server::process_request".to_string(),
        file_path: "src/server.rs".to_string(),
        start_line: 10, end_line: 20,
        start_column: 0, end_column: 1,
        signature: Some("fn process_request(req: Request) -> Response".to_string()),
        docstring: Some("Processes an incoming HTTP request".to_string()),
        visibility: Visibility::Pub,
        is_async: true, updated_at: 0,
    };
    db.insert_node(&node).unwrap();

    let results = db.search_nodes("process", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].node.name, "process_request");

    db.close();
}

#[test]
fn test_get_stats() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let node = Node {
        id: "function:stats_test".to_string(),
        kind: NodeKind::Function,
        name: "test_fn".to_string(),
        qualified_name: "test_fn".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1, end_line: 5,
        start_column: 0, end_column: 1,
        signature: None, docstring: None,
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };
    db.insert_node(&node).unwrap();

    let stats = db.get_stats().unwrap();
    assert_eq!(stats.node_count, 1);

    db.close();
}

#[test]
fn test_delete_nodes_by_file() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let node = Node {
        id: "function:del_test".to_string(),
        kind: NodeKind::Function,
        name: "to_delete".to_string(),
        qualified_name: "to_delete".to_string(),
        file_path: "src/old.rs".to_string(),
        start_line: 1, end_line: 5,
        start_column: 0, end_column: 1,
        signature: None, docstring: None,
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };
    db.insert_node(&node).unwrap();
    assert!(db.get_node_by_id("function:del_test").unwrap().is_some());

    db.delete_nodes_by_file("src/old.rs").unwrap();
    assert!(db.get_node_by_id("function:del_test").unwrap().is_none());

    db.close();
}

#[test]
fn test_unresolved_refs() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("codegraph.db");
    let db = Database::initialize(&db_path).unwrap();

    let uref = UnresolvedRef {
        from_node_id: "function:caller".to_string(),
        reference_name: "some_fn".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 10,
        column: 4,
        file_path: "src/lib.rs".to_string(),
    };
    db.insert_unresolved_ref(&uref).unwrap();

    let refs = db.get_unresolved_refs().unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reference_name, "some_fn");

    db.close();
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --test db_test
```

Expected: FAIL — module `codegraph::db` not found.

**Step 3: Create the SQL schema**

Create `src/db/schema.sql`:

```sql
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_versions (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL,
    description TEXT
);

-- Code symbols
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    start_column INTEGER NOT NULL,
    end_column INTEGER NOT NULL,
    docstring TEXT,
    signature TEXT,
    visibility TEXT NOT NULL DEFAULT 'private',
    is_async INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

-- Relationships between nodes
CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    target TEXT NOT NULL,
    kind TEXT NOT NULL,
    line INTEGER,
    FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Tracked files
CREATE TABLE IF NOT EXISTS files (
    path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL,
    node_count INTEGER NOT NULL DEFAULT 0
);

-- Pending reference resolution
CREATE TABLE IF NOT EXISTS unresolved_refs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_node_id TEXT NOT NULL,
    reference_name TEXT NOT NULL,
    reference_kind TEXT NOT NULL,
    line INTEGER NOT NULL,
    col INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Embedding vectors
CREATE TABLE IF NOT EXISTS vectors (
    node_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    model TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Full-text search on nodes
CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
    name, qualified_name, docstring, signature,
    content='nodes',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
    INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
    VALUES (new.rowid, new.name, new.qualified_name, new.docstring, new.signature);
END;

CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
    VALUES ('delete', old.rowid, old.name, old.qualified_name, old.docstring, old.signature);
END;

CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, docstring, signature)
    VALUES ('delete', old.rowid, old.name, old.qualified_name, old.docstring, old.signature);
    INSERT INTO nodes_fts(rowid, name, qualified_name, docstring, signature)
    VALUES (new.rowid, new.name, new.qualified_name, new.docstring, new.signature);
END;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
CREATE INDEX IF NOT EXISTS idx_nodes_file_line ON nodes(file_path, start_line);

CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);

CREATE INDEX IF NOT EXISTS idx_unresolved_from ON unresolved_refs(from_node_id);
CREATE INDEX IF NOT EXISTS idx_unresolved_name ON unresolved_refs(reference_name);
CREATE INDEX IF NOT EXISTS idx_unresolved_file ON unresolved_refs(file_path);

-- Record initial schema version
INSERT OR IGNORE INTO schema_versions (version, applied_at, description)
VALUES (1, strftime('%s', 'now'), 'Initial schema');
```

**Step 4: Implement connection.rs and queries**

Create `src/db/mod.rs`:

```rust
mod connection;
mod queries;

pub use connection::Database;
```

Create `src/db/connection.rs`:

```rust
use crate::errors::{CodeGraphError, Result};
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Initialize a new database with schema.
    pub fn initialize(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "initialize".to_string(),
        })?;

        // Set pragmas for performance
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 120000;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -65536;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;",
        )
        .map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "set_pragmas".to_string(),
        })?;

        // Apply schema
        let schema = include_str!("schema.sql");
        conn.execute_batch(schema).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "apply_schema".to_string(),
        })?;

        Ok(Self { conn })
    }

    /// Open an existing database.
    pub fn open(db_path: &Path) -> Result<Self> {
        if !db_path.exists() {
            return Err(CodeGraphError::Database {
                message: format!("database not found: {}", db_path.display()),
                operation: "open".to_string(),
            });
        }
        Self::initialize(db_path)
    }

    /// Get a reference to the underlying connection.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Close the database connection.
    pub fn close(self) {
        drop(self.conn);
    }

    /// Optimize the database (VACUUM + ANALYZE).
    pub fn optimize(&self) -> Result<()> {
        self.conn
            .execute_batch("VACUUM; ANALYZE;")
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "optimize".to_string(),
            })
    }

    /// Get database file size in bytes.
    pub fn size(&self) -> Result<u64> {
        let path: String =
            self.conn
                .query_row("PRAGMA database_list", [], |row| row.get(2))
                .map_err(|e| CodeGraphError::Database {
                    message: e.to_string(),
                    operation: "get_size".to_string(),
                })?;
        Ok(std::fs::metadata(path).map(|m| m.len()).unwrap_or(0))
    }
}
```

Create `src/db/queries.rs` — This is a larger file implementing all query methods on `Database`:

```rust
use crate::errors::{CodeGraphError, Result};
use crate::types::*;
use rusqlite::params;

use super::Database;

impl Database {
    // ── Node Operations ──

    pub fn insert_node(&self, node: &Node) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO nodes
             (id, kind, name, qualified_name, file_path,
              start_line, end_line, start_column, end_column,
              docstring, signature, visibility, is_async, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                node.id,
                node.kind.as_str(),
                node.name,
                node.qualified_name,
                node.file_path,
                node.start_line,
                node.end_line,
                node.start_column,
                node.end_column,
                node.docstring,
                node.signature,
                visibility_to_str(node.visibility),
                node.is_async as i32,
                node.updated_at,
            ],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_node".to_string(),
        })?;
        Ok(())
    }

    pub fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_nodes_tx".to_string(),
        })?;
        for node in nodes {
            self.insert_node(node)?;
        }
        tx.commit().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_nodes_commit".to_string(),
        })?;
        Ok(())
    }

    pub fn get_node_by_id(&self, id: &str) -> Result<Option<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes WHERE id = ?1"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_node_by_id".to_string(),
        })?;

        let node = stmt.query_row(params![id], row_to_node).optional().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_node_by_id".to_string(),
        })?;

        Ok(node)
    }

    pub fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes WHERE file_path = ?1 ORDER BY start_line"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_nodes_by_file".to_string(),
        })?;

        let nodes = stmt.query_map(params![file_path], row_to_node)
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_nodes_by_file".to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(nodes)
    }

    pub fn get_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes WHERE kind = ?1"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_nodes_by_kind".to_string(),
        })?;

        let nodes = stmt.query_map(params![kind.as_str()], row_to_node)
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_nodes_by_kind".to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(nodes)
    }

    pub fn get_all_nodes(&self) -> Result<Vec<Node>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_all_nodes".to_string(),
        })?;

        let nodes = stmt.query_map([], row_to_node)
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_all_nodes".to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(nodes)
    }

    pub fn delete_nodes_by_file(&self, file_path: &str) -> Result<()> {
        // Delete edges referencing these nodes first
        self.conn().execute(
            "DELETE FROM edges WHERE source IN (SELECT id FROM nodes WHERE file_path = ?1)
             OR target IN (SELECT id FROM nodes WHERE file_path = ?1)",
            params![file_path],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "delete_edges_by_file".to_string(),
        })?;

        // Delete unresolved refs
        self.conn().execute(
            "DELETE FROM unresolved_refs WHERE from_node_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
            params![file_path],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "delete_unresolved_by_file".to_string(),
        })?;

        // Delete vectors
        self.conn().execute(
            "DELETE FROM vectors WHERE node_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
            params![file_path],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "delete_vectors_by_file".to_string(),
        })?;

        // Delete nodes
        self.conn().execute(
            "DELETE FROM nodes WHERE file_path = ?1",
            params![file_path],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "delete_nodes_by_file".to_string(),
        })?;

        Ok(())
    }

    // ── Edge Operations ──

    pub fn insert_edge(&self, edge: &Edge) -> Result<()> {
        self.conn().execute(
            "INSERT INTO edges (source, target, kind, line) VALUES (?1, ?2, ?3, ?4)",
            params![edge.source, edge.target, edge.kind.as_str(), edge.line],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_edge".to_string(),
        })?;
        Ok(())
    }

    pub fn insert_edges(&self, edges: &[Edge]) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_edges_tx".to_string(),
        })?;
        for edge in edges {
            self.insert_edge(edge)?;
        }
        tx.commit().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_edges_commit".to_string(),
        })?;
        Ok(())
    }

    pub fn get_outgoing_edges(&self, source_id: &str, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut stmt = self.conn().prepare(
                "SELECT source, target, kind, line FROM edges WHERE source = ?1"
            ).map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_outgoing_edges".to_string(),
            })?;

            let edges = stmt.query_map(params![source_id], row_to_edge)
                .map_err(|e| CodeGraphError::Database {
                    message: e.to_string(),
                    operation: "get_outgoing_edges".to_string(),
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(edges)
        } else {
            let kind_strs: Vec<&str> = kinds.iter().map(|k| k.as_str()).collect();
            let placeholders: Vec<String> = (0..kind_strs.len()).map(|i| format!("?{}", i + 2)).collect();
            let sql = format!(
                "SELECT source, target, kind, line FROM edges WHERE source = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );
            let mut stmt = self.conn().prepare(&sql).map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_outgoing_edges_filtered".to_string(),
            })?;

            let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(source_id.to_string())];
            for k in &kind_strs {
                params_vec.push(Box::new(k.to_string()));
            }

            let edges = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|b| b.as_ref())), row_to_edge)
                .map_err(|e| CodeGraphError::Database {
                    message: e.to_string(),
                    operation: "get_outgoing_edges_filtered".to_string(),
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(edges)
        }
    }

    pub fn get_incoming_edges(&self, target_id: &str, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        if kinds.is_empty() {
            let mut stmt = self.conn().prepare(
                "SELECT source, target, kind, line FROM edges WHERE target = ?1"
            ).map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_incoming_edges".to_string(),
            })?;

            let edges = stmt.query_map(params![target_id], row_to_edge)
                .map_err(|e| CodeGraphError::Database {
                    message: e.to_string(),
                    operation: "get_incoming_edges".to_string(),
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(edges)
        } else {
            let kind_strs: Vec<&str> = kinds.iter().map(|k| k.as_str()).collect();
            let placeholders: Vec<String> = (0..kind_strs.len()).map(|i| format!("?{}", i + 2)).collect();
            let sql = format!(
                "SELECT source, target, kind, line FROM edges WHERE target = ?1 AND kind IN ({})",
                placeholders.join(", ")
            );
            let mut stmt = self.conn().prepare(&sql).map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_incoming_edges_filtered".to_string(),
            })?;

            let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(target_id.to_string())];
            for k in &kind_strs {
                params_vec.push(Box::new(k.to_string()));
            }

            let edges = stmt.query_map(rusqlite::params_from_iter(params_vec.iter().map(|b| b.as_ref())), row_to_edge)
                .map_err(|e| CodeGraphError::Database {
                    message: e.to_string(),
                    operation: "get_incoming_edges_filtered".to_string(),
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(edges)
        }
    }

    pub fn delete_edges_by_source(&self, source_id: &str) -> Result<()> {
        self.conn().execute(
            "DELETE FROM edges WHERE source = ?1",
            params![source_id],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "delete_edges_by_source".to_string(),
        })?;
        Ok(())
    }

    // ── File Operations ──

    pub fn upsert_file(&self, file: &FileRecord) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO files
             (path, content_hash, size, modified_at, indexed_at, node_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![file.path, file.content_hash, file.size, file.modified_at, file.indexed_at, file.node_count],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "upsert_file".to_string(),
        })?;
        Ok(())
    }

    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut stmt = self.conn().prepare(
            "SELECT path, content_hash, size, modified_at, indexed_at, node_count
             FROM files WHERE path = ?1"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_file".to_string(),
        })?;

        let file = stmt.query_row(params![path], row_to_file).optional().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_file".to_string(),
        })?;
        Ok(file)
    }

    pub fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn().prepare(
            "SELECT path, content_hash, size, modified_at, indexed_at, node_count FROM files"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_all_files".to_string(),
        })?;

        let files = stmt.query_map([], row_to_file)
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_all_files".to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(files)
    }

    pub fn delete_file(&self, path: &str) -> Result<()> {
        self.delete_nodes_by_file(path)?;
        self.conn().execute("DELETE FROM files WHERE path = ?1", params![path])
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "delete_file".to_string(),
            })?;
        Ok(())
    }

    // ── Unresolved References ──

    pub fn insert_unresolved_ref(&self, uref: &UnresolvedRef) -> Result<()> {
        self.conn().execute(
            "INSERT INTO unresolved_refs
             (from_node_id, reference_name, reference_kind, line, col, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                uref.from_node_id,
                uref.reference_name,
                uref.reference_kind.as_str(),
                uref.line,
                uref.column,
                uref.file_path,
            ],
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_unresolved_ref".to_string(),
        })?;
        Ok(())
    }

    pub fn insert_unresolved_refs(&self, refs: &[UnresolvedRef]) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_unresolved_refs_tx".to_string(),
        })?;
        for uref in refs {
            self.insert_unresolved_ref(uref)?;
        }
        tx.commit().map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "insert_unresolved_refs_commit".to_string(),
        })?;
        Ok(())
    }

    pub fn get_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>> {
        let mut stmt = self.conn().prepare(
            "SELECT from_node_id, reference_name, reference_kind, line, col, file_path
             FROM unresolved_refs"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_unresolved_refs".to_string(),
        })?;

        let refs = stmt.query_map([], row_to_unresolved_ref)
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "get_unresolved_refs".to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(refs)
    }

    pub fn clear_unresolved_refs(&self) -> Result<()> {
        self.conn().execute("DELETE FROM unresolved_refs", [])
            .map_err(|e| CodeGraphError::Database {
                message: e.to_string(),
                operation: "clear_unresolved_refs".to_string(),
            })?;
        Ok(())
    }

    // ── Search ──

    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        // Try FTS5 first
        let fts_query = format!("{}*", query);
        let mut stmt = self.conn().prepare(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_async, n.updated_at,
                    rank
             FROM nodes_fts fts
             JOIN nodes n ON n.rowid = fts.rowid
             WHERE nodes_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "search_nodes_fts".to_string(),
        })?;

        let results: Vec<SearchResult> = stmt.query_map(params![fts_query, limit as i64], |row| {
            let node = row_to_node(row)?;
            let rank: f64 = row.get(14)?;
            Ok(SearchResult { node, score: -rank }) // FTS5 rank is negative
        }).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "search_nodes_fts".to_string(),
        })?
        .filter_map(|r| r.ok())
        .collect();

        if !results.is_empty() {
            return Ok(results);
        }

        // Fall back to LIKE search
        let like_query = format!("%{}%", query);
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, name, qualified_name, file_path,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_async, updated_at
             FROM nodes
             WHERE name LIKE ?1 OR qualified_name LIKE ?1
             LIMIT ?2"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "search_nodes_like".to_string(),
        })?;

        let results = stmt.query_map(params![like_query, limit as i64], |row| {
            let node = row_to_node(row)?;
            Ok(SearchResult { node, score: 0.5 })
        }).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "search_nodes_like".to_string(),
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(results)
    }

    // ── Statistics ──

    pub fn get_stats(&self) -> Result<GraphStats> {
        let node_count: usize = self.conn()
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .unwrap_or(0);

        let edge_count: usize = self.conn()
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))
            .unwrap_or(0);

        let file_count: usize = self.conn()
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = self.conn().prepare(
            "SELECT kind, COUNT(*) FROM nodes GROUP BY kind ORDER BY COUNT(*) DESC"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_stats_nodes_by_kind".to_string(),
        })?;

        let nodes_by_kind: Vec<(String, usize)> = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        }).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_stats_nodes_by_kind".to_string(),
        })?
        .filter_map(|r| r.ok())
        .collect();

        let mut stmt = self.conn().prepare(
            "SELECT kind, COUNT(*) FROM edges GROUP BY kind ORDER BY COUNT(*) DESC"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_stats_edges_by_kind".to_string(),
        })?;

        let edges_by_kind: Vec<(String, usize)> = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        }).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "get_stats_edges_by_kind".to_string(),
        })?
        .filter_map(|r| r.ok())
        .collect();

        let db_size_bytes = self.size().unwrap_or(0);

        Ok(GraphStats {
            node_count,
            edge_count,
            file_count,
            nodes_by_kind,
            edges_by_kind,
            db_size_bytes,
            last_updated: 0,
        })
    }

    // ── Clear ──

    pub fn clear(&self) -> Result<()> {
        self.conn().execute_batch(
            "DELETE FROM vectors;
             DELETE FROM unresolved_refs;
             DELETE FROM edges;
             DELETE FROM nodes;
             DELETE FROM files;"
        ).map_err(|e| CodeGraphError::Database {
            message: e.to_string(),
            operation: "clear".to_string(),
        })?;
        Ok(())
    }
}

// ── Row Conversion Functions ──

fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
    Ok(Node {
        id: row.get(0)?,
        kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap_or(NodeKind::Function),
        name: row.get(2)?,
        qualified_name: row.get(3)?,
        file_path: row.get(4)?,
        start_line: row.get(5)?,
        end_line: row.get(6)?,
        start_column: row.get(7)?,
        end_column: row.get(8)?,
        docstring: row.get(9)?,
        signature: row.get(10)?,
        visibility: visibility_from_str(&row.get::<_, String>(11)?),
        is_async: row.get::<_, i32>(12)? != 0,
        updated_at: row.get(13)?,
    })
}

fn row_to_edge(row: &rusqlite::Row) -> rusqlite::Result<Edge> {
    Ok(Edge {
        source: row.get(0)?,
        target: row.get(1)?,
        kind: EdgeKind::from_str(&row.get::<_, String>(2)?).unwrap_or(EdgeKind::Contains),
        line: row.get(3)?,
    })
}

fn row_to_file(row: &rusqlite::Row) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        path: row.get(0)?,
        content_hash: row.get(1)?,
        size: row.get(2)?,
        modified_at: row.get(3)?,
        indexed_at: row.get(4)?,
        node_count: row.get(5)?,
    })
}

fn row_to_unresolved_ref(row: &rusqlite::Row) -> rusqlite::Result<UnresolvedRef> {
    Ok(UnresolvedRef {
        from_node_id: row.get(0)?,
        reference_name: row.get(1)?,
        reference_kind: EdgeKind::from_str(&row.get::<_, String>(2)?).unwrap_or(EdgeKind::Calls),
        line: row.get(3)?,
        column: row.get(4)?,
        file_path: row.get(5)?,
    })
}

fn visibility_to_str(v: Visibility) -> &'static str {
    match v {
        Visibility::Pub => "public",
        Visibility::PubCrate => "pub_crate",
        Visibility::PubSuper => "pub_super",
        Visibility::Private => "private",
    }
}

fn visibility_from_str(s: &str) -> Visibility {
    match s {
        "public" => Visibility::Pub,
        "pub_crate" => Visibility::PubCrate,
        "pub_super" => Visibility::PubSuper,
        _ => Visibility::Private,
    }
}

// Extension trait for optional query results
trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
```

**Step 5: Update lib.rs**

```rust
pub mod config;
pub mod db;
pub mod errors;
pub mod types;
```

**Step 6: Run tests to verify they pass**

```bash
cargo test --test db_test
```

Expected: All PASS.

**Step 7: Commit**

```bash
git add src/db/ src/lib.rs tests/db_test.rs
git commit -m "feat: add SQLite database layer with FTS5 search"
```

---

## Task 4: Tree-Sitter Extraction for Rust

**Files:**
- Create: `src/extraction/mod.rs`
- Create: `src/extraction/rust_extractor.rs`
- Modify: `src/lib.rs`
- Test: `tests/extraction_test.rs`

**Step 1: Write the failing test**

Create `tests/extraction_test.rs`:

```rust
use codegraph::extraction::RustExtractor;
use codegraph::types::*;

#[test]
fn test_extract_function() {
    let source = r#"
/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    let result = RustExtractor::extract("src/math.rs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let functions: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name, "add");
    assert_eq!(functions[0].visibility, Visibility::Pub);
    assert!(functions[0].signature.as_ref().unwrap().contains("fn add"));
    assert!(functions[0].docstring.as_ref().unwrap().contains("Adds two numbers"));
}

#[test]
fn test_extract_struct_with_fields() {
    let source = r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
}
"#;
    let result = RustExtractor::extract("src/geo.rs", source);

    let structs: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Struct).collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");

    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert_eq!(fields.len(), 2);

    // Check contains edges
    let contains: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
    assert!(contains.len() >= 2); // struct contains fields
}

#[test]
fn test_extract_enum() {
    let source = r#"
pub enum Color {
    Red,
    Green,
    Blue,
}
"#;
    let result = RustExtractor::extract("src/color.rs", source);

    let enums: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Enum).collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");

    let variants: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::EnumVariant).collect();
    assert_eq!(variants.len(), 3);
}

#[test]
fn test_extract_trait() {
    let source = r#"
pub trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}
"#;
    let result = RustExtractor::extract("src/draw.rs", source);

    let traits: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Trait).collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Drawable");

    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_extract_impl_block() {
    let source = r#"
struct Circle {
    radius: f64,
}

impl Circle {
    pub fn new(radius: f64) -> Self {
        Circle { radius }
    }

    pub fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
}
"#;
    let result = RustExtractor::extract("src/circle.rs", source);

    let impls: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Impl).collect();
    assert_eq!(impls.len(), 1);

    let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Method).collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_extract_trait_impl() {
    let source = r#"
trait Greet {
    fn hello(&self) -> String;
}

struct Person {
    name: String,
}

impl Greet for Person {
    fn hello(&self) -> String {
        format!("Hello, {}", self.name)
    }
}
"#;
    let result = RustExtractor::extract("src/greet.rs", source);

    let implements: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::Implements).collect();
    assert!(!implements.is_empty(), "should have implements edge");
}

#[test]
fn test_extract_use_declarations() {
    let source = r#"
use std::collections::HashMap;
use crate::types::Node;
"#;
    let result = RustExtractor::extract("src/lib.rs", source);

    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(uses.len(), 2);
}

#[test]
fn test_extract_call_sites() {
    let source = r#"
fn helper() -> i32 {
    42
}

fn main() {
    let x = helper();
    println!("{}", x);
}
"#;
    let result = RustExtractor::extract("src/main.rs", source);

    // Should have unresolved call references
    assert!(!result.unresolved_refs.is_empty(), "should have unresolved refs for calls");
    let call_refs: Vec<_> = result.unresolved_refs.iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
}

#[test]
fn test_extract_async_function() {
    let source = r#"
pub async fn fetch_data(url: &str) -> Result<String, Error> {
    Ok("data".to_string())
}
"#;
    let result = RustExtractor::extract("src/http.rs", source);

    let functions: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(functions.len(), 1);
    assert!(functions[0].is_async);
}

#[test]
fn test_extract_const_and_static() {
    let source = r#"
pub const MAX_SIZE: usize = 1024;
static COUNTER: AtomicU64 = AtomicU64::new(0);
"#;
    let result = RustExtractor::extract("src/globals.rs", source);

    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MAX_SIZE");

    let statics: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Static).collect();
    assert_eq!(statics.len(), 1);
    assert_eq!(statics[0].name, "COUNTER");
}

#[test]
fn test_extract_type_alias() {
    let source = r#"
pub type Result<T> = std::result::Result<T, Error>;
"#;
    let result = RustExtractor::extract("src/types.rs", source);

    let aliases: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::TypeAlias).collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "Result");
}

#[test]
fn test_extract_module() {
    let source = r#"
pub mod utils {
    pub fn helper() {}
}
"#;
    let result = RustExtractor::extract("src/lib.rs", source);

    let modules: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Module).collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "utils");
}

#[test]
fn test_extract_derive_macros() {
    let source = r#"
#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub name: String,
}
"#;
    let result = RustExtractor::extract("src/config.rs", source);

    let derives: Vec<_> = result.edges.iter().filter(|e| e.kind == EdgeKind::DerivesMacro).collect();
    assert!(!derives.is_empty(), "should have derives_macro edges");
}

#[test]
fn test_file_node_is_root() {
    let source = "fn main() {}";
    let result = RustExtractor::extract("src/main.rs", source);

    let files: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "src/main.rs");
}

#[test]
fn test_qualified_names() {
    let source = r#"
mod server {
    pub fn handle_request() {}
}
"#;
    let result = RustExtractor::extract("src/lib.rs", source);

    let fns: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
    assert_eq!(fns.len(), 1);
    assert!(fns[0].qualified_name.contains("server"));
    assert!(fns[0].qualified_name.contains("handle_request"));
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --test extraction_test
```

Expected: FAIL — module `codegraph::extraction` not found.

**Step 3: Implement the Rust extractor**

Create `src/extraction/mod.rs`:

```rust
mod rust_extractor;

pub use rust_extractor::RustExtractor;
```

Create `src/extraction/rust_extractor.rs` — This is the core AST extraction module. It uses `tree-sitter-rust` to parse Rust source and emit nodes and edges. The implementation should:

1. Parse source with tree-sitter
2. Create a file node as root
3. Walk the AST recursively with `visit_node()`
4. Maintain a `node_stack` for parent context and qualified names
5. For each relevant AST node type, extract a `Node` with metadata
6. Emit `Contains` edges from parent to child automatically
7. Emit unresolved references for call sites, use declarations
8. Extract docstrings from preceding comment nodes
9. Extract signatures from function/method declarations
10. Detect visibility from `visibility_modifier` nodes
11. Detect async functions
12. Extract derive macro attributes

Key tree-sitter-rust node types to handle:
- `function_item` → Function or Method (if inside impl)
- `struct_item` → Struct
- `enum_item` → Enum
- `enum_variant` → EnumVariant
- `trait_item` → Trait
- `impl_item` → Impl
- `use_declaration` → Use
- `const_item` → Const
- `static_item` → Static
- `type_item` → TypeAlias
- `field_declaration` → Field
- `mod_item` → Module
- `call_expression` → unresolved Calls ref
- `macro_invocation` → unresolved Calls ref
- `attribute_item` with `derive` → DerivesMacro edges

**Step 4: Update lib.rs**

```rust
pub mod config;
pub mod db;
pub mod errors;
pub mod extraction;
pub mod types;
```

**Step 5: Run tests to verify they pass**

```bash
cargo test --test extraction_test
```

Expected: All PASS.

**Step 6: Commit**

```bash
git add src/extraction/ src/lib.rs tests/extraction_test.rs
git commit -m "feat: add tree-sitter Rust extraction with full AST node coverage"
```

---

## Task 5: Reference Resolution

**Files:**
- Create: `src/resolution/mod.rs`
- Create: `src/resolution/imports.rs`
- Create: `src/resolution/names.rs`
- Modify: `src/lib.rs`
- Test: `tests/resolution_test.rs`

**Step 1: Write the failing test**

Create `tests/resolution_test.rs`:

```rust
use codegraph::db::Database;
use codegraph::resolution::ReferenceResolver;
use codegraph::types::*;
use tempfile::TempDir;

fn setup_db_with_nodes() -> (TempDir, Database) {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();

    // Insert a function that is called
    let callee = Node {
        id: generate_node_id("src/utils.rs", NodeKind::Function, "helper", 1),
        kind: NodeKind::Function,
        name: "helper".to_string(),
        qualified_name: "src/utils.rs::helper".to_string(),
        file_path: "src/utils.rs".to_string(),
        start_line: 1, end_line: 5,
        start_column: 0, end_column: 1,
        signature: Some("fn helper() -> i32".to_string()),
        docstring: None,
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };

    // Insert the caller
    let caller = Node {
        id: generate_node_id("src/main.rs", NodeKind::Function, "main", 1),
        kind: NodeKind::Function,
        name: "main".to_string(),
        qualified_name: "src/main.rs::main".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 1, end_line: 5,
        start_column: 0, end_column: 1,
        signature: Some("fn main()".to_string()),
        docstring: None,
        visibility: Visibility::Private,
        is_async: false, updated_at: 0,
    };

    db.insert_node(&callee).unwrap();
    db.insert_node(&caller).unwrap();

    (dir, db)
}

#[test]
fn test_resolve_exact_name_match() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let uref = UnresolvedRef {
        from_node_id: generate_node_id("src/main.rs", NodeKind::Function, "main", 1),
        reference_name: "helper".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 3,
        column: 12,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_some(), "should resolve 'helper' by exact name");
    let resolved = result.unwrap();
    assert!(resolved.confidence >= 0.7);
}

#[test]
fn test_resolve_all() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let refs = vec![
        UnresolvedRef {
            from_node_id: generate_node_id("src/main.rs", NodeKind::Function, "main", 1),
            reference_name: "helper".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3, column: 12,
            file_path: "src/main.rs".to_string(),
        },
    ];

    let result = resolver.resolve_all(&refs);
    assert_eq!(result.total, 1);
    assert_eq!(result.resolved_count, 1);
    assert_eq!(result.resolved.len(), 1);
}

#[test]
fn test_unresolvable_reference() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let uref = UnresolvedRef {
        from_node_id: "function:caller".to_string(),
        reference_name: "nonexistent_function".to_string(),
        reference_kind: EdgeKind::Calls,
        line: 5, column: 8,
        file_path: "src/main.rs".to_string(),
    };

    let result = resolver.resolve_one(&uref);
    assert!(result.is_none(), "should not resolve nonexistent function");
}

#[test]
fn test_creates_edges_from_resolved() {
    let (_dir, db) = setup_db_with_nodes();
    let resolver = ReferenceResolver::new(&db);

    let resolved = ResolvedRef {
        original: UnresolvedRef {
            from_node_id: generate_node_id("src/main.rs", NodeKind::Function, "main", 1),
            reference_name: "helper".to_string(),
            reference_kind: EdgeKind::Calls,
            line: 3, column: 12,
            file_path: "src/main.rs".to_string(),
        },
        target_node_id: generate_node_id("src/utils.rs", NodeKind::Function, "helper", 1),
        confidence: 0.9,
        resolved_by: "exact-match".to_string(),
    };

    let edges = resolver.create_edges(&[resolved]);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, EdgeKind::Calls);
    assert_eq!(edges[0].line, Some(3));
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --test resolution_test
```

**Step 3: Implement resolution module**

Create `src/resolution/mod.rs`, `src/resolution/imports.rs`, `src/resolution/names.rs`.

The resolver should:
1. Build in-memory caches from all nodes (by name, qualified name, kind)
2. For each unresolved ref, try strategies in order:
   - Exact name match (confidence 0.9 for single match, 0.7 for multiple with scoring)
   - Qualified name match (confidence 0.95)
   - Use-path resolution (follow `use crate::` paths)
3. Score candidates: same file +100, same module +50, exported +10
4. Create edges from resolved references

**Step 4: Update lib.rs**

```rust
pub mod config;
pub mod db;
pub mod errors;
pub mod extraction;
pub mod resolution;
pub mod types;
```

**Step 5: Run tests**

```bash
cargo test --test resolution_test
```

**Step 6: Commit**

```bash
git add src/resolution/ src/lib.rs tests/resolution_test.rs
git commit -m "feat: add reference resolution with name and import-based matching"
```

---

## Task 6: Graph Traversal & Queries

**Files:**
- Create: `src/graph/mod.rs`
- Create: `src/graph/traversal.rs`
- Create: `src/graph/queries.rs`
- Modify: `src/lib.rs`
- Test: `tests/graph_test.rs`

**Step 1: Write the failing test**

Create `tests/graph_test.rs`:

```rust
use codegraph::db::Database;
use codegraph::graph::{GraphTraverser, GraphQueryManager};
use codegraph::types::*;
use tempfile::TempDir;

fn setup_call_graph() -> (TempDir, Database) {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();

    // Create: main -> process -> validate -> check
    let nodes = vec!["main", "process", "validate", "check"];
    for (i, name) in nodes.iter().enumerate() {
        let node = Node {
            id: format!("function:{}", name),
            kind: NodeKind::Function,
            name: name.to_string(),
            qualified_name: format!("src/lib.rs::{}", name),
            file_path: "src/lib.rs".to_string(),
            start_line: (i as u32) * 10 + 1,
            end_line: (i as u32) * 10 + 9,
            start_column: 0, end_column: 1,
            signature: Some(format!("fn {}()", name)),
            docstring: None,
            visibility: Visibility::Pub,
            is_async: false, updated_at: 0,
        };
        db.insert_node(&node).unwrap();
    }

    let call_edges = vec![
        ("main", "process"),
        ("process", "validate"),
        ("validate", "check"),
    ];
    for (source, target) in call_edges {
        let edge = Edge {
            source: format!("function:{}", source),
            target: format!("function:{}", target),
            kind: EdgeKind::Calls,
            line: None,
        };
        db.insert_edge(&edge).unwrap();
    }

    (dir, db)
}

#[test]
fn test_get_callers() {
    let (_dir, db) = setup_call_graph();
    let traverser = GraphTraverser::new(&db);

    let callers = traverser.get_callers("function:process", 1).unwrap();
    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0].0.name, "main");
}

#[test]
fn test_get_callees() {
    let (_dir, db) = setup_call_graph();
    let traverser = GraphTraverser::new(&db);

    let callees = traverser.get_callees("function:process", 1).unwrap();
    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].0.name, "validate");
}

#[test]
fn test_impact_radius() {
    let (_dir, db) = setup_call_graph();
    let traverser = GraphTraverser::new(&db);

    // Impact of "check" should include validate, process, main
    let impact = traverser.get_impact_radius("function:check", 10).unwrap();
    assert!(impact.nodes.len() >= 3, "impact should include transitive callers");
}

#[test]
fn test_call_graph_bidirectional() {
    let (_dir, db) = setup_call_graph();
    let traverser = GraphTraverser::new(&db);

    let graph = traverser.get_call_graph("function:process", 2).unwrap();
    // Should include main (caller) and validate (callee)
    assert!(graph.nodes.len() >= 3);
}

#[test]
fn test_bfs_traversal_with_depth_limit() {
    let (_dir, db) = setup_call_graph();
    let traverser = GraphTraverser::new(&db);

    let opts = TraversalOptions {
        max_depth: 1,
        direction: TraversalDirection::Outgoing,
        ..Default::default()
    };

    let subgraph = traverser.traverse_bfs("function:main", &opts).unwrap();
    // Depth 1: main + process only
    assert!(subgraph.nodes.len() <= 2);
}

#[test]
fn test_find_dead_code() {
    let (_dir, db) = setup_call_graph();
    let qm = GraphQueryManager::new(&db);

    // Add an isolated function (no incoming edges)
    let orphan = Node {
        id: "function:orphan".to_string(),
        kind: NodeKind::Function,
        name: "orphan".to_string(),
        qualified_name: "src/lib.rs::orphan".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 50, end_line: 55,
        start_column: 0, end_column: 1,
        signature: None, docstring: None,
        visibility: Visibility::Private, // private, no callers = dead code
        is_async: false, updated_at: 0,
    };
    db.insert_node(&orphan).unwrap();

    let dead = qm.find_dead_code(&[NodeKind::Function]).unwrap();
    let dead_names: Vec<_> = dead.iter().map(|n| n.name.as_str()).collect();
    assert!(dead_names.contains(&"orphan"), "orphan should be dead code");
    // main has no incoming edges but is named "main" — should be excluded
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --test graph_test
```

**Step 3: Implement graph module**

Implement `src/graph/traversal.rs` with `GraphTraverser` providing BFS/DFS traversal, callers/callees, impact radius, call graph, type hierarchy, and path finding.

Implement `src/graph/queries.rs` with `GraphQueryManager` providing dead code detection, node metrics, file dependencies, and circular dependency detection.

**Step 4: Run tests**

```bash
cargo test --test graph_test
```

**Step 5: Commit**

```bash
git add src/graph/ src/lib.rs tests/graph_test.rs
git commit -m "feat: add graph traversal with BFS/DFS, impact analysis, and dead code detection"
```

---

## Task 7: CLI Interface

**Files:**
- Modify: `src/main.rs`
- Create: `src/codegraph.rs` (main orchestrator)
- Modify: `src/lib.rs`
- Test: manual CLI testing

**Step 1: Implement the CodeGraph orchestrator**

Create `src/codegraph.rs` — the central orchestrator that wires all subsystems together:

```rust
pub struct CodeGraph {
    db: Database,
    config: CodeGraphConfig,
    project_root: PathBuf,
}
```

Methods:
- `init(project_root)` — create `.codegraph/`, init DB, save config
- `open(project_root)` — open existing project
- `index_all()` — scan files, extract, resolve, store
- `sync()` — incremental update via content hashing
- `search(query, limit)` — FTS5 search
- `get_stats()` — graph statistics
- All graph query delegations (callers, callees, impact, etc.)

**Step 2: Implement CLI with clap**

Modify `src/main.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "codegraph", about = "Code intelligence for Rust codebases")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init { path: Option<String> },
    Index { path: Option<String>, #[arg(short, long)] force: bool },
    Sync { path: Option<String> },
    Status { path: Option<String>, #[arg(short, long)] json: bool },
    Query { search: String, #[arg(short, long)] path: Option<String>, #[arg(short, long, default_value = "10")] limit: usize },
    Context { task: String, #[arg(short, long)] path: Option<String> },
    Serve { #[arg(short, long)] path: Option<String> },
}
```

**Step 3: Test CLI manually**

```bash
cargo run -- init .
cargo run -- index .
cargo run -- status .
cargo run -- query "main"
```

**Step 4: Commit**

```bash
git add src/main.rs src/codegraph.rs src/lib.rs
git commit -m "feat: add CLI with init, index, sync, status, query, context, serve commands"
```

---

## Task 8: Context Builder

**Files:**
- Create: `src/context/mod.rs`
- Create: `src/context/builder.rs`
- Create: `src/context/formatter.rs`
- Modify: `src/lib.rs`
- Test: `tests/context_test.rs`

**Step 1: Write the failing test**

Create `tests/context_test.rs`:

```rust
use codegraph::context::*;
use codegraph::db::Database;
use codegraph::graph::GraphTraverser;
use codegraph::types::*;
use tempfile::TempDir;

fn setup_context_db() -> (TempDir, Database) {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();

    let node = Node {
        id: "function:process_request".to_string(),
        kind: NodeKind::Function,
        name: "process_request".to_string(),
        qualified_name: "src/server.rs::process_request".to_string(),
        file_path: "src/server.rs".to_string(),
        start_line: 10, end_line: 25,
        start_column: 0, end_column: 1,
        signature: Some("pub fn process_request(req: Request) -> Response".to_string()),
        docstring: Some("Handles incoming HTTP requests".to_string()),
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };
    db.insert_node(&node).unwrap();
    (dir, db)
}

#[test]
fn test_extract_symbols_from_query() {
    let symbols = extract_symbols_from_query("fix the process_request function");
    assert!(symbols.contains(&"process_request".to_string()));
}

#[test]
fn test_extract_camel_case_symbols() {
    let symbols = extract_symbols_from_query("update UserService handler");
    assert!(symbols.contains(&"UserService".to_string()));
}

#[test]
fn test_format_context_markdown() {
    let context = TaskContext {
        query: "test query".to_string(),
        summary: "Test summary".to_string(),
        subgraph: Subgraph::default(),
        entry_points: vec![],
        code_blocks: vec![],
        related_files: vec![],
    };

    let md = format_context_as_markdown(&context);
    assert!(md.contains("## Code Context"));
    assert!(md.contains("test query"));
}

#[test]
fn test_format_context_json() {
    let context = TaskContext {
        query: "test".to_string(),
        summary: "Summary".to_string(),
        subgraph: Subgraph::default(),
        entry_points: vec![],
        code_blocks: vec![],
        related_files: vec![],
    };

    let json = format_context_as_json(&context);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["query"], "test");
}
```

**Step 2: Implement context module**

The `ContextBuilder` should:
1. Extract symbol names from natural language queries (CamelCase, snake_case patterns)
2. Search for matching nodes via FTS5 and exact name lookup
3. Expand graph around entry points using BFS
4. Extract code blocks by reading source files
5. Format output as markdown or JSON

**Step 3: Run tests, commit**

```bash
cargo test --test context_test
git add src/context/ tests/context_test.rs
git commit -m "feat: add context builder with symbol extraction and markdown/JSON formatting"
```

---

## Task 9: Vector Embeddings

**Files:**
- Create: `src/vectors/mod.rs`
- Create: `src/vectors/embedder.rs`
- Create: `src/vectors/search.rs`
- Modify: `src/lib.rs`
- Test: `tests/vectors_test.rs`

**Step 1: Write the failing test**

Create `tests/vectors_test.rs`:

```rust
use codegraph::vectors::*;
use codegraph::db::Database;
use codegraph::types::*;
use tempfile::TempDir;

#[test]
fn test_cosine_similarity_identical() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 1e-6);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 1e-6);
}

#[test]
fn test_store_and_retrieve_vector() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();

    // Must have a node to reference
    let node = Node {
        id: "function:test_fn".to_string(),
        kind: NodeKind::Function,
        name: "test_fn".to_string(),
        qualified_name: "test_fn".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1, end_line: 5,
        start_column: 0, end_column: 1,
        signature: None, docstring: None,
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };
    db.insert_node(&node).unwrap();

    let embedding: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    store_vector(&db, "function:test_fn", &embedding, "test-model").unwrap();

    let retrieved = get_vector(&db, "function:test_fn").unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.len(), 5);
    assert!((retrieved[0] - 0.1).abs() < 1e-6);
}

#[test]
fn test_brute_force_search() {
    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();

    // Insert nodes and embeddings
    for i in 0..5 {
        let node = Node {
            id: format!("function:fn_{}", i),
            kind: NodeKind::Function,
            name: format!("fn_{}", i),
            qualified_name: format!("fn_{}", i),
            file_path: "src/lib.rs".to_string(),
            start_line: i + 1, end_line: i + 5,
            start_column: 0, end_column: 1,
            signature: None, docstring: None,
            visibility: Visibility::Pub,
            is_async: false, updated_at: 0,
        };
        db.insert_node(&node).unwrap();

        let mut embedding = vec![0.0f32; 5];
        embedding[i as usize] = 1.0; // one-hot encoding
        store_vector(&db, &format!("function:fn_{}", i), &embedding, "test").unwrap();
    }

    // Search for vector close to fn_2
    let query = vec![0.0, 0.0, 0.9, 0.1, 0.0];
    let results = brute_force_search(&db, &query, 3).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, "function:fn_2"); // closest match
}

#[test]
fn test_create_node_text() {
    let node = Node {
        id: "function:test".to_string(),
        kind: NodeKind::Function,
        name: "process_data".to_string(),
        qualified_name: "src/lib.rs::process_data".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1, end_line: 10,
        start_column: 0, end_column: 1,
        signature: Some("fn process_data(input: &str) -> Result<Data>".to_string()),
        docstring: Some("Processes raw data input".to_string()),
        visibility: Visibility::Pub,
        is_async: false, updated_at: 0,
    };

    let text = create_node_text(&node);
    assert!(text.contains("process_data"));
    assert!(text.contains("function"));
    assert!(text.contains("Processes raw data"));
}
```

**Step 2: Implement vectors module**

The vectors module should provide:
- `cosine_similarity(a, b)` — compute cosine similarity
- `store_vector(db, node_id, embedding, model)` — store as BLOB
- `get_vector(db, node_id)` — retrieve and decode BLOB
- `brute_force_search(db, query, limit)` — load all vectors, compute similarity, return top-k
- `create_node_text(node)` — create searchable text representation
- `TextEmbedder` — wrapper around `ort` for ONNX inference (initialize with model path, embed text, embed query)

For the ONNX embedder, use the `ort` crate with `nomic-embed-text-v1.5` model. Add "search_query: " / "search_document: " prefixes per nomic model requirements.

**Step 3: Run tests, commit**

```bash
cargo test --test vectors_test
git add src/vectors/ tests/vectors_test.rs
git commit -m "feat: add vector embeddings with brute-force cosine similarity search"
```

---

## Task 10: MCP Server

**Files:**
- Create: `src/mcp/mod.rs`
- Create: `src/mcp/server.rs`
- Create: `src/mcp/tools.rs`
- Create: `src/mcp/transport.rs`
- Modify: `src/lib.rs`
- Test: `tests/mcp_test.rs`

**Step 1: Write the failing test**

Create `tests/mcp_test.rs`:

```rust
use codegraph::mcp::transport::*;
use codegraph::mcp::tools::*;
use serde_json::json;

#[test]
fn test_parse_jsonrpc_request() {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    let request: JsonRpcRequest = serde_json::from_value(msg).unwrap();
    assert_eq!(request.method, "tools/list");
    assert_eq!(request.id, serde_json::Value::Number(1.into()));
}

#[test]
fn test_tool_definitions() {
    let tools = get_tool_definitions();
    assert!(!tools.is_empty());

    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"codegraph_search"));
    assert!(tool_names.contains(&"codegraph_context"));
    assert!(tool_names.contains(&"codegraph_callers"));
    assert!(tool_names.contains(&"codegraph_callees"));
    assert!(tool_names.contains(&"codegraph_impact"));
    assert!(tool_names.contains(&"codegraph_node"));
    assert!(tool_names.contains(&"codegraph_status"));
}

#[test]
fn test_serialize_jsonrpc_response() {
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(1.into()),
        result: Some(json!({"tools": []})),
        error: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
}

#[test]
fn test_error_response() {
    let response = JsonRpcResponse::error(
        serde_json::Value::Number(1.into()),
        ErrorCode::MethodNotFound,
        "Method not found".to_string(),
    );

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("-32601"));
}
```

**Step 2: Implement MCP module**

The MCP server should:
- Read JSON-RPC 2.0 messages from stdin line-by-line
- Handle `initialize`, `tools/list`, `tools/call`, `ping` requests
- Expose 7 tools: search, context, callers, callees, impact, node, status
- Format output for minimal token usage
- Truncate responses > 15000 chars
- Use `tokio` for async I/O

**Step 3: Run tests, commit**

```bash
cargo test --test mcp_test
git add src/mcp/ tests/mcp_test.rs
git commit -m "feat: add MCP server with JSON-RPC transport and tool handlers"
```

---

## Task 11: Incremental Sync

**Files:**
- Create: `src/sync.rs`
- Modify: `src/codegraph.rs`
- Modify: `src/lib.rs`
- Test: `tests/sync_test.rs`

**Step 1: Write the failing test**

Create `tests/sync_test.rs`:

```rust
use codegraph::sync::*;

#[test]
fn test_content_hash_deterministic() {
    let content = "fn main() {}";
    let hash1 = content_hash(content);
    let hash2 = content_hash(content);
    assert_eq!(hash1, hash2);
}

#[test]
fn test_content_hash_different_for_different_content() {
    let hash1 = content_hash("fn main() {}");
    let hash2 = content_hash("fn main() { println!(\"hello\"); }");
    assert_ne!(hash1, hash2);
}

#[test]
fn test_detect_changed_files() {
    // Test that changed files are detected by comparing stored vs current hashes
    use codegraph::db::Database;
    use codegraph::types::FileRecord;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let db = Database::initialize(&dir.path().join("test.db")).unwrap();

    // Store a file with hash "old_hash"
    db.upsert_file(&FileRecord {
        path: "src/main.rs".to_string(),
        content_hash: "old_hash".to_string(),
        size: 100,
        modified_at: 1000,
        indexed_at: 1001,
        node_count: 5,
    }).unwrap();

    // Current hash is different
    let current_hashes = vec![
        ("src/main.rs".to_string(), "new_hash".to_string()),
    ];

    let stale = find_stale_files(&db, &current_hashes).unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0], "src/main.rs");
}
```

**Step 2: Implement sync module**

The sync module should:
- `content_hash(content)` — SHA256 hash of file content
- `find_stale_files(db, current_hashes)` — compare stored vs current content hashes
- `find_new_files(db, current_files)` — files not yet in database
- `find_removed_files(db, current_files)` — files in DB but not on disk

**Step 3: Run tests, commit**

```bash
cargo test --test sync_test
git add src/sync.rs tests/sync_test.rs
git commit -m "feat: add incremental sync with content hash change detection"
```

---

## Task 12: Integration Test & Polish

**Files:**
- Create: `tests/integration_test.rs`
- Modify: various files for fixes

**Step 1: Write end-to-end integration test**

Create `tests/integration_test.rs`:

```rust
use codegraph::codegraph::CodeGraph;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_full_pipeline() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    // Create a small Rust project
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), r#"
use crate::utils::helper;

mod utils;

fn main() {
    let result = helper();
    println!("{}", result);
}
"#).unwrap();

    fs::write(project.join("src/utils.rs"), r#"
/// Returns a greeting string.
pub fn helper() -> String {
    format_greeting("world")
}

fn format_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#).unwrap();

    // Init
    let cg = CodeGraph::init(project).unwrap();

    // Index
    let index_result = cg.index_all().unwrap();
    assert!(index_result.file_count > 0);
    assert!(index_result.node_count > 0);

    // Stats
    let stats = cg.get_stats().unwrap();
    assert!(stats.node_count > 0);
    assert!(stats.file_count >= 2);

    // Search
    let results = cg.search("helper", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.node.name == "helper"));

    // Status
    let stats = cg.get_stats().unwrap();
    assert!(stats.edge_count > 0); // should have contains + calls edges
}

#[test]
fn test_incremental_sync() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "pub fn original() {}").unwrap();

    let cg = CodeGraph::init(project).unwrap();
    cg.index_all().unwrap();

    let initial_stats = cg.get_stats().unwrap();

    // Modify file
    fs::write(project.join("src/lib.rs"), "pub fn modified() {}\npub fn added() {}").unwrap();

    // Sync
    let sync_result = cg.sync().unwrap();
    assert!(sync_result.files_modified > 0 || sync_result.files_added > 0);

    let new_stats = cg.get_stats().unwrap();
    // Should have the new function
    let results = cg.search("modified", 10).unwrap();
    assert!(!results.is_empty());
}
```

**Step 2: Run integration tests**

```bash
cargo test --test integration_test
```

**Step 3: Fix any issues found**

**Step 4: Run full test suite**

```bash
cargo test
```

**Step 5: Run clippy and fix warnings**

```bash
cargo clippy --all-targets
cargo fmt --all
```

**Step 6: Commit**

```bash
git add .
git commit -m "feat: add integration tests and polish for full pipeline"
```

---

## Summary

| Task | Module | Estimated Complexity |
|------|--------|---------------------|
| 1 | Project scaffold, types, errors | Low |
| 2 | Configuration | Low |
| 3 | SQLite database layer | Medium |
| 4 | Tree-sitter Rust extraction | High |
| 5 | Reference resolution | Medium |
| 6 | Graph traversal & queries | Medium |
| 7 | CLI interface | Low |
| 8 | Context builder | Medium |
| 9 | Vector embeddings | Medium |
| 10 | MCP server | Medium |
| 11 | Incremental sync | Low |
| 12 | Integration tests & polish | Low |
