//! Reading an OKF bundle directory (OKF v0.2 §3).

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use super::document;
use super::document::BodyOutline;
use super::document::Document;
use super::document::FrontmatterError;

/// Resource limits for reading a bundle.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Most directory entries (files and directories) visited.
    pub max_entries: usize,
    /// Largest markdown file read, in bytes.
    pub max_file_bytes: u64,
    /// Largest total of markdown bytes read.
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_file_bytes: 4 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
        }
    }
}

/// The meaning OKF gives a markdown file by its name (§3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Concept,
    Index,
    Log,
}

/// The result of reading one markdown file.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Parsed {
        document: Document,
        outline: BodyOutline,
    },
    BadFrontmatter(FrontmatterError),
    NotUtf8,
}

/// One markdown file of a bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleFile {
    /// Bundle-relative path with `/` separators, e.g. `tables/orders.md`.
    pub path: String,
    pub role: Role,
    pub content: Content,
}

impl BundleFile {
    /// The concept ID: the path without its `.md` suffix (§2).
    pub fn concept_id(&self) -> &str {
        self.path.strip_suffix(".md").unwrap_or(&self.path)
    }

    /// The parsed frontmatter, when present and valid.
    pub fn frontmatter(&self) -> Option<&serde_json::Value> {
        match &self.content {
            Content::Parsed { document, .. } => document.frontmatter.as_ref(),
            _ => None,
        }
    }

    /// The parsed body outline, when the file was readable.
    pub fn outline(&self) -> Option<&BodyOutline> {
        match &self.content {
            Content::Parsed { outline, .. } => Some(outline),
            _ => None,
        }
    }
}

/// Why an entry was not read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Symlink,
    HiddenDirectory,
    TooLarge,
    Unreadable,
}

/// An entry the reader did not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub path: String,
    pub reason: SkipReason,
}

/// A bundle read from disk.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    /// Markdown files in path order.
    pub files: Vec<BundleFile>,
    /// Every regular file and directory path seen, for link checking.
    /// Directories end with `/`.
    pub entries: BTreeSet<String>,
    pub skipped: Vec<Skipped>,
    /// False when a limit stopped the walk early.
    pub complete: bool,
}

fn relative(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn role_of(name: &str) -> Role {
    match name {
        "index.md" => Role::Index,
        "log.md" => Role::Log,
        _ => Role::Concept,
    }
}

/// Reads every markdown file under `root`.
///
/// Symbolic links are never followed and directories whose name starts with
/// `.` (such as `.git`) are skipped. The walk stops at the configured limits
/// and then reports `complete: false`.
pub fn read(root: &Path, limits: Limits) -> std::io::Result<Bundle> {
    if !std::fs::metadata(root)?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "OKF bundle path must be a directory",
        ));
    }
    let mut bundle = Bundle {
        complete: true,
        ..Bundle::default()
    };
    let mut total_bytes = 0u64;
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();
    let mut visited = 0usize;
    let mut walker = walker;
    while let Some(entry) = walker.next() {
        if crate::cancel::is_requested() {
            bundle.complete = false;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let path = e.path().map(|p| relative(root, p)).unwrap_or_default();
                bundle.skipped.push(Skipped {
                    path,
                    reason: SkipReason::Unreadable,
                });
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        visited += 1;
        if visited > limits.max_entries {
            bundle.complete = false;
            break;
        }
        let rel = relative(root, entry.path());
        let name = entry.file_name().to_string_lossy();
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            bundle.skipped.push(Skipped {
                path: rel,
                reason: SkipReason::Symlink,
            });
            continue;
        }
        if file_type.is_dir() {
            if name.starts_with('.') {
                walker.skip_current_dir();
                bundle.skipped.push(Skipped {
                    path: format!("{rel}/"),
                    reason: SkipReason::HiddenDirectory,
                });
            } else {
                bundle.entries.insert(format!("{rel}/"));
            }
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        bundle.entries.insert(rel.clone());
        if !name.ends_with(".md") {
            continue;
        }
        match read_capped(entry.path(), limits.max_file_bytes) {
            Ok(Some(bytes)) => {
                total_bytes = total_bytes.saturating_add(bytes.len() as u64);
                if total_bytes > limits.max_total_bytes {
                    bundle.complete = false;
                    break;
                }
                bundle.files.push(BundleFile {
                    role: role_of(&name),
                    content: parse_bytes(&bytes),
                    path: rel,
                });
            }
            Ok(None) => bundle.skipped.push(Skipped {
                path: rel,
                reason: SkipReason::TooLarge,
            }),
            Err(_) => bundle.skipped.push(Skipped {
                path: rel,
                reason: SkipReason::Unreadable,
            }),
        }
    }
    Ok(bundle)
}

