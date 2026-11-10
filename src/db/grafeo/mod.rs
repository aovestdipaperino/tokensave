//! Experimental Grafeo storage backend (#904).
//!
//! Selected at build time with the `grafeo` cargo feature, it implements the
//! same [`Database`] API as the libSQL backend over a Grafeo labelled
//! property graph:
//!
//! - every tokensave node is a `Symbol` graph node carrying its fields as
//!   properties, found through the `id` property index;
//! - every tokensave edge is a native graph edge whose type is the edge
//!   kind, with `line` and `resolved_by` as properties. `contains` edges are
//!   stored as the child's `parent_id` property, as in the libSQL backend;
//! - files, unresolved references, metadata and the other side records
//!   (ambiguous calls, fingerprints, session memory, executable bodies) are
//!   nodes with their own labels and property names, because Grafeo's
//!   property indexes are global across labels.
//!
//! Point lookups and traversals use Grafeo's direct API (property index,
//! then adjacency) rather than GQL: a GQL pattern anchored on its target
//! node is not planned through the property index and was four orders of
//! magnitude slower in the spike recorded on #904. Whole-graph analyses
//! scan a label with one GQL query and aggregate in Rust.
//!
//! State the libSQL backend keeps in derived tables is computed here
//! instead: the reverse trait-dispatch caller map is rebuilt lazily after
//! edges change, and full-text search ranks an in-memory index that is
//! rebuilt after nodes change (see `search`).
//!
//! Grafeo locks a read-write database file exclusively across processes, so
//! this backend serves one process at a time. It exists to benchmark Grafeo
//! against libSQL.
#![allow(
    clippy::unused_async,
    clippy::unused_async_trait_impl,
    reason = "the Database API is async for the libSQL backend; Grafeo's calls are synchronous"
)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use grafeo::store::PropertyKey;
use grafeo::{GrafeoDB, NodeId, Value};

use crate::db::model::CachedTraitDispatchCaller;
use crate::errors::{Result, TokenSaveError};
use crate::types::{Edge, EdgeKind, FileKind, FileRecord, Node, NodeKind, ResolvedBy, Visibility};

mod ambiguous;
mod analysis;
mod edges;
mod files;
mod metadata;
mod nodes;
pub mod probe;
mod records;
mod routes;
mod search;
pub mod snapshot;
mod stats;
mod telemetry;
mod unresolved;

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
/// Label of recorded ambiguous call sites.
const AMBIGUOUS: &str = "AmbiguousCall";
/// Label of node fingerprints.
const FINGERPRINT: &str = "Fingerprint";
/// Label of session-memory decisions.
const DECISION: &str = "Decision";
/// Label of session-memory code areas.
const CODE_AREA: &str = "CodeArea";
/// Label of executable-body search documents.
const BODY: &str = "ExecutableBody";

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
    // AmbiguousCall
    "amb_from",
    "amb_file",
    // Fingerprint
    "fp_node",
    // CodeArea
    "area_path",
    // ExecutableBody
    "body_file",
];

/// Reverse trait-dispatch callers, by concrete method id.
type DispatchMap = HashMap<String, Vec<CachedTraitDispatchCaller>>;

