//! Performance phases, each in a fresh child process.
//!
//! The parent (`perf`) spawns `fts-compare perf-child <phase> <adapter>
//! <corpus> <dir>` once per phase, adapter and repetition, alternating the
//! adapter order between repetitions. A child prints one JSON object with
//! its timings and `/proc/self/status` memory figures, so peak RSS belongs
//! to exactly one engine and one phase.
//!
//! Phases:
//! * `build-bulk` — production bulk path: rows first, full-text index after.
//! * `build-incremental` — FTS live from the start, all rows in one
//!   transaction (what a first sync without the bulk path costs).
//! * `startup` — open an existing database and run the first queries; the
//!   parent runs it twice per repetition, after evicting the database files
//!   from the OS page cache (`cold`) and again with them cached (`warm`).
//! * `query` — warm-up, then measured passes over every query category.
//!   Categories hold only queries both engines can express, except `prefix`
//!   (turso reports every query as unsupported).
//! * `update` — incremental file edits, single-row updates and deletes, each
//!   followed by correctness checks, then query latency on the edited index.
//! * `concurrency` — the same read workload on 1 and 4 connections.
//!
//! Query, startup, update and concurrency phases start from a byte copy of
//! one database built by `build-bulk`, so they never see each other's writes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};

use crate::adapter::{disk_bytes, Adapter, AdapterKind, SearchError, DB_FILE};
use crate::corpus::{self, Corpus, Rng};
use crate::query::{render_tantivy, PrefixPolicy, Q};
use crate::relevance::{sample_queries, Intent, Query};
use crate::stats::summary;

const MEASURED_PASSES: usize = 3;
const REPEATED_QUERY_RUNS: usize = 500;
const EDIT_FILES: usize = 50;
const ROW_UPDATES: usize = 100;
const DELETE_FILES: usize = 20;
const READERS: usize = 4;

