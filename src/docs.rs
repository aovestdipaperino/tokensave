// Rust guideline compliant 2026-07-26
//! Companion documentation discovery (#154, phase 1).
//!
//! Large legacy files are expensive for an agent to read in full when a short
//! prose summary would answer the question. This module discovers Markdown
//! docs that describe source files and resolves them to the set of files each
//! one covers, so the index can record the mapping and tools can point an
//! agent at the summary before it reads a 3000-line class.
//!
//! Two conventions are supported, deliberately both:
//!
//! * **Sidecar** — `Foo.cs` alongside `Foo.readme.md`. Zero configuration, the
//!   doc travels with the code in review, discovery is a filename match. It
//!   cannot express one doc covering many files.
//! * **Docs directory** — `tokensave-docs/` by default (overridable via the
//!   `docs_dir` config field), where each Markdown file carries YAML front
//!   matter with an `applies_to` list of globs. This keeps the source tree
//!   clean and expresses doc-to-many-files coverage, which is the
//!   `**/*.es8.cs` vs `**/*.es7.cs` case from the issue.
//!
//! Both resolve to the same internal shape: a [`DocFile`] plus the set of
//! project-relative source paths it covers.
//!
//! Section-level anchors (line ranges or symbol names) are deliberately out of
//! scope here; phase 1 is whole-doc granularity.

use std::collections::BTreeSet;
use std::path::Path;

/// Default directory (relative to the project root) scanned for docs that
/// declare their coverage via front matter.
pub const DEFAULT_DOCS_DIR: &str = "tokensave-docs";

/// Filename suffix that marks a sidecar doc: `Foo.cs` -> `Foo.readme.md`.
const SIDECAR_SUFFIX: &str = ".readme.md";

/// A discovered documentation file and the source files it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocFile {
    /// Project-relative path of the Markdown doc itself.
    pub path: String,
    /// Project-relative paths of the source files this doc documents, sorted
    /// and deduplicated.
    pub covers: Vec<String>,
    /// How the doc was discovered.
    pub origin: DocOrigin,
}

/// Which convention produced a [`DocFile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocOrigin {
    /// `Foo.readme.md` next to `Foo.cs`.
    Sidecar,
    /// A file under the docs directory with an `applies_to` front matter list.
    DocsDir,
}

impl DocOrigin {
    /// Stable string form used in tool output.
    pub fn as_str(self) -> &'static str {
        match self {
            DocOrigin::Sidecar => "sidecar",
            DocOrigin::DocsDir => "docs_dir",
        }
    }
}

/// Returns the source path a sidecar doc documents, if `path` is a sidecar.
///
/// `src/Foo.readme.md` documents `src/Foo.*`; the extension of the covered
/// file is not encoded in the doc name, so the caller matches the returned
/// stem against indexed files.
pub fn sidecar_stem(path: &str) -> Option<&str> {
    let normalized = path;
    // Case-insensitive suffix match without allocating for the common miss.
    let len = normalized.len();
    if len <= SIDECAR_SUFFIX.len() {
        return None;
    }
    // Compare the trailing bytes rather than splitting the `str` first: a
    // multi-byte character can straddle the split point and `split_at` panics
    // on a non-char boundary. The suffix is pure ASCII, so a byte match also
    // proves the split point is a char boundary and the slice below is safe.
    let split = len - SIDECAR_SUFFIX.len();
    let bytes = normalized.as_bytes();
    if !bytes[split..].eq_ignore_ascii_case(SIDECAR_SUFFIX.as_bytes()) {
        return None;
    }
    let stem = &normalized[..split];
    if stem.is_empty() {
        return None;
    }
    Some(stem)
}

/// Returns `true` when `path` lives under the configured docs directory.
pub fn is_in_docs_dir(path: &str, docs_dir: &str) -> bool {
    let dir = docs_dir.trim_end_matches('/');
    if dir.is_empty() {
        return false;
    }
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Parsed YAML front matter of a docs-directory file.
///
/// Only `applies_to` is read in phase 1; unknown keys are ignored so the
/// schema can grow without breaking existing docs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FrontMatter {
    /// Glob patterns naming the files this doc covers.
    pub applies_to: Vec<String>,
}

