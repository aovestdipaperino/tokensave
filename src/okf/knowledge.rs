//! Search and lookup over imported OKF bundles (#858, #1060).
//!
//! Imported bundles ([`super::store`]) are read with the bounded bundle
//! reader and kept in a small process-wide cache, which is rebuilt when a
//! bundle is imported, replaced or removed. Search is a simple term match
//! over each concept's title, ID, tags, type, description and body; it needs
//! no database and no index.
//!
//! Each bundle is read with the rules of the OKF version it declares
//! ([`ReadMode`]). Concepts carry their trust tier (§5.3), lifecycle (§5.4)
//! and, for Attested Computations, whether they can be attested (§10.2).
//! The concept graph has an untyped, directed edge for every Markdown link
//! and every path field that resolves to a concept (§6.4).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use serde_json::Value;

use super::bundle;
use super::bundle::Content;
use super::bundle::FieldTarget;
use super::bundle::Role;
use super::bundle::Target;
use super::consumer;
use super::consumer::BundleMeta;
use super::consumer::NotAttestable;
use super::consumer::ReadMode;
use super::consumer::Supersession;
use super::store;
use super::store::ImportRecord;
use super::trust;
use super::trust::Freshness;
use super::trust::Trust;
use crate::errors::Result;

/// Frontmatter keys whose string value may name a source file of the
/// project: tokensave's own export (`x-tokensave-path`, and
/// `tokensave_path` before OKF v0.3 support), Graphify's export, and OKF
/// `resource` when it is a relative path that names nothing in the bundle.
pub const CODE_PATH_KEYS: [&str; 4] = [
    "x-tokensave-path",
    "tokensave_path",
    "graphify_source_file",
    "resource",
];

const MAX_TERMS: usize = 16;
const MAX_BODY_HITS: usize = 5;
const SNIPPET_CHARS: usize = 200;

/// An edge from a path field (§6.2) to a concept of the same bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLink {
    /// The field, e.g. `sources[0].resource` or `superseded_by`.
    pub field: String,
    /// The target concept ID.
    pub id: String,
}

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
    /// Lifecycle status (§5.4); `stable` when absent.
    pub status: String,
    /// The raw `superseded_by` value, if any.
    pub superseded_by: Option<String>,
    /// The concept `superseded_by` resolves to, if it names one.
    pub superseded_by_id: Option<String>,
    pub trust: Trust,
    /// For an Attested Computation: why it cannot be attested; empty when it
    /// can (§10.2). `None` for other types.
    pub attestation: Option<Vec<NotAttestable>>,
    pub frontmatter: Value,
    pub body: String,
    /// Concept IDs this concept's Markdown links point to, in order. A link
    /// whose `.md` target is missing is kept: it may be knowledge nobody has
    /// written yet (§6.1).
    pub links: Vec<String>,
    /// Path fields that resolve to a concept of the bundle (§6.4).
    pub field_links: Vec<FieldLink>,
    /// Project-relative paths named by [`CODE_PATH_KEYS`].
    pub code_paths: Vec<String>,
    lower_title: String,
    lower_id: String,
    lower_tags: String,
    lower_type: String,
    lower_description: String,
    lower_body: String,
}

impl Concept {
    /// Whether this is current knowledge: status `stable` (§5.4).
    pub fn is_current(&self) -> bool {
        self.status == "stable"
    }

    /// Freshness at `now` (Unix seconds), from `stale_after` only (§5.5).
    pub fn freshness(&self, now: i64) -> Freshness {
        trust::freshness(&self.frontmatter, now)
    }

    /// Every concept this one points to: links, then path fields.
    pub fn targets(&self) -> impl Iterator<Item = &str> {
        self.links
            .iter()
            .map(String::as_str)
            .chain(self.field_links.iter().map(|l| l.id.as_str()))
    }
}

/// One imported bundle as loaded.
#[derive(Debug, Clone)]
pub struct LoadedBundle {
    pub record: ImportRecord,
    /// Bundle metadata of the root `index.md` (§8.1).
    pub meta: BundleMeta,
    pub read_mode: ReadMode,
}

