//! Export of the indexed code graph as an OKF v0.2 bundle.
//!
//! Each indexed source file becomes one concept (`type: Source File`) under
//! `files/`, mirroring the repository layout. Its body lists the file's
//! symbols and links to the files it depends on and that depend on it,
//! aggregated from cross-file graph edges. TokenSave-specific metadata uses
//! `tokensave_`-prefixed frontmatter keys, which OKF consumers preserve or
//! ignore (§4.1). A `tokensave-export.md` concept records provenance.
//!
//! The export reads the index read-only, never syncs or migrates it, and
//! writes the bundle to a new directory: an existing output path is
//! refused.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::json;
use serde_json::Value;

use super::time;
use super::yaml;
use super::SPEC_VERSION;
use crate::errors::Result;
use crate::errors::TokenSaveError;

/// The `type` of a concept describing one indexed source file.
pub const SOURCE_FILE_TYPE: &str = "Source File";
/// The `type` of the concept that records how the bundle was produced.
pub const EXPORT_TYPE: &str = "TokenSave Export";
/// Directory that holds source-file concepts.
pub const FILES_DIR: &str = "files";

const MAX_TEXT_CHARS: usize = 512;
const MAX_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;

/// Bounds and fixed inputs for an export.
#[derive(Debug, Clone)]
pub struct Options {
    /// Most source files exported; larger indexes are refused.
    pub max_files: usize,
    /// Most symbols listed per file; the rest are counted, not listed.
    pub max_symbols_per_file: usize,
    /// Most distinct file-to-file dependency rows read.
    pub max_links: usize,
    /// `generated.at` for every concept, as Unix seconds. `None` uses now.
    pub generated_at: Option<i64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_symbols_per_file: 500,
            max_links: 200_000,
            generated_at: None,
        }
    }
}

/// What an export wrote.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Summary {
    pub files: usize,
    pub symbols: usize,
    pub links: usize,
    /// Files whose symbol list was cut at `max_symbols_per_file`.
    pub truncated_files: usize,
    pub bytes: u64,
}

fn invalid(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

fn clip(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_TEXT_CHARS)
        .collect();
    if text.chars().count() > MAX_TEXT_CHARS {
        out.push('…');
    }
    out
}

