//! Static registry of profiled operations.
//!
//! Every measured operation is a variant of one of the enums below. The
//! variant's name is its metric key, so no SQL text, parameter, path or other
//! dynamic string can enter a series key, and the number of series is fixed
//! at compile time. Database query variants are added as call sites migrate
//! to the profiled execution API (`crate::db::Database::profiled_query`); a
//! variant names a query method, not a SQL string.

/// Declares a fieldless operation enum with an `ALL` table and a static,
/// dot-qualified `name()`.
macro_rules! op_enum {
    (
        $(#[$meta:meta])*
        $name:ident, $prefix:literal {
            $($(#[$vmeta:meta])* $variant:ident => $label:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Stable metric key, e.g. `db.transaction`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!($prefix, ".", $label),)+
                }
            }
        }
    };
}

op_enum! {
    /// Database operations. Time spent in these is attributed to the
    /// enclosing invocation as database time.
    DbOp, "db" {
        /// Opening a connection and applying connection pragmas.
        Open => "open",
        /// Waiting on the write lock that serializes write transactions.
        WriteLockWait => "write_lock_wait",
        /// A write transaction, from BEGIN to COMMIT or ROLLBACK.
        Transaction => "transaction",
        /// Bulk node insert.
        InsertNodes => "insert_nodes",
        /// Bulk edge insert.
        InsertEdges => "insert_edges",
        /// Executable-body FTS document insert.
        InsertExecutableBodyDocuments => "insert_executable_body_documents",
        /// Unresolved reference insert.
        InsertUnresolvedRefs => "insert_unresolved_refs",
        /// Batch file record upsert.
        UpsertFiles => "upsert_files",
        /// Single file record upsert.
        UpsertFile => "upsert_file",
        /// Leaving bulk-load mode: edge dedup, index and FTS rebuild.
        EndBulkLoad => "end_bulk_load",
        /// Look up one node by id.
        GetNodeById => "get_node_by_id",
        /// Batch node lookup by ids.
        GetNodesByIds => "get_nodes_by_ids",
        /// Nodes of one file.
        GetNodesByFile => "get_nodes_by_file",
        /// Children of one node.
        GetChildrenOf => "get_children_of",
        /// Children of several nodes.
        GetChildrenOfMany => "get_children_of_many",
        /// Trait methods an impl method implements.
        GetTraitMethodsForImplMethod => "get_trait_methods_for_impl_method",
        /// Nodes of one kind.
        GetNodesByKind => "get_nodes_by_kind",
        /// Nodes matching a filter.
        GetNodesFiltered => "get_nodes_filtered",
        /// Nodes by short name.
        GetNodesByName => "get_nodes_by_name",
        /// Nodes by qualified name.
        GetNodesByQualifiedName => "get_nodes_by_qualified_name",
        /// Outgoing edges of one node.
        GetOutgoingEdges => "get_outgoing_edges",
        /// Incoming edges of one node.
        GetIncomingEdges => "get_incoming_edges",
        /// Incoming edges of several nodes.
        GetIncomingEdgesBulk => "get_incoming_edges_bulk",
        /// Call edges into a set of targets.
        CallsEdgesInto => "calls_edges_into",
        /// All call edges, optionally under a path prefix.
        GetCallEdges => "get_call_edges",
        /// Symbol search (non-FTS path).
        SearchNodes => "search_nodes",
        /// FTS5 symbol search.
        SearchNodesFts => "search_nodes_fts",
        /// Bounded FTS5 symbol search.
        SearchNodesBounded => "search_nodes_bounded",
        /// Exact-name symbol search.
        SearchNodesByExactName => "search_nodes_by_exact_name",
        /// FTS5 search over executable bodies.
        SearchExecutableBodies => "search_executable_bodies",
        /// Incoming call counts for ranking.
        BatchIncomingCallCounts => "batch_incoming_call_counts",
        /// Dead-code candidate query.
        FindDeadCode => "find_dead_code",
        /// Live `GDScript` override query.
        GdscriptLiveOverrides => "gdscript_live_overrides",
        /// Files a file depends on.
        GetFileDependencies => "get_file_dependencies",
        /// Files depending on a file.
        GetFileDependents => "get_file_dependents",
        /// File-level dependency adjacency.
        BuildFileAdjacency => "build_file_adjacency",
    }
}

op_enum! {
    /// Phases of an MCP `tools/call`.
    McpPhase, "mcp" {
        /// The whole call, end to end.
        Total => "total",
        /// Request parsing, validation and dispatch.
        Dispatch => "dispatch",
        /// Resolving the project and branch to serve.
        ProjectResolution => "project_resolution",
        /// The on-demand index freshness check.
        Freshness => "freshness",
        /// Graph traversal inside the handler.
        GraphTraversal => "graph_traversal",
        /// The tool handler itself, from dispatch to its result; what
        /// `serve --timings` reports as `_meta.duration_us`.
        Handler => "handler",
        /// Ranking and result processing.
        Ranking => "ranking",
        /// Serializing the response.
        Serialization => "serialization",
    }
}

op_enum! {
    /// Phases of an index or sync run.
    IndexPhase, "index" {
        /// The whole run, end to end.
        Total => "total",
        /// Discovering candidate files.
        Discovery => "discovery",
        /// Scanning file metadata for changes.
        Scan => "scan",
        /// Content hashing.
        Hash => "hash",
        /// Parsing and extraction.
        Extract => "extract",
        /// Writing nodes and edges.
        DbWrite => "db_write",
        /// Full-text index maintenance.
        FtsMaintenance => "fts_maintenance",
        /// Cross-file reference resolution.
        Resolve => "resolve",
        /// Building edges from resolved references.
        EdgeBuild => "edge_build",
        /// Committing the run.
        Commit => "commit",
    }
}

op_enum! {
    /// Phases of an incremental sync run. Kept apart from [`IndexPhase`]
    /// (full index) so each run kind has its own series.
    SyncPhase, "sync" {
        /// The whole run, end to end.
        Total => "total",
        /// Discovering files and stat-checking them for changes.
        Scan => "scan",
        /// Hashing changed files.
        Hash => "hash",
        /// Parsing, extraction and writing the changed files.
        Extract => "extract",
        /// Reference resolution and edge insertion.
        Resolve => "resolve",
        /// Rebuilding derived edges (Ruby reopenings, Rails routes, trait
        /// dispatch).
        EdgeBuild => "edge_build",
        /// Recording sync metadata.
        Commit => "commit",
    }
}

op_enum! {
    /// Whole runs by kind and bucketed repository size (file count), so
    /// latency can be compared across repository sizes without recording
    /// any repository identity.
    RunBucket, "run" {
        IndexLt100 => "index.files_lt_100",
        Index100To1k => "index.files_100_1k",
        Index1kTo10k => "index.files_1k_10k",
        Index10kTo100k => "index.files_10k_100k",
        IndexGe100k => "index.files_ge_100k",
        SyncLt100 => "sync.files_lt_100",
        Sync100To1k => "sync.files_100_1k",
        Sync1kTo10k => "sync.files_1k_10k",
        Sync10kTo100k => "sync.files_10k_100k",
        SyncGe100k => "sync.files_ge_100k",
    }
}

impl RunBucket {
    /// Bucket for a full index (`full`) or incremental sync over `files`
    /// project files.
    pub const fn for_run(full: bool, files: usize) -> Self {
        match (full, files) {
            (true, 0..=99) => Self::IndexLt100,
            (true, 100..=999) => Self::Index100To1k,
            (true, 1_000..=9_999) => Self::Index1kTo10k,
            (true, 10_000..=99_999) => Self::Index10kTo100k,
            (true, _) => Self::IndexGe100k,
            (false, 0..=99) => Self::SyncLt100,
            (false, 100..=999) => Self::Sync100To1k,
            (false, 1_000..=9_999) => Self::Sync1kTo10k,
            (false, 10_000..=99_999) => Self::Sync10kTo100k,
            (false, _) => Self::SyncGe100k,
        }
    }
}

/// Broad grouping of an [`Op`], used for attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Db,
    Mcp,
    Index,
    Sync,
    Run,
}

