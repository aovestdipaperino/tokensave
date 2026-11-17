//! Functional search scenarios, run before any timing.
//!
//! Each scenario runs on a fresh copy of a small hand-built fixture and
//! compares what an engine returns with an expectation written by hand from
//! the fixture text (not from either engine's output). The libsql control
//! must pass every scenario it runs, which validates the expectation.
//!
//! A turso result that differs is classified:
//! * `expected-semantic` — the difference follows from a documented engine
//!   property that cannot be configured (no stemming, no diacritic folding)
//!   and the result equals what that property predicts;
//! * `configurable` — a different turso index configuration changes it
//!   (tokenizer choice), and the result equals what this configuration
//!   predicts;
//! * `approximation` — the n-gram arm returns a superset because substring
//!   matching stands in for prefix matching;
//! * `unsupported` — the intent cannot be expressed natively;
//! * `hazard` — a silent wrong-semantics path that the harness detects;
//! * `correctness-failure` — anything else.

use serde_json::{json, Value};

use crate::adapter::{Adapter, AdapterKind, Hit, SearchError};
use crate::corpus::{BodyRec, Decision, NodeRec};
use crate::db::Val;
use crate::query::Q;

fn node(
    id: &str,
    kind: &str,
    name: &str,
    qn: &str,
    doc: Option<&str>,
    sig: Option<&str>,
    terms: &str,
) -> NodeRec {
    NodeRec {
        id: id.into(),
        kind: kind.into(),
        name: name.into(),
        qualified_name: qn.into(),
        file_path: qn.split("::").next().unwrap_or(qn).into(),
        docstring: doc.map(Into::into),
        signature: sig.map(Into::into),
        search_terms: terms.into(),
    }
}

pub fn fixture_nodes() -> Vec<NodeRec> {
    vec![
        node(
            "parse_config",
            "function",
            "parse_config",
            "crate::config::parse_config",
            Some("Parses the configuration file and returns settings."),
            Some("fn parse_config(path: &Path) -> Result<Config>"),
            "",
        ),
        node(
            "parser_state",
            "struct",
            "ParserState",
            "crate::parser::ParserState",
            Some("Holds the running state of the parser."),
            Some("pub struct ParserState"),
            "Parser State",
        ),
        node(
            "sparse_matrix",
            "struct",
            "sparse_matrix",
            "crate::linalg::sparse_matrix",
            Some("Compressed sparse matrix storage."),
            Some("pub struct sparse_matrix"),
            "",
        ),
        node(
            "update_cloud",
            "function",
            "updateCloudClient",
            "web/client.ts::updateCloudClient",
            Some("Pushes local changes to the remote service."),
            Some("function updateCloudClient(client: CloudClient): Promise<void>"),
            "update Cloud Client",
        ),
        node(
            "data_api",
            "class",
            "DataApi",
            "web/api.ts::DataApi",
            Some("The data-api client used for database query batching."),
            Some("class DataApi"),
            "Data Api",
        ),
        node(
            "run_query",
            "function",
            "run_query",
            "crate::db::run_query",
            Some("Executes a database query and streams rows."),
            Some("fn run_query(conn: &Connection, sql: &str) -> Rows"),
            "",
        ),
        node(
            "runs",
            "function",
            "runs",
            "crate::stats::runs",
            Some("Counts runs of equal values."),
            Some("fn runs(values: &[i64]) -> usize"),
            "",
        ),
        node(
            "run_loop",
            "function",
            "run_loop",
            "crate::event::run_loop",
            Some("Event loop that runs until shutdown."),
            Some("fn run_loop()"),
            "",
        ),
        node(
            "drain_events",
            "function",
            "drain_events",
            "crate::event::drain_events",
            Some("Drains the pending event loop queue."),
            Some("fn drain_events()"),
            "",
        ),
        node(
            "long_ident",
            "class",
            "AbstractSingletonProxyFactoryBeanConfigurationHolder",
            "spring/beans.java::AbstractSingletonProxyFactoryBeanConfigurationHolder",
            Some("Holds bean configuration."),
            None,
            "Abstract Singleton Proxy Factory Bean Configuration Holder",
        ),
        node(
            "groesse",
            "function",
            "größe_berechnen",
            "crate::maß::größe_berechnen",
            Some("Berechnet die Größe eines Cafés."),
            Some("fn größe_berechnen() -> f64"),
            "",
        ),
        node(
            "cjk",
            "function",
            "数据处理",
            "app/data.py::数据处理",
            Some("处理 数据"),
            Some("def 数据处理(x)"),
            "",
        ),
        node(
            "cyrillic",
            "function",
            "разобрать_строку",
            "app/text.py::разобрать_строку",
            Some("Разбирает строку."),
            Some("def разобрать_строку(s)"),
            "",
        ),
        node(
            "cafe_menu",
            "function",
            "café_menu",
            "app/menu.py::café_menu",
            Some("Lists the menu."),
            Some("def café_menu()"),
            "",
        ),
        node(
            "utf8_decode",
            "function",
            "utf8_decode",
            "crate::text::utf8_decode",
            Some("Decodes UTF-8 bytes."),
            Some("fn utf8_decode(bytes: &[u8]) -> String"),
            "",
        ),
        node(
            "http_server",
            "struct",
            "HTTPServer",
            "net/server.go::HTTPServer",
            Some("Serves HTTP requests."),
            Some("type HTTPServer struct"),
            "HTTP Server",
        ),
        node(
            "parser_file",
            "file",
            "parser.rs",
            "src/parser.rs",
            None,
            None,
            "",
        ),
        node(
            "parsing_doc",
            "doc",
            "Parsing guide",
            "docs/parsing.md",
            Some("How parsing works."),
            None,
            "",
        ),
    ]
}