/// A project index stored in Grafeo.
pub struct Database {
    db: GrafeoDB,
    path: PathBuf,
    read_only: bool,
    /// Reverse trait-dispatch callers; rebuilt when `dispatch_dirty` is set.
    trait_dispatch: RwLock<DispatchMap>,
    /// Set when edges or nodes change, so the caller map is recomputed on
    /// its next read.
    dispatch_dirty: AtomicBool,
    /// Last sequence number given to an unresolved reference, so references
    /// can be paged in insertion order like the libSQL autoincrement id.
    ref_seq: AtomicI64,
    /// Bumped whenever a write may change search results.
    search_generation: AtomicU64,
    /// Full-text index over symbols, tagged with the generation it was
    /// built at.
    search_cache: Mutex<Option<(u64, Arc<search::SearchIndex>)>>,
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
        let db = GrafeoDB::open_read_only(db_path).map_err(|e| db_error("open_read_only", &e))?;
        let database = Self::with_handle(db, db_path, true, "open_read_only")?;
        database.check_schema_version("open_read_only").await?;
        Ok(database)
    }

    fn open_read_write(db_path: &Path, operation: &str) -> Result<(Self, bool)> {
        let db = GrafeoDB::open(db_path).map_err(|e| db_error(operation, &e))?;
        for property in INDEXED_PROPERTIES {
            db.create_property_index(property)
                .map_err(|e| db_error(operation, &e))?;
        }
        Ok((Self::with_handle(db, db_path, false, operation)?, false))
    }

    fn with_handle(db: GrafeoDB, db_path: &Path, read_only: bool, operation: &str) -> Result<Self> {
        let database = Self {
            db,
            path: db_path.to_path_buf(),
            read_only,
            trait_dispatch: RwLock::new(HashMap::new()),
            dispatch_dirty: AtomicBool::new(true),
            ref_seq: AtomicI64::new(0),
            search_generation: AtomicU64::new(0),
            search_cache: Mutex::new(None),
        };
        let last_seq = database.max_ref_seq(operation)?;
        database.ref_seq.store(last_seq, Ordering::SeqCst);
        Ok(database)
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
            .map_err(|e| db_error("checkpoint", &e))
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

    /// The search index is rebuilt from the graph on demand; dropping it is
    /// the rebuild.
    pub async fn rebuild_fts(&self) -> Result<()> {
        self.graph_changed();
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
            .map_err(|e| db_error("clear", &e))?;
        self.ref_seq.store(0, Ordering::SeqCst);
        self.graph_changed();
        Ok(())
    }

    /// Marks the derived caches (trait-dispatch callers, search index) as
    /// stale after a write to nodes or edges.
    fn graph_changed(&self) {
        self.dispatch_dirty.store(true, Ordering::SeqCst);
        self.search_generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Ids of graph nodes whose indexed `property` equals `value`.
    fn find(&self, property: &str, value: &str, operation: &str) -> Result<Vec<NodeId>> {
        self.find_value(property, &Value::from(value), operation)
    }

    /// Ids of graph nodes whose indexed `property` equals `value`.
    fn find_value(&self, property: &str, value: &Value, operation: &str) -> Result<Vec<NodeId>> {
        self.db
            .find_nodes_by_property(property, value)
            .map_err(|e| db_error(operation, &e))
    }

    /// The graph node of the tokensave node `id`.
    fn symbol_node_id(&self, id: &str, operation: &str) -> Result<Option<NodeId>> {
        Ok(self.find("id", id, operation)?.into_iter().next())
    }

    /// Reads a graph node; `None` if it was deleted meanwhile.
    fn graph_node(&self, id: NodeId, operation: &str) -> Result<Option<grafeo::store::Node>> {
        self.db.get_node(id).map_err(|e| db_error(operation, &e))
    }

    /// Deletes graph nodes, without their edges.
    fn delete_graph_nodes(&self, ids: &[NodeId], operation: &str) -> Result<()> {
        for gid in ids {
            self.db
                .delete_node(*gid)
                .map_err(|e| db_error(operation, &e))?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Scans
    // -----------------------------------------------------------------------

    /// Runs a GQL query and returns its rows.
    fn query_rows(&self, query: &str, operation: &str) -> Result<Vec<Vec<Value>>> {
        Ok(self
            .db
            .execute(query)
            .map_err(|e| db_error(operation, &e))?
            .into_rows())
    }

    /// The properties of every graph node with `label`, in graph-id order.
    /// Each map also carries the graph id under `_id`.
    fn scan_label(&self, label: &str, operation: &str) -> Result<Vec<Props>> {
        let mut out: Vec<Props> = self
            .query_rows(&format!("MATCH (n:{label}) RETURN n"), operation)?
            .into_iter()
            .filter_map(|row| match row.into_iter().next() {
                Some(Value::Map(map)) => Some(map),
                _ => None,
            })
            .collect();
        out.sort_by_key(|p| graph_id(p.as_ref()).map_or(u64::MAX, |id| id.0));
        Ok(out)
    }

    /// Every code-graph node, in insertion order.
    fn all_symbols(&self, operation: &str) -> Result<Vec<Node>> {
        Ok(self
            .scan_label(SYMBOL, operation)?
            .iter()
            .map(|p| node_from(p.as_ref()))
            .collect())
    }

    /// Every code-graph edge whose kind is in `kinds`; every edge when
    /// `kinds` is empty.
    fn scan_edges(&self, kinds: &[EdgeKind], operation: &str) -> Result<Vec<Edge>> {
        let pattern = match kinds {
            [kind] => format!("[e:{}]", kind.as_str()),
            _ => "[e]".to_string(),
        };
        let rows = self.query_rows(
            &format!(
                "MATCH (a:{SYMBOL})-{pattern}->(b:{SYMBOL}) \
                 RETURN a.id, b.id, type(e), e.line, e.resolved_by"
            ),
            operation,
        )?;
        let wanted: Vec<&str> = kinds.iter().map(EdgeKind::as_str).collect();
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let [source, target, kind, line, resolved_by] = row.as_slice() else {
                continue;
            };
            let kind = kind.as_str().unwrap_or_default();
            if !wanted.is_empty() && !wanted.contains(&kind) {
                continue;
            }
            out.push(Edge {
                source: source.as_str().unwrap_or_default().to_string(),
                target: target.as_str().unwrap_or_default().to_string(),
                kind: EdgeKind::from_str(kind).unwrap_or(EdgeKind::Uses),
                line: line.as_int64().and_then(|v| u32::try_from(v).ok()),
                resolved_by: resolved_by.as_int64().and_then(ResolvedBy::from_code),
            });
        }
        Ok(out)
    }

    /// The largest reference sequence number in the index; 0 when empty.
    fn max_ref_seq(&self, operation: &str) -> Result<i64> {
        let rows = self.query_rows(
            &format!("MATCH (r:{UNRESOLVED}) RETURN max(r.ref_seq)"),
            operation,
        )?;
        Ok(rows
            .first()
            .and_then(|row| row.first())
            .and_then(Value::as_int64)
            .unwrap_or(0))
    }
}

/// Wraps a Grafeo error with the operation that failed.
fn db_error(operation: &str, e: &grafeo::Error) -> TokenSaveError {
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
        Ok(m) if m.is_dir() => std::fs::read_dir(path).map_or(0, |entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|e| path_size(&e.path()))
                .sum()
        }),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// SQL predicate equivalents
// ---------------------------------------------------------------------------

/// Whether `file_path` is `prefix` or lies under it, as the libSQL path
/// filter (`file_path = p OR file_path LIKE 'p/%'`) decides. `LIKE` ignores
/// ASCII case, so the directory match does too.
fn under_path(file_path: &str, prefix: &str) -> bool {
    let exact = prefix.trim_end_matches('/');
    if file_path == exact {
        return true;
    }
    file_path.len() > exact.len()
        && file_path.as_bytes().get(exact.len()) == Some(&b'/')
        && file_path
            .get(..exact.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(exact))
}

/// `value LIKE '%suffix'`: ends with `suffix`, ignoring ASCII case.
fn ends_with_ci(value: &str, suffix: &str) -> bool {
    value.len() >= suffix.len()
        && value
            .get(value.len() - suffix.len()..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
}

/// `value LIKE 'prefix%'`: starts with `prefix`, ignoring ASCII case.
fn starts_with_ci(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

/// `value LIKE '%needle%'`: contains `needle`, ignoring ASCII case.
fn contains_ci(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

// ---------------------------------------------------------------------------
// Property conversion
// ---------------------------------------------------------------------------

type PropMap = HashMap<PropertyKey, Value>;

/// The properties of a scanned node, as a GQL `RETURN n` yields them.
type Props = Arc<BTreeMap<PropertyKey, Value>>;

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

impl PropSource for BTreeMap<PropertyKey, Value> {
    fn prop(&self, key: &str) -> Option<&Value> {
        self.get(&PropertyKey::from(key))
    }
}

/// The graph id a scanned node carries under `_id`.
fn graph_id(props: &BTreeMap<PropertyKey, Value>) -> Option<NodeId> {
    props
        .prop("_id")
        .and_then(Value::as_int64)
        .and_then(|v| u64::try_from(v).ok())
        .map(NodeId::new)
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

fn get_str(props: &(impl PropSource + ?Sized), key: &str) -> String {
    props
        .prop(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn get_opt_str(props: &(impl PropSource + ?Sized), key: &str) -> Option<String> {
    props.prop(key).and_then(Value::as_str).map(str::to_string)
}

fn get_i64(props: &(impl PropSource + ?Sized), key: &str) -> Option<i64> {
    props.prop(key).and_then(Value::as_int64)
}

fn get_u32(props: &(impl PropSource + ?Sized), key: &str) -> u32 {
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

fn node_from(p: &(impl PropSource + ?Sized)) -> Node {
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

fn file_from(p: &(impl PropSource + ?Sized)) -> FileRecord {
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

#[cfg(test)]
mod tests {
    use super::{contains_ci, ends_with_ci, starts_with_ci, under_path};

    #[test]
    fn under_path_matches_the_directory_and_its_contents_only() {
        assert!(under_path("src", "src"));
        assert!(under_path("src/a.rs", "src"));
        assert!(under_path("src/a.rs", "src/"));
        assert!(under_path("SRC/a.rs", "src"));
        assert!(!under_path("srcfoo/a.rs", "src"));
        assert!(!under_path("a_b/x.rs", "axb"));
    }

    #[test]
    fn like_equivalents_ignore_ascii_case() {
        assert!(ends_with_ci("main.GO", ".go"));
        assert!(!ends_with_ci("go", ".go"));
        assert!(starts_with_ci("Test_parse", "test"));
        assert!(!starts_with_ci("te", "test"));
        assert!(contains_ci("skip-TEST-coverage", "test"));
    }
}