/// Any profiled operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    Db(DbOp),
    Mcp(McpPhase),
    Index(IndexPhase),
    Sync(SyncPhase),
    Run(RunBucket),
}

const DB_BASE: usize = 0;
const MCP_BASE: usize = DB_BASE + DbOp::ALL.len();
const INDEX_BASE: usize = MCP_BASE + McpPhase::ALL.len();
const SYNC_BASE: usize = INDEX_BASE + IndexPhase::ALL.len();
const RUN_BASE: usize = SYNC_BASE + SyncPhase::ALL.len();

impl Op {
    /// Number of distinct operations, and so the number of series a
    /// recorder holds.
    pub const COUNT: usize = RUN_BASE + RunBucket::ALL.len();

    /// Dense index in `0..Op::COUNT`.
    pub const fn index(self) -> usize {
        match self {
            Self::Db(op) => DB_BASE + op as usize,
            Self::Mcp(op) => MCP_BASE + op as usize,
            Self::Index(op) => INDEX_BASE + op as usize,
            Self::Sync(op) => SYNC_BASE + op as usize,
            Self::Run(op) => RUN_BASE + op as usize,
        }
    }

    /// Inverse of [`Op::index`].
    pub fn from_index(index: usize) -> Option<Self> {
        if index < MCP_BASE {
            DbOp::ALL.get(index - DB_BASE).copied().map(Self::Db)
        } else if index < INDEX_BASE {
            McpPhase::ALL.get(index - MCP_BASE).copied().map(Self::Mcp)
        } else if index < SYNC_BASE {
            IndexPhase::ALL
                .get(index - INDEX_BASE)
                .copied()
                .map(Self::Index)
        } else if index < RUN_BASE {
            SyncPhase::ALL
                .get(index - SYNC_BASE)
                .copied()
                .map(Self::Sync)
        } else {
            RunBucket::ALL.get(index - RUN_BASE).copied().map(Self::Run)
        }
    }