pub fn fixture_bodies() -> Vec<BodyRec> {
    let b = |id: &str, body: &str| BodyRec {
        node_id: id.into(),
        file_path: format!("src/{id}.rs"),
        body: body.into(),
    };
    vec![
        b(
            "b_precond",
            "let precond = build_preconditioner(&matrix); solve(precond, rhs)",
        ),
        b(
            "b_matrix",
            "fn multiply(matrix: &Matrix) -> Matrix { matrix * matrix }",
        ),
        b(
            "b_precheck",
            "if precondition_failed { return Err(Error::Precondition) }",
        ),
    ]
}

/// Decision rows in insertion order (rowid 1..=7). `created_at` is not
/// monotonic in rowid, so rowid order and recency order differ.
pub fn fixture_decisions() -> Vec<Decision> {
    let d = |text: &str, ts: i64| Decision {
        text: text.into(),
        reason: Some("Recorded by the functional fixture.".into()),
        created_at: ts,
    };
    vec![
        d("Use the cache for symbol lookups", 500), // 1
        d("Invalidate the cache after sync", 900),  // 2
        d("Keep the cache bounded", 100),           // 3
        d("Prefer streaming over buffering", 1000), // 4 (no "cache")
        d("Warm the cache on startup", 700),        // 5
        d("Cache eviction is LRU", 300),            // 6
        d("Never cache read errors", 800),          // 7
    ]
}

#[derive(Clone, Debug)]
pub enum Expect {
    /// Same ids in any order.
    Set(&'static [&'static str]),
    /// Same ids in this order.
    Ordered(&'static [&'static str]),
    /// This id ranks first (other results unconstrained).
    Top(&'static str),
    /// Multi-step observations, compared literally.
    Steps(&'static [&'static str]),
    /// Ranked-result API contract (see [`check_contract`]).
    Contract,
}

impl Expect {
    fn matches(&self, got: &[String]) -> bool {
        match self {
            Expect::Set(want) => {
                let mut g: Vec<&str> = got.iter().map(String::as_str).collect();
                let mut w = want.to_vec();
                g.sort_unstable();
                w.sort_unstable();
                g == w
            }
            Expect::Ordered(want) | Expect::Steps(want) => {
                got.len() == want.len() && got.iter().zip(want.iter()).all(|(a, b)| a == b)
            }
            Expect::Top(id) => got.first().is_some_and(|g| g == id),
            Expect::Contract => false,
        }
    }

