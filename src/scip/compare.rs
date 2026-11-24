//! Read-only comparison of a SCIP index with the tokensave graph (#1100).
//!
//! The index is treated as an external oracle: its definitions are mapped
//! onto existing nodes, its references onto `(file, line, target)` keys, and
//! each graph edge in a verified document is classified as confirmed,
//! contradicted or unverifiable. SCIP references that no edge covers become
//! a dry-run import plan. Nothing is written: no sync, migration, or database
//! write happens, and the graph's contracts are unchanged. The project's sync
//! lock is held for the duration, so the graph cannot change mid-comparison.

use super::symbol::{self, Suffix};
use super::wire::{self, Document, Limits, ROLE_DEFINITION, ROLE_IMPORT};
use crate::errors::{Result, TokenSaveError};
use crate::types::ResolvedBy;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const SOURCE_BYTES: u64 = 16 * 1024 * 1024;
/// Report format version; bump on incompatible changes to the JSON shape.
pub const REPORT_VERSION: u32 = 1;

/// Inputs to [`compare`].
#[derive(Debug, Clone)]
pub struct Options {
    pub root: PathBuf,
    pub index: PathBuf,
    /// Accept an index whose `project_root` names another directory. Each
    /// document is still verified against this checkout's sources.
    pub rebase_root: bool,
    pub max_index_bytes: u64,
    /// Examples kept per report category.
    pub samples: usize,
}

fn invalid(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

#[derive(Debug, Clone)]
struct NodeRow {
    id: String,
    name: String,
    kind: String,
    qualified_name: String,
    start_line: u32,
    end_line: u32,
}

impl NodeRow {
    fn span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line)
    }
}

/// Kinds that are never a reference source or a mapped definition: the file
/// itself, imports, and syntax that has no node-level identity.
fn ignored_kind(kind: &str) -> bool {
    matches!(kind, "file" | "use" | "doc" | "generic_param" | "include")
}

/// Why a document took no part in the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Skip {
    UnsafePath,
    Missing,
    NotUtf8,
    TooLarge,
    NotIndexed,
    GraphStale,
    SourceMismatch,
    UnsupportedEncoding,
    MalformedRange,
    RangeOutOfBounds,
    DefinitionMismatch,
}

impl Skip {
    fn as_str(self) -> &'static str {
        match self {
            Skip::UnsafePath => "unsafe_path",
            Skip::Missing => "missing_source",
            Skip::NotUtf8 => "source_not_utf8",
            Skip::TooLarge => "source_too_large",
            Skip::NotIndexed => "not_indexed_by_tokensave",
            Skip::GraphStale => "tokensave_index_stale",
            Skip::SourceMismatch => "scip_stale_source_mismatch",
            Skip::UnsupportedEncoding => "unsupported_position_encoding",
            Skip::MalformedRange => "malformed_range",
            Skip::RangeOutOfBounds => "scip_stale_range_out_of_bounds",
            Skip::DefinitionMismatch => "scip_stale_definition_mismatch",
        }
    }
}

/// Converts a SCIP column to a UTF-8 byte boundary in the source line.
fn byte_column(line: &str, column: u32, encoding: u64) -> Option<usize> {
    let column = column as usize;
    match encoding {
        0 | 1 => line.is_char_boundary(column).then_some(column),
        2 | 3 => {
            let mut units = 0;
            for (offset, ch) in line.char_indices() {
                if units >= column {
                    return (units == column).then_some(offset);
                }
                units += if encoding == 2 { ch.len_utf16() } else { 1 };
            }
            (units == column).then_some(line.len())
        }
        _ => None,
    }
}

/// Returns the text a SCIP range covers on one line.
fn slice(line: &str, start: u32, end: u32, encoding: u64) -> Option<&str> {
    line.get(byte_column(line, start, encoding)?..byte_column(line, end, encoding)?)
}

fn follows_call_syntax(line: &str, end: u32, encoding: u64) -> bool {
    byte_column(line, end, encoding).is_some_and(|offset| {
        let after = line[offset..].trim_start();
        after.starts_with('(') || after.starts_with("::<")
    })
}

