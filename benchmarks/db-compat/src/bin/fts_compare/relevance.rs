//! Search relevance on real corpora, judged independently of both engines.
//!
//! Queries are sampled with a fixed seed from each corpus's own symbols, one
//! set per intent. Graded relevance judgments (0–3) come from explicit rules
//! over the record text (see [`judge`]), computed over every record of the
//! corpus. Neither engine's output is used as ground truth.
//!
//! Each query runs in two modes:
//! * `exact` — the intent as quoted terms/phrases only, which both engines
//!   support, so differences come from tokenization and ranking;
//! * `production` — production's query construction (`search_nodes`: a
//!   phrase channel plus an OR of quoted prefix terms). Turso arms that
//!   cannot express a prefix report the query as unsupported, which counts
//!   as an empty result; the `prefix-as-term` arm shows what a degraded
//!   rewrite would return.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::adapter::{Adapter, AdapterKind, Hit, SearchError};
use crate::corpus::{self, NodeRec, Rng};
use crate::query::{PrefixPolicy, Q};

pub const K: usize = 10;

/// One query's ranked ids, or why the arm could not run it.
type Ranked = Result<Vec<String>, SearchError>;
pub(crate) const QUERIES_PER_INTENT: usize = 40;

/// Kinds that are not searchable symbols in their own right.
const NON_SYMBOL_KINDS: &[&str] = &[
    "file",
    "doc",
    "use",
    "import",
    "include",
    "module",
    "annotation_usage",
    "decorator",
    "preprocessor_def",
    "package",
];

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "this", "that", "from", "into", "when", "will", "are", "was",
    "were", "been", "have", "has", "not", "but", "you", "your", "its", "it's", "can", "may",
    "should", "would", "could", "which", "than", "then", "also", "only", "each", "other", "such",
    "some", "more", "most", "used", "uses", "using", "returns", "return", "true", "false", "none",
    "null", "self", "there", "their", "they", "them", "these", "those", "what", "where", "while",
    "about", "over", "under", "between", "after", "before", "does", "done", "make", "must",
];

fn lower_tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Identifier sub-words: alphanumeric runs, further split at camelCase and
/// letter/digit boundaries, lowercased.
fn subwords(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for run in s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        let chars: Vec<char> = run.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let (p, c) = (chars[i - 1], chars[i]);
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let boundary = (p.is_lowercase() && c.is_uppercase())
                || (p.is_uppercase() && c.is_uppercase() && next_lower)
                || (p.is_alphabetic() != c.is_alphabetic());
            if boundary {
                out.push(chars[start..i].iter().collect::<String>().to_lowercase());
                start = i;
            }
        }
        out.push(chars[start..].iter().collect::<String>().to_lowercase());
    }
    out
}

/// Light English suffix stripping, used only to judge natural-language
/// documentation intents (a reader searching "parse files" considers
/// "parsing a file" relevant).
fn norm(w: &str) -> String {
    for suf in ["ing", "ed", "es", "s", "ly"] {
        if w.len() > suf.len() + 3 && w.ends_with(suf) {
            return w[..w.len() - suf.len()].to_string();
        }
    }
    w.to_string()
}

