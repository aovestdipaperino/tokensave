//! Search and lookup over imported OKF bundles (#858).
//!
//! Imported bundles ([`super::store`]) are read with the bounded bundle
//! reader and kept in a small process-wide cache, which is rebuilt when a
//! bundle is imported, replaced or removed. Search is a simple term match
//! over each concept's title, ID, tags, type, description and body; it needs
//! no database and no index.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use serde_json::Value;

use super::bundle;
use super::bundle::Content;
use super::bundle::Role;
use super::bundle::Target;
use super::store;
use super::store::ImportRecord;
use crate::errors::Result;

/// Frontmatter keys whose string value may name a source file of the
/// project: tokensave's own export, Graphify's export, and OKF `resource`
/// when it is a relative path.
pub const CODE_PATH_KEYS: [&str; 3] = ["tokensave_path", "graphify_source_file", "resource"];

const MAX_TERMS: usize = 16;
const MAX_BODY_HITS: usize = 5;
const SNIPPET_CHARS: usize = 200;

/// One searchable concept of an imported bundle.
#[derive(Debug, Clone)]
pub struct Concept {
    pub bundle: String,
    /// Concept ID: the bundle-relative path without `.md`.
    pub id: String,
    pub concept_type: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub frontmatter: Value,
    pub body: String,
    /// Concept IDs this concept links to inside its bundle, in order.
    pub links: Vec<String>,
    /// Project-relative paths named by [`CODE_PATH_KEYS`].
    pub code_paths: Vec<String>,
    lower_title: String,
    lower_id: String,
    lower_tags: String,
    lower_type: String,
    lower_description: String,
    lower_body: String,
}

/// The loaded imported bundles of one project.
#[derive(Debug, Default)]
pub struct Knowledge {
    pub bundles: Vec<ImportRecord>,
    pub concepts: Vec<Concept>,
}

/// A concept reached while traversing links and backlinks.
#[derive(Debug, Clone, Copy)]
pub struct Related<'a> {
    pub concept: &'a Concept,
    pub distance: usize,
}

fn string_field(frontmatter: &Value, key: &str) -> Option<String> {
    frontmatter
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `tags` as a list, or as the comma-separated string some producers write.
fn tags_of(frontmatter: &Value) -> Vec<String> {
    match frontmatter.get("tags") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// A relative, scheme-less path that could name a project file.
fn as_project_path(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains("://") || value.starts_with('/') || value.contains('\\') {
        return None;
    }
    if value.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+')
    }) {
        return None;
    }
    let path = value.strip_prefix("./").unwrap_or(value);
    if path.split('/').any(|seg| seg == ".." || seg.is_empty()) {
        return None;
    }
    Some(path.to_string())
}