/// Checks a document's ranges against the current source. A SCIP index has
/// no content hash, so a definition whose range no longer spells its own name
/// is the evidence that the index predates the source.
fn verify(doc: &Document, lines: &[&str]) -> std::result::Result<(), Skip> {
    if doc.position_encoding > 3 {
        return Err(Skip::UnsupportedEncoding);
    }
    for occ in &doc.occurrences {
        let Some(r) = occ.range else {
            return Err(Skip::MalformedRange);
        };
        let (Some(start), Some(end)) = (
            lines.get(r.start_line as usize),
            lines.get(r.end_line as usize),
        ) else {
            return Err(Skip::RangeOutOfBounds);
        };
        if byte_column(start, r.start_char, doc.position_encoding).is_none()
            || byte_column(end, r.end_char, doc.position_encoding).is_none()
        {
            return Err(Skip::RangeOutOfBounds);
        }
        // A zero-width definition (scip-typescript's module symbol at 0:0)
        // spells nothing, so it cannot prove staleness.
        if occ.roles & ROLE_DEFINITION == 0
            || r.start_line != r.end_line
            || r.start_char == r.end_char
        {
            continue;
        }
        let Some(parsed) = symbol::parse(&occ.symbol) else {
            continue;
        };
        // A type parameter's range spells the parameter, not the item that
        // `primary` names.
        if parsed.last_is_type_parameter() {
            continue;
        }
        let Some(primary) = parsed.primary().cloned() else {
            continue;
        };
        if is_synthetic_name(&primary.name) {
            continue;
        }
        let text = slice(start, r.start_char, r.end_char, doc.position_encoding);
        let raw = format!("r#{}", primary.name);
        if text != Some(primary.name.as_str()) && text != Some(raw.as_str()) {
            return Err(Skip::DefinitionMismatch);
        }
    }
    Ok(())
}

/// Descriptor names that never appear verbatim at their range: empty names,
/// rust-analyzer's `impl` blocks, and angle-bracketed names such as
/// scip-typescript's `<constructor>`.
fn is_synthetic_name(name: &str) -> bool {
    name.is_empty() || name == "impl" || (name.starts_with('<') && name.ends_with('>'))
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Decodes a `file://` URI into a path. Percent escapes are decoded; other
/// schemes, hosts and invalid escapes return `None`.
fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    if !rest.starts_with('/') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= limit).then_some(bytes))
}

fn ms(start: Instant) -> f64 {
    (start.elapsed().as_secs_f64() * 1e6).round() / 1e3
}

struct Samples {
    limit: usize,
    items: BTreeMap<&'static str, Vec<Value>>,
}

impl Samples {
    fn push(&mut self, category: &'static str, value: impl FnOnce() -> Value) {
        let list = self.items.entry(category).or_default();
        if list.len() < self.limit {
            list.push(value());
        }
    }
}

/// A SCIP reference mapped onto the graph.
struct ScipRef {
    file: String,
    line: u32,
    target: String,
    symbol: String,
    source: Option<String>,
    import: bool,
    call_syntax: bool,
}

/// Classification of one graph edge against SCIP.
#[derive(Default)]
struct Tally {
    confirmed: u64,
    contradicted: u64,
    unverifiable: u64,
}