fn status_kib(key: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

fn memory() -> Value {
    json!({ "rss_kib": status_kib("VmRSS:"), "peak_rss_kib": status_kib("VmHWM:") })
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn us(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e6
}

/// Drops the database files from the OS page cache (Linux). Clean pages are
/// released immediately; the files were checkpointed and closed beforehand.
fn evict_page_cache(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.filter_map(Result::ok) {
        if let Ok(f) = std::fs::File::open(e.path()) {
            use std::os::fd::AsRawFd;
            // SAFETY: valid open descriptor for the duration of the call.
            unsafe {
                libc::fsync(f.as_raw_fd());
                libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
            }
        }
    }
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    for e in std::fs::read_dir(src)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        if e.path().is_file() {
            std::fs::copy(e.path(), dst.join(e.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Size of the FTS5 shadow tables via `dbstat` (libsql only).
async fn fts_storage(a: &Adapter) -> Value {
    if a.kind.is_turso() {
        // turso keeps segments in internal `fts_dir_<index>` tables that SQL
        // cannot read; the report derives FTS size from the file-size delta
        // of the bulk build instead.
        json!("not exposed by turso; see disk_bytes - disk_bytes_before_fts_build")
    } else {
        match a
            .db
            .query(
                "SELECT CASE WHEN name LIKE 'nodes_fts%' THEN 'nodes_fts' \
                 WHEN name LIKE 'executable_body_fts%' THEN 'executable_body_fts' \
                 WHEN name LIKE 'memory_decisions_fts%' THEN 'memory_decisions_fts' END AS t, \
                 SUM(pgsize) FROM dbstat WHERE t IS NOT NULL GROUP BY t",
                &[],
            )
            .await
        {
            Ok(rows) => Value::Object(
                rows.iter()
                    .map(|r| (r[0].text(), json!(r[1].int())))
                    .collect(),
            ),
            Err(e) => json!(format!("unavailable: {e}")),
        }
    }
}

struct Workload {
    exact_name: Vec<Q>,
    prefix: Vec<Q>,
    phrase: Vec<Q>,
    complex: Vec<Q>,
    bounded: Vec<Q>,
    production: Vec<String>,
    recall: Vec<Q>,
    bodies: Vec<Q>,
}

fn workload(c: &Corpus) -> Workload {
    let qs: Vec<Query> = sample_queries(&c.nodes, corpus::SEED);
    let by = |i: Intent| -> Vec<&Query> { qs.iter().filter(|q| q.intent == i).collect() };
    let exact = |i: Intent| -> Vec<Q> { by(i).iter().filter_map(|q| q.exact.clone()).collect() };
    let mut rng = Rng::new(corpus::SEED ^ 0xB0D1);
    let recall_words: Vec<Q> = {
        let mut words: Vec<String> = c
            .decisions
            .iter()
            .flat_map(|d| {
                d.text
                    .split(|ch: char| !ch.is_alphanumeric())
                    .map(str::to_lowercase)
                    .collect::<Vec<_>>()
            })
            .filter(|w| w.len() >= 5 && w.chars().all(char::is_alphabetic))
            .collect();
        words.sort();
        words.dedup();
        rng.shuffle(&mut words);
        words.into_iter().take(40).map(Q::Term).collect()
    };
    let bodies: Vec<Q> = {
        let mut picks = Vec::new();
        let mut attempts = 0;
        while picks.len() < 40 && attempts < 4000 && !c.bodies.is_empty() {
            attempts += 1;
            let b = &c.bodies[rng.below(c.bodies.len())];
            let mut toks: Vec<String> = b
                .body
                .split(|ch: char| !ch.is_alphanumeric())
                .filter(|t| t.len() >= 5 && t.chars().all(char::is_alphabetic))
                .map(str::to_lowercase)
                .collect();
            toks.sort();
            toks.dedup();
            if toks.len() < 3 {
                continue;
            }
            rng.shuffle(&mut toks);
            let (x, y, z) = (toks[0].clone(), toks[1].clone(), toks[2].clone());
            let pair = |l: &str, r: &str| Q::and(vec![Q::term(l), Q::term(r)]);
            // Production's pair expansion, with exact terms (both engines).
            picks.push(Q::or(vec![pair(&x, &y), pair(&x, &z), pair(&y, &z)]));
        }
        picks
    };
    Workload {
        exact_name: exact(Intent::ExactName),
        prefix: by(Intent::Prefix)
            .iter()
            .map(|q| Q::prefix(&q.raw))
            .collect(),
        phrase: [Intent::Qualified, Intent::Signature, Intent::DocPhrase]
            .into_iter()
            .flat_map(exact)
            .collect(),
        complex: [Intent::Boolean, Intent::DocNl]
            .into_iter()
            .flat_map(exact)
            .collect(),
        bounded: exact(Intent::PartialName),
        // Only inputs whose production query turso can express, so both
        // engines time the same statements (the rest are prefix-only).
        production: by(Intent::ExactName)
            .iter()
            .chain(by(Intent::DocNl).iter())
            .map(|q| q.raw.clone())
            .filter(|raw| {
                [Q::production_match(raw), Q::production_phrase(raw)]
                    .into_iter()
                    .flatten()
                    .all(|q| render_tantivy(&q, PrefixPolicy::Strict).is_ok())
            })
            .collect(),
        recall: recall_words,
        bodies,
    }
}

/// Times every query `passes` times after one warm-up pass. Unsupported
/// queries are counted, never timed; any other error aborts the category.
async fn time_category<F, Fut>(queries: usize, passes: usize, mut run: F) -> Value
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<usize, SearchError>>,
{
    let mut unsupported = 0;
    let mut supported = vec![true; queries];
    for (i, ok) in supported.iter_mut().enumerate() {
        match run(i).await {
            Ok(_) => {}
            Err(SearchError::Unsupported(_)) => {
                unsupported += 1;
                *ok = false;
            }
            Err(e) => return json!({ "error": e.to_string() }),
        }
    }
    let mut samples = Vec::new();
    let mut rows = 0usize;
    for _ in 0..passes {
        for (i, ok) in supported.iter().enumerate() {
            if !ok {
                continue;
            }
            let t = Instant::now();
            match run(i).await {
                Ok(n) => rows += n,
                Err(e) => return json!({ "error": e.to_string() }),
            }
            samples.push(us(t));
        }
    }
    json!({
        "queries": queries,
        "unsupported": unsupported,
        "rows_returned": rows,
        "latency_us": summary(&samples),
    })
}

async fn query_phase(a: &Adapter, w: &Workload) -> Value {
    let p = MEASURED_PASSES;
    let mut out = serde_json::Map::new();
    out.insert(
        "single_exact_name".into(),
        time_category(w.exact_name.len(), p, |i| async move {
            a.search_nodes(&w.exact_name[i], 10, false, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "prefix".into(),
        time_category(w.prefix.len(), p, |i| async move {
            a.search_nodes(&w.prefix[i], 10, false, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "phrase".into(),
        time_category(w.phrase.len(), p, |i| async move {
            a.search_nodes(&w.phrase[i], 10, false, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "complex_boolean_or".into(),
        time_category(w.complex.len(), p, |i| async move {
            a.search_nodes(&w.complex[i], 10, false, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "bounded_kind_filter".into(),
        time_category(w.bounded.len(), p, |i| async move {
            a.search_nodes(&w.bounded[i], 10, true, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "production_search_nodes_common_subset".into(),
        time_category(w.production.len(), p, |i| async move {
            a.production_search(&w.production[i], 10, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "memory_recall".into(),
        time_category(w.recall.len(), p, |i| async move {
            a.recall(&w.recall[i], Some(0), 20, false)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    out.insert(
        "executable_body_pairs".into(),
        time_category(w.bodies.len(), p, |i| async move {
            a.search_bodies(&w.bodies[i], 200, None)
                .await
                .map(|h| h.len())
        })
        .await,
    );
    // Repeated identical query: throughput of a hot statement.
    if let Some(q) = w.exact_name.first() {
        let mut samples = Vec::with_capacity(REPEATED_QUERY_RUNS);
        let total = Instant::now();
        for _ in 0..REPEATED_QUERY_RUNS {
            let t = Instant::now();
            if let Err(e) = a.search_nodes(q, 10, false, None).await {
                samples.clear();
                out.insert("repeated".into(), json!({ "error": e.to_string() }));
                break;
            }
            samples.push(us(t));
        }
        if !samples.is_empty() {
            let secs = total.elapsed().as_secs_f64();
            out.insert(
                "repeated".into(),
                json!({
                    "runs": REPEATED_QUERY_RUNS,
                    "queries_per_sec": REPEATED_QUERY_RUNS as f64 / secs,
                    "latency_us": summary(&samples),
                }),
            );
        }
    }
    Value::Object(out)
}

async fn found(a: &Adapter, term: &str, id: &str) -> Result<bool, String> {
    a.search_nodes(&Q::term(term), 1000, false, None)
        .await
        .map(|h| h.iter().any(|x| x.id == id))
        .map_err(|e| e.to_string())
}

async fn update_phase(a: &Adapter, c: &Corpus, w: &Workload) -> Result<Value, String> {
    // 1. Incremental file edits, one transaction per file (production sync).
    let edits = corpus::edit_plan(c, EDIT_FILES, corpus::SEED);
    let mut edit_ms = Vec::new();
    let mut rows = 0usize;
    let total = Instant::now();
    for e in &edits {
        let t = Instant::now();
        a.begin().await?;
        a.delete_file(&e.file_path).await?;
        a.insert_nodes(&e.nodes).await?;
        a.insert_bodies(&e.bodies).await?;
        a.commit().await?;
        edit_ms.push(ms(t));
        rows += e.nodes.len() + e.bodies.len();
    }
    let edit_total_s = total.elapsed().as_secs_f64();
    let added = a
        .search_nodes(&Q::term("bench_added_symbol"), 10_000, false, None)
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|h| h.id.ends_with("#bench_added"))
        .count();
    let mut renamed_ok = 0;
    let mut renamed_checked = 0;
    for n in edits
        .iter()
        .flat_map(|e| &e.nodes)
        .filter(|n| n.name.ends_with("_revised"))
        .take(20)
    {
        renamed_checked += 1;
        if found(a, &n.name, &n.id).await? {
            renamed_ok += 1;
        }
    }

    // 2. Single-row updates in autocommit mode (write + reindex latency).
    let mut rng = Rng::new(corpus::SEED ^ 0x0BDA7E);
    let edited: std::collections::HashSet<&str> =
        edits.iter().map(|e| e.file_path.as_str()).collect();
    let pool: Vec<&corpus::NodeRec> = c
        .nodes
        .iter()
        .filter(|n| !edited.contains(n.file_path.as_str()))
        .collect();
    let mut upd_ms = Vec::new();
    let mut updated = Vec::new();
    for i in 0..ROW_UPDATES.min(pool.len()) {
        let mut n = pool[rng.below(pool.len())].clone();
        if updated.iter().any(|(id, _): &(String, String)| *id == n.id) {
            continue;
        }
        let marker = format!("benchmarkmarker{i}");
        n.docstring = Some(format!("{} {marker}", n.docstring.unwrap_or_default()));
        let t = Instant::now();
        a.update_node(&n).await?;
        upd_ms.push(ms(t));
        updated.push((n.id.clone(), marker));
    }
    let mut upd_ok = 0;
    for (id, marker) in updated.iter().take(20) {
        if found(a, marker, id).await? {
            upd_ok += 1;
        }
    }

    // 3. Deletes: whole files, one transaction each.
    let mut files: Vec<&str> = pool.iter().map(|n| n.file_path.as_str()).collect();
    files.sort_unstable();
    files.dedup();
    rng.shuffle(&mut files);
    files.truncate(DELETE_FILES);
    let deleted: Vec<&corpus::NodeRec> = pool
        .iter()
        .copied()
        .filter(|n| files.contains(&n.file_path.as_str()))
        .collect();
    let mut del_ms = Vec::new();
    for f in &files {
        let t = Instant::now();
        a.begin().await?;
        a.delete_file(f).await?;
        a.commit().await?;
        del_ms.push(ms(t));
    }
    let mut del_checked = 0;
    let mut del_still_found = 0;
    for n in deleted
        .iter()
        .filter(|n| crate::query::tantivy_tokens(&n.name).len() == 1)
        .take(20)
    {
        del_checked += 1;
        if found(a, &n.name, &n.id).await? {
            del_still_found += 1;
        }
    }

    // 4. Query latency on the edited index (turso: more segments).
    let mut post = Vec::new();
    for q in &w.exact_name {
        let t = Instant::now();
        a.search_nodes(q, 10, false, None)
            .await
            .map_err(|e| e.to_string())?;
        post.push(us(t));
    }
    let t = Instant::now();
    a.optimize_nodes_fts().await?;
    let optimize_ms = ms(t);
    let mut post_optimize = Vec::new();
    for q in &w.exact_name {
        let t = Instant::now();
        a.search_nodes(q, 10, false, None)
            .await
            .map_err(|e| e.to_string())?;
        post_optimize.push(us(t));
    }
    a.checkpoint().await?;

    Ok(json!({
        "file_edits": {
            "transactions": edits.len(),
            "rows_written": rows,
            "rows_per_sec": rows as f64 / edit_total_s,
            "latency_ms": summary(&edit_ms),
            "check_added_symbols_found": format!("{added}/{}", edits.len()),
            "check_renamed_found": format!("{renamed_ok}/{renamed_checked}"),
        },
        "row_updates": {
            "updates": upd_ms.len(),
            "latency_ms": summary(&upd_ms),
            "check_new_text_found": format!("{upd_ok}/{}", updated.len().min(20)),
        },
        "file_deletes": {
            "transactions": del_ms.len(),
            "nodes_deleted": deleted.len(),
            "latency_ms": summary(&del_ms),
            "check_deleted_not_found": format!("{}/{del_checked}", del_checked - del_still_found),
        },
        "exact_name_after_updates_latency_us": summary(&post),
        "optimize_nodes_fts_ms": optimize_ms,
        "exact_name_after_optimize_latency_us": summary(&post_optimize),
        "disk_bytes_after_updates": a.disk_bytes(),
    }))
}

async fn concurrency_phase(a: Adapter, w: &Workload) -> Result<Value, String> {
    let queries: Vec<Q> = w.exact_name.clone();
    let passes = 5;
    // Baseline: one connection.
    let t = Instant::now();
    for _ in 0..passes {
        for q in &queries {
            a.search_nodes(q, 10, false, None)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    let single_qps = (passes * queries.len()) as f64 / t.elapsed().as_secs_f64();
    let mut readers = vec![a];
    for _ in 1..READERS {
        let extra = readers[0].connect_another().await?;
        readers.push(extra);
    }
    let t = Instant::now();
    let mut tasks = Vec::new();
    for r in readers {
        let qs = queries.clone();
        tasks.push(tokio::spawn(async move {
            for _ in 0..passes {
                for q in &qs {
                    r.search_nodes(q, 10, false, None)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok::<_, String>(())
        }));
    }
    for task in tasks {
        task.await.map_err(|e| e.to_string())??;
    }
    let multi_qps = (READERS * passes * queries.len()) as f64 / t.elapsed().as_secs_f64();
    Ok(json!({
        "readers": READERS,
        "execution_model": "one connection per tokio task on a multi-thread runtime, same process",
        "single_connection_qps": single_qps,
        "multi_connection_qps": multi_qps,
        "scaling": multi_qps / single_qps,
    }))
}

async fn build(
    kind: AdapterKind,
    c: &Corpus,
    dir: &Path,
    bulk: bool,
) -> Result<(Adapter, Value), String> {
    let t_open = Instant::now();
    let a = Adapter::open(kind, dir).await?;
    let open_ms = ms(t_open);
    if bulk {
        a.create_base().await?;
    } else {
        a.create_schema().await?;
    }
    let t = Instant::now();
    a.begin().await?;
    let t_part = Instant::now();
    a.insert_nodes(&c.nodes).await?;
    let nodes_ms = ms(t_part);
    let t_part = Instant::now();
    a.insert_bodies(&c.bodies).await?;
    let bodies_ms = ms(t_part);
    let t_part = Instant::now();
    a.insert_decisions(&c.decisions).await?;
    let decisions_ms = ms(t_part);
    let t_part = Instant::now();
    a.commit().await?;
    let commit_ms = ms(t_part);
    let load_ms = ms(t);
    a.checkpoint().await?;
    let bytes_before_fts = a.disk_bytes();
    let mut fts_ms = 0.0;
    if bulk {
        let t = Instant::now();
        a.build_fts().await?;
        fts_ms = ms(t);
        a.checkpoint().await?;
    }
    let records = c.nodes.len() + c.bodies.len() + c.decisions.len();
    let total_ms = load_ms + fts_ms;
    Ok((
        a,
        json!({
            "open_ms": open_ms,
            "row_load_ms": load_ms,
            "row_load_split_ms": {
                "nodes_200_row_inserts": nodes_ms,
                "bodies_single_row_inserts": bodies_ms,
                "decisions_single_row_inserts": decisions_ms,
                "commit": commit_ms,
            },
            "fts_build_ms": fts_ms,
            "total_ms": total_ms,
            "records_per_sec": records as f64 / (total_ms / 1e3),
            "disk_bytes_before_fts_build": bytes_before_fts,
        }),
    ))
}

/// Entry point of a child process; prints one JSON object.
pub async fn child(args: &[String]) {
    let get = |i: usize| args.get(i).cloned().unwrap_or_default();
    let (phase, adapter, corpus_path, dir) = (get(2), get(3), get(4), PathBuf::from(get(5)));
    let baseline = memory();
    let result: Result<Value, String> = async {
        let kind = AdapterKind::parse(&adapter).ok_or("adapter")?;
        let c = corpus::load(Path::new(&corpus_path))?;
        let w = workload(&c);
        let after_load = memory();
        let v = match phase.as_str() {
            "build-bulk" | "build-incremental" => {
                let (a, mut v) = build(kind, &c, &dir, phase == "build-bulk").await?;
                v["disk_bytes"] = json!(a.disk_bytes());
                v["fts_storage_bytes"] = fts_storage(&a).await;
                v
            }
            "startup" => {
                let t = Instant::now();
                let a = Adapter::open(kind, &dir).await?;
                let open_ms = ms(t);
                let q = w.exact_name.first().ok_or("no queries")?;
                let t = Instant::now();
                a.search_nodes(q, 10, false, None)
                    .await
                    .map_err(|e| e.to_string())?;
                let first = ms(t);
                let t = Instant::now();
                a.search_nodes(q, 10, false, None)
                    .await
                    .map_err(|e| e.to_string())?;
                let second = ms(t);
                json!({ "open_ms": open_ms, "first_query_ms": first, "second_query_ms": second })
            }
            "query" => {
                let a = Adapter::open(kind, &dir).await?;
                let v = query_phase(&a, &w).await;
                json!({ "categories": v, "steady_state": memory() })
            }
            "update" => {
                let a = Adapter::open(kind, &dir).await?;
                update_phase(&a, &c, &w).await?
            }
            "concurrency" => {
                let a = Adapter::open(kind, &dir).await?;
                concurrency_phase(a, &w).await?
            }
            other => return Err(format!("unknown phase {other}")),
        };
        Ok(json!({ "result": v, "memory_after_corpus_load": after_load }))
    }
    .await;
    let out = match result {
        Ok(mut v) => {
            v["memory_baseline"] = baseline;
            v["memory_end"] = memory();
            v
        }
        Err(e) => json!({ "error": e }),
    };
    println!("{out}");
}

fn spawn(phase: &str, kind: AdapterKind, corpus: &Path, dir: &Path) -> Value {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("fts_compare"));
    let out = std::process::Command::new(exe)
        .arg("perf-child")
        .arg(phase)
        .arg(kind.label())
        .arg(corpus)
        .arg(dir)
        .output();
    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .last()
                .and_then(|l| serde_json::from_str(l).ok())
                .unwrap_or_else(|| {
                    json!({
                        "error": format!(
                            "child exited {:?}: {}",
                            o.status.code(),
                            String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or_default()
                        )
                    })
                })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

pub async fn run(corpora: &[PathBuf], kinds: &[AdapterKind], reps: usize) -> Value {
    let mut per_corpus = Vec::new();
    for path in corpora {
        let c = match corpus::load(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                std::process::exit(2);
            }
        };
        let mut phases: HashMap<(String, &'static str), Vec<Value>> = HashMap::new();
        let mut golden: HashMap<&'static str, tempfile::TempDir> = HashMap::new();
        let rotated = |rep: usize| -> Vec<AdapterKind> {
            let mut v = kinds.to_vec();
            v.rotate_left(rep % kinds.len().max(1));
            v
        };
        let mut record = |phase: &str, kind: AdapterKind, v: Value| {
            if let Some(e) = v.get("error") {
                eprintln!("  {} {phase}: {e}", kind.label());
            }
            phases
                .entry((phase.to_string(), kind.label()))
                .or_default()
                .push(v);
        };
        for rep in 0..reps {
            for kind in rotated(rep) {
                eprintln!("perf {} rep {rep} {}: build", c.name, kind.label());
                for phase in ["build-bulk", "build-incremental"] {
                    let Ok(dir) = tempfile::tempdir() else {
                        continue;
                    };
                    let v = spawn(phase, kind, path, dir.path());
                    record(phase, kind, v);
                    if phase == "build-bulk" && !golden.contains_key(kind.label()) {
                        golden.insert(kind.label(), dir);
                    }
                }
            }
        }
        for rep in 0..reps {
            for kind in rotated(rep) {
                let Some(src) = golden.get(kind.label()) else {
                    continue;
                };
                eprintln!(
                    "perf {} rep {rep} {}: startup/query/update",
                    c.name,
                    kind.label()
                );
                for phase in ["startup", "query", "update", "concurrency"] {
                    if phase == "concurrency" && rep > 0 {
                        continue;
                    }
                    let Ok(dir) = tempfile::tempdir() else {
                        continue;
                    };
                    if let Err(e) = copy_dir(src.path(), dir.path()) {
                        record(phase, kind, json!({ "error": e }));
                        continue;
                    }
                    if phase == "startup" {
                        evict_page_cache(dir.path());
                        let cold = spawn(phase, kind, path, dir.path());
                        record("startup-cold-cache", kind, cold);
                        let warm = spawn(phase, kind, path, dir.path());
                        record("startup-warm-cache", kind, warm);
                    } else {
                        let v = spawn(phase, kind, path, dir.path());
                        record(phase, kind, v);
                    }
                }
            }
        }
        let golden_bytes: HashMap<&str, u64> = golden
            .iter()
            .map(|(k, d)| (*k, disk_bytes(&d.path().join(DB_FILE))))
            .collect();
        let mut phase_map = serde_json::Map::new();
        for ((phase, adapter), runs) in phases {
            phase_map
                .entry(phase)
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .map(|m| m.insert(adapter.to_string(), json!(runs)));
        }
        per_corpus.push(json!({
            "corpus": c.name,
            "nodes": c.nodes.len(),
            "bodies": c.bodies.len(),
            "decisions": c.decisions.len(),
            "body_bytes": c.bodies.iter().map(|b| b.body.len()).sum::<usize>(),
            "golden_disk_bytes": golden_bytes,
            "phases": phase_map,
        }));
    }
    json!({
        "environment": crate::sysinfo::collect(),
        "repetitions": reps,
        "measured_passes_per_query": MEASURED_PASSES,
        "seed": corpus::SEED,
        "corpora": per_corpus,
    })
}
