use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Kinds of nodes in the code graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[allow(clippy::should_implement_trait)]
impl NodeKind {
    /// Returns the string representation of this node kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Module => "module",
            NodeKind::Struct => "struct",
            NodeKind::Enum => "enum",
            NodeKind::EnumVariant => "enum_variant",
            NodeKind::Trait => "trait",
            NodeKind::Function => "function",
            NodeKind::Method => "method",
            NodeKind::Impl => "impl",
            NodeKind::Const => "const",
            NodeKind::Static => "static",
            NodeKind::TypeAlias => "type_alias",
            NodeKind::Field => "field",
            NodeKind::Macro => "macro",
            NodeKind::Use => "use",
        }
    }

    /// Parses a string into a `NodeKind`, returning `None` for unrecognized values.
    pub fn from_str(s: &str) -> Option<NodeKind> {
        match s {
            "file" => Some(NodeKind::File),
            "module" => Some(NodeKind::Module),
            "struct" => Some(NodeKind::Struct),
            "enum" => Some(NodeKind::Enum),
            "enum_variant" => Some(NodeKind::EnumVariant),
            "trait" => Some(NodeKind::Trait),
            "function" => Some(NodeKind::Function),
            "method" => Some(NodeKind::Method),
            "impl" => Some(NodeKind::Impl),
            "const" => Some(NodeKind::Const),
            "static" => Some(NodeKind::Static),
            "type_alias" => Some(NodeKind::TypeAlias),
            "field" => Some(NodeKind::Field),
            "macro" => Some(NodeKind::Macro),
            "use" => Some(NodeKind::Use),
            _ => None,
        }
    }
}

/// Kinds of edges in the code graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Contains,
    Calls,
    Uses,
    Implements,
    TypeOf,
    Returns,
    DerivesMacro,
}

#[allow(clippy::should_implement_trait)]
impl EdgeKind {
    /// Returns the string representation of this edge kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Calls => "calls",
            EdgeKind::Uses => "uses",
            EdgeKind::Implements => "implements",
            EdgeKind::TypeOf => "type_of",
            EdgeKind::Returns => "returns",
            EdgeKind::DerivesMacro => "derives_macro",
        }
    }

    /// Parses a string into an `EdgeKind`, returning `None` for unrecognized values.
    pub fn from_str(s: &str) -> Option<EdgeKind> {
        match s {
            "contains" => Some(EdgeKind::Contains),
            "calls" => Some(EdgeKind::Calls),
            "uses" => Some(EdgeKind::Uses),
            "implements" => Some(EdgeKind::Implements),
            "type_of" => Some(EdgeKind::TypeOf),
            "returns" => Some(EdgeKind::Returns),
            "derives_macro" => Some(EdgeKind::DerivesMacro),
            _ => None,
        }
    }
}

/// Visibility of a code item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Visibility {
    Pub,
    PubCrate,
    PubSuper,
    #[default]
    Private,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pub => "public",
            Self::PubCrate => "pub_crate",
            Self::PubSuper => "pub_super",
            Self::Private => "private",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "public" | "pub" => Some(Self::Pub),
            "pub_crate" => Some(Self::PubCrate),
            "pub_super" => Some(Self::PubSuper),
            "private" => Some(Self::Private),
            _ => None,
        }
    }
}

/// A node in the code graph representing a code entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub updated_at: u64,
}

/// An edge in the code graph representing a relationship between nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub line: Option<u32>,
}

/// Record tracking an indexed file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub content_hash: String,
    pub size: u64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub node_count: u32,
}

/// An unresolved reference found during parsing, to be resolved later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub from_node_id: String,
    pub reference_name: String,
    pub reference_kind: EdgeKind,
    pub line: u32,
    pub column: u32,
    pub file_path: String,
}

/// Result of extracting code entities from a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// A subgraph containing a subset of nodes and edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub roots: Vec<String>,
}

/// A search result pairing a node with a relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
}

/// Direction for graph traversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraversalDirection {
    Outgoing,
    Incoming,
    Both,
}

/// Options controlling graph traversal behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalOptions {
    pub max_depth: u32,
    pub edge_kinds: Option<Vec<EdgeKind>>,
    pub node_kinds: Option<Vec<NodeKind>>,
    pub direction: TraversalDirection,
    pub limit: u32,
    pub include_start: bool,
}

impl Default for TraversalOptions {
    fn default() -> Self {
        TraversalOptions {
            max_depth: 3,
            edge_kinds: None,
            node_kinds: None,
            direction: TraversalDirection::Outgoing,
            limit: 100,
            include_start: true,
        }
    }
}

/// Statistics about the code graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub file_count: u64,
    pub nodes_by_kind: HashMap<String, u64>,
    pub edges_by_kind: HashMap<String, u64>,
    pub db_size_bytes: u64,
    pub last_updated: u64,
}

/// Options for building an LLM context from the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        BuildContextOptions {
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

/// Output format for CLI results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Markdown,
    Json,
}

/// Context assembled for a task, combining graph data with code blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub query: String,
    pub summary: String,
    pub subgraph: Subgraph,
    pub entry_points: Vec<Node>,
    pub code_blocks: Vec<CodeBlock>,
    pub related_files: Vec<String>,
}

/// A block of source code extracted from a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub content: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub node_id: Option<String>,
}

/// Generates a deterministic node ID from file path, kind, name, and line number.
///
/// The ID format is `"kind:32hexchars"` where the hex portion is the first 32
/// characters of the SHA-256 hash of the input components.
pub fn generate_node_id(file_path: &str, kind: &NodeKind, name: &str, line: u32) -> String {
    let input = format!("{}:{}:{}:{}", file_path, kind.as_str(), name, line);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    let hex_str = hex::encode(hash);
    format!("{}:{}", kind.as_str(), &hex_str[..32])
}

/// Result of resolving references in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionResult {
    pub resolved: Vec<ResolvedRef>,
    pub unresolved: Vec<UnresolvedRef>,
    pub total: usize,
    pub resolved_count: usize,
}

/// A reference that has been resolved to a target node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRef {
    pub original: UnresolvedRef,
    pub target_node_id: String,
    pub confidence: f64,
    pub resolved_by: String,
}