fn read_capped(path: &Path, max: u64) -> std::io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= max).then_some(bytes))
}

/// Parses the bytes of one markdown file.
pub fn parse_bytes(bytes: &[u8]) -> Content {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Content::NotUtf8;
    };
    match document::parse(text) {
        Ok(document) => {
            let outline = document::outline(&document.body, document.body_line);
            Content::Parsed { document, outline }
        }
        Err(e) => Content::BadFrontmatter(e),
    }
}

/// Where a link or path-valued field points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An absolute URL or other scheme (`https:`, `mailto:`), or a fragment
    /// within the same document. Not checked.
    External,
    /// A bundle-relative path with `/` separators; directories end with `/`.
    Internal(String),
    /// A relative path that climbs above the bundle root.
    Escapes,
}

fn has_scheme(target: &str) -> bool {
    let mut chars = target.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return false;
        }
    }
    false
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let high = (bytes[i + 1] as char).to_digit(16);
            let low = (bytes[i + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolves a link destination written in the file at `from` (§6.1).
pub fn resolve(from: &str, target: &str) -> Target {
    let target = target.trim();
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("//")
        || has_scheme(target)
    {
        return Target::External;
    }
    let end = target.find(['#', '?']).unwrap_or(target.len());
    let path = percent_decode(&target[..end]);
    if path.is_empty() {
        return Target::External;
    }
    let mut segments: Vec<&str> = Vec::new();
    if !path.starts_with('/') {
        segments.extend(from.split('/'));
        segments.pop();
    }
    let is_dir = path.ends_with('/');
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Target::Escapes;
                }
            }
            other => segments.push(other),
        }
    }
    let mut joined = segments.join("/");
    if is_dir && !joined.is_empty() {
        joined.push('/');
    }
    Target::Internal(joined)
}

impl Bundle {
    /// Whether a resolved internal path names something in the bundle. The
    /// bundle root (an empty path) always exists.
    pub fn contains(&self, path: &str) -> bool {
        path.is_empty()
            || self.entries.contains(path)
            || (!path.ends_with('/') && self.entries.contains(&format!("{path}/")))
    }
}

/// Reads a bundle rooted at `root` with default limits.
pub fn read_default(root: &Path) -> std::io::Result<Bundle> {
    read(root, Limits::default())
}

/// Returns `root` joined with a bundle-relative path, one component at a
/// time.
pub fn join(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative.split('/').filter(|p| !p.is_empty()) {
        path.push(part);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_absolute_relative_and_external_links() {
        assert_eq!(
            resolve("metrics/revenue.md", "/tables/orders.md"),
            Target::Internal("tables/orders.md".into())
        );
        assert_eq!(
            resolve("metrics/revenue.md", "../computations/x.md#frag"),
            Target::Internal("computations/x.md".into())
        );
        assert_eq!(
            resolve("a.md", "sub%20dir/"),
            Target::Internal("sub dir/".into())
        );
        assert_eq!(resolve("a.md", "../out.md"), Target::Escapes);
        assert_eq!(resolve("a.md", "https://x.y/z.md"), Target::External);
        assert_eq!(resolve("a.md", "mailto:a@b"), Target::External);
        assert_eq!(resolve("a.md", "#section"), Target::External);
    }

    #[test]
    fn percent_decoding_ignores_malformed_escapes() {
        assert_eq!(percent_decode("a%2Fb%zz%4"), "a/b%zz%4");
    }

    #[test]
    fn roles_follow_reserved_names() {
        assert_eq!(role_of("index.md"), Role::Index);
        assert_eq!(role_of("log.md"), Role::Log);
        assert_eq!(role_of("Index.md"), Role::Concept);
    }
}