    /// Stable metric key.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Db(op) => op.name(),
            Self::Mcp(op) => op.name(),
            Self::Index(op) => op.name(),
            Self::Sync(op) => op.name(),
            Self::Run(op) => op.name(),
        }
    }

    pub const fn category(self) -> Category {
        match self {
            Self::Db(_) => Category::Db,
            Self::Mcp(_) => Category::Mcp,
            Self::Index(_) => Category::Index,
            Self::Sync(_) => Category::Sync,
            Self::Run(_) => Category::Run,
        }
    }

    /// Looks an operation up by its metric key.
    pub fn from_name(name: &str) -> Option<Self> {
        (0..Self::COUNT)
            .filter_map(Self::from_index)
            .find(|op| op.name() == name)
    }
}

impl From<DbOp> for Op {
    fn from(op: DbOp) -> Self {
        Self::Db(op)
    }
}

impl From<McpPhase> for Op {
    fn from(op: McpPhase) -> Self {
        Self::Mcp(op)
    }
}

impl From<IndexPhase> for Op {
    fn from(op: IndexPhase) -> Self {
        Self::Index(op)
    }
}

impl From<SyncPhase> for Op {
    fn from(op: SyncPhase) -> Self {
        Self::Sync(op)
    }
}

impl From<RunBucket> for Op {
    fn from(op: RunBucket) -> Self {
        Self::Run(op)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn indexes_are_dense_and_round_trip() {
        for index in 0..Op::COUNT {
            let op = Op::from_index(index).unwrap();
            assert_eq!(op.index(), index);
            assert_eq!(Op::from_name(op.name()), Some(op));
        }
        assert_eq!(Op::from_index(Op::COUNT), None);
    }

    #[test]
    fn names_are_unique_and_prefixed_by_category() {
        let mut seen = HashSet::new();
        for op in (0..Op::COUNT).filter_map(Op::from_index) {
            assert!(seen.insert(op.name()), "duplicate name {}", op.name());
            let prefix = match op.category() {
                Category::Db => "db.",
                Category::Mcp => "mcp.",
                Category::Index => "index.",
                Category::Sync => "sync.",
                Category::Run => "run.",
            };
            assert!(op.name().starts_with(prefix), "{}", op.name());
        }
    }

    #[test]
    fn run_buckets_split_by_kind_and_size() {
        assert_eq!(RunBucket::for_run(true, 0), RunBucket::IndexLt100);
        assert_eq!(RunBucket::for_run(true, 100), RunBucket::Index100To1k);
        assert_eq!(RunBucket::for_run(false, 9_999), RunBucket::Sync1kTo10k);
        assert_eq!(RunBucket::for_run(false, 10_000), RunBucket::Sync10kTo100k);
        assert_eq!(RunBucket::for_run(true, usize::MAX), RunBucket::IndexGe100k);
    }
}