    fn describe(&self) -> String {
        match self {
            Expect::Set(w) => format!("set {w:?}"),
            Expect::Ordered(w) => format!("ordered {w:?}"),
            Expect::Top(id) => format!("top = {id}"),
            Expect::Steps(w) => format!("steps {w:?}"),
            Expect::Contract => "ranked-result contract".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Pass,
    ExpectedSemantic,
    Configurable,
    Approximation,
    Unsupported,
    Hazard,
    CorrectnessFailure,
    NotApplicable,
}

impl Class {
    pub fn label(self) -> &'static str {
        match self {
            Class::Pass => "pass",
            Class::ExpectedSemantic => "expected-semantic",
            Class::Configurable => "configurable",
            Class::Approximation => "approximation",
            Class::Unsupported => "unsupported",
            Class::Hazard => "hazard",
            Class::CorrectnessFailure => "correctness-failure",
            Class::NotApplicable => "n/a",
        }
    }
}

/// A documented, predicted deviation for some turso arms.
pub struct Known {
    pub arms: &'static [AdapterKind],
    pub expect: Expect,
    pub class: Class,
    pub reason: &'static str,
}

pub struct Spec {
    pub id: &'static str,
    pub number: &'static str,
    pub title: &'static str,
    pub expect: Expect,
    pub known: Vec<Known>,
    pub turso_only: bool,
    /// A plan-guard refusal is the documented hazard, not a failure.
    pub fallback_is_hazard: bool,
}

const T: AdapterKind = AdapterKind::TursoTantivy;
const TS: AdapterKind = AdapterKind::TursoTantivySimple;
const TN: AdapterKind = AdapterKind::TursoTantivyNgram;

fn spec(id: &'static str, number: &'static str, title: &'static str, expect: Expect) -> Spec {
    Spec {
        id,
        number,
        title,
        expect,
        known: Vec::new(),
        turso_only: false,
        fallback_is_hazard: false,
    }
}

impl Spec {
    fn known(
        mut self,
        arms: &'static [AdapterKind],
        expect: Expect,
        class: Class,
        reason: &'static str,
    ) -> Self {
        self.known.push(Known {
            arms,
            expect,
            class,
            reason,
        });
        self
    }
}

pub fn specs() -> Vec<Spec> {
    const NO_STEM: &str = "no stemming tokenizer is available (FTS5 uses porter)";
    const NO_FOLD: &str = "no diacritic folding (FTS5 unicode61 remove_diacritics=1)";
    const SUBSTRING: &str = "n-gram index matches substrings, a superset of prefix matches";
    const CASE: &str = "the 'simple' tokenizer does not lowercase";
    vec![
        spec("exact_symbol", "1", "Exact symbol name lookup", Expect::Set(&["parse_config"])),
        spec("partial_symbol", "2", "Partial symbol lookup (camelCase inner word via search_terms)",
             Expect::Set(&["update_cloud"]))
            .known(&[TS], Expect::Set(&[]), Class::Configurable, CASE),
        spec("prefix_single", "3", "Prefix matching, single token (\"pars\"*)",
             Expect::Set(&["parse_config", "parser_state", "parser_file", "parsing_doc"]))
            .known(&[TN], Expect::Set(&["parse_config", "parser_state", "parser_file", "parsing_doc", "sparse_matrix"]),
                   Class::Approximation, SUBSTRING),
        spec("prefix_phrase", "3b", "Prefix matching, multi-token phrase prefix (\"data-api\"*)",
             Expect::Set(&["data_api"])),
        spec("qualified_name", "4", "Fully qualified name search", Expect::Set(&["parse_config"])),
        spec("signature", "5", "Function signature search (\"sql: &str\")", Expect::Set(&["run_query"])),
        spec("nl_doc", "6", "Natural-language documentation search (OR of words)",
             Expect::Set(&["parse_config", "long_ident"])),
        spec("nl_doc_stemmed", "6b", "Documentation search relying on stemming (\"running\")",
             Expect::Set(&["parser_state", "runs", "run_loop", "run_query"]))
            .known(&[T, TS], Expect::Set(&["parser_state"]), Class::ExpectedSemantic, NO_STEM)
            .known(&[TN], Expect::Set(&["parser_state"]), Class::ExpectedSemantic, NO_STEM),
        spec("phrase", "7", "Phrase matching (\"database query\")", Expect::Set(&["data_api", "run_query"])),
        spec("bool_and", "8a", "Boolean AND", Expect::Set(&["data_api", "run_query"])),
        spec("bool_or", "8b", "Boolean OR", Expect::Set(&["sparse_matrix", "update_cloud"]))
            .known(&[TS], Expect::Set(&["sparse_matrix"]), Class::Configurable, CASE),
        spec("bool_not", "8c", "Boolean NOT", Expect::Set(&["run_query"])),
        spec("bool_phrase_prefix", "8d", "Boolean OR of phrase prefixes, with NOT",
             Expect::Set(&["data_api", "run_loop"])),
        spec("ident_snake", "9a", "Identifier tokenization: snake_case part", Expect::Set(&["parse_config"]))
            .known(&[TN], Expect::Set(&["parse_config", "long_ident"]), Class::Approximation, SUBSTRING),
        spec("ident_camel", "9b", "Identifier tokenization: camelCase word from search_terms",
             Expect::Set(&["update_cloud", "data_api"])),
        spec("ident_digits", "9c", "Identifier tokenization: alphanumeric token (utf8)",
             Expect::Set(&["utf8_decode"])),
        spec("ident_long", "9d", "Identifier tokenization: single token longer than 40 bytes",
             Expect::Set(&["long_ident"]))
            .known(&[T], Expect::Set(&[]), Class::Configurable,
                   "the 'default' tokenizer drops tokens over 40 bytes; 'simple' keeps them but is case-sensitive"),
        spec("case_upper", "10a", "Case-insensitive lookup (PARSE_CONFIG)", Expect::Set(&["parse_config"]))
            .known(&[TS], Expect::Set(&[]), Class::Configurable, CASE),
        spec("case_acronym", "10b", "Case-insensitive lookup (httpserver vs HTTPServer)",
             Expect::Set(&["http_server"]))
            .known(&[TS], Expect::Set(&[]), Class::Configurable, CASE),
        spec("unicode_latin", "11a", "Unicode identifier (größe)", Expect::Set(&["groesse"])),
        spec("unicode_fold", "11b", "Unicode diacritic folding (cafe matches café / Cafés)",
             Expect::Set(&["cafe_menu", "groesse"]))
            .known(&[T, TS], Expect::Set(&[]), Class::ExpectedSemantic, NO_FOLD)
            .known(&[TN], Expect::Set(&[]), Class::ExpectedSemantic, NO_FOLD),
        spec("unicode_cjk", "11c", "Unicode identifier (CJK 数据处理)", Expect::Set(&["cjk"])),
        spec("unicode_cyrillic_case", "11d", "Unicode case folding (РАЗОБРАТЬ_СТРОКУ)",
             Expect::Set(&["cyrillic"]))
            .known(&[TS], Expect::Set(&[]), Class::Configurable, CASE),
        spec("update", "12", "Update indexed content (rename sparse_matrix to dense_grid)",
             Expect::Steps(&["sparse=[]", "dense=[sparse_matrix]"])),
        spec("delete", "13", "Delete indexed content", Expect::Steps(&["before=[utf8_decode]", "after=[]"])),
        spec("after_commit", "14", "Search immediately after commit (second connection)",
             Expect::Steps(&["writer=[fresh]", "reader=[fresh]"])),
        spec("uncommitted", "15", "Search within an uncommitted transaction",
             Expect::Steps(&["writer=[pending]", "reader=[]", "after_rollback=[]"])),
        spec("weighting", "16", "Column weighting (name hit outranks docstring hit)",
             Expect::Ordered(&["run_loop", "drain_events"])),
        spec("contract", "17", "Ranked-result API contract (limit, order, score sign)", Expect::Contract),
        spec("bounded_exclude", "18", "Bounded search excluding file/doc kinds", Expect::Set(&["parser_state"])),
        spec("production_search", "19", "Production search_nodes (phrase channel + OR of prefixes)",
             Expect::Top("parse_config")),
        spec("column_filter", "20", "Single-column filter (name : parse)",
             Expect::Set(&["parse_config", "parsing_doc"]))
            .known(&[T, TS], Expect::Set(&["parse_config"]), Class::ExpectedSemantic, NO_STEM)
            .known(&[TN], Expect::Set(&["parse_config", "sparse_matrix", "parser_state", "parser_file"]),
                   Class::Approximation,
                   "no stemming (misses Parsing) and substring matching (adds every name containing 'parse')"),
        spec("recall_production_shape", "21a",
             "Memory recall, production SQL shape (newest first, LIMIT 2)", Expect::Ordered(&["2", "7"])),
        spec("recall_adapted", "21b", "Memory recall, subquery form (newest first, LIMIT 2)",
             Expect::Ordered(&["2", "7"])),
        spec("recall_since", "21c", "Memory recall with since filter", Expect::Ordered(&["2", "7", "5"])),
        spec("body_prefix_pairs", "22a", "Executable-body search, production prefix pairs",
             Expect::Set(&["b_precond"])),
        spec("body_exact_pairs", "22b", "Executable-body search, exact-term pairs", Expect::Set(&["b_precond"])),
        spec("incremental_path", "23", "Index created before rows, then incremental inserts",
             Expect::Set(&["parse_config"])),
        spec("reopen", "24", "Search after closing and reopening the database", Expect::Set(&["parse_config"])),
        // `fts_match(name, …)` names one column of the five-column index.
        // "batching" occurs only in a docstring, so an empty result means
        // the column list was honored.
        Spec {
            turso_only: true,
            fallback_is_hazard: true,
            ..spec("planner_guard", "25",
                   "FTS call naming a subset of the indexed columns (fts_match(name, 'batching'))",
                   Expect::Set(&[]))
        },
        Spec {
            turso_only: true,
            ..spec("stem_tokenizer", "26",
                   "Stemming tokenizer for a native index (tokenizer='en_stem')",
                   Expect::Steps(&["created"]))
        },
    ]
}

fn ids(hits: &[Hit]) -> Vec<String> {
    hits.iter().map(|h| h.id.clone()).collect()
}

fn fmt_ids(v: &[String]) -> String {
    format!("[{}]", v.join(","))
}

/// Ranked-result contract that production callers depend on: at most
/// `limit` rows, every id is a real node, scores finite, positive and
/// non-increasing (best first, "higher is better").
fn check_contract(hits: &[Hit], limit: usize, known_ids: &[String]) -> Result<(), String> {
    if hits.is_empty() {
        return Err("no results".into());
    }
    if hits.len() > limit {
        return Err(format!("{} rows > limit {limit}", hits.len()));
    }
    for w in hits.windows(2) {
        if w[1].score > w[0].score + 1e-9 {
            return Err(format!(
                "scores increase: {} then {}",
                w[0].score, w[1].score
            ));
        }
    }
    if let Some(h) = hits.iter().find(|h| !h.score.is_finite() || h.score <= 0.0) {
        return Err(format!("score {} is not a positive finite number", h.score));
    }
    if let Some(h) = hits.iter().find(|h| !known_ids.contains(&h.id)) {
        return Err(format!("unknown id {}", h.id));
    }
    Ok(())
}

async fn load_fixture(a: &Adapter, index_first: bool) -> Result<(), String> {
    if index_first {
        a.create_schema().await?;
    } else {
        a.create_base().await?;
    }
    a.begin().await?;
    a.insert_nodes(&fixture_nodes()).await?;
    a.insert_bodies(&fixture_bodies()).await?;
    a.insert_decisions(&fixture_decisions()).await?;
    a.commit().await?;
    if !index_first {
        a.build_fts().await?;
    }
    Ok(())
}

type Obs = Result<Vec<String>, SearchError>;

async fn search(a: &Adapter, q: Q) -> Obs {
    a.search_nodes(&q, 50, false, None).await.map(|h| ids(&h))
}

async fn run_one(id: &str, kind: AdapterKind, dir: &std::path::Path) -> Result<Obs, String> {
    let a = Adapter::open(kind, dir).await?;
    load_fixture(&a, id == "incremental_path").await?;
    let e = SearchError::Engine;
    Ok(match id {
        "exact_symbol" | "incremental_path" => search(&a, Q::term("parse_config")).await,
        "partial_symbol" => search(&a, Q::term("cloud")).await,
        "prefix_single" => search(&a, Q::prefix("pars")).await,
        "prefix_phrase" => search(&a, Q::prefix("data-api")).await,
        "qualified_name" => search(&a, Q::term("crate::config::parse_config")).await,
        "signature" => search(&a, Q::term("sql: &str")).await,
        "nl_doc" => search(&a, Q::exact_match("configuration settings").ok_or("query")?).await,
        "nl_doc_stemmed" => search(&a, Q::term("running")).await,
        "phrase" => search(&a, Q::term("database query")).await,
        "bool_and" => search(&a, Q::and(vec![Q::term("database"), Q::term("query")])).await,
        "bool_or" => search(&a, Q::or(vec![Q::term("sparse"), Q::term("cloud")])).await,
        "bool_not" => search(&a, Q::not(Q::term("query"), Q::term("batching"))).await,
        "bool_phrase_prefix" => {
            search(
                &a,
                Q::not(
                    Q::or(vec![Q::prefix("data api"), Q::prefix("run loo")]),
                    Q::term("shutdownx"),
                ),
            )
            .await
        }
        "ident_snake" => search(&a, Q::term("config")).await,
        "ident_camel" => search(&a, Q::term("client")).await,
        "ident_digits" => search(&a, Q::term("utf8")).await,
        "ident_long" => {
            search(
                &a,
                Q::term("AbstractSingletonProxyFactoryBeanConfigurationHolder"),
            )
            .await
        }
        "case_upper" => search(&a, Q::term("PARSE_CONFIG")).await,
        "case_acronym" => search(&a, Q::term("httpserver")).await,
        "unicode_latin" => search(&a, Q::term("größe")).await,
        "unicode_fold" => search(&a, Q::term("cafe")).await,
        "unicode_cjk" => search(&a, Q::term("数据处理")).await,
        "unicode_cyrillic_case" => search(&a, Q::term("РАЗОБРАТЬ_СТРОКУ")).await,
        "update" => {
            let mut n = fixture_nodes()
                .into_iter()
                .find(|n| n.id == "sparse_matrix")
                .ok_or("fixture")?;
            n.name = "dense_grid".into();
            n.qualified_name = "crate::linalg::dense_grid".into();
            n.docstring = Some("Compressed grid storage.".into());
            n.signature = Some("pub struct dense_grid".into());
            a.update_node(&n).await?;
            let old = search(&a, Q::term("sparse")).await;
            let new = search(&a, Q::term("dense")).await;
            match (old, new) {
                (Ok(o), Ok(n)) => Ok(vec![
                    format!("sparse={}", fmt_ids(&o)),
                    format!("dense={}", fmt_ids(&n)),
                ]),
                (Err(x), _) | (_, Err(x)) => Err(x),
            }
        }
        "delete" => {
            let before = search(&a, Q::term("utf8")).await;
            a.delete_node("utf8_decode").await?;
            let after = search(&a, Q::term("utf8")).await;
            match (before, after) {
                (Ok(b), Ok(af)) => Ok(vec![
                    format!("before={}", fmt_ids(&b)),
                    format!("after={}", fmt_ids(&af)),
                ]),
                (Err(x), _) | (_, Err(x)) => Err(x),
            }
        }
        "after_commit" | "uncommitted" => {
            let fresh = id == "after_commit";
            let (nid, name) = if fresh {
                ("fresh", "freshly_committed")
            } else {
                ("pending", "pending_symbol")
            };
            let reader = a.connect_another().await?;
            let mut n = fixture_nodes().remove(0);
            n.id = nid.into();
            n.name = name.into();
            n.qualified_name = format!("crate::x::{name}");
            n.docstring = None;
            n.signature = None;
            a.begin().await?;
            a.insert_nodes(&[n]).await?;
            let q = Q::term(name);
            if fresh {
                a.commit().await?;
            }
            let w = search(&a, q.clone()).await;
            let r = search(&reader, q.clone()).await;
            let mut steps = Vec::new();
            match (w, r) {
                (Ok(w), Ok(r)) => {
                    steps.push(format!("writer={}", fmt_ids(&w)));
                    steps.push(format!("reader={}", fmt_ids(&r)));
                }
                (Err(x), _) | (_, Err(x)) => return Ok(Err(x)),
            }
            if !fresh {
                a.rollback().await?;
                match search(&a, q).await {
                    Ok(v) => steps.push(format!("after_rollback={}", fmt_ids(&v))),
                    Err(x) => return Ok(Err(x)),
                }
            }
            drop(reader);
            Ok(steps)
        }
        "weighting" => search(&a, Q::term("loop")).await,
        "contract" => {
            let known: Vec<String> = fixture_nodes().into_iter().map(|n| n.id).collect();
            match a
                .search_nodes(
                    &Q::or(vec![Q::term("run"), Q::term("query"), Q::term("parser")]),
                    3,
                    false,
                    None,
                )
                .await
            {
                Ok(h) => match check_contract(&h, 3, &known) {
                    Ok(()) => Ok(vec!["contract=ok".into()]),
                    Err(m) => Ok(vec![format!("contract violated: {m}; got {:?}", h)]),
                },
                Err(x) => Err(x),
            }
        }
        "bounded_exclude" => a
            .search_nodes(&Q::term("parser"), 50, true, None)
            .await
            .map(|h| ids(&h)),
        "production_search" => a
            .production_search("parse config", 10, None)
            .await
            .map(|h| ids(&h)),
        "column_filter" => search(&a, Q::Column("name", Box::new(Q::term("parse")))).await,
        "recall_production_shape" => a.recall(&Q::term("cache"), None, 2, true).await,
        "recall_adapted" => a.recall(&Q::term("cache"), None, 2, false).await,
        "recall_since" => a.recall(&Q::term("cache"), Some(600), 10, false).await,
        "body_prefix_pairs" | "body_exact_pairs" => {
            let pair = |l: &str, r: &str| {
                if id == "body_prefix_pairs" {
                    Q::and(vec![Q::prefix(l), Q::prefix(r)])
                } else {
                    Q::and(vec![Q::term(l), Q::term(r)])
                }
            };
            let (l, r) = if id == "body_prefix_pairs" {
                ("preco", "matri")
            } else {
                ("precond", "matrix")
            };
            a.search_bodies(&Q::or(vec![pair(l, r), pair("zzzzz", "yyyyy")]), 50, None)
                .await
        }
        "reopen" => {
            drop(a);
            let b = Adapter::open(kind, dir).await?;
            search(&b, Q::term("parse_config")).await
        }
        "stem_tokenizer" => {
            // Tantivy ships `en_stem`; this checks whether turso exposes it.
            match a
                .db
                .batch("CREATE TABLE stem_probe (x TEXT); CREATE INDEX stem_probe_fts ON stem_probe USING fts (x) WITH (tokenizer='en_stem')")
                .await
            {
                Ok(()) => Ok(vec!["created".into()]),
                Err(m) => Err(SearchError::Unsupported(m)),
            }
        }
        "planner_guard" => {
            let sql = "SELECT id, fts_score(name, ?1) FROM nodes WHERE fts_match(name, ?1)";
            let params = [Val::from("\"batching\"")];
            match a.guarded_query(sql, &params).await {
                Err(SearchError::PlanFallback(m)) => {
                    let raw = a.db.query(sql, &params).await.map_err(e);
                    let detail = match raw {
                        Ok(rows) => format!(
                            "unguarded statement returned {:?}",
                            rows.iter()
                                .map(|r| format!("{}:{}", r[0].text(), r[1].text()))
                                .collect::<Vec<_>>()
                        ),
                        Err(x) => format!("unguarded statement failed: {x}"),
                    };
                    Err(SearchError::PlanFallback(format!("{m}; {detail}")))
                }
                Err(x) => Err(x),
                Ok(rows) => Ok(rows.iter().map(|r| r[0].text()).collect()),
            }
        }
        other => return Err(format!("unknown scenario {other}")),
    })
}

pub struct Outcome {
    pub class: Class,
    pub got: String,
    pub reason: String,
}

pub async fn evaluate(spec: &Spec, kind: AdapterKind) -> Outcome {
    if spec.turso_only && !kind.is_turso() {
        return Outcome {
            class: Class::NotApplicable,
            got: String::new(),
            reason: "turso-only check".into(),
        };
    }
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return Outcome {
                class: Class::CorrectnessFailure,
                got: String::new(),
                reason: format!("tempdir: {e}"),
            }
        }
    };
    let obs = run_one(spec.id, kind, dir.path()).await;
    let fail = |got: String, reason: String| Outcome {
        class: Class::CorrectnessFailure,
        got,
        reason,
    };
    match obs {
        Err(setup) => fail(String::new(), format!("setup: {setup}")),
        Ok(Err(SearchError::Unsupported(m))) => Outcome {
            class: Class::Unsupported,
            got: String::new(),
            reason: m,
        },
        Ok(Err(SearchError::PlanFallback(m))) if spec.fallback_is_hazard => Outcome {
            class: Class::Hazard,
            got: String::new(),
            reason: m,
        },
        Ok(Err(x)) => fail(String::new(), x.to_string()),
        Ok(Ok(got)) => {
            let shown = fmt_ids(&got);
            let pass = match spec.expect {
                Expect::Contract => got == ["contract=ok"],
                ref e => e.matches(&got),
            };
            if pass {
                return Outcome {
                    class: Class::Pass,
                    got: shown,
                    reason: String::new(),
                };
            }
            if let Some(k) = spec
                .known
                .iter()
                .find(|k| k.arms.contains(&kind) && k.expect.matches(&got))
            {
                return Outcome {
                    class: k.class,
                    got: shown,
                    reason: k.reason.into(),
                };
            }
            fail(shown, format!("expected {}", spec.expect.describe()))
        }
    }
}

pub async fn run(kinds: &[AdapterKind]) -> (Value, bool) {
    let specs = specs();
    let mut rows = Vec::new();
    let mut control_ok = true;
    println!(
        "| # | Scenario | {} |",
        kinds
            .iter()
            .map(|k| k.label())
            .collect::<Vec<_>>()
            .join(" | ")
    );
    println!("|---|---|{}", "---|".repeat(kinds.len()));
    for s in &specs {
        let mut cells = Vec::new();
        for &kind in kinds {
            let o = evaluate(s, kind).await;
            if kind == AdapterKind::LibsqlFts5
                && !matches!(o.class, Class::Pass | Class::NotApplicable)
            {
                control_ok = false;
                eprintln!("control failed {}: got {} ({})", s.id, o.got, o.reason);
            }
            cells.push(o.class.label());
            rows.push(json!({
                "scenario": s.id,
                "number": s.number,
                "title": s.title,
                "expected": s.expect.describe(),
                "adapter": kind.label(),
                "class": o.class.label(),
                "got": o.got,
                "reason": o.reason,
            }));
        }
        println!("| {} | {} | {} |", s.number, s.title, cells.join(" | "));
    }
    (json!(rows), control_ok)
}