/// Escapes text for markdown prose and table cells.
fn md_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in clip(text).chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '#' | '!' | '~'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A segment that is safe on every file system and matches the concept-ID
/// segment rule of the OKF reference implementation: `[A-Za-z0-9_][A-Za-z0-9_.-]*`.
pub(crate) fn safe_segment(segment: &str, is_leaf: bool) -> String {
    let mut out: String = segment
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !out
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        out.insert(0, '_');
    }
    // Trailing dots and spaces are stripped by Windows.
    while out.ends_with('.') {
        out.pop();
        out.push('_');
    }
    let reserved_windows = {
        let stem = out.split('.').next().unwrap_or("").to_ascii_lowercase();
        matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
            || ((stem.starts_with("com") || stem.starts_with("lpt"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit())
    };
    // `index.md` and `log.md` are reserved OKF names (§3.1).
    if reserved_windows || (is_leaf && matches!(out.to_ascii_lowercase().as_str(), "index" | "log"))
    {
        out.push('_');
    }
    out
}

/// Maps repository paths to unique concept IDs under `files/`.
///
/// Segments are made safe and deduplicated case-insensitively per
/// directory, so the bundle checks out intact on every file system.
#[derive(Default)]
struct IdMapper {
    assigned: HashMap<(String, String), String>,
    used: HashSet<(String, String)>,
}

impl IdMapper {
    fn map(&mut self, source_path: &str) -> String {
        let parts: Vec<&str> = source_path.split('/').filter(|p| !p.is_empty()).collect();
        let mut parent = FILES_DIR.to_string();
        for (index, part) in parts.iter().enumerate() {
            let is_leaf = index + 1 == parts.len();
            let key = (
                parent.clone(),
                format!("{}{part}", if is_leaf { "f:" } else { "d:" }),
            );
            let segment = if let Some(existing) = self.assigned.get(&key) {
                existing.clone()
            } else {
                let base = safe_segment(part, is_leaf);
                let mut candidate = base.clone();
                let mut n = 2;
                while self
                    .used
                    .contains(&(parent.clone(), candidate.to_ascii_lowercase()))
                {
                    candidate = format!("{base}-{n}");
                    n += 1;
                }
                self.used
                    .insert((parent.clone(), candidate.to_ascii_lowercase()));
                self.assigned.insert(key, candidate.clone());
                candidate
            };
            parent = format!("{parent}/{segment}");
        }
        parent
    }
}

struct FileRow {
    path: String,
    content_hash: String,
    size: i64,
    node_count: i64,
    id: String,
}

struct SymbolRow {
    kind: String,
    name: String,
    qualified_name: String,
    start_line: i64,
    end_line: i64,
    visibility: String,
}

/// File-to-file dependencies keyed by (source file, target file), with a
/// count per edge kind.
type LinkMap = BTreeMap<(usize, usize), BTreeMap<String, i64>>;

/// Exports the project's index at `root` into the new directory `output`.
pub async fn export(root: &Path, output: &Path, options: &Options) -> Result<Summary> {
    if output.exists() {
        return Err(invalid("output already exists; choose a new directory"));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(invalid("the output's parent directory does not exist"));
    }
    let root = root.canonicalize()?;
    let db_path = crate::graphify::database_path(&root)?;
    crate::db::compatibility::preflight(&db_path, crate::db::migrations::latest_version()).await?;
    let db = libsql::Builder::new_local(&db_path)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await?;
    let conn = db.connect()?;
    conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=1000; PRAGMA cache_size=-8192;")
        .await?;
    let tx = conn.transaction().await?;
    let schema_version = {
        let mut rows = tx.query("PRAGMA user_version", ()).await?;
        rows.next()
            .await?
            .ok_or_else(|| invalid("missing schema version"))?
            .get::<i64>(0)?
    };
    if schema_version != i64::from(crate::db::migrations::latest_version()) {
        return Err(invalid(
            "index schema is not current; run `tokensave sync` first",
        ));
    }

    let files = read_files(&tx, options.max_files).await?;
    let index_of: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.as_str(), i))
        .collect();
    let links = read_links(&tx, &index_of, options.max_links).await?;

    let staging = tempfile::Builder::new()
        .prefix(".tokensave-okf-")
        .tempdir_in(parent)?;
    let at = time::format_utc(options.generated_at.unwrap_or_else(time::now));
    let generated = json!({"by": format!("tokensave/{}", env!("CARGO_PKG_VERSION")), "at": at});
    let mut writer = BundleWriter {
        root: staging.path(),
        written: 0,
    };
    let mut summary = Summary::default();
    let mut incoming: HashMap<usize, Vec<Neighbour<'_>>> = HashMap::new();
    let mut outgoing: HashMap<usize, Vec<Neighbour<'_>>> = HashMap::new();
    for ((from, to), kinds) in &links {
        outgoing.entry(*from).or_default().push((*to, kinds));
        incoming.entry(*to).or_default().push((*from, kinds));
    }
    summary.links = links.len();

    let mut rows = tx
        .query(
            "SELECT n.file_path, substr(n.kind,1,64), substr(n.name,1,513), \
             substr(n.qualified_name,1,513), n.start_line, n.end_line, substr(n.visibility,1,32) \
             FROM nodes n JOIN files f ON f.path = n.file_path \
             WHERE f.kind = 'code' AND n.kind NOT IN ('file', 'doc') \
             ORDER BY n.file_path, n.start_line, n.id",
            (),
        )
        .await?;
    let mut current: Option<usize> = None;
    let mut symbols: Vec<SymbolRow> = Vec::new();
    let mut total = 0usize;
    let mut seen = vec![false; files.len()];
    loop {
        if crate::cancel::is_requested() {
            return Err(invalid("export cancelled"));
        }
        let row = rows.next().await?;
        let next = match &row {
            Some(row) => {
                let path: String = row.get(0)?;
                index_of.get(path.as_str()).copied()
            }
            None => None,
        };
        if row.is_none() || next != current {
            if let Some(index) = current {
                let concept = FileConcept {
                    index,
                    symbols: &symbols,
                    total,
                    outgoing: outgoing.get(&index).map(Vec::as_slice).unwrap_or_default(),
                    incoming: incoming.get(&index).map(Vec::as_slice).unwrap_or_default(),
                };
                write_file_concept(&mut writer, &files, &concept, &generated)?;
                summary.symbols += symbols.len();
                summary.truncated_files += usize::from(total > symbols.len());
                seen[index] = true;
                symbols.clear();
                total = 0;
            }
            current = next;
        }
        let Some(row) = row else { break };
        if next.is_none() {
            continue;
        }
        total += 1;
        if symbols.len() < options.max_symbols_per_file {
            symbols.push(SymbolRow {
                kind: row.get(1)?,
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                visibility: row.get(6)?,
            });
        }
    }
    drop(rows);
    for (index, done) in seen.iter().enumerate() {
        if !done {
            let concept = FileConcept {
                index,
                symbols: &[],
                total: 0,
                outgoing: outgoing.get(&index).map(Vec::as_slice).unwrap_or_default(),
                incoming: incoming.get(&index).map(Vec::as_slice).unwrap_or_default(),
            };
            write_file_concept(&mut writer, &files, &concept, &generated)?;
        }
    }
    tx.commit().await?;
    summary.files = files.len();

    let branch = crate::branch::current_branch(&root);
    let commit = crate::graphify::checkout_commit(&root);
    write_export_concept(
        &mut writer,
        &summary,
        schema_version,
        branch.as_deref(),
        commit.as_deref(),
        &generated,
        options,
    )?;
    write_indexes(&mut writer, &files)?;
    write_log(&mut writer, &at, &summary)?;
    summary.bytes = writer.written;

    if crate::cancel::is_requested() {
        return Err(invalid("export cancelled"));
    }
    if output.exists() {
        return Err(invalid(
            "output appeared during export; nothing was written",
        ));
    }
    let staged = staging.keep();
    if let Err(e) = std::fs::rename(&staged, output) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(e.into());
    }
    Ok(summary)
}