fn contains_seq(hay: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

struct Doc {
    id: String,
    name_lc: String,
    name_sub: Vec<String>,
    qn_tokens: Vec<String>,
    qn_sub: HashSet<String>,
    sig_tokens: Vec<String>,
    sig_sub: HashSet<String>,
    doc_tokens: Vec<String>,
    doc_norm: HashSet<String>,
    all_tokens: HashSet<String>,
    all_sub: Vec<String>,
}

impl Doc {
    fn new(n: &NodeRec) -> Self {
        let sig = n.signature.clone().unwrap_or_default();
        let doc = n.docstring.clone().unwrap_or_default();
        let doc_tokens = lower_tokens(&doc);
        let mut all_tokens: HashSet<String> = HashSet::new();
        for f in [&n.name, &n.qualified_name, &sig, &doc, &n.search_terms] {
            all_tokens.extend(lower_tokens(f));
        }
        let mut all_sub = subwords(&n.qualified_name);
        all_sub.extend(subwords(&sig));
        all_sub.extend(doc_tokens.iter().cloned());
        Doc {
            id: n.id.clone(),
            name_lc: n.name.to_lowercase(),
            name_sub: subwords(&n.name),
            qn_tokens: lower_tokens(&n.qualified_name),
            qn_sub: subwords(&n.qualified_name).into_iter().collect(),
            sig_tokens: lower_tokens(&sig),
            sig_sub: subwords(&sig).into_iter().collect(),
            doc_norm: doc_tokens.iter().map(|t| norm(t)).collect(),
            doc_tokens,
            all_tokens,
            all_sub,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Intent {
    ExactName,
    PartialName,
    Prefix,
    Qualified,
    Signature,
    DocNl,
    /// The `DocNl` queries judged without suffix stripping.
    DocNlLiteral,
    DocPhrase,
    Boolean,
}

impl Intent {
    const ALL: [Intent; 9] = [
        Self::ExactName,
        Self::PartialName,
        Self::Prefix,
        Self::Qualified,
        Self::Signature,
        Self::DocNl,
        Self::DocNlLiteral,
        Self::DocPhrase,
        Self::Boolean,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::ExactName => "exact_name",
            Self::PartialName => "partial_name",
            Self::Prefix => "prefix",
            Self::Qualified => "qualified_name",
            Self::Signature => "signature",
            Self::DocNl => "doc_natural_language",
            Self::DocNlLiteral => "doc_natural_language_literal",
            Self::DocPhrase => "doc_phrase",
            Self::Boolean => "boolean_and",
        }
    }
}

pub(crate) struct Query {
    pub(crate) intent: Intent,
    /// What a user would type (production mode input).
    pub(crate) raw: String,
    /// Parameters the judgment rule needs.
    pub(crate) terms: Vec<String>,
    /// Exact-mode rendering (`None`: the intent has no exact form).
    pub(crate) exact: Option<Q>,
}

fn content_words(text: &str) -> Vec<String> {
    lower_tokens(text)
        .into_iter()
        .filter(|w| {
            w.len() >= 4 && w.chars().all(char::is_alphabetic) && !STOPWORDS.contains(&w.as_str())
        })
        .collect()
}

pub(crate) fn sample_queries(nodes: &[NodeRec], seed: u64) -> Vec<Query> {
    let mut rng = Rng::new(seed);
    let symbols: Vec<&NodeRec> = nodes
        .iter()
        .filter(|n| !NON_SYMBOL_KINDS.contains(&n.kind.as_str()) && !n.name.trim().is_empty())
        .collect();
    let mut out = Vec::new();
    for intent in Intent::ALL {
        if intent == Intent::DocNlLiteral {
            let copies: Vec<Query> = out
                .iter()
                .filter(|q: &&Query| q.intent == Intent::DocNl)
                .map(|q| Query {
                    intent,
                    raw: q.raw.clone(),
                    terms: q.terms.clone(),
                    exact: q.exact.clone(),
                })
                .collect();
            out.extend(copies);
            continue;
        }
        let mut made = 0;
        let mut seen = HashSet::new();
        let mut attempts = 0;
        while made < QUERIES_PER_INTENT
            && attempts < QUERIES_PER_INTENT * 200
            && !symbols.is_empty()
        {
            attempts += 1;
            let s = symbols[rng.below(symbols.len())];
            let q = match intent {
                Intent::ExactName => {
                    let name = s.name.trim().to_string();
                    (name.split_whitespace().count() == 1).then(|| Query {
                        intent,
                        exact: Some(Q::Term(name.clone())),
                        terms: vec![name.to_lowercase()],
                        raw: name,
                    })
                }
                Intent::PartialName => {
                    let subs = subwords(&s.name);
                    let picks: Vec<&String> = subs
                        .iter()
                        .filter(|w| w.len() >= 4 && w.chars().all(char::is_alphabetic))
                        .collect();
                    (subs.len() >= 2 && !picks.is_empty()).then(|| {
                        let w = picks[rng.below(picks.len())].clone();
                        Query {
                            intent,
                            exact: Some(Q::Term(w.clone())),
                            terms: vec![w.clone()],
                            raw: w,
                        }
                    })
                }
                Intent::Prefix => {
                    let subs = subwords(&s.name);
                    let picks: Vec<&String> = subs
                        .iter()
                        .filter(|w| w.len() >= 6 && w.chars().all(char::is_alphabetic))
                        .collect();
                    (!picks.is_empty()).then(|| {
                        let w = picks[rng.below(picks.len())];
                        let p: String = w.chars().take(4).collect();
                        Query {
                            intent,
                            exact: None,
                            terms: vec![p.clone()],
                            raw: p,
                        }
                    })
                }
                Intent::Qualified => {
                    let toks = lower_tokens(&s.qualified_name);
                    (toks.len() >= 3 && !s.qualified_name.contains(char::is_whitespace)).then(
                        || Query {
                            intent,
                            exact: Some(Q::Term(s.qualified_name.clone())),
                            terms: toks,
                            raw: s.qualified_name.clone(),
                        },
                    )
                }
                Intent::Signature => {
                    let toks = lower_tokens(s.signature.as_deref().unwrap_or_default());
                    (toks.len() >= 4).then(|| {
                        let start = rng.below(toks.len() - 2);
                        let tri: Vec<String> = toks[start..start + 3].to_vec();
                        Query {
                            intent,
                            exact: Some(Q::Term(tri.join(" "))),
                            raw: tri.join(" "),
                            terms: tri,
                        }
                    })
                }
                Intent::DocNl => {
                    let mut words = content_words(s.docstring.as_deref().unwrap_or_default());
                    words.dedup();
                    let uniq: Vec<String> = {
                        let mut seen_w = HashSet::new();
                        words
                            .into_iter()
                            .filter(|w| seen_w.insert(w.clone()))
                            .collect()
                    };
                    (uniq.len() >= 6).then(|| {
                        let mut idx: Vec<usize> = (0..uniq.len()).collect();
                        rng.shuffle(&mut idx);
                        let picked: Vec<String> =
                            idx[..3].iter().map(|&i| uniq[i].clone()).collect();
                        let raw = picked.join(" ");
                        Query {
                            intent,
                            exact: Q::exact_match(&raw),
                            raw,
                            terms: picked,
                        }
                    })
                }
                Intent::DocNlLiteral => None,
                Intent::DocPhrase => {
                    let toks = lower_tokens(s.docstring.as_deref().unwrap_or_default());
                    let pairs: Vec<(String, String)> = toks
                        .windows(2)
                        .filter(|w| {
                            w.iter().all(|t| {
                                t.len() >= 4
                                    && t.chars().all(char::is_alphabetic)
                                    && !STOPWORDS.contains(&t.as_str())
                            })
                        })
                        .map(|w| (w[0].clone(), w[1].clone()))
                        .collect();
                    (!pairs.is_empty()).then(|| {
                        let (a, b) = pairs[rng.below(pairs.len())].clone();
                        let raw = format!("{a} {b}");
                        Query {
                            intent,
                            exact: Some(Q::Term(raw.clone())),
                            raw,
                            terms: vec![a, b],
                        }
                    })
                }
                Intent::Boolean => {
                    let words = content_words(s.docstring.as_deref().unwrap_or_default());
                    let uniq: Vec<String> = {
                        let mut seen_w = HashSet::new();
                        words
                            .into_iter()
                            .filter(|w| seen_w.insert(w.clone()))
                            .collect()
                    };
                    (uniq.len() >= 4).then(|| {
                        let a = uniq[0].clone();
                        let b = uniq[uniq.len() - 1].clone();
                        Query {
                            intent,
                            exact: Some(Q::and(vec![Q::Term(a.clone()), Q::Term(b.clone())])),
                            raw: format!("{a} {b}"),
                            terms: vec![a, b],
                        }
                    })
                }
            };
            if let Some(q) = q {
                if seen.insert(q.raw.clone()) {
                    out.push(q);
                    made += 1;
                }
            }
        }
    }
    out
}

/// Graded relevance of one record for one query. The rules restate each
/// intent; they use plain alphanumeric tokenization (plus camelCase
/// splitting for identifiers) and no engine.
fn judge(q: &Query, d: &Doc) -> u8 {
    let t = &q.terms;
    match q.intent {
        Intent::ExactName => {
            if d.name_lc == t[0] {
                3
            } else {
                let want = subwords(&q.raw);
                u8::from(!want.is_empty() && want.iter().all(|w| d.name_sub.contains(w)))
            }
        }
        Intent::PartialName => {
            if d.name_sub.contains(&t[0]) {
                2
            } else {
                u8::from(d.qn_sub.contains(&t[0]) || d.sig_sub.contains(&t[0]))
            }
        }
        Intent::Prefix => {
            if d.name_sub.iter().any(|w| w.starts_with(&t[0])) {
                2
            } else {
                u8::from(d.all_sub.iter().any(|w| w.starts_with(&t[0])))
            }
        }
        Intent::Qualified => {
            if d.qn_tokens == *t {
                3
            } else {
                u8::from(contains_seq(&d.qn_tokens, t))
            }
        }
        Intent::Signature => 2 * u8::from(contains_seq(&d.sig_tokens, t)),
        Intent::DocNl => {
            let hits = t.iter().filter(|w| d.doc_norm.contains(&norm(w))).count();
            match hits {
                3 => 2,
                2 => 1,
                _ => 0,
            }
        }
        Intent::DocNlLiteral => {
            let hits = t.iter().filter(|w| d.doc_tokens.contains(w)).count();
            match hits {
                3 => 2,
                2 => 1,
                _ => 0,
            }
        }
        Intent::DocPhrase => 2 * u8::from(contains_seq(&d.doc_tokens, t)),
        Intent::Boolean => u8::from(t.iter().all(|w| d.all_tokens.contains(w))),
    }
}

#[derive(Default, Clone)]
struct Metrics {
    n: usize,
    unsupported: usize,
    errors: usize,
    no_relevant: usize,
    recall5: f64,
    recall10: f64,
    precision5: f64,
    mrr10: f64,
    ndcg10: f64,
    overlap10: f64,
    rbo: f64,
    compared: usize,
}

fn ndcg(ranked: &[String], qrels: &HashMap<String, u8>, k: usize) -> f64 {
    let gain = |g: u8| (2f64).powi(i32::from(g)) - 1.0;
    let dcg: f64 = ranked
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, id)| gain(*qrels.get(id).unwrap_or(&0)) / ((i + 2) as f64).log2())
        .sum();
    let mut ideal: Vec<u8> = qrels.values().copied().collect();
    ideal.sort_unstable_by(|a, b| b.cmp(a));
    let idcg: f64 = ideal
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, g)| gain(*g) / ((i + 2) as f64).log2())
        .sum();
    if idcg > 0.0 {
        dcg / idcg
    } else {
        0.0
    }
}

