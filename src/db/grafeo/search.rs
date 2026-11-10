//! Full-text search over symbols and executable bodies.
//!
//! The libSQL backend ranks symbols with an FTS5 table. Grafeo's text index
//! scores one property at a time and has no prefix terms, so this backend
//! evaluates the same FTS5 expressions itself: an in-memory index over the
//! five columns the libSQL table holds (name, qualified name, docstring,
//! signature, split identifier words), ranked with FTS5's BM25 and the same
//! column weights. The index is built on the first search after nodes
//! change and reused until the next change.
//!
//! Tokens are lowercase alphanumeric runs, as FTS5's `unicode61` tokenizer
//! produces. The libSQL symbol table also applies the Porter stemmer, which
//! this index does not, so a stemmed form (`running` for `run`) matches only
//! through prefix terms.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, PoisonError};

use grafeo::store::Direction;

use super::{contains_ci, db_error, get_str, text, Database, PropMap, BODY, SYMBOL};
use crate::db::fts_query::{to_fts_match_query, to_fts_phrase_query};
use crate::errors::Result;
use crate::types::{EdgeKind, ExecutableBodyDocument, Node, NodeKind, SearchResult};

/// BM25 weights of the five symbol columns, as the libSQL backend's
/// `bm25(10.0, 5.0, 1.0, 2.0, 3.0)`.
const COLUMN_WEIGHTS: [f64; 5] = [10.0, 5.0, 1.0, 2.0, 3.0];
/// FTS5's BM25 parameters.
const K1: f64 = 1.2;
const B: f64 = 0.75;

/// Splits text into lowercase alphanumeric tokens, as FTS5's `unicode61`
/// tokenizer does.
fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// One quoted FTS5 phrase; with `prefix`, its last token matches as a
/// prefix (`"token"*`).
#[derive(Debug, PartialEq)]
struct Phrase {
    tokens: Vec<String>,
    prefix: bool,
}

impl Phrase {
    /// How many times the phrase occurs in `tokens`.
    fn occurrences(&self, tokens: &[String]) -> usize {
        let n = self.tokens.len();
        if n == 0 || tokens.len() < n {
            return 0;
        }
        tokens
            .windows(n)
            .filter(|window| {
                window
                    .iter()
                    .zip(&self.tokens)
                    .enumerate()
                    .all(|(i, (t, q))| {
                        if self.prefix && i == n - 1 {
                            t.starts_with(q.as_str())
                        } else {
                            t == q
                        }
                    })
            })
            .count()
    }
}

/// An FTS5 expression of quoted phrases joined with `OR`, the form
/// [`to_fts_match_query`] and [`to_fts_phrase_query`] produce.
#[derive(Debug, PartialEq)]
pub(super) struct FtsQuery {
    phrases: Vec<Phrase>,
}

impl FtsQuery {
    /// Parses each `"..."` or `"..."*` term; `OR` and anything outside quotes
    /// is ignored.
    pub(super) fn parse(query: &str) -> Self {
        let mut phrases = Vec::new();
        let mut rest = query;
        while let Some(open) = rest.find('"') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else {
                break;
            };
            let tail = &after[close + 1..];
            let prefix = tail.starts_with('*');
            let tokens = tokenize(&after[..close]);
            if !tokens.is_empty() {
                phrases.push(Phrase { tokens, prefix });
            }
            rest = tail;
        }
        Self { phrases }
    }

    /// Whether any phrase occurs in any of `columns`.
    pub(super) fn matches_any(&self, columns: &[&str]) -> bool {
        let columns: Vec<Vec<String>> = columns.iter().map(|c| tokenize(c)).collect();
        self.phrases
            .iter()
            .any(|p| columns.iter().any(|c| p.occurrences(c) > 0))
    }
}

/// One symbol in the search index.
struct Document {
    id: String,
    kind: NodeKind,
    name_lower: String,
    columns: [Vec<String>; 5],
    len: usize,
}

/// The symbol search index (see the module docs).
pub(super) struct SearchIndex {
    documents: Vec<Document>,
    average_len: f64,
}

