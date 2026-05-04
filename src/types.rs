use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

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
    // Scala-specific
    ScalaObject,
    CaseClass,
    ScalaPackage,
    ValField,
    VarField,
    // Shared
    GenericParam,
    // TypeScript/JavaScript-specific
    ArrowFunction,
    Decorator,
    Export,
    Namespace,
    // C/C++-specific
    Union,
    Typedef,
    Include,
    PreprocessorDef,
    Template,
    // Kotlin-specific
    DataClass,
    SealedClass,
    CompanionObject,
    KotlinObject,
    KotlinPackage,
    Property,
    // Dart-specific
    Mixin,
    Extension,
    Library,
    // C#-specific
    Delegate,
    Event,
    Record,
    CSharpProperty,
    // Pascal-specific
    Procedure,
    PascalUnit,
    PascalProgram,
    PascalRecord,
    // Protobuf-specific
    #[cfg(feature = "lang-protobuf")]
    ProtoMessage,
    #[cfg(feature = "lang-protobuf")]
    ProtoService,
    #[cfg(feature = "lang-protobuf")]
    ProtoRpc,
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
            NodeKind::ScalaObject => "object",
            NodeKind::CaseClass => "case_class",
            NodeKind::ScalaPackage => "scala_package",
            NodeKind::ValField => "val",
            NodeKind::VarField => "var",
            NodeKind::GenericParam => "generic_param",
            NodeKind::ArrowFunction => "arrow_function",
            NodeKind::Decorator => "decorator",
            NodeKind::Export => "export",
            NodeKind::Namespace => "namespace",
            NodeKind::Union => "union",
            NodeKind::Typedef => "typedef",
            NodeKind::Include => "include",
            NodeKind::PreprocessorDef => "preprocessor_def",
            NodeKind::Template => "template",
            NodeKind::DataClass => "data_class",
            NodeKind::SealedClass => "sealed_class",
            NodeKind::CompanionObject => "companion_object",
            NodeKind::KotlinObject => "kotlin_object",
            NodeKind::KotlinPackage => "kotlin_package",
            NodeKind::Property => "property",
            NodeKind::Mixin => "mixin",
            NodeKind::Extension => "extension",
            NodeKind::Library => "library",
            NodeKind::Delegate => "delegate",
            NodeKind::Event => "event",
            NodeKind::Record => "record",
            NodeKind::CSharpProperty => "csharp_property",
            NodeKind::Procedure => "procedure",
            NodeKind::PascalUnit => "pascal_unit",
            NodeKind::PascalProgram => "pascal_program",
            NodeKind::PascalRecord => "pascal_record",
            #[cfg(feature = "lang-protobuf")]
            NodeKind::ProtoMessage => "proto_message",
            #[cfg(feature = "lang-protobuf")]
            NodeKind::ProtoService => "proto_service",
            #[cfg(feature = "lang-protobuf")]
            NodeKind::ProtoRpc => "proto_rpc",
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
            "object" => Some(NodeKind::ScalaObject),
            "case_class" => Some(NodeKind::CaseClass),
            "scala_package" => Some(NodeKind::ScalaPackage),
            "val" => Some(NodeKind::ValField),
            "var" => Some(NodeKind::VarField),
            "generic_param" => Some(NodeKind::GenericParam),
            "arrow_function" => Some(NodeKind::ArrowFunction),
            "decorator" => Some(NodeKind::Decorator),
            "export" => Some(NodeKind::Export),
            "namespace" => Some(NodeKind::Namespace),
            "union" => Some(NodeKind::Union),
            "typedef" => Some(NodeKind::Typedef),
            "include" => Some(NodeKind::Include),
            "preprocessor_def" => Some(NodeKind::PreprocessorDef),
            "template" => Some(NodeKind::Template),
            "data_class" => Some(NodeKind::DataClass),
            "sealed_class" => Some(NodeKind::SealedClass),
            "companion_object" => Some(NodeKind::CompanionObject),
            "kotlin_object" => Some(NodeKind::KotlinObject),
            "kotlin_package" => Some(NodeKind::KotlinPackage),
            "property" => Some(NodeKind::Property),
            "mixin" => Some(NodeKind::Mixin),
            "extension" => Some(NodeKind::Extension),
            "library" => Some(NodeKind::Library),
            "delegate" => Some(NodeKind::Delegate),
            "event" => Some(NodeKind::Event),
            "record" => Some(NodeKind::Record),
            "csharp_property" => Some(NodeKind::CSharpProperty),
            "procedure" => Some(NodeKind::Procedure),
            "pascal_unit" => Some(NodeKind::PascalUnit),
            "pascal_program" => Some(NodeKind::PascalProgram),
            "pascal_record" => Some(NodeKind::PascalRecord),
            #[cfg(feature = "lang-protobuf")]
            "proto_message" => Some(NodeKind::ProtoMessage),
            #[cfg(feature = "lang-protobuf")]
            "proto_service" => Some(NodeKind::ProtoService),
            #[cfg(feature = "lang-protobuf")]
            "proto_rpc" => Some(NodeKind::ProtoRpc),
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
    Extends,
    Annotates,
    Receives,
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
            EdgeKind::Extends => "extends",
            EdgeKind::Annotates => "annotates",
            EdgeKind::Receives => "receives",
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
            "extends" => Some(EdgeKind::Extends),
            "annotates" => Some(EdgeKind::Annotates),
            "receives" => Some(EdgeKind::Receives),
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
    /// First line of the leading doc-comment / attribute block, or `start_line`
    /// when no such block exists. Lets refactoring tools select the full span
    /// of an item (delete, move, rewrite) without losing its documentation.
    pub attrs_start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub visibility: Visibility,
    pub is_async: bool,
    /// Number of branching statements (if, match/switch arms, ternary).
    /// 0 for non-function nodes. Cyclomatic complexity = branches + 1.
    pub branches: u32,
    /// Number of loop constructs (for, while, loop).
    pub loops: u32,
    /// Number of early-exit statements (return, break, continue, throw).
    pub returns: u32,
    /// Maximum brace nesting depth within the function body.
    pub max_nesting: u32,
    /// Number of unsafe blocks/statements within the function body.
    pub unsafe_blocks: u32,
    /// Number of unchecked/force-unwrap calls (e.g. `.unwrap()`, `!!`, `.get()` on Optional).
    pub unchecked_calls: u32,
    /// Number of assertion calls (e.g. `assert!`, `assertEquals`, `expect`).
    pub assertions: u32,
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