/// Rank-biased overlap (Webber et al.) truncated at the longer list depth,
/// persistence 0.9. 1.0 = identical prefixes.
fn rbo(a: &[String], b: &[String], p: f64) -> f64 {
    let depth = a.len().max(b.len());
    if depth == 0 {
        return 1.0;
    }
    let mut sa = HashSet::new();
    let mut sb = HashSet::new();
    let mut sum = 0.0;
    for d in 1..=depth {
        if let Some(x) = a.get(d - 1) {
            sa.insert(x);
        }
        if let Some(x) = b.get(d - 1) {
            sb.insert(x);
        }
        let overlap = sa.intersection(&sb).count() as f64;
        sum += p.powi(d as i32 - 1) * overlap / d as f64;
    }
    // Normalized so two identical lists of length `depth` score 1.0.
    (1.0 - p) * sum / (1.0 - p.powi(depth as i32))
}

fn jaccard(a: &[String], b: &[String]) -> f64 {
    let sa: HashSet<&String> = a.iter().collect();
    let sb: HashSet<&String> = b.iter().collect();
    let union = sa.union(&sb).count();
    if union == 0 {
        1.0
    } else {
        sa.intersection(&sb).count() as f64 / union as f64
    }
}

#[derive(Clone, Copy)]
struct Arm {
    kind: AdapterKind,
    policy: Option<PrefixPolicy>,
    label: &'static str,
}