impl Tally {
    fn json(&self) -> Value {
        let decided = self.confirmed + self.contradicted;
        json!({
            "confirmed": self.confirmed,
            "contradicted": self.contradicted,
            "unverifiable": self.unverifiable,
            "precision": ratio(self.confirmed, decided),
        })
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(num: u64, den: u64) -> Value {
    if den == 0 {
        Value::Null
    } else {
        json!(((num as f64 / den as f64) * 1e4).round() / 1e4)
    }
}

/// Compares a SCIP index with the project's tokensave graph and returns the
/// JSON report. See the module documentation for the method.
pub async fn compare(options: &Options) -> Result<Value> {
    let total = Instant::now();
    let root = options.root.canonicalize()?;
    let samples_limit = options.samples.min(1000);

    // 1. Read and decode the index under budget.
    let t = Instant::now();
    let meta = std::fs::metadata(&options.index)?;
    if !meta.is_file() {
        return Err(invalid("SCIP index must be a regular file"));
    }
    if meta.len() > options.max_index_bytes {
        return Err(invalid(format!(
            "SCIP index is {} bytes, over the {}-byte limit",
            meta.len(),
            options.max_index_bytes
        )));
    }
    let bytes = read_bounded(&options.index, options.max_index_bytes)?
        .ok_or_else(|| invalid("SCIP index grew past the byte limit while reading"))?;
    let read_ms = ms(t);
    let t = Instant::now();
    let index = wire::decode_index(&bytes, &Limits::default())
        .map_err(|e| invalid(format!("malformed SCIP index: {e}")))?;
    let decode_ms = ms(t);
    let index_bytes = bytes.len();
    drop(bytes);

    // 2. Refuse incompatible indexes before touching the graph.
    if index.metadata.version != 0 {
        return Err(invalid(format!(
            "unsupported SCIP protocol version {}",
            index.metadata.version
        )));
    }
    let root_status = match file_uri_path(&index.metadata.project_root) {
        Some(p) if p.canonicalize().ok().as_deref() == Some(root.as_path()) => "match",
        _ if options.rebase_root => "rebased",
        Some(_) => {
            return Err(invalid(
                "SCIP project_root names a different directory; pass --rebase-root to verify its documents against this checkout",
            ))
        }
        None => {
            return Err(invalid(
                "SCIP project_root is not a file:// URI; pass --rebase-root to verify its documents against this checkout",
            ))
        }
    };

    // 3. Open the graph read-only, exactly as the Graphify export does.
    let t = Instant::now();
    let db_path = crate::graphify::database_path(&root)?;
    // Sync commits files, nodes and edges in separate transactions, so a
    // snapshot taken mid-sync can pass the per-file hash check while that
    // file's edges are not yet resolved. Holding the sync lock for the whole
    // comparison (well under a second on measured crates) keeps every
    // snapshot complete; a running sync makes the comparison fail instead.
    let _sync_lock = crate::tokensave::try_acquire_sync_lock(&root)?;
    crate::db::compatibility::preflight(&db_path, crate::db::migrations::latest_version()).await?;
    let db_bytes = std::fs::metadata(&db_path).map_or(0, |m| m.len());
    let db = libsql::Builder::new_local(&db_path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await?;
    let conn = db.connect()?;
    conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-8192;")
        .await?;
    let tx = conn.transaction().await?;
    let mut rows = tx.query("PRAGMA user_version", ()).await?;
    let version = rows
        .next()
        .await?
        .ok_or_else(|| invalid("missing schema version"))?
        .get::<i64>(0)?;
    drop(rows);
    if version != i64::from(crate::db::migrations::latest_version()) {
        return Err(invalid("index schema is not current; run sync separately"));
    }

    // 4. Verify each document and load the nodes of the comparable ones.
    let mut samples = Samples {
        limit: samples_limit,
        items: BTreeMap::new(),
    };
    let mut skipped: BTreeMap<Skip, u64> = BTreeMap::new();
    let mut docs: BTreeMap<String, (&Document, Vec<String>)> = BTreeMap::new();
    let mut duplicate_documents = 0u64;
    let mut nodes: HashMap<String, Vec<NodeRow>> = HashMap::new();
    for doc in &index.documents {
        if crate::cancel::is_requested() {
            return Err(invalid("comparison cancelled"));
        }
        let path = doc.relative_path.as_str();
        let mut skip = |reason: Skip| {
            *skipped.entry(reason).or_default() += 1;
            samples.push(
                "skipped_documents",
                || json!({"path": path, "reason": reason.as_str()}),
            );
        };
        if docs.contains_key(path) {
            // The spec allows one Document per path; refusing to merge keeps
            // the comparison deterministic.
            duplicate_documents += 1;
            continue;
        }
        if !safe_relative(path) {
            skip(Skip::UnsafePath);
            continue;
        }
        let Ok(source_path) = root.join(path).canonicalize() else {
            skip(Skip::Missing);
            continue;
        };
        if !source_path.starts_with(&root) || !source_path.is_file() {
            skip(Skip::UnsafePath);
            continue;
        }
        let Some(raw) = read_bounded(&source_path, SOURCE_BYTES)? else {
            skip(Skip::TooLarge);
            continue;
        };
        let Ok(source) = String::from_utf8(raw) else {
            skip(Skip::NotUtf8);
            continue;
        };
        let mut hashes = tx
            .query(
                "SELECT content_hash FROM files WHERE path=?1 AND kind='code'",
                [path],
            )
            .await?;
        let indexed: Option<String> = match hashes.next().await? {
            Some(row) => Some(row.get(0)?),
            None => None,
        };
        drop(hashes);
        let source_hash = crate::sync::content_hash(&source);
        match indexed {
            None => {
                skip(Skip::NotIndexed);
                continue;
            }
            Some(h) if h != source_hash => {
                skip(Skip::GraphStale);
                continue;
            }
            Some(_) => {}
        }
        if doc
            .text_hash
            .as_ref()
            .is_some_and(|hash| hash != &source_hash)
        {
            skip(Skip::SourceMismatch);
            continue;
        }
        let lines: Vec<String> = source
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
            .collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        if let Err(reason) = verify(doc, &refs) {
            skip(reason);
            continue;
        }
        let mut rows = tx
            .query(
                "SELECT id, name, kind, qualified_name, start_line, end_line FROM nodes WHERE file_path=?1",
                [path],
            )
            .await?;
        let mut file_nodes = Vec::new();
        while let Some(row) = rows.next().await? {
            file_nodes.push(NodeRow {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                qualified_name: row.get(3)?,
                start_line: u32::try_from(row.get::<i64>(4)?).unwrap_or(0),
                end_line: u32::try_from(row.get::<i64>(5)?).unwrap_or(0),
            });
        }
        drop(rows);
        nodes.insert(path.to_owned(), file_nodes);
        docs.insert(path.to_owned(), (doc, lines));
    }
    let db_load_ms = ms(t);

    // 5. Map SCIP definitions onto nodes: same file, same name, the node's
    //    line span holds the definition; the narrowest such node wins.
    let t = Instant::now();
    let mut by_symbol: HashMap<&str, &NodeRow> = HashMap::new();
    let mut definitions = 0u64;
    let mut unmapped_definitions = 0u64;
    let mut unmapped_by_suffix: BTreeMap<String, u64> = BTreeMap::new();
    for (path, (doc, _)) in &docs {
        let file_nodes = &nodes[path];
        for occ in &doc.occurrences {
            if occ.roles & ROLE_DEFINITION == 0 {
                continue;
            }
            let Some(primary) = symbol::parse(&occ.symbol).and_then(|s| s.primary().cloned())
            else {
                continue;
            };
            let Some(r) = occ.range else { continue };
            definitions += 1;
            // Node and edge lines are zero-based, like SCIP's.
            let line = r.start_line;
            let best = file_nodes
                .iter()
                .filter(|n| {
                    !ignored_kind(&n.kind)
                        && n.name == primary.name
                        && n.start_line <= line
                        && line <= n.end_line
                })
                .min_by_key(|n| (n.span(), n.start_line));
            if let Some(node) = best {
                by_symbol.insert(occ.symbol.as_str(), node);
            } else {
                unmapped_definitions += 1;
                *unmapped_by_suffix
                    .entry(format!("{:?}", primary.suffix).to_lowercase())
                    .or_default() += 1;
                samples.push(
                    "unmapped_definitions",
                    || json!({"file": path, "line": line + 1, "symbol": occ.symbol}),
                );
            }
        }
    }

    // 6. Map references onto `(file, line, target)` keys.
    let mut scip_refs: Vec<ScipRef> = Vec::new();
    let mut external_refs: HashMap<(String, u32), Vec<String>> = HashMap::new();
    let mut local_refs = 0u64;
    for (path, (doc, lines)) in &docs {
        let file_nodes = &nodes[path];
        for occ in &doc.occurrences {
            if occ.roles & ROLE_DEFINITION != 0 {
                continue;
            }
            let Some(r) = occ.range else { continue };
            let line = r.start_line;
            let Some(parsed) = symbol::parse(&occ.symbol) else {
                continue;
            };
            let Some(primary) = parsed.primary() else {
                local_refs += 1;
                continue;
            };
            let Some(target) = by_symbol.get(occ.symbol.as_str()) else {
                if !matches!(primary.suffix, Suffix::Parameter | Suffix::TypeParameter) {
                    external_refs
                        .entry((path.clone(), line))
                        .or_default()
                        .push(primary.name.clone());
                }
                continue;
            };
            let source = file_nodes
                .iter()
                .filter(|n| !ignored_kind(&n.kind) && n.start_line <= line && line <= n.end_line)
                .min_by_key(|n| (n.span(), std::cmp::Reverse(n.start_line)))
                .map(|n| n.id.clone());
            let call_syntax = lines
                .get(r.end_line as usize)
                .is_some_and(|line| follows_call_syntax(line, r.end_char, doc.position_encoding));
            scip_refs.push(ScipRef {
                file: path.clone(),
                line,
                target: target.id.clone(),
                symbol: occ.symbol.clone(),
                source,
                import: occ.roles & ROLE_IMPORT != 0,
                call_syntax,
            });
        }
    }
    let map_ms = ms(t);

    // 7. Classify every non-containment graph edge whose source is in a
    //    verified document.
    let t = Instant::now();
    let id_to_node: HashMap<&str, &NodeRow> = nodes
        .values()
        .flatten()
        .map(|n| (n.id.as_str(), n))
        .collect();
    // Targets are compared by qualified name, not node id: a Rust type and
    // each of its `impl` blocks are separate nodes with one qualified name,
    // and an edge to either is the same binding.
    let qualified = |id: &str| id_to_node.get(id).map_or("", |n| n.qualified_name.as_str());
    let ref_keys: HashSet<(&str, u32, &str)> = scip_refs
        .iter()
        .map(|r| (r.file.as_str(), r.line, qualified(&r.target)))
        .collect();
    let mut ref_names: HashMap<(&str, u32), Vec<(&str, &str)>> = HashMap::new();
    for r in &scip_refs {
        if let Some(n) = id_to_node.get(r.target.as_str()) {
            ref_names
                .entry((r.file.as_str(), r.line))
                .or_default()
                .push((n.name.as_str(), n.qualified_name.as_str()));
        }
    }
    let mut overall = Tally::default();
    let mut by_resolver: BTreeMap<&'static str, Tally> = BTreeMap::new();
    let mut by_kind: BTreeMap<String, Tally> = BTreeMap::new();
    let mut edge_keys: HashSet<(String, u32, String)> = HashSet::new();
    let mut edges_without_line = 0u64;
    let mut implements_edges: HashSet<(String, String)> = HashSet::new();
    for path in docs.keys() {
        if crate::cancel::is_requested() {
            return Err(invalid("comparison cancelled"));
        }
        let mut rows = tx
            .query(
                "SELECT e.source, e.kind, e.line, e.resolved_by, t.name, t.qualified_name, t.file_path \
                 FROM edges e JOIN nodes s ON s.id = e.source JOIN nodes t ON t.id = e.target \
                 WHERE s.file_path = ?1 AND e.kind != 'contains'",
                [path.as_str()],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let source: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let line: Option<i64> = row.get(2)?;
            let resolved_by = row
                .get::<Option<i64>>(3)?
                .and_then(ResolvedBy::from_code)
                .map_or("extractor", ResolvedBy::as_str);
            let target_name: String = row.get(4)?;
            let target_qualified: String = row.get(5)?;
            let target_file: String = row.get(6)?;
            if matches!(kind.as_str(), "implements" | "extends") {
                let source_qualified = id_to_node
                    .get(source.as_str())
                    .map_or_else(String::new, |n| n.qualified_name.clone());
                implements_edges.insert((source_qualified, target_qualified.clone()));
            }
            let Some(line) = line.and_then(|l| u32::try_from(l).ok()) else {
                edges_without_line += 1;
                continue;
            };
            edge_keys.insert((path.clone(), line, target_qualified.clone()));
            let verdict = if ref_keys.contains(&(path.as_str(), line, target_qualified.as_str())) {
                "confirmed"
            } else if let Some(expected) = ref_names
                .get(&(path.as_str(), line))
                .map(|v| {
                    v.iter()
                        .filter(|(n, _)| *n == target_name)
                        .map(|(_, q)| *q)
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .filter(|v| !v.is_empty())
            {
                samples.push("contradicted_edges", || {
                    json!({"file": path, "line": line + 1, "kind": kind, "resolved_by": resolved_by,
                           "tokensave_target": target_qualified, "tokensave_target_file": target_file,
                           "scip_targets": expected})
                });
                "contradicted"
            } else if external_refs
                .get(&(path.clone(), line))
                .is_some_and(|v| v.contains(&target_name))
            {
                samples.push("contradicted_edges", || {
                    json!({"file": path, "line": line + 1, "kind": kind, "resolved_by": resolved_by,
                           "tokensave_target": target_qualified, "tokensave_target_file": target_file,
                           "scip_targets": ["<outside the project>"]})
                });
                "contradicted"
            } else {
                "unverifiable"
            };
            for tally in [
                &mut overall,
                by_resolver.entry(resolved_by).or_default(),
                by_kind.entry(kind.clone()).or_default(),
            ] {
                match verdict {
                    "confirmed" => tally.confirmed += 1,
                    "contradicted" => tally.contradicted += 1,
                    _ => tally.unverifiable += 1,
                }
            }
        }
    }

    // 8. SCIP references no edge covers: recall and the dry-run import plan.
    let mut eligible = 0u64;
    let mut covered = 0u64;
    // [call syntax, other] × [eligible, covered]: tokensave models calls
    // but not every type mention, so recall is reported for each.
    let mut by_syntax = [[0u64; 2]; 2];
    let mut imports = 0u64;
    let mut self_refs = 0u64;
    let mut proposed = HashSet::new();
    let mut proposed_calls = 0u64;
    for r in &scip_refs {
        if r.import {
            imports += 1;
            continue;
        }
        let Some(source) = r.source.as_deref() else {
            continue;
        };
        if source == r.target {
            self_refs += 1;
            continue;
        }
        eligible += 1;
        let syntax = usize::from(!r.call_syntax);
        by_syntax[syntax][0] += 1;
        if edge_keys.contains(&(r.file.clone(), r.line, qualified(&r.target).to_owned())) {
            covered += 1;
            by_syntax[syntax][1] += 1;
            continue;
        }
        let edge_kind = if r.call_syntax { "calls" } else { "uses" };
        if proposed.insert((source, r.target.as_str(), edge_kind, r.line)) {
            if r.call_syntax {
                proposed_calls += 1;
            }
            samples.push("proposed_import", || {
                let target = id_to_node.get(r.target.as_str());
                json!({"file": r.file, "line": r.line + 1, "kind": edge_kind,
                       "source_id": source, "target_id": r.target,
                       "source": id_to_node.get(source).map(|n| n.qualified_name.as_str()),
                       "target": target.map(|n| n.qualified_name.as_str()),
                       "provenance": {"origin": "scip", "tool": index.metadata.tool_name,
                                      "tool_version": index.metadata.tool_version,
                                      "symbol": r.symbol}})
            });
        }
    }

    // 9. Implementation relationships against implements/extends edges.
    let mut rel_total = 0u64;
    let mut rel_covered = 0u64;
    for (doc, _) in docs.values() {
        for info in &doc.symbols {
            let Some(from) = by_symbol.get(info.symbol.as_str()) else {
                continue;
            };
            for rel in info.relationships.iter().filter(|r| r.is_implementation) {
                let Some(to) = by_symbol.get(rel.symbol.as_str()) else {
                    continue;
                };
                rel_total += 1;
                if implements_edges
                    .contains(&(from.qualified_name.clone(), to.qualified_name.clone()))
                {
                    rel_covered += 1;
                } else {
                    samples.push("missing_implementations", || {
                        json!({"implementer": from.qualified_name, "implemented": to.qualified_name})
                    });
                }
            }
        }
    }
    drop(tx);
    let compare_ms = ms(t);

    let documents_total = index.documents.len() as u64;
    Ok(json!({
        "report_version": REPORT_VERSION,
        "mode": "read_only_comparison",
        "index": {
            "tool": index.metadata.tool_name,
            "tool_version": index.metadata.tool_version,
            "protocol_version": index.metadata.version,
            "text_encoding": index.metadata.text_encoding,
            "project_root": root_status,
            "bytes": index_bytes,
            "external_symbols": index.external_symbols,
        },
        "documents": {
            "total": documents_total,
            "compared": docs.len(),
            "duplicates_ignored": duplicate_documents,
            "skipped": skipped.iter().map(|(k, v)| (k.as_str(), *v)).collect::<BTreeMap<_, _>>(),
        },
        "definitions": {
            "total": definitions,
            "mapped": definitions - unmapped_definitions,
            "unmapped": unmapped_definitions,
            "unmapped_by_descriptor": unmapped_by_suffix,
        },
        "references": {
            "mapped_to_project_nodes": scip_refs.len(),
            "imports": imports,
            "self_references": self_refs,
            "locals_ignored": local_refs,
            "eligible": eligible,
            "covered_by_tokensave": covered,
            "recall": ratio(covered, eligible),
            "call_syntax": {"eligible": by_syntax[0][0], "covered": by_syntax[0][1],
                            "recall": ratio(by_syntax[0][1], by_syntax[0][0])},
            "other": {"eligible": by_syntax[1][0], "covered": by_syntax[1][1],
                      "recall": ratio(by_syntax[1][1], by_syntax[1][0])},
        },
        "tokensave_edges": {
            "classified": overall.json(),
            "without_line": edges_without_line,
            "by_resolved_by": by_resolver.iter().map(|(k, v)| (*k, v.json())).collect::<BTreeMap<_, _>>(),
            "by_kind": by_kind.iter().map(|(k, v)| (k.clone(), v.json())).collect::<BTreeMap<_, _>>(),
        },
        "implementations": {
            "scip_relationships": rel_total,
            "covered_by_tokensave": rel_covered,
        },
        "proposed_import": {
            "edges": proposed.len(),
            "calls": proposed_calls,
            "uses": proposed.len() as u64 - proposed_calls,
            "applied": false,
        },
        "cost": {
            "read_ms": read_ms,
            "decode_ms": decode_ms,
            "db_load_ms": db_load_ms,
            "map_ms": map_ms,
            "compare_ms": compare_ms,
            "total_ms": ms(total),
            "tokensave_db_bytes": db_bytes,
        },
        "samples": samples.items,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::scip::wire::{Occurrence, Range};

    fn doc(occurrences: Vec<Occurrence>) -> Document {
        Document {
            relative_path: "a.rs".into(),
            occurrences,
            ..Document::default()
        }
    }

    fn occ(line: u32, start: u32, end: u32, symbol: &str, roles: i32) -> Occurrence {
        Occurrence {
            range: Some(Range {
                start_line: line,
                start_char: start,
                end_line: line,
                end_char: end,
            }),
            symbol: symbol.into(),
            roles,
        }
    }

    #[test]
    fn verify_detects_stale_and_out_of_bounds_ranges() {
        let lines = ["fn alpha() {}", "fn r#type() {}"];
        let ok = doc(vec![
            occ(0, 3, 8, "s m p v alpha().", ROLE_DEFINITION),
            occ(1, 3, 9, "s m p v type().", ROLE_DEFINITION),
        ]);
        assert_eq!(verify(&ok, &lines), Ok(()));
        let renamed = doc(vec![occ(0, 3, 8, "s m p v beta().", ROLE_DEFINITION)]);
        assert_eq!(verify(&renamed, &lines), Err(Skip::DefinitionMismatch));
        let shifted = doc(vec![occ(5, 0, 1, "s m p v alpha().", 0)]);
        assert_eq!(verify(&shifted, &lines), Err(Skip::RangeOutOfBounds));
        let past_end = doc(vec![occ(0, 3, 80, "s m p v x.", 0)]);
        assert_eq!(verify(&past_end, &lines), Err(Skip::RangeOutOfBounds));
        let mut malformed = doc(vec![occ(0, 0, 1, "s m p v x.", 0)]);
        malformed.occurrences[0].range = None;
        assert_eq!(verify(&malformed, &lines), Err(Skip::MalformedRange));
        let mut encoding = doc(vec![]);
        encoding.position_encoding = 9;
        assert_eq!(verify(&encoding, &lines), Err(Skip::UnsupportedEncoding));
    }

    #[test]
    fn verify_ignores_definitions_that_spell_no_name() {
        // Shapes scip-typescript 0.6.1 emits: a zero-width module symbol, a
        // synthetic `<constructor>` and a type parameter.
        let lines = ["export class Box<T> {", "  constructor() {}", "}"];
        let ok = doc(vec![
            occ(0, 0, 0, "s m p v src/`box.ts`/", ROLE_DEFINITION),
            occ(0, 13, 16, "s m p v src/`box.ts`/Box#", ROLE_DEFINITION),
            occ(0, 17, 18, "s m p v src/`box.ts`/Box#[T]", ROLE_DEFINITION),
            occ(
                1,
                2,
                13,
                "s m p v src/`box.ts`/Box#`<constructor>`().",
                ROLE_DEFINITION,
            ),
        ]);
        assert_eq!(verify(&ok, &lines), Ok(()));
        let renamed = doc(vec![occ(
            0,
            13,
            16,
            "s m p v src/`box.ts`/Bag#",
            ROLE_DEFINITION,
        )]);
        assert_eq!(verify(&renamed, &lines), Err(Skip::DefinitionMismatch));
    }

    #[test]
    fn slices_by_position_encoding() {
        let line = "é😀x";
        assert_eq!(slice(line, 6, 7, 1), Some("x"));
        assert_eq!(slice(line, 3, 4, 2), Some("x"));
        assert_eq!(slice(line, 2, 3, 3), Some("x"));
        assert_eq!(slice(line, 1, 2, 1), None, "inside a UTF-8 sequence");
        assert_eq!(slice(line, 2, 3, 2), None, "inside a surrogate pair");
    }

    #[test]
    fn detects_call_syntax_after_multibyte_text() {
        let line = "é😀helper()";
        assert!(follows_call_syntax(line, 12, 1));
        assert!(follows_call_syntax(line, 9, 2));
        assert!(follows_call_syntax(line, 8, 3));
        assert!(!follows_call_syntax(line, 2, 2));
    }

    #[test]
    fn decodes_file_uris_and_rejects_other_forms() {
        assert_eq!(
            file_uri_path("file:///a%20b/c"),
            Some(PathBuf::from("/a b/c"))
        );
        assert!(file_uri_path("https://x/y").is_none());
        assert!(file_uri_path("file://host/y").is_none());
        assert!(file_uri_path("file:///bad%zz").is_none());
    }

    #[test]
    fn rejects_unsafe_document_paths() {
        for bad in [
            "",
            "../x.rs",
            "/etc/passwd",
            "a\\b.rs",
            "a/../../b.rs",
            "./a.rs",
        ] {
            assert!(!safe_relative(bad), "{bad:?}");
        }
        assert!(safe_relative("src/a.rs"));
    }
}