async fn read_files(tx: &libsql::Transaction, max_files: usize) -> Result<Vec<FileRow>> {
    let mut rows = tx
        .query(
            "SELECT path, substr(content_hash,1,128), size, node_count FROM files \
             WHERE kind = 'code' ORDER BY path LIMIT ?1",
            [(max_files + 1) as i64],
        )
        .await?;
    let mut mapper = IdMapper::default();
    let mut files = Vec::new();
    while let Some(row) = rows.next().await? {
        if files.len() == max_files {
            return Err(invalid(format!(
                "the index holds more than {max_files} source files; raise --max-files"
            )));
        }
        let path: String = row.get(0)?;
        files.push(FileRow {
            id: mapper.map(&path),
            content_hash: row.get(1)?,
            size: row.get(2)?,
            node_count: row.get(3)?,
            path,
        });
    }
    Ok(files)
}

async fn read_links(
    tx: &libsql::Transaction,
    index_of: &HashMap<&str, usize>,
    max_links: usize,
) -> Result<LinkMap> {
    let mut rows = tx
        .query(
            "SELECT s.file_path, t.file_path, substr(e.kind,1,64), COUNT(*) \
             FROM edges e JOIN nodes s ON s.id = e.source JOIN nodes t ON t.id = e.target \
             WHERE s.file_path <> t.file_path \
             GROUP BY s.file_path, t.file_path, e.kind \
             ORDER BY s.file_path, t.file_path, e.kind LIMIT ?1",
            [(max_links + 1) as i64],
        )
        .await?;
    let mut links = LinkMap::new();
    let mut count = 0usize;
    while let Some(row) = rows.next().await? {
        count += 1;
        if count > max_links {
            return Err(invalid(format!(
                "the index holds more than {max_links} file dependencies; raise --max-links"
            )));
        }
        let from: String = row.get(0)?;
        let to: String = row.get(1)?;
        let (Some(&from), Some(&to)) = (index_of.get(from.as_str()), index_of.get(to.as_str()))
        else {
            continue;
        };
        links
            .entry((from, to))
            .or_default()
            .insert(row.get(2)?, row.get(3)?);
    }
    Ok(links)
}

struct BundleWriter<'a> {
    root: &'a Path,
    written: u64,
}

impl BundleWriter<'_> {
    fn write(&mut self, relative: &str, content: &str) -> Result<()> {
        self.written = self.written.saturating_add(content.len() as u64);
        if self.written > MAX_OUTPUT_BYTES {
            return Err(invalid("export exceeds 512 MiB; refusing to continue"));
        }
        let path = super::bundle::join(self.root, relative);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }
}

fn frontmatter(fields: &[(&str, Value)]) -> String {
    let mut out = String::from("---\n");
    for (key, value) in fields {
        if value.is_null() {
            continue;
        }
        let _ = writeln!(out, "{key}: {}", yaml::flow(value));
    }
    out.push_str("---\n");
    out
}