/// Extracts YAML front matter from the head of a Markdown document.
///
/// Front matter is the conventional `---` fenced block at the very start of
/// the file. Returns `None` when the document has no front matter at all,
/// which is the signal that a docs-directory file declares no coverage.
pub fn parse_front_matter(content: &str) -> Option<FrontMatter> {
    // A leading BOM is common in Windows-authored Markdown and would
    // otherwise hide the opening fence.
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let rest = content.strip_prefix("---")?;
    // The opening fence must be its own line.
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let mut body = String::new();
    let mut closed = false;
    for line in rest.lines() {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    if !closed {
        return None;
    }
    Some(FrontMatter {
        applies_to: parse_applies_to(&body),
    })
}

/// Reads the `applies_to` sequence out of a front-matter body.
///
/// Supports both YAML sequence forms, since doc authors write both:
///
/// ```yaml
/// applies_to:
///   - "**/*.es8.cs"
/// applies_to: ["**/*.es7.cs", "src/Legacy.cs"]
/// ```
///
/// A hand-rolled reader rather than a YAML dependency: the schema is one
/// key holding a list of strings, and phase 1 does not justify pulling in a
/// parser for it. Anything it cannot understand yields no globs, which
/// degrades to "this doc covers nothing" rather than a hard error.
fn parse_applies_to(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false;
    for raw in body.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if in_block {
            let trimmed = line.trim_start();
            if let Some(item) = trimmed.strip_prefix("- ") {
                out.push(unquote(item.trim()));
                continue;
            }
            if trimmed == "-" {
                continue;
            }
            // Any other key at this point ends the sequence.
            in_block = false;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() != "applies_to" {
            continue;
        }
        let value = value.trim();
        if value.is_empty() {
            in_block = true;
            continue;
        }
        if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            out.extend(
                inner
                    .split(',')
                    .map(|item| unquote(item.trim()))
                    .filter(|item| !item.is_empty()),
            );
        } else {
            out.push(unquote(value));
        }
    }
    out.retain(|item| !item.is_empty());
    out
}

/// Strips one layer of matching single or double quotes.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

/// Resolves the coverage globs of a docs-directory file against the indexed
/// file list, returning matched project-relative paths.
///
/// Globs are matched against the full project-relative path. A pattern that
/// matches nothing contributes nothing — callers surface that as a doc
/// covering zero files rather than treating it as an error, because a glob can
/// legitimately go empty when files are removed.
pub fn resolve_globs(patterns: &[String], indexed_files: &[String]) -> Vec<String> {
    let compiled: Vec<glob::Pattern> = patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();
    if compiled.is_empty() {
        return Vec::new();
    }
    let mut matched: BTreeSet<&str> = BTreeSet::new();
    for file in indexed_files {
        if compiled.iter().any(|p| p.matches(file)) {
            matched.insert(file.as_str());
        }
    }
    matched.into_iter().map(ToString::to_string).collect()
}

/// Resolves a sidecar doc to the indexed source files sharing its stem.
///
/// `src/Foo.readme.md` covers every indexed `src/Foo.<ext>`. The doc itself is
/// never treated as its own subject, and a bare `src/Foo` with no extension is
/// not matched — a sidecar documents a source *file*, and requiring the dot
/// keeps `Foo.readme.md` from claiming an unrelated `Foo` directory entry.
pub fn resolve_sidecar(stem: &str, indexed_files: &[String]) -> Vec<String> {
    let prefix = format!("{stem}.");
    let mut matched: BTreeSet<&str> = BTreeSet::new();
    for file in indexed_files {
        if file.len() <= prefix.len() || !file.starts_with(&prefix) {
            continue;
        }
        // Never let a doc document another doc, including itself.
        if sidecar_stem(file).is_some() {
            continue;
        }
        // The remainder must be a bare extension, not a nested path.
        if file[prefix.len()..].contains('/') {
            continue;
        }
        matched.insert(file.as_str());
    }
    matched.into_iter().map(ToString::to_string).collect()
}