/// The loaded imported bundles of one project.
#[derive(Debug, Default)]
pub struct Knowledge {
    pub bundles: Vec<LoadedBundle>,
    pub concepts: Vec<Concept>,
    /// (bundle, concept ID) to an index into `concepts`.
    index: HashMap<(String, String), usize>,
}

/// A concept reached while traversing links and backlinks.
#[derive(Debug, Clone, Copy)]
pub struct Related<'a> {
    pub concept: &'a Concept,
    pub distance: usize,
}

fn string_field(frontmatter: &Value, key: &str) -> Option<String> {
    consumer::string(frontmatter, key).map(str::to_string)
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

fn code_paths_of(
    bundle: &bundle::Bundle,
    file: &bundle::BundleFile,
    frontmatter: &Value,
    mode: ReadMode,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for key in CODE_PATH_KEYS {
        let Some(value) = frontmatter.get(key).and_then(Value::as_str) else {
            continue;
        };
        // An in-bundle `resource` is a path field (§6.2), not a project file.
        if key == "resource"
            && matches!(
                bundle.resolve_field(&file.path, value, mode.legacy()),
                FieldTarget::Found { .. }
            )
        {
            continue;
        }
        if let Some(path) = as_project_path(value) {
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

fn concept_id(path: &str) -> &str {
    path.strip_suffix(".md").unwrap_or(path)
}

/// Whether a resolved link target could be a concept file: a `.md` file
/// that is not reserved (§3.1, §6.1).
#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "OKF paths are case-sensitive (§6), and the bundle reader treats only `.md` as Markdown"
)]
fn is_concept_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.ends_with(".md") && name != "index.md" && name != "log.md"
}

fn concepts_of(loaded: &LoadedBundle, bundle: &bundle::Bundle) -> Vec<Concept> {
    let mode = loaded.read_mode;
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
        let Some(concept_type) = file.concept_type().map(str::to_string) else {
            continue;
        };
        let mut links: Vec<String> = Vec::new();
        for link in &outline.links {
            if let Target::Internal(path) = bundle::resolve(&file.path, &link.target) {
                let id = concept_id(&path).to_string();
                if is_concept_path(&path) && !links.contains(&id) {
                    links.push(id);
                }
            }
        }
        let mut field_links: Vec<FieldLink> = Vec::new();
        for (field, value) in consumer::path_fields(frontmatter) {
            if let FieldTarget::Found { path, .. } =
                bundle.resolve_field(&file.path, value, mode.legacy())
            {
                if bundle.is_concept(&path) {
                    field_links.push(FieldLink {
                        field,
                        id: concept_id(&path).to_string(),
                    });
                }
            }
        }
        let superseded_by_id = match consumer::superseded_by(bundle, file, mode) {
            Some(Supersession::Concept(path)) => Some(concept_id(&path).to_string()),
            _ => None,
        };
        let attestation = (concept_type == consumer::ATTESTED_COMPUTATION)
            .then(|| consumer::attestation_gaps(bundle, file, mode));
        let title = string_field(frontmatter, "title");
        let description = string_field(frontmatter, "description");
        let tags = consumer::tags(frontmatter, mode);
        let id = file.concept_id().to_string();
        out.push(Concept {
            bundle: loaded.record.name.clone(),
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
            status: consumer::status(frontmatter),
            superseded_by: string_field(frontmatter, "superseded_by"),
            superseded_by_id,
            trust: trust::assess(frontmatter, mode),
            attestation,
            code_paths: code_paths_of(bundle, file, frontmatter, mode),
            frontmatter: frontmatter.clone(),
            body: document.body.clone(),
            links,
            field_links,
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
        let meta = BundleMeta::of(&bundle);
        let loaded = LoadedBundle {
            record: entry.record.clone(),
            read_mode: meta.read_mode(),
            meta,
        };
        knowledge.concepts.extend(concepts_of(&loaded, &bundle));
        knowledge.bundles.push(loaded);
    }
    knowledge.index = knowledge
        .concepts
        .iter()
        .enumerate()
        .map(|(i, c)| ((c.bundle.clone(), c.id.clone()), i))
        .collect();
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
    /// With [`Filter::current_only`]: the deprecated concept that matched
    /// and that `concept` replaces (§5.4).
    pub replaces: Option<&'a Concept>,
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
    /// Only current knowledge (status `stable`). A deprecated match is
    /// replaced by the concept its `superseded_by` chain leads to (§5.4).
    pub current_only: bool,
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
                        replaces: None,
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
                    replaces: None,
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
        if filter.current_only {
            let mut seen: HashSet<(&str, &str)> = HashSet::new();
            hits = hits
                .into_iter()
                .filter_map(|hit| {
                    let concept = hit.concept;
                    if concept.is_current() {
                        Some(hit)
                    } else {
                        let replacement = self.replacement(concept)?;
                        Some(Hit {
                            concept: replacement,
                            replaces: Some(concept),
                            ..hit
                        })
                    }
                })
                .filter(|hit| seen.insert((hit.concept.bundle.as_str(), hit.concept.id.as_str())))
                .collect();
        }
        hits.truncate(limit);
        hits
    }

    /// The concept with this ID in this bundle.
    pub fn get(&self, bundle: &str, id: &str) -> Option<&Concept> {
        self.index
            .get(&(bundle.to_string(), id.to_string()))
            .and_then(|&i| self.concepts.get(i))
    }

    /// Concepts with this ID, optionally in one bundle. A trailing `.md` is
    /// ignored.
    pub fn find(&self, bundle: Option<&str>, id: &str) -> Vec<&Concept> {
        let id = id.trim().trim_start_matches('/');
        let id = id.strip_suffix(".md").unwrap_or(id);
        match bundle {
            Some(bundle) => self.get(bundle, id).into_iter().collect(),
            None => self.concepts.iter().filter(|c| c.id == id).collect(),
        }
    }

    /// The current concept that replaces a deprecated one: the end of its
    /// `superseded_by` chain, when that is current knowledge (§5.4).
    /// Returns `None` for a current concept, a concept that is not
    /// deprecated, a missing or external replacement, and a cycle.
    pub fn replacement<'a>(&'a self, concept: &'a Concept) -> Option<&'a Concept> {
        let mut visited: HashSet<&str> = HashSet::from([concept.id.as_str()]);
        let mut current = concept;
        for _ in 0..consumer::MAX_SUPERSESSION_HOPS {
            if current.status != "deprecated" {
                return None;
            }
            let next = self.get(&current.bundle, current.superseded_by_id.as_deref()?)?;
            if !visited.insert(next.id.as_str()) {
                return None;
            }
            if next.is_current() {
                return Some(next);
            }
            current = next;
        }
        None
    }

    /// Concepts of the same bundle that link to `concept`, through a link or
    /// a path field.
    pub fn backlinks<'a>(&'a self, concept: &'a Concept) -> impl Iterator<Item = &'a Concept> + 'a {
        self.concepts.iter().filter(move |c| {
            c.bundle == concept.bundle && c.targets().any(|target| target == concept.id)
        })
    }

    /// Traverse both links and backlinks within one bundle. The starting
    /// concept is not returned; cycles are visited once.
    pub fn related<'a>(
        &'a self,
        start: &'a Concept,
        max_depth: usize,
        limit: usize,
    ) -> Vec<Related<'a>> {
        let mut visited = BTreeSet::from([start.id.as_str()]);
        let mut queue = VecDeque::from([(start, 0usize)]);
        let mut out = Vec::new();
        while let Some((concept, distance)) = queue.pop_front() {
            if distance >= max_depth {
                continue;
            }
            let neighbors = concept
                .targets()
                .filter_map(|id| self.get(&start.bundle, id))
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

    /// Whether concept `id` exists in `bundle`.
    pub fn exists(&self, bundle: &str, id: &str) -> bool {
        self.get(bundle, id).is_some()
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
    fn concept_paths_exclude_reserved_files_and_assets() {
        assert!(is_concept_path("metrics/revenue.md"));
        assert!(!is_concept_path("metrics/index.md"));
        assert!(!is_concept_path("log.md"));
        assert!(!is_concept_path("attesters/sql_equality.py"));
        assert!(!is_concept_path("metrics/"));
    }
}