fn link_line(files: &[FileRow], other: usize, kinds: &BTreeMap<String, i64>) -> String {
    let detail = kinds
        .iter()
        .map(|(kind, n)| format!("{} ×{n}", md_text(kind)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "* [{}](/{}.md) - {detail}\n",
        md_text(&files[other].path),
        files[other].id
    )
}

/// Another file and the edge kinds (with counts) linking it to this one.
type Neighbour<'a> = (usize, &'a BTreeMap<String, i64>);
type Neighbours<'a> = &'a [Neighbour<'a>];

/// The parts of one source-file concept.
struct FileConcept<'a> {
    index: usize,
    /// Listed symbols, at most `max_symbols_per_file`.
    symbols: &'a [SymbolRow],
    /// All symbols of the file, listed or not.
    total: usize,
    outgoing: Neighbours<'a>,
    incoming: Neighbours<'a>,
}

fn write_file_concept(
    writer: &mut BundleWriter<'_>,
    files: &[FileRow],
    concept: &FileConcept<'_>,
    generated: &Value,
) -> Result<()> {
    let file = &files[concept.index];
    let total = concept.total;
    let truncated = total > concept.symbols.len();
    let description = match total {
        0 => "Source file with no indexed symbols.".to_string(),
        1 => "Source file with 1 indexed symbol.".to_string(),
        n => format!("Source file with {n} indexed symbols."),
    };
    let mut text = frontmatter(&[
        ("type", json!(SOURCE_FILE_TYPE)),
        ("title", json!(clip(&file.path))),
        ("description", json!(description)),
        ("tags", json!(["tokensave/source-file"])),
        ("generated", generated.clone()),
        ("tokensave_path", json!(file.path)),
        ("tokensave_content_hash", json!(file.content_hash)),
        ("tokensave_size", json!(file.size)),
        ("tokensave_node_count", json!(file.node_count)),
        (
            "tokensave_symbols_truncated",
            if truncated { json!(true) } else { Value::Null },
        ),
    ]);
    let _ = write!(text, "\n# {}\n", md_text(&file.path));
    if !concept.symbols.is_empty() {
        text.push_str("\n# Symbols\n\n| Kind | Name | Qualified name | Lines | Visibility |\n");
        text.push_str("|------|------|----------------|-------|------------|\n");
        for symbol in concept.symbols {
            let _ = writeln!(
                text,
                "| {} | {} | {} | {}-{} | {} |",
                md_text(&symbol.kind),
                md_text(&symbol.name),
                md_text(&symbol.qualified_name),
                symbol.start_line,
                symbol.end_line,
                md_text(&symbol.visibility)
            );
        }
        if truncated {
            let _ = writeln!(
                text,
                "\n{} more symbols are not listed.",
                total - concept.symbols.len()
            );
        }
    }
    if !concept.outgoing.is_empty() {
        text.push_str("\n# Dependencies\n\nFiles this file references, by edge kind:\n\n");
        for (other, kinds) in concept.outgoing {
            text.push_str(&link_line(files, *other, kinds));
        }
    }
    if !concept.incoming.is_empty() {
        text.push_str("\n# Dependents\n\nFiles that reference this file, by edge kind:\n\n");
        for (other, kinds) in concept.incoming {
            text.push_str(&link_line(files, *other, kinds));
        }
    }
    writer.write(&format!("{}.md", file.id), &text)
}

fn write_export_concept(
    writer: &mut BundleWriter<'_>,
    summary: &Summary,
    schema_version: i64,
    branch: Option<&str>,
    commit: Option<&str>,
    generated: &Value,
    options: &Options,
) -> Result<()> {
    let mut text = frontmatter(&[
        ("type", json!(EXPORT_TYPE)),
        ("title", json!("TokenSave index export")),
        (
            "description",
            json!("How this bundle was produced from a TokenSave code index."),
        ),
        ("tags", json!(["tokensave/export"])),
        ("generated", generated.clone()),
        ("tokensave_schema_version", json!(schema_version)),
        ("tokensave_branch", json!(branch)),
        ("tokensave_commit", json!(commit)),
        ("tokensave_files", json!(summary.files)),
        ("tokensave_symbols", json!(summary.symbols)),
        ("tokensave_file_links", json!(summary.links)),
        ("tokensave_truncated_files", json!(summary.truncated_files)),
    ]);
    let _ = write!(
        text,
        "\n# TokenSave index export\n\n\
         This bundle describes {files} indexed source files as `{SOURCE_FILE_TYPE}` concepts \
         under [files](/{FILES_DIR}/index.md). Each lists its symbols and links to the files \
         it depends on and that depend on it, aggregated from cross-file graph edges.\n\n\
         # Scope\n\n\
         * Symbols listed per file: at most {max_symbols}.\n\
         * Omitted: source bodies, signatures, docstrings, metrics and unresolved references.\n\
         * Frontmatter keys starting with `tokensave_` are TokenSave extensions (OKF §4.1).\n",
        files = summary.files,
        max_symbols = options.max_symbols_per_file,
    );
    writer.write("tokensave-export.md", &text)
}