/// Builds the doc-coverage map for a project.
///
/// `markdown_files` and `indexed_files` are project-relative paths; `read` is
/// how a doc's content is fetched (injected so this stays pure and testable).
/// Docs covering no files are dropped: they carry no mapping worth indexing.
pub fn discover_docs<F>(
    markdown_files: &[String],
    indexed_files: &[String],
    docs_dir: &str,
    mut read: F,
) -> Vec<DocFile>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut docs: Vec<DocFile> = Vec::new();
    for path in markdown_files {
        if let Some(stem) = sidecar_stem(path) {
            let covers = resolve_sidecar(stem, indexed_files);
            if !covers.is_empty() {
                docs.push(DocFile {
                    path: path.clone(),
                    covers,
                    origin: DocOrigin::Sidecar,
                });
            }
            continue;
        }
        if !is_in_docs_dir(path, docs_dir) {
            continue;
        }
        let Some(content) = read(path) else { continue };
        let Some(front) = parse_front_matter(&content) else {
            continue;
        };
        let covers = resolve_globs(&front.applies_to, indexed_files);
        if !covers.is_empty() {
            docs.push(DocFile {
                path: path.clone(),
                covers,
                origin: DocOrigin::DocsDir,
            });
        }
    }
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    docs
}

/// Joins a project-relative path onto a root, for callers that need the
/// absolute location of a discovered doc.
pub fn absolute_doc_path(project_root: &Path, relative: &str) -> std::path::PathBuf {
    project_root.join(relative)
}

/// Extracts a one-line summary from a doc's content for the node's docstring.
///
/// Front matter and Markdown heading markers are skipped so the summary is
/// prose the agent can act on, not `# Title`. Returns `None` for a doc with no
/// prose body.
pub fn doc_summary(content: &str) -> Option<String> {
    // Cap so a doc with one very long unwrapped paragraph cannot bloat every
    // response that mentions the file it covers.
    const MAX: usize = 200;
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    // Skip a front-matter block if present.
    let body = if content.starts_with("---") {
        let mut lines = content.lines();
        lines.next();
        let mut rest = String::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line.trim_end() == "---" {
                closed = true;
                break;
            }
        }
        if closed {
            for line in lines {
                rest.push_str(line);
                rest.push('\n');
            }
            rest
        } else {
            content.to_string()
        }
    } else {
        content.to_string()
    };

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let summary = if trimmed.chars().count() > MAX {
            let cut: String = trimmed.chars().take(MAX).collect();
            format!("{cut}…")
        } else {
            trimmed.to_string()
        };
        return Some(summary);
    }
    None
}

/// Builds the `Doc` nodes and `Documents` edges for a set of discovered docs.
///
/// `file_node_ids` maps a project-relative source path to the id of its `File`
/// node. A covered file with no `File` node in the graph yields no edge —
/// not every extractor emits one, and a dangling edge is worse than a missing
/// one. `summaries` supplies the optional one-line docstring per doc path.
pub fn build_doc_graph<S: std::hash::BuildHasher>(
    docs: &[DocFile],
    file_node_ids: &std::collections::HashMap<String, String, S>,
    summaries: &std::collections::HashMap<String, String, S>,
) -> (Vec<crate::types::Node>, Vec<crate::types::Edge>) {
    use crate::types::{generate_node_id, Edge, EdgeKind, Node, NodeKind, Visibility};

    let mut nodes = Vec::with_capacity(docs.len());
    let mut edges = Vec::new();
    for doc in docs {
        let id = generate_node_id(&doc.path, &NodeKind::Doc, &doc.path, 0);
        let name = doc.path.rsplit('/').next().unwrap_or(&doc.path).to_string();
        nodes.push(Node {
            id: id.clone(),
            kind: NodeKind::Doc,
            name,
            qualified_name: doc.path.clone(),
            file_path: doc.path.clone(),
            start_line: 0,
            attrs_start_line: 0,
            end_line: 0,
            start_column: 0,
            end_column: 0,
            signature: Some(format!(
                "{} doc covering {} file(s)",
                doc.origin.as_str(),
                doc.covers.len()
            )),
            docstring: summaries.get(&doc.path).cloned(),
            visibility: Visibility::Pub,
            is_async: false,
            branches: 0,
            loops: 0,
            returns: 0,
            max_nesting: 0,
            unsafe_blocks: 0,
            unchecked_calls: 0,
            assertions: 0,
            cognitive_complexity: 0,
            distinct_operators: 0,
            distinct_operands: 0,
            total_operators: 0,
            total_operands: 0,
            updated_at: 0,
            parent_id: None,
        });
        for covered in &doc.covers {
            if let Some(target) = file_node_ids.get(covered) {
                edges.push(Edge {
                    source: id.clone(),
                    target: target.clone(),
                    kind: EdgeKind::Documents,
                    line: None,
                });
            }
        }
    }
    (nodes, edges)
}