fn arms(kinds: &[AdapterKind]) -> Vec<Arm> {
    let mut out = Vec::new();
    for &kind in kinds {
        out.push(Arm {
            kind,
            policy: None,
            label: kind.label(),
        });
        if kind == AdapterKind::TursoTantivy {
            out.push(Arm {
                kind,
                policy: Some(PrefixPolicy::DegradeToExact),
                label: "turso-tantivy/prefix-as-term",
            });
        }
    }
    out
}

async fn execute(
    a: &Adapter,
    arm: &Arm,
    q: &Query,
    production: bool,
) -> Result<Vec<String>, SearchError> {
    let hits: Vec<Hit> = if production {
        a.production_search(&q.raw, K, arm.policy).await?
    } else {
        match &q.exact {
            Some(e) => a.search_nodes(e, K, false, arm.policy).await?,
            None => return Err(SearchError::Unsupported("intent has no exact form".into())),
        }
    };
    Ok(hits.into_iter().map(|h| h.id).collect())
}

async fn build(
    kind: AdapterKind,
    dir: &std::path::Path,
    c: &corpus::Corpus,
) -> Result<Adapter, String> {
    let a = Adapter::open(kind, dir).await?;
    a.create_base().await?;
    a.begin().await?;
    a.insert_nodes(&c.nodes).await?;
    a.commit().await?;
    a.build_fts().await?;
    Ok(a)
}