fn code_paths_of(frontmatter: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for key in CODE_PATH_KEYS {
        if let Some(path) = frontmatter
            .get(key)
            .and_then(Value::as_str)
            .and_then(as_project_path)
        {
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

fn concepts_of(record: &ImportRecord, bundle: &bundle::Bundle) -> Vec<Concept> {
    let mut out = Vec::new();
    for file in &bundle.files {
        if file.role != Role::Concept {
            continue;
        }
        let Content::Parsed { document, outline } = &file.content else {
            continue;
        };
        let Some(frontmatter) = &document.frontmatter else {
            continue;
        };
        let Some(concept_type) = string_field(frontmatter, "type") else {
            continue;
        };
        let mut links: Vec<String> = Vec::new();
        for link in &outline.links {
            if let Target::Internal(path) = bundle::resolve(&file.path, &link.target) {
                let id = path.strip_suffix(".md").unwrap_or(&path).to_string();
                if !id.is_empty() && !links.contains(&id) {
                    links.push(id);
                }
            }
        }
        let title = string_field(frontmatter, "title");
        let description = string_field(frontmatter, "description");
        let tags = tags_of(frontmatter);
        let id = file.concept_id().to_string();
        out.push(Concept {
            bundle: record.name.clone(),
            lower_title: title.as_deref().unwrap_or_default().to_lowercase(),
            lower_id: id.to_lowercase(),
            lower_tags: tags.join(" ").to_lowercase(),
            lower_type: concept_type.to_lowercase(),
            lower_description: description.as_deref().unwrap_or_default().to_lowercase(),
            lower_body: document.body.to_lowercase(),
            id,
            concept_type,
            title,
            description,
            tags,
            status: string_field(frontmatter, "status"),
            code_paths: code_paths_of(frontmatter),
            frontmatter: frontmatter.clone(),
            body: document.body.clone(),
            links,
        });
    }
    out
}

/// Reads every imported bundle of `project_root`.
///
/// # Errors
/// Fails when the store cannot be listed. A bundle that cannot be read is
/// left out.
pub fn load_uncached(project_root: &Path) -> Result<Knowledge> {
    let stored = store::list(project_root)?;
    Ok(build(&stored))
}

fn build(stored: &[store::StoredBundle]) -> Knowledge {
    let mut knowledge = Knowledge::default();
    for entry in stored {
        let Ok(bundle) = bundle::read(&entry.bundle_dir, store::import_limits()) else {
            continue;
        };
        knowledge
            .concepts
            .extend(concepts_of(&entry.record, &bundle));
        knowledge.bundles.push(entry.record.clone());
    }
    knowledge
}

type Stamp = Vec<(String, Option<std::time::SystemTime>, u64)>;

struct CacheEntry {
    store: PathBuf,
    stamp: Stamp,
    knowledge: Arc<Knowledge>,
}

fn cache() -> &'static Mutex<Option<CacheEntry>> {
    static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// The imported bundles of `project_root`, cached until one is imported,
/// replaced or removed. Only the most recently used project is cached.
///
/// # Errors
/// Fails when the store cannot be listed.
pub fn load(project_root: &Path) -> Result<Arc<Knowledge>> {
    let stored = store::list(project_root)?;
    let store_path = store::store_dir(project_root);
    let stamp: Stamp = stored
        .iter()
        .map(|s| (s.record.name.clone(), s.stamp.0, s.stamp.1))
        .collect();
    {
        let guard = cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = guard.as_ref() {
            if entry.store == store_path && entry.stamp == stamp {
                return Ok(Arc::clone(&entry.knowledge));
            }
        }
    }
    let knowledge = Arc::new(build(&stored));
    let mut guard = cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(CacheEntry {
        store: store_path,
        stamp,
        knowledge: Arc::clone(&knowledge),
    });
    Ok(knowledge)
}

/// Lowercase search terms of `query`: runs of letters, digits and `_`.
pub fn terms(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for term in query
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
    {
        let term = term.to_lowercase();
        if !out.contains(&term) {
            out.push(term);
        }
        if out.len() == MAX_TERMS {
            break;
        }
    }
    out
}

/// A search result.
#[derive(Debug, Clone)]
pub struct Hit<'a> {
    pub concept: &'a Concept,
    pub score: u32,
    /// Terms that matched somewhere in the concept.
    pub matched: usize,
    /// The first body line that contains a term, clipped.
    pub snippet: Option<String>,
}

fn score(concept: &Concept, term: &str) -> u32 {
    let mut score = 0u32;
    if concept.lower_title.contains(term) {
        score += 6;
    }
    if concept.lower_id.contains(term) {
        score += 4;
    }
    if concept.lower_tags.contains(term) {
        score += 4;
    }
    if concept.lower_type.contains(term) {
        score += 3;
    }
    if concept.lower_description.contains(term) {
        score += 2;
    }
    let body_hits = concept
        .lower_body
        .match_indices(term)
        .take(MAX_BODY_HITS)
        .count();
    score + u32::try_from(body_hits).unwrap_or(0)
}

fn snippet(concept: &Concept, terms: &[String]) -> Option<String> {
    concept
        .body
        .lines()
        .map(str::trim)
        .find(|line| {
            let lower = line.to_lowercase();
            terms.iter().any(|t| lower.contains(t.as_str()))
        })
        .map(|line| {
            let mut out: String = line.chars().take(SNIPPET_CHARS).collect();
            if line.chars().count() > SNIPPET_CHARS {
                out.push('…');
            }
            out
        })
}

/// Filters for [`Knowledge::search`].
#[derive(Debug, Clone, Default)]
pub struct Filter<'a> {
    pub bundle: Option<&'a str>,
    /// Case-insensitive `type`.
    pub concept_type: Option<&'a str>,
    /// Only concepts that name this project file.
    pub file: Option<&'a str>,
}

impl Knowledge {
    fn filtered<'a>(&'a self, filter: &'a Filter<'a>) -> impl Iterator<Item = &'a Concept> + 'a {
        self.concepts.iter().filter(move |c| {
            filter.bundle.is_none_or(|b| c.bundle == b)
                && filter
                    .concept_type
                    .is_none_or(|t| c.concept_type.eq_ignore_ascii_case(t))
                && filter
                    .file
                    .is_none_or(|f| c.code_paths.iter().any(|p| p == f))
        })
    }

    /// Concepts matching `query`, best first. Concepts that match more
    /// terms rank first, then by score. An empty query lists the filtered
    /// concepts in bundle and ID order.
    pub fn search<'a>(&'a self, query: &str, filter: &'a Filter<'a>, limit: usize) -> Vec<Hit<'a>> {
        let terms = terms(query);
        let mut hits: Vec<Hit<'a>> = self
            .filtered(filter)
            .filter_map(|concept| {
                if terms.is_empty() {
                    return Some(Hit {
                        concept,
                        score: 0,
                        matched: 0,
                        snippet: None,
                    });
                }
                let mut total = 0u32;
                let mut matched = 0usize;
                for term in &terms {
                    let s = score(concept, term);
                    if s > 0 {
                        matched += 1;
                        total += s;
                    }
                }
                (matched > 0).then(|| Hit {
                    concept,
                    score: total,
                    matched,
                    snippet: snippet(concept, &terms),
                })
            })
            .collect();
        hits.sort_by(|a, b| {
            b.matched
                .cmp(&a.matched)
                .then(b.score.cmp(&a.score))
                .then(a.concept.bundle.cmp(&b.concept.bundle))
                .then(a.concept.id.cmp(&b.concept.id))
        });
        hits.truncate(limit);
        hits
    }

    /// Concepts with this ID, optionally in one bundle. A trailing `.md` is
    /// ignored.
    pub fn find(&self, bundle: Option<&str>, id: &str) -> Vec<&Concept> {
        let id = id.trim().trim_start_matches('/');
        let id = id.strip_suffix(".md").unwrap_or(id);
        self.concepts
            .iter()
            .filter(|c| c.id == id && bundle.is_none_or(|b| c.bundle == b))
            .collect()
    }

    /// Concepts of the same bundle that link to `concept`, by ID.
    pub fn backlinks<'a>(&'a self, concept: &'a Concept) -> impl Iterator<Item = &'a Concept> + 'a {
        self.concepts
            .iter()
            .filter(move |c| c.bundle == concept.bundle && c.links.contains(&concept.id))
    }

    /// Traverse both links and backlinks within one bundle. The starting
    /// concept is not returned; cycles are visited once.
    pub fn related<'a>(
        &'a self,
        start: &'a Concept,
        max_depth: usize,
        limit: usize,
    ) -> Vec<Related<'a>> {
        let concepts_by_id: BTreeMap<&str, &Concept> = self
            .concepts
            .iter()
            .filter(|concept| concept.bundle == start.bundle)
            .map(|concept| (concept.id.as_str(), concept))
            .collect();
        let mut visited = BTreeSet::from([start.id.as_str()]);
        let mut queue = VecDeque::from([(start, 0usize)]);
        let mut out = Vec::new();
        while let Some((concept, distance)) = queue.pop_front() {
            if distance >= max_depth {
                continue;
            }
            let neighbors = concept
                .links
                .iter()
                .filter_map(|id| concepts_by_id.get(id.as_str()).copied())
                .chain(self.backlinks(concept));
            for neighbor in neighbors {
                if !visited.insert(neighbor.id.as_str()) {
                    continue;
                }
                let next_distance = distance + 1;
                out.push(Related {
                    concept: neighbor,
                    distance: next_distance,
                });
                if out.len() >= limit {
                    return out;
                }
                queue.push_back((neighbor, next_distance));
            }
        }
        out
    }

    /// Number of searchable concepts per bundle.
    pub fn counts(&self) -> BTreeMap<&str, usize> {
        let mut out = BTreeMap::new();
        for concept in &self.concepts {
            *out.entry(concept.bundle.as_str()).or_insert(0) += 1;
        }
        out
    }

    /// Whether `concept` links to an existing concept `id` of its bundle.
    pub fn exists(&self, bundle: &str, id: &str) -> bool {
        self.concepts
            .iter()
            .any(|c| c.bundle == bundle && c.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_lowercase_unique_words() {
        assert_eq!(terms("Gross margin, gross-MARGIN!"), ["gross", "margin"]);
        assert_eq!(terms("  --- "), Vec::<String>::new());
    }

    #[test]
    fn only_relative_paths_name_project_files() {
        assert_eq!(
            as_project_path("src/main.rs").as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(as_project_path("./model.py").as_deref(), Some("model.py"));
        assert_eq!(as_project_path("https://x.y/z"), None);
        assert_eq!(as_project_path("bigquery:proj.ds.t"), None);
        assert_eq!(as_project_path("/etc/passwd"), None);
        assert_eq!(as_project_path("../up.rs"), None);
        assert_eq!(as_project_path("a\\b.rs"), None);
    }

    #[test]
    fn tags_accept_lists_and_comma_strings() {
        let fm = serde_json::json!({"tags": ["a", 2, " b "]});
        assert_eq!(tags_of(&fm), ["a", "2", "b"]);
        let fm = serde_json::json!({"tags": "x, y,,z"});
        assert_eq!(tags_of(&fm), ["x", "y", "z"]);
    }
}
