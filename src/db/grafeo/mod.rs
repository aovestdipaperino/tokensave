//! Experimental Grafeo storage backend (#904).
//!
//! Selected at build time with the `grafeo` cargo feature, it implements the
//! same [`Database`] API as the libSQL backend over a Grafeo labelled
//! property graph:
//!
//! - every tokensave node is a `Symbol` graph node carrying its fields as
//!   properties, found through the `id` property index;
//! - every tokensave edge is a native graph edge whose type is the edge
//!   kind, with `line` and `resolved_by` as properties;
//! - files, unresolved references and metadata are nodes with their own
//!   labels and property names, because Grafeo's property indexes are
//!   global across labels.
//!
//! Traversals use Grafeo's direct API (property index, then adjacency)
//! rather than GQL: a GQL pattern anchored on its target node is not
//! planned through the property index and was four orders of magnitude
//! slower in the spike recorded on #904.
//!
//! Grafeo locks a read-write database file exclusively across processes, so
//! this backend serves one process at a time. It exists to benchmark Grafeo
//! against libSQL; methods outside the benchmark fail with a
//! "not supported by the grafeo storage backend yet" error (see `pending`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use grafeo::{GrafeoDB, NodeId, Value};

use crate::errors::{Result, TokenSaveError};
use crate::types::{Edge, EdgeKind, FileKind, FileRecord, Node, NodeKind, ResolvedBy, Visibility};

mod edges;
mod files;
mod metadata;
mod nodes;
mod pending;
pub mod probe;
pub mod snapshot;
mod telemetry;

/// Version of the graph layout this build writes, stored as metadata.
const SCHEMA_VERSION: u32 = 1;

/// Label of tokensave's code-graph nodes.
const SYMBOL: &str = "Symbol";
/// Label of indexed-file records.
const FILE: &str = "File";
/// Label of unresolved references awaiting resolution.
const UNRESOLVED: &str = "UnresolvedRef";
/// Label of key/value metadata.
const META: &str = "Meta";

/// Properties with an index. Each name belongs to one label only, because
/// Grafeo's property indexes span all labels.
const INDEXED_PROPERTIES: &[&str] = &[
    // Symbol
    "id",
    "file_path",
    "name",
    "qualified_name",
    "kind",
    "parent_id",
    // File
    "path",
    // UnresolvedRef
    "ref_from",
    "ref_file_path",
    // Meta
    "meta_key",
];

/// A project index stored in Grafeo.
pub struct Database {
    db: GrafeoDB,
    path: PathBuf,
    read_only: bool,
}