#[derive(Default)]
struct DirEntry {
    dirs: BTreeSet<String>,
    concepts: Vec<(String, String, String)>,
}

/// Writes an `index.md` for every directory (§8) and the bundle root.
fn write_indexes(writer: &mut BundleWriter<'_>, files: &[FileRow]) -> Result<()> {
    let mut dirs: BTreeMap<String, DirEntry> = BTreeMap::new();
    dirs.entry(FILES_DIR.to_string()).or_default();
    for file in files {
        let (dir, leaf) = file.id.rsplit_once('/').unwrap_or(("", file.id.as_str()));
        dirs.entry(dir.to_string()).or_default().concepts.push((
            format!("{leaf}.md"),
            file.path.clone(),
            format!("{} indexed symbols", file.node_count),
        ));
        let mut child = dir;
        while let Some((parent, name)) = child.rsplit_once('/') {
            dirs.entry(parent.to_string())
                .or_default()
                .dirs
                .insert(name.to_string());
            child = parent;
        }
    }
    for (dir, entry) in &dirs {
        let title = dir.rsplit('/').next().unwrap_or(dir);
        let mut text = format!("# {}\n", md_text(title));
        if !entry.dirs.is_empty() {
            text.push_str("\n## Directories\n\n");
            for name in &entry.dirs {
                let _ = writeln!(text, "* [{}/]({name}/index.md)", md_text(name));
            }
        }
        if !entry.concepts.is_empty() {
            text.push_str("\n## Source files\n\n");
            for (file, title, description) in &entry.concepts {
                let _ = writeln!(text, "* [{}]({file}) - {description}", md_text(title));
            }
        }
        writer.write(&format!("{dir}/index.md"), &text)?;
    }
    let root = format!(
        "---\nokf_version: \"{SPEC_VERSION}\"\n---\n\n# TokenSave code index\n\n\
         * [TokenSave index export](tokensave-export.md) - How this bundle was produced.\n\
         * [files/]({FILES_DIR}/index.md) - {} indexed source files, mirroring the repository \
         layout.\n",
        files.len()
    );
    writer.write("index.md", &root)
}

fn write_log(writer: &mut BundleWriter<'_>, at: &str, summary: &Summary) -> Result<()> {
    let date = at.get(..10).unwrap_or(at);
    let text = format!(
        "# Bundle history\n\n## {date}\n\n* **Export**: TokenSave exported {} source files \
         and {} file dependencies from its code index.\n",
        summary.files, summary.links
    );
    writer.write("log.md", &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_segments_avoid_reserved_and_unportable_names() {
        assert_eq!(safe_segment("main.rs", true), "main.rs");
        assert_eq!(safe_segment(".github", false), "_.github");
        assert_eq!(safe_segment("index", true), "index_");
        assert_eq!(safe_segment("index", false), "index");
        assert_eq!(safe_segment("a b(1).py", true), "a_b_1_.py");
        assert_eq!(safe_segment("con.h", true), "con.h_");
        assert_eq!(safe_segment("x.", true), "x_");
    }

    #[test]
    fn ids_are_unique_case_insensitively() {
        let mut mapper = IdMapper::default();
        assert_eq!(mapper.map("src/A.rs"), "files/src/A.rs");
        assert_eq!(mapper.map("src/a.rs"), "files/src/a.rs-2");
        assert_eq!(mapper.map("Src/b.rs"), "files/Src-2/b.rs");
        assert_eq!(mapper.map("src/c d.rs"), "files/src/c_d.rs");
        assert_eq!(mapper.map("src/c_d.rs"), "files/src/c_d.rs-2");
    }

    #[test]
    fn markdown_text_is_escaped() {
        assert_eq!(md_text("a|b_[c]\nd"), "a\\|b\\_\\[c\\] d");
    }
}