impl ExtractionResult {
    /// Strip nodes with empty names and remove any edges or unresolved refs
    /// that reference their IDs. Tree-sitter can produce empty-name nodes
    /// from complex declarators (especially C/C++); if we skip the node at
    /// insert time but keep its edges, we get FK constraint violations.
    pub fn sanitize(&mut self) {
        let before = self.nodes.len();
        let bad_ids: std::collections::HashSet<String> = self
            .nodes
            .iter()
            .filter(|n| n.name.is_empty())
            .map(|n| n.id.clone())
            .collect();

        if bad_ids.is_empty() {
            return;
        }

        self.nodes.retain(|n| !n.name.is_empty());
        self.edges
            .retain(|e| !bad_ids.contains(&e.source) && !bad_ids.contains(&e.target));
        self.unresolved_refs
            .retain(|r| !bad_ids.contains(&r.from_node_id));

        let removed = before - self.nodes.len();
        if removed > 0 {
            self.errors
                .push(format!("stripped {removed} node(s) with empty names"));
        }
    }
}

/// A subgraph containing a subset of nodes and edges.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Total bytes of all indexed source files.
    pub total_source_bytes: u64,
    /// Number of indexed files per language (e.g. "Rust" -> 42).
    pub files_by_language: HashMap<String, u64>,
    /// Timestamp of the most recent incremental sync (0 if never synced).
    pub last_sync_at: u64,
    /// Timestamp of the most recent full (re)index (0 if never indexed).
    pub last_full_sync_at: u64,
    /// Duration in milliseconds of the most recent sync (0 if unknown).
    pub last_sync_duration_ms: u64,
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
    /// Additional keywords to search for beyond those extracted from the query.
    /// Enables agent-driven synonym expansion (e.g. `"authentication"` → `["login", "session"]`).
    pub extra_keywords: Vec<String>,
    /// Node IDs to exclude from results (for session deduplication across calls).
    pub exclude_node_ids: HashSet<String>,
    /// When true, merge code blocks from the same file whose line ranges are
    /// adjacent or overlapping into a single block.
    pub merge_adjacent: bool,
    /// Maximum symbols from a single file in context results. Prevents one
    /// large file from dominating the output. `None` means no cap (defaults
    /// to `max_nodes`).
    pub max_per_file: Option<usize>,
    /// When set, only nodes whose `file_path` starts with this prefix are
    /// considered as entry points. Graph expansion may still traverse outside
    /// the prefix (traversals are unscoped).
    pub path_prefix: Option<String>,
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
            min_score: 0.0,
            extra_keywords: Vec::new(),
            exclude_node_ids: HashSet::new(),
            merge_adjacent: false,
            max_per_file: None,
            path_prefix: None,
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
    /// IDs of all nodes returned as entry points (pass to next call's `exclude_node_ids` for dedup).
    pub seen_node_ids: Vec<String>,
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
    debug_assert!(
        !name.is_empty(),
        "generate_node_id called with empty name for {file_path}:{line}"
    );
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

/// Result of a single string replacement edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub success: bool,
    pub file_path: String,
    pub matched_str: String,
    pub new_str: String,
    pub message: String,
}

/// Result of a multi-string replacement edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiEditResult {
    pub success: bool,
    pub file_path: String,
    pub applied_count: usize,
    pub message: String,
}

/// Result of an insert-at operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertResult {
    pub success: bool,
    pub file_path: String,
    pub anchor_line: u32,
    pub content: String,
    pub before: bool,
    pub message: String,
}

/// Result of an ast-grep rewrite operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstGrepResult {
    pub success: bool,
    pub file_path: String,
    pub pattern: String,
    pub rewrite: String,
    pub message: String,
}