impl SearchIndex {
    /// The `limit` best documents for `query` with their BM25 scores (higher
    /// is better), skipping documents `skip` rejects.
    fn rank(
        &self,
        query: &FtsQuery,
        limit: usize,
        skip: impl Fn(&Document) -> bool,
    ) -> Vec<(usize, f64)> {
        let total = self.documents.len() as f64;
        let mut frequencies: Vec<Vec<f64>> = Vec::with_capacity(query.phrases.len());
        let mut idfs = Vec::with_capacity(query.phrases.len());
        for phrase in &query.phrases {
            let per_doc: Vec<f64> = self
                .documents
                .iter()
                .map(|doc| {
                    doc.columns
                        .iter()
                        .zip(COLUMN_WEIGHTS)
                        .map(|(column, weight)| weight * phrase.occurrences(column) as f64)
                        .sum()
                })
                .collect();
            let hits = per_doc.iter().filter(|f| **f > 0.0).count() as f64;
            let idf = ((total - hits + 0.5) / (hits + 0.5)).ln();
            idfs.push(if idf <= 0.0 { 1e-6 } else { idf });
            frequencies.push(per_doc);
        }
        let mut scored: Vec<(usize, f64)> = self
            .documents
            .iter()
            .enumerate()
            .filter(|(_, doc)| !skip(doc))
            .filter_map(|(i, doc)| {
                let norm = K1 * (1.0 - B + B * doc.len as f64 / self.average_len.max(1.0));
                let mut score = 0.0;
                let mut matched = false;
                for (phrase, idf) in frequencies.iter().zip(&idfs) {
                    let f = phrase[i];
                    if f > 0.0 {
                        matched = true;
                        score += idf * (f * (K1 + 1.0)) / (f + norm);
                    }
                }
                matched.then_some((i, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.truncate(limit);
        scored
    }
}

impl Database {
    /// Inserts executable-body documents.
    pub async fn insert_executable_body_documents(
        &self,
        documents: &[ExecutableBodyDocument],
    ) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        let props: Vec<PropMap> = documents
            .iter()
            .map(|doc| {
                PropMap::from([
                    ("body_node".into(), text(&doc.node_id)),
                    ("body_file".into(), text(&doc.file_path)),
                    ("body_text".into(), text(&doc.body)),
                ])
            })
            .collect();
        self.db
            .batch_create_nodes_with_props(BODY, props)
            .map_err(|e| db_error("insert_executable_body_documents", &e))?;
        Ok(())
    }

    /// Returns nodes whose executable body matches at least two distinct
    /// conceptual terms, each as a five-character prefix, with the number of
    /// terms matched; at most `candidate_limit` bodies are considered.
    pub async fn search_executable_bodies(
        &self,
        terms: &[String],
        candidate_limit: usize,
    ) -> Result<Vec<(Node, usize)>> {
        if terms.len() < 2 || candidate_limit == 0 {
            return Ok(Vec::new());
        }
        let mut seen = HashSet::new();
        let prefixes: Vec<String> = terms
            .iter()
            .filter_map(|term| {
                let prefix = term
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(5)
                    .collect::<String>()
                    .to_lowercase();
                (prefix.len() >= 4 && seen.insert(prefix.clone())).then_some(prefix)
            })
            .collect();
        if prefixes.len() < 2 {
            return Ok(Vec::new());
        }
        let mut hits: HashMap<String, usize> = HashMap::new();
        let mut candidates = 0;
        for body in self.scan_label(BODY, "search_executable_bodies")? {
            if candidates == candidate_limit {
                break;
            }
            let tokens = tokenize(&get_str(body.as_ref(), "body_text"));
            let matched = prefixes
                .iter()
                .filter(|prefix| tokens.iter().any(|t| t.starts_with(prefix.as_str())))
                .count();
            if matched >= 2 {
                candidates += 1;
                hits.insert(get_str(body.as_ref(), "body_node"), matched);
            }
        }
        let ids: Vec<String> = hits.keys().cloned().collect();
        let mut nodes: Vec<(Node, usize)> = self
            .get_nodes_by_ids(&ids)
            .await?
            .into_iter()
            .filter_map(|node| hits.get(&node.id).copied().map(|count| (node, count)))
            .collect();
        nodes.sort_by(|(a, a_hits), (b, b_hits)| {
            b_hits.cmp(a_hits).then_with(|| {
                a.end_line
                    .saturating_sub(a.start_line)
                    .cmp(&b.end_line.saturating_sub(b.start_line))
            })
        });
        Ok(nodes)
    }

    /// Returns symbols whose indexed text contains `query` as an exact
    /// phrase; empty for a single-word query. Best-effort, as in the libSQL
    /// backend.
    pub async fn search_nodes_phrase(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        match to_fts_phrase_query(query) {
            Some(phrase) => Ok(self
                .search_nodes_fts(&phrase, limit)
                .await
                .unwrap_or_default()),
            None => Ok(Vec::new()),
        }
    }

    /// Searches nodes by name, qualified name, docstring or signature: exact
    /// phrase hits first, then BM25-ranked prefix matches, falling back to a
    /// substring match when nothing ranks.
    pub async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let Some(fts_query) = to_fts_match_query(query) else {
            return Ok(Vec::new());
        };
        let phrase_hits: Vec<SearchResult> = match to_fts_phrase_query(query) {
            Some(phrase) => self
                .search_nodes_fts(&phrase, limit)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let merge = |general: Vec<SearchResult>| -> Vec<SearchResult> {
            if phrase_hits.is_empty() {
                return general;
            }
            let seen: HashSet<String> = phrase_hits.iter().map(|r| r.node.id.clone()).collect();
            let mut merged = phrase_hits.clone();
            merged.extend(general.into_iter().filter(|r| !seen.contains(&r.node.id)));
            merged.truncate(limit);
            merged
        };

        let ranked = self.search_nodes_fts(&fts_query, limit).await?;
        if !ranked.is_empty() {
            return Ok(merge(ranked));
        }

        // Fallback: substring match, as the libSQL `LIKE` query.
        let mut results = Vec::new();
        for node in self.all_symbols("search_nodes")? {
            if results.len() == limit {
                break;
            }
            let found = contains_ci(&node.name, query)
                || contains_ci(&node.qualified_name, query)
                || node
                    .docstring
                    .as_deref()
                    .is_some_and(|d| contains_ci(d, query))
                || node
                    .signature
                    .as_deref()
                    .is_some_and(|s| contains_ci(s, query));
            if found {
                results.push(SearchResult { node, score: 1.0 });
            }
        }
        Ok(merge(results))
    }

    /// Returns the BM25-ranked top `limit` symbols for one prefix term,
    /// excluding `file` and `doc` nodes, which have their own channels.
    pub async fn search_nodes_bounded(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let sanitized: String = query.chars().filter(|c| *c != '"').collect();
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }
        let parsed = FtsQuery::parse(&format!("\"{sanitized}\"*"));
        self.ranked_results(&parsed, limit, |doc| {
            matches!(doc.kind, NodeKind::File | NodeKind::Doc)
        })
    }

    /// Ranks symbols for an FTS5 expression of quoted phrases.
    pub(crate) async fn search_nodes_fts(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        self.ranked_results(&FtsQuery::parse(fts_query), limit, |_| false)
    }

    /// Returns `node_id` → incoming `calls` edge count for the given ids,
    /// omitting ids with none.
    pub async fn batch_incoming_call_counts(
        &self,
        node_ids: &[String],
    ) -> Result<HashMap<String, u64>> {
        let op = "batch_incoming_call_counts";
        let mut counts = HashMap::new();
        for id in node_ids {
            let Some(gid) = self.symbol_node_id(id, op)? else {
                continue;
            };
            let calls = self
                .db
                .get_node_edges(gid, Direction::Incoming, &[EdgeKind::Calls.as_str()])
                .map_err(|e| db_error(op, &e))?
                .len();
            if calls > 0 {
                counts.insert(id.clone(), u64::try_from(calls).unwrap_or(u64::MAX));
            }
        }
        Ok(counts)
    }

    /// Finds nodes whose name matches one of `names`, ignoring case, at most
    /// `limit`.
    pub async fn search_nodes_by_exact_name(
        &self,
        names: &[String],
        limit: usize,
    ) -> Result<Vec<Node>> {
        if names.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let wanted: HashSet<String> = names.iter().map(|n| n.to_lowercase()).collect();
        let index = self.search_index()?;
        let ids: Vec<String> = index
            .documents
            .iter()
            .filter(|doc| wanted.contains(&doc.name_lower))
            .take(limit)
            .map(|doc| doc.id.clone())
            .collect();
        self.get_nodes_by_ids(&ids).await
    }

    /// Ranked symbols with their nodes.
    fn ranked_results(
        &self,
        query: &FtsQuery,
        limit: usize,
        skip: impl Fn(&Document) -> bool,
    ) -> Result<Vec<SearchResult>> {
        let index = self.search_index()?;
        let mut results = Vec::new();
        for (i, score) in index.rank(query, limit, skip) {
            let Some(doc) = index.documents.get(i) else {
                continue;
            };
            if let Some(node) = self.node_by_id(&doc.id, "search_nodes")? {
                results.push(SearchResult { node, score });
            }
        }
        Ok(results)
    }

    /// The search index for the current graph, built if nodes changed since
    /// the last one.
    fn search_index(&self) -> Result<Arc<SearchIndex>> {
        let generation = self
            .search_generation
            .load(std::sync::atomic::Ordering::SeqCst);
        if let Some((built_at, index)) = self
            .search_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            if *built_at == generation {
                return Ok(Arc::clone(index));
            }
        }
        let mut documents = Vec::new();
        let mut total_len = 0;
        for props in self.scan_label(SYMBOL, "search_nodes")? {
            let p = props.as_ref();
            let name = get_str(p, "name");
            let qualified_name = get_str(p, "qualified_name");
            let columns = [
                tokenize(&name),
                tokenize(&qualified_name),
                tokenize(&get_str(p, "docstring")),
                tokenize(&get_str(p, "signature")),
                tokenize(&crate::text::search_terms(&name, &qualified_name)),
            ];
            let len = columns.iter().map(Vec::len).sum();
            total_len += len;
            documents.push(Document {
                id: get_str(p, "id"),
                kind: NodeKind::from_str(&get_str(p, "kind")).unwrap_or(NodeKind::Function),
                name_lower: name.to_ascii_lowercase(),
                columns,
                len,
            });
        }
        let average_len = if documents.is_empty() {
            0.0
        } else {
            total_len as f64 / documents.len() as f64
        };
        let index = Arc::new(SearchIndex {
            documents,
            average_len,
        });
        *self
            .search_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((generation, Arc::clone(&index)));
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::{tokenize, FtsQuery, Phrase};

    #[test]
    fn tokenize_splits_on_non_alphanumerics_and_lowercases() {
        assert_eq!(tokenize("get_UserName(a-b)"), ["get", "username", "a", "b"]);
    }

    #[test]
    fn parse_reads_quoted_prefix_terms_and_phrases() {
        let query = FtsQuery::parse("\"data-api\"* OR \"load\"*");
        assert_eq!(
            query.phrases,
            [
                Phrase {
                    tokens: vec!["data".into(), "api".into()],
                    prefix: true
                },
                Phrase {
                    tokens: vec!["load".into()],
                    prefix: true
                },
            ]
        );
        let phrase = FtsQuery::parse("\"exact words\"");
        assert!(phrase.phrases.first().is_some_and(|p| !p.prefix));
    }

    #[test]
    fn phrases_match_consecutive_tokens_with_a_prefix_last_token() {
        let query = FtsQuery::parse("\"data ap\"*");
        assert!(query.matches_any(&["load data-api now"]));
        assert!(!query.matches_any(&["api data"]));
        let exact = FtsQuery::parse("\"data ap\"");
        assert!(!exact.matches_any(&["data api"]));
    }
}
