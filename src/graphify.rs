//! Experimental, bounded, read-only single-file Graphify interchange.
use crate::errors::{Result, TokenSaveError};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const TARGET_COMMIT: &str = "0a8cd4e1e342235fe53e881fb5d591bf0460071f";
const SOURCE_BYTES: u64 = 16 * 1024 * 1024;
const OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const TEXT_CHARS: usize = 4096;

fn invalid(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

fn text(row: &libsql::Row, column: i32) -> Result<String> {
    let value: String = row.get(column)?;
    if value.chars().count() > TEXT_CHARS {
        return Err(invalid("Graphify export field exceeds 4096 characters"));
    }
    Ok(value)
}

fn source_hash(path: &Path) -> Result<String> {
    if !path.is_file() {
        return Err(invalid("selected source must be a regular file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > SOURCE_BYTES {
        return Err(invalid("selected source exceeds 16 MiB"));
    }
    // Match indexing's UTF-8 input contract; refuse rather than normalize bytes.
    let source = std::str::from_utf8(&bytes).map_err(|_| invalid("source must be UTF-8"))?;
    Ok(crate::sync::content_hash(source))
}

fn checkout_commit(root: &Path) -> Option<String> {
    gix::open(root)
        .ok()
        .and_then(|repo| repo.head_id().ok().map(|id| id.to_string()))
}

fn database_path(root: &Path) -> Result<PathBuf> {
    let dir = crate::config::get_tokensave_dir(root);
    let metadata = dir.join("branch-meta.json");
    if !metadata.exists() {
        return Ok(dir.join("tokensave.db"));
    }
    if !metadata.is_file() {
        return Err(invalid("branch metadata must be a regular file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(metadata)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("branch metadata exceeds 1 MiB"));
    }
    let meta: crate::branch_meta::BranchMeta = serde_json::from_slice(&bytes)?;
    let branch = crate::branch::current_branch(root)
        .ok_or_else(|| invalid("export requires a named, tracked checkout branch"))?;
    let entry = meta
        .branches
        .get(&branch)
        .ok_or_else(|| invalid("current branch is not indexed; refusing fallback graph"))?;
    let path = dir.join(&entry.db_file).canonicalize()?;
    if !path.starts_with(dir.canonicalize()?) {
        return Err(invalid("branch database escapes project state"));
    }
    Ok(path)
}

/// Export one source file and its internal edges. Over-budget graphs are refused.
///
/// The caller selects a new output path. Existing files are never overwritten.
/// No automatic indexing, migration, Graphify process, or network call occurs.
pub async fn export_file(
    root: &Path,
    file: &str,
    output: &Path,
    max_nodes: usize,
    max_edges: usize,
) -> Result<()> {
    if !(1..=1000).contains(&max_nodes) || !(1..=5000).contains(&max_edges) {
        return Err(invalid(
            "export budgets must be 1..=1000 nodes and 1..=5000 edges",
        ));
    }
    if file.is_empty()
        || file.contains('\\')
        || Path::new(file)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid(
            "--file requires a repository-relative path using / separators",
        ));
    }
    let extension = Path::new(file)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(
        extension.as_deref(),
        Some("md" | "markdown" | "mdx" | "txt" | "rst" | "pdf")
    ) {
        return Err(invalid(
            "document export is outside this code-only prototype",
        ));
    }
    let root = root.canonicalize()?;
    let source = root.join(file).canonicalize()?;
    if !source.starts_with(&root) {
        return Err(invalid("selected source escapes project"));
    }
    if output.exists() {
        return Err(invalid("output already exists; choose a new snapshot path"));
    }
    let before = source_hash(&source)?;
    let branch_before = crate::branch::current_branch(&root);
    let commit_before = checkout_commit(&root);
    let path = database_path(&root)?;
    crate::db::compatibility::preflight(&path, crate::db::migrations::latest_version()).await?;
    // Avoid Database::open_read_only's repository-wide trait-dispatch cache.
    let db = libsql::Builder::new_local(path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await?;
    let conn = db.connect()?;
    conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-2048;")
        .await?;
    let tx = conn.transaction().await?;
    let mut versions = tx.query("PRAGMA user_version", ()).await?;
    let version = versions
        .next()
        .await?
        .ok_or_else(|| invalid("missing schema version"))?
        .get::<i64>(0)?;
    if version != i64::from(crate::db::migrations::latest_version()) {
        return Err(invalid("index schema is not current; run sync separately"));
    }
    drop(versions);
    let mut files = tx
        .query(
            "SELECT substr(content_hash,1,65) FROM files WHERE path=?1 AND kind='code'",
            [file],
        )
        .await?;
    let indexed: String = files
        .next()
        .await?
        .ok_or_else(|| invalid("file is not indexed; run sync separately"))?
        .get(0)?;
    if indexed != before {
        return Err(invalid("selected source is stale; run sync separately"));
    }
    drop(files);
    let mut rows = tx.query(
        "SELECT substr(id,1,4097), substr(qualified_name,1,4097), substr(kind,1,4097), start_line, end_line FROM nodes WHERE file_path=?1 ORDER BY id LIMIT ?2",
        libsql::params![file, (max_nodes + 1) as i64]).await?;
    let mut nodes = Vec::new();
    let mut ids = HashSet::new();
    while let Some(row) = rows.next().await? {
        if crate::cancel::is_requested() {
            return Err(invalid("export cancelled"));
        }
        if nodes.len() == max_nodes {
            return Err(invalid("node budget exceeded; select a smaller file"));
        }
        let id = text(&row, 0)?;
        ids.insert(id.clone());
        nodes.push(json!({"id":id,"label":text(&row,1)?,"type":text(&row,2)?,"file_type":"code","source_file":file,"source_location":format!("L{}",row.get::<i64>(3)?),"tokensave_end_line":row.get::<i64>(4)?}));
    }
    drop(rows);
    if nodes.is_empty() {
        return Err(invalid("selected file has no indexed symbols"));
    }
    let mut rows = tx.query(
        "SELECT substr(e.source,1,4097), substr(e.target,1,4097), substr(e.kind,1,4097), e.line, e.resolved_by FROM edges e JOIN nodes s ON s.id=e.source JOIN nodes t ON t.id=e.target WHERE s.file_path=?1 AND t.file_path=?1 ORDER BY e.source,e.target,e.kind,e.line,e.id LIMIT ?2",
        libsql::params![file, (max_edges + 1) as i64]).await?;
    let mut links = Vec::new();
    while let Some(row) = rows.next().await? {
        if crate::cancel::is_requested() {
            return Err(invalid("export cancelled"));
        }
        if links.len() == max_edges {
            return Err(invalid("edge budget exceeded; select a smaller file"));
        }
        let resolved: Option<i64> = row.get(4)?;
        links.push(json!({"source":text(&row,0)?,"target":text(&row,1)?,"key":links.len(),"relation":text(&row,2)?,"confidence":if resolved.is_some(){"INFERRED"}else{"AMBIGUOUS"},"source_file":file,"tokensave_line":row.get::<Option<i64>>(3)?,"tokensave_resolved_by":resolved}));
    }
    drop(rows);
    tx.commit().await?;
    // Include graph contents so resolver changes produce a different namespace.
    let mut hash = Sha256::new();
    for part in [
        root.to_string_lossy().as_bytes(),
        before.as_bytes(),
        branch_before.as_deref().unwrap_or("").as_bytes(),
        commit_before.as_deref().unwrap_or("").as_bytes(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.update(b"nodes");
    serde_json::to_writer(HashWriter(&mut hash), &nodes)?;
    hash.update(b"links");
    serde_json::to_writer(HashWriter(&mut hash), &links)?;
    let generation = hex::encode(hash.finalize());
    let scoped = |id: &str| format!("{generation}:{id}");
    for node in &mut nodes {
        let raw = node["id"]
            .as_str()
            .ok_or_else(|| invalid("invalid internal node ID"))?;
        node["id"] = json!(scoped(raw));
    }
    for edge in &mut links {
        for endpoint in ["source", "target"] {
            let raw = edge[endpoint]
                .as_str()
                .ok_or_else(|| invalid("invalid internal edge ID"))?;
            if !ids.contains(raw) {
                return Err(invalid("edge endpoint missing from snapshot"));
            }
            edge[endpoint] = json!(scoped(raw));
        }
    }
    let data = json!({"directed":true,"multigraph":true,"graph":{
        "schema_version":1,"tokensave_interchange_version":1,"producer":"tokensave",
        "producer_version":env!("CARGO_PKG_VERSION"),"target_graphify_commit":TARGET_COMMIT,
        "tokensave_schema_version":version,"generation":generation,"source_hash":before,
        "branch":branch_before,"commit":commit_before,"scope":"single_file_internal_edges","source_file":file,
        "omissions":["cross_file_edges","documents","source_bodies","signatures","docstrings"],
        "freshness":"selected source hash checked; other resolver inputs unverified",
        "limits":{"nodes":max_nodes,"edges":max_edges,"bytes":OUTPUT_BYTES},"truncated":false
    },"nodes":nodes,"links":links});
    // Serializer writes through a capped sink, avoiding an unbounded JSON buffer.
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(
        CappedWriter {
            file: temp.as_file_mut(),
            left: OUTPUT_BYTES,
        },
        &data,
    )?;
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;
    if crate::cancel::is_requested() {
        return Err(invalid("export cancelled"));
    }
    if root.join(file).canonicalize()? != source
        || source_hash(&source)? != before
        || crate::branch::current_branch(&root) != branch_before
        || checkout_commit(&root) != commit_before
    {
        return Err(invalid("source or branch changed during export; retry"));
    }
    temp.persist_noclobber(output)
        .map_err(|e| TokenSaveError::Io(e.error))?;
    Ok(())
}

struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct CappedWriter<'a> {
    file: &'a mut std::fs::File,
    left: usize,
}
impl Write for CappedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.left {
            return Err(std::io::Error::other("export exceeds 8 MiB"));
        }
        let n = self.file.write(bytes)?;
        self.left -= n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn capped_writer_refuses_overflow_without_partial_write() {
        let mut file = tempfile::tempfile().unwrap();
        let mut sink = CappedWriter {
            file: &mut file,
            left: 3,
        };
        sink.write_all(b"ab").unwrap();
        assert!(sink.write_all(b"cd").is_err());
        assert_eq!(sink.left, 1);
        assert_eq!(sink.file.metadata().unwrap().len(), 2);
    }
}