impl Database {
    /// Opens the index at `db_path`, creating it and its indexes if needed.
    /// Returns `(Self, migrated)`; this backend has no migrations yet, so
    /// `migrated` is always `false`.
    pub async fn initialize(db_path: &Path) -> Result<(Self, bool)> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TokenSaveError::Database {
                message: format!("failed to create database directory: {e}"),
                operation: "initialize".to_string(),
            })?;
        }
        Self::open_read_write(db_path, "initialize")
    }

    /// Opens the index at `db_path` for reading and writing.
    pub async fn open(db_path: &Path) -> Result<(Self, bool)> {
        Self::open_read_write(db_path, "open")
    }

    /// Opens the index at `db_path` without allowing writes. Several readers
    /// may share the file, but not while a writer has it open.
    pub async fn open_read_only(db_path: &Path) -> Result<Self> {
        let db = GrafeoDB::open_read_only(db_path).map_err(|e| db_error("open_read_only", e))?;
        let database = Self {
            db,
            path: db_path.to_path_buf(),
            read_only: true,
        };
        database.check_schema_version("open_read_only").await?;
        Ok(database)
    }

    fn open_read_write(db_path: &Path, operation: &str) -> Result<(Self, bool)> {
        let db = GrafeoDB::open(db_path).map_err(|e| db_error(operation, e))?;
        for property in INDEXED_PROPERTIES {
            db.create_property_index(property)
                .map_err(|e| db_error(operation, e))?;
        }
        let database = Self {
            db,
            path: db_path.to_path_buf(),
            read_only: false,
        };
        Ok((database, false))
    }

    /// Refuses an index written by a newer layout than this build knows.
    async fn check_schema_version(&self, operation: &str) -> Result<()> {
        let version = self.schema_version().await?;
        if version > SCHEMA_VERSION {
            return Err(TokenSaveError::Database {
                message: format!(
                    "index layout version {version} is newer than this build supports ({SCHEMA_VERSION})"
                ),
                operation: operation.to_string(),
            });
        }
        Ok(())
    }

    /// Returns whether this handle was opened read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Closes the index, checkpointing it to its file.
    pub fn close(self) {
        // Best effort, like dropping a libSQL connection: dropping the
        // database checkpoints it as well.
        let _ = self.db.close();
    }

    /// Layout version this build creates.
    pub fn latest_schema_version() -> u32 {
        SCHEMA_VERSION
    }

    /// A Grafeo build keeps its state in its own directory (see
    /// `config::storage_dir`), so there is no other backend's file to refuse
    /// here; an unreadable file fails in [`Self::open`].
    pub async fn preflight(_db_path: &Path) -> Result<()> {
        Ok(())
    }

    /// Whether `e` clears by itself, such as another process holding the
    /// index's lock.
    pub fn is_environmental_error(e: &TokenSaveError) -> bool {
        matches!(e, TokenSaveError::Database { message, .. }
            if message.contains("Database locked") || message.contains("held by another process"))
    }

    /// This backend has no migrations, so no error is a migration failure.
    pub fn is_migration_error(_e: &TokenSaveError) -> bool {
        false
    }

    /// Returns `true` if the error reports a damaged index file.
    pub fn is_corruption_error(e: &TokenSaveError) -> bool {
        matches!(e, TokenSaveError::Database { message, .. }
            if message.contains("corrupt") || message.contains("checksum"))
    }

    /// Returns the layout version stored in the index; 0 for a new one.
    pub async fn schema_version(&self) -> Result<u32> {
        Ok(self
            .get_metadata("grafeo_schema_version")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    /// Grafeo reads see a consistent committed state per call; a snapshot
    /// spanning several calls is not available, so this is a no-op.
    pub async fn begin_read_snapshot(&self) -> Result<()> {
        Ok(())
    }

    /// Writes committed changes from the write-ahead log into the file.
    pub async fn checkpoint(&self) -> Result<()> {
        self.db
            .wal_checkpoint()
            .map_err(|e| db_error("checkpoint", e))
    }

    /// Grafeo maintains its own storage; there is nothing to optimize.
    pub async fn optimize(&self) -> Result<()> {
        Ok(())
    }

    /// Returns the on-disk size of the index file and its write-ahead log.
    pub async fn size(&self) -> Result<u64> {
        Ok(path_size(&self.path) + path_size(&with_suffix(&self.path, ".wal")))
    }

    /// Grafeo verifies its file on open; an open index is consistent.
    pub async fn quick_check(&self) -> Result<bool> {
        Ok(true)
    }

    /// Bulk loads need no finalization: edges are deduplicated on insert.
    pub async fn needs_bulk_load_finalization(&self) -> Result<bool> {
        Ok(false)
    }

    /// The text index is maintained on every write.
    pub async fn rebuild_fts(&self) -> Result<()> {
        Ok(())
    }

    /// Bulk loads use the same write path as incremental ones.
    pub async fn begin_bulk_load(&self) -> Result<()> {
        Ok(())
    }

    /// See [`Self::begin_bulk_load`].
    pub async fn end_bulk_load(&self) -> Result<()> {
        // Record the layout version once data has been written, so a
        // read-only open can refuse a layout it does not know.
        self.set_metadata("grafeo_schema_version", &SCHEMA_VERSION.to_string())
            .await
    }

    /// Removes every node and edge.
    pub async fn clear(&self) -> Result<()> {
        self.db
            .execute("MATCH (n) DETACH DELETE n")
            .map_err(|e| db_error("clear", e))?;
        Ok(())
    }

    /// Ids of graph nodes whose indexed `property` equals `value`.
    fn find(&self, property: &str, value: &str, operation: &str) -> Result<Vec<NodeId>> {
        self.db
            .find_nodes_by_property(property, &Value::from(value))
            .map_err(|e| db_error(operation, e))
    }

    /// The graph node of the tokensave node `id`.
    fn symbol_node_id(&self, id: &str, operation: &str) -> Result<Option<NodeId>> {
        Ok(self.find("id", id, operation)?.into_iter().next())
    }

    /// Reads a graph node; `None` if it was deleted meanwhile.
    fn graph_node(&self, id: NodeId, operation: &str) -> Result<Option<grafeo::store::Node>> {
        self.db.get_node(id).map_err(|e| db_error(operation, e))
    }
}

/// Wraps a Grafeo error with the operation that failed.
fn db_error(operation: &str, e: grafeo::Error) -> TokenSaveError {
    TokenSaveError::Database {
        message: e.to_string(),
        operation: operation.to_string(),
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Size of a file, or the summed size of a directory's files; 0 if absent.
fn path_size(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| path_size(&e.path()))
                    .sum()
            })
            .unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// Property conversion
// ---------------------------------------------------------------------------

type PropMap = HashMap<grafeo::store::PropertyKey, Value>;

/// Read access to the properties of a Grafeo node or edge.
trait PropSource {
    fn prop(&self, key: &str) -> Option<&Value>;
}

impl PropSource for grafeo::store::Node {
    fn prop(&self, key: &str) -> Option<&Value> {
        self.properties.get(&key.into())
    }
}

impl PropSource for grafeo::store::Edge {
    fn prop(&self, key: &str) -> Option<&Value> {
        self.properties.get(&key.into())
    }
}

fn text(value: &str) -> Value {
    Value::from(value)
}

fn int(value: impl Into<i64>) -> Value {
    Value::Int64(value.into())
}

fn put_opt(props: &mut PropMap, key: &str, value: Option<&str>) {
    if let Some(v) = value {
        props.insert(key.into(), text(v));
    }
}

fn get_str(props: &impl PropSource, key: &str) -> String {
    props
        .prop(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn get_opt_str(props: &impl PropSource, key: &str) -> Option<String> {
    props.prop(key).and_then(Value::as_str).map(str::to_string)
}

fn get_i64(props: &impl PropSource, key: &str) -> Option<i64> {
    props.prop(key).and_then(Value::as_int64)
}

fn get_u32(props: &impl PropSource, key: &str) -> u32 {
    get_i64(props, key)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0)
}

fn node_props(node: &Node) -> PropMap {
    let mut p = PropMap::with_capacity(28);
    p.insert("id".into(), text(&node.id));
    p.insert("kind".into(), text(node.kind.as_str()));
    p.insert("name".into(), text(&node.name));
    p.insert("qualified_name".into(), text(&node.qualified_name));
    p.insert("file_path".into(), text(&node.file_path));
    p.insert("start_line".into(), int(node.start_line));
    p.insert("attrs_start_line".into(), int(node.attrs_start_line));
    p.insert("end_line".into(), int(node.end_line));
    p.insert("start_column".into(), int(node.start_column));
    p.insert("end_column".into(), int(node.end_column));
    put_opt(&mut p, "signature", node.signature.as_deref());
    put_opt(&mut p, "docstring", node.docstring.as_deref());
    p.insert("visibility".into(), text(node.visibility.as_str()));
    p.insert("is_async".into(), Value::Bool(node.is_async));
    p.insert("branches".into(), int(node.branches));
    p.insert("loops".into(), int(node.loops));
    p.insert("returns".into(), int(node.returns));
    p.insert("max_nesting".into(), int(node.max_nesting));
    p.insert("unsafe_blocks".into(), int(node.unsafe_blocks));
    p.insert("unchecked_calls".into(), int(node.unchecked_calls));
    p.insert("assertions".into(), int(node.assertions));
    p.insert(
        "cognitive_complexity".into(),
        int(node.cognitive_complexity),
    );
    p.insert("distinct_operators".into(), int(node.distinct_operators));
    p.insert("distinct_operands".into(), int(node.distinct_operands));
    p.insert("total_operators".into(), int(node.total_operators));
    p.insert("total_operands".into(), int(node.total_operands));
    p.insert(
        "updated_at".into(),
        int(i64::try_from(node.updated_at).unwrap_or(i64::MAX)),
    );
    put_opt(&mut p, "parent_id", node.parent_id.as_deref());
    p
}

fn node_from(p: &grafeo::store::Node) -> Node {
    let start_line = get_u32(p, "start_line");
    let attrs = get_u32(p, "attrs_start_line");
    Node {
        id: get_str(p, "id"),
        kind: NodeKind::from_str(&get_str(p, "kind")).unwrap_or(NodeKind::Function),
        name: get_str(p, "name"),
        qualified_name: get_str(p, "qualified_name"),
        file_path: get_str(p, "file_path"),
        start_line,
        // Mirrors the libSQL mapper: 0 means "not recorded".
        attrs_start_line: if attrs == 0 { start_line } else { attrs },
        end_line: get_u32(p, "end_line"),
        start_column: get_u32(p, "start_column"),
        end_column: get_u32(p, "end_column"),
        signature: get_opt_str(p, "signature"),
        docstring: get_opt_str(p, "docstring"),
        visibility: Visibility::from_str(&get_str(p, "visibility")).unwrap_or_default(),
        is_async: p.prop("is_async").and_then(Value::as_bool).unwrap_or(false),
        branches: get_u32(p, "branches"),
        loops: get_u32(p, "loops"),
        returns: get_u32(p, "returns"),
        max_nesting: get_u32(p, "max_nesting"),
        unsafe_blocks: get_u32(p, "unsafe_blocks"),
        unchecked_calls: get_u32(p, "unchecked_calls"),
        assertions: get_u32(p, "assertions"),
        cognitive_complexity: get_u32(p, "cognitive_complexity"),
        distinct_operators: get_u32(p, "distinct_operators"),
        distinct_operands: get_u32(p, "distinct_operands"),
        total_operators: get_u32(p, "total_operators"),
        total_operands: get_u32(p, "total_operands"),
        updated_at: get_i64(p, "updated_at")
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(0),
        parent_id: get_opt_str(p, "parent_id"),
    }
}

fn edge_props(edge: &Edge) -> Vec<(&'static str, Value)> {
    let mut p = Vec::with_capacity(2);
    if let Some(line) = edge.line {
        p.push(("line", int(line)));
    }
    if let Some(resolved_by) = edge.resolved_by {
        p.push(("resolved_by", int(resolved_by.code())));
    }
    p
}

fn edge_from(source: String, target: String, edge: &grafeo::store::Edge) -> Edge {
    Edge {
        source,
        target,
        kind: EdgeKind::from_str(&edge.edge_type).unwrap_or(EdgeKind::Uses),
        line: get_i64(edge, "line").and_then(|v| u32::try_from(v).ok()),
        resolved_by: get_i64(edge, "resolved_by").and_then(ResolvedBy::from_code),
    }
}

fn file_props(file: &FileRecord) -> PropMap {
    let mut p = PropMap::with_capacity(7);
    p.insert("path".into(), text(&file.path));
    p.insert("content_hash".into(), text(&file.content_hash));
    p.insert(
        "size".into(),
        int(i64::try_from(file.size).unwrap_or(i64::MAX)),
    );
    p.insert("modified_at".into(), int(file.modified_at));
    p.insert("indexed_at".into(), int(file.indexed_at));
    p.insert("node_count".into(), int(file.node_count));
    p.insert("file_kind".into(), text(file.kind.as_str()));
    p
}

fn file_from(p: &grafeo::store::Node) -> FileRecord {
    FileRecord {
        path: get_str(p, "path"),
        content_hash: get_str(p, "content_hash"),
        size: get_i64(p, "size")
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(0),
        modified_at: get_i64(p, "modified_at").unwrap_or(0),
        indexed_at: get_i64(p, "indexed_at").unwrap_or(0),
        node_count: get_u32(p, "node_count"),
        kind: FileKind::from_str_or_code(&get_str(p, "file_kind")),
    }
}