pub async fn run(corpora: &[PathBuf], kinds: &[AdapterKind]) -> Value {
    let mut per_corpus = Vec::new();
    for path in corpora {
        let c = match corpus::load(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                std::process::exit(2);
            }
        };
        eprintln!("relevance: {} ({} nodes)", c.name, c.nodes.len());
        let docs: Vec<Doc> = c.nodes.iter().map(Doc::new).collect();
        let queries = sample_queries(&c.nodes, corpus::SEED);
        let qrels: Vec<HashMap<String, u8>> = queries
            .iter()
            .map(|q| {
                docs.iter()
                    .filter_map(|d| {
                        let g = judge(q, d);
                        (g > 0).then(|| (d.id.clone(), g))
                    })
                    .collect()
            })
            .collect();

        let mut dirs = Vec::new();
        let mut adapters: HashMap<&'static str, Adapter> = HashMap::new();
        for &kind in kinds {
            let dir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("tempdir: {e}");
                    std::process::exit(2);
                }
            };
            match build(kind, dir.path(), &c).await {
                Ok(a) => {
                    adapters.insert(kind.label(), a);
                }
                Err(e) => eprintln!("{}: build failed: {e}", kind.label()),
            }
            dirs.push(dir);
        }

        let arm_list = arms(kinds);
        // results[(production mode, arm)][query]
        let mut results: HashMap<(bool, &'static str), Vec<Ranked>> = HashMap::new();
        for production in [false, true] {
            for arm in &arm_list {
                let Some(a) = adapters.get(arm.kind.label()) else {
                    continue;
                };
                let mut v = Vec::with_capacity(queries.len());
                for q in &queries {
                    v.push(execute(a, arm, q, production).await);
                }
                results.insert((production, arm.label), v);
            }
        }

        // Stability: repeat on the same connection, then after reopening.
        let mut stability: HashMap<&'static str, (usize, usize, usize)> = HashMap::new();
        for arm in &arm_list {
            let Some(a) = adapters.get(arm.kind.label()) else {
                continue;
            };
            let first = &results[&(false, arm.label)];
            let mut same = 0;
            let mut n = 0;
            for (q, r) in queries.iter().zip(first) {
                if let Ok(r) = r {
                    n += 1;
                    if execute(a, arm, q, false).await.as_ref() == Ok(r) {
                        same += 1;
                    }
                }
            }
            stability.insert(arm.label, (n, same, 0));
        }
        let kinds_built: Vec<AdapterKind> = kinds
            .iter()
            .copied()
            .filter(|k| adapters.contains_key(k.label()))
            .collect();
        drop(adapters);
        for (kind, dir) in kinds.iter().zip(&dirs) {
            if !kinds_built.contains(kind) {
                continue;
            }
            let Ok(a) = Adapter::open(*kind, dir.path()).await else {
                continue;
            };
            for arm in arm_list.iter().filter(|arm| arm.kind == *kind) {
                let first = &results[&(false, arm.label)];
                let mut same = 0;
                for (q, r) in queries.iter().zip(first) {
                    if let Ok(r) = r {
                        if execute(&a, arm, q, false).await.as_ref() == Ok(r) {
                            same += 1;
                        }
                    }
                }
                if let Some(s) = stability.get_mut(arm.label) {
                    s.2 = same;
                }
            }
        }

        let mut rows = Vec::new();
        for production in [false, true] {
            let mode = if production { "production" } else { "exact" };
            for intent in Intent::ALL {
                for arm in &arm_list {
                    let Some(res) = results.get(&(production, arm.label)) else {
                        continue;
                    };
                    let control = results.get(&(production, AdapterKind::LibsqlFts5.label()));
                    let mut m = Metrics::default();
                    for (i, q) in queries
                        .iter()
                        .enumerate()
                        .filter(|(_, q)| q.intent == intent)
                    {
                        if !production && q.exact.is_none() {
                            continue;
                        }
                        m.n += 1;
                        let ranked: Vec<String> = match &res[i] {
                            Ok(r) => r.clone(),
                            Err(SearchError::Unsupported(_)) => {
                                m.unsupported += 1;
                                Vec::new()
                            }
                            Err(_) => {
                                m.errors += 1;
                                Vec::new()
                            }
                        };
                        if let Some(Ok(ctrl)) = control.map(|c| &c[i]) {
                            m.overlap10 += jaccard(&ranked, ctrl);
                            m.rbo += rbo(&ranked, ctrl, 0.9);
                            m.compared += 1;
                        }
                        let rel = &qrels[i];
                        if rel.is_empty() {
                            m.no_relevant += 1;
                            continue;
                        }
                        let relevant = |id: &String| rel.contains_key(id);
                        let hits = |k: usize| {
                            ranked.iter().take(k).filter(|id| relevant(id)).count() as f64
                        };
                        m.recall5 += hits(5) / rel.len() as f64;
                        m.recall10 += hits(10) / rel.len() as f64;
                        m.precision5 += hits(5) / 5.0;
                        m.mrr10 += ranked
                            .iter()
                            .take(10)
                            .position(relevant)
                            .map_or(0.0, |p| 1.0 / (p + 1) as f64);
                        m.ndcg10 += ndcg(&ranked, rel, 10);
                    }
                    if m.n == 0 {
                        continue;
                    }
                    let judged = (m.n - m.no_relevant).max(1) as f64;
                    let compared = m.compared.max(1) as f64;
                    let (stab_n, stab_same, stab_reopen) =
                        stability.get(arm.label).copied().unwrap_or_default();
                    rows.push(json!({
                        "corpus": c.name,
                        "mode": mode,
                        "intent": intent.label(),
                        "arm": arm.label,
                        "queries": m.n,
                        "unsupported": m.unsupported,
                        "errors": m.errors,
                        "queries_without_relevant": m.no_relevant,
                        "recall_at_5": m.recall5 / judged,
                        "recall_at_10": m.recall10 / judged,
                        "precision_at_5": m.precision5 / judged,
                        "mrr_at_10": m.mrr10 / judged,
                        "ndcg_at_10": m.ndcg10 / judged,
                        "overlap_at_10_vs_libsql": m.overlap10 / compared,
                        "rbo_vs_libsql": m.rbo / compared,
                        "stability_exact_mode": {
                            "queries": stab_n,
                            "identical_on_repeat": stab_same,
                            "identical_after_reopen": stab_reopen,
                        },
                    }));
                }
            }
        }
        // Records Tantivy's `default` tokenizer cannot fully index: any
        // alphanumeric run over 40 bytes is dropped (RemoveLongFilter).
        let long_token_nodes = c
            .nodes
            .iter()
            .filter(|n| {
                [
                    Some(&n.name),
                    Some(&n.qualified_name),
                    n.docstring.as_ref(),
                    n.signature.as_ref(),
                ]
                .into_iter()
                .flatten()
                .any(|f| crate::query::tantivy_tokens(f).iter().any(|t| t.len() > 40))
            })
            .count();
        let long_name_nodes = c
            .nodes
            .iter()
            .filter(|n| {
                crate::query::tantivy_tokens(&n.name)
                    .iter()
                    .any(|t| t.len() > 40)
            })
            .count();
        per_corpus.push(json!({
            "corpus": c.name,
            "nodes": c.nodes.len(),
            "nodes_with_indexed_token_over_40_bytes": long_token_nodes,
            "nodes_with_name_token_over_40_bytes": long_name_nodes,
            "queries": queries.len(),
            "queries_per_intent": Intent::ALL.iter().map(|i| (i.label(), queries.iter().filter(|q| q.intent == *i).count())).collect::<HashMap<_, _>>(),
            "rows": rows,
        }));
        drop(dirs);
    }
    json!({
        "environment": crate::sysinfo::collect(),
        "k": K,
        "seed": corpus::SEED,
        "corpora": per_corpus,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subwords_split_camel_snake_and_digits() {
        assert_eq!(
            subwords("parseHTTPRequest_v2"),
            ["parse", "http", "request", "v", "2"]
        );
        assert_eq!(subwords("größe_berechnen"), ["größe", "berechnen"]);
    }

    #[test]
    fn rbo_bounds() {
        let a: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let b: Vec<String> = ["x", "y", "z"].iter().map(|s| s.to_string()).collect();
        assert!((rbo(&a, &a, 0.9) - 1.0).abs() < 1e-9);
        assert!(rbo(&a, &b, 0.9).abs() < 1e-9);
    }

    #[test]
    fn ndcg_perfect_is_one() {
        let ranked: Vec<String> = vec!["a".into(), "b".into()];
        let q: HashMap<String, u8> = [("a".to_string(), 3), ("b".to_string(), 1)].into();
        assert!((ndcg(&ranked, &q, 10) - 1.0).abs() < 1e-9);
    }
}
