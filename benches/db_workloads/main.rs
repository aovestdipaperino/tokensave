//! Database workload bench: per-operation latency of tokensave's storage
//! paths, with correctness gates and machine-readable output.
//!
//! ```text
//! cargo bench --bench db_workloads -- [--project DIR] [--out FILE]
//!     [--iterations N] [--warmup N] [--index-runs N] [--inputs N]
//! ```
//!
//! The project (default: this crate's `src/`) is copied into a temporary
//! directory and indexed there, so the source tree is never modified. Every
//! workload first runs once per input to record a result fingerprint; each
//! timed operation must reproduce it. Any error or mismatch fails the run
//! (exit 1) and is not counted as a latency sample.
//!
//! Measurements are warm: same process, OS page cache populated, one client.
//! Extraction runs in-process here (the worker pool only starts from the
//! `tokensave` binary), so `index.full` is not the CLI's wall time.

mod manifest;
mod stats;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use stats::Recorder;
use tokensave::profile::{self, Mode, Snapshot};
use tokensave::tokensave::TokenSave;
use tokensave::types::NodeKind;

fn drain_profile() -> Snapshot {
    profile::recorder().map_or_else(Snapshot::new, profile::Recorder::drain)
}

type OpFuture<'a> = Pin<Box<dyn Future<Output = Result<String, String>> + 'a>>;
type Op = for<'a> fn(&'a TokenSave, String) -> OpFuture<'a>;

#[derive(Clone)]
struct Args {
    project: Option<PathBuf>,
    out: Option<PathBuf>,
    iterations: usize,
    warmup: usize,
    index_runs: usize,
    inputs: usize,
    profile: Mode,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        project: None,
        out: None,
        iterations: 1000,
        warmup: 100,
        index_runs: 3,
        inputs: 64,
        profile: Mode::Off,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        let number = |v: String| v.parse::<usize>().map_err(|e| format!("{flag}: {e}"));
        match flag.as_str() {
            "--profile" => {
                args.profile =
                    Mode::parse(&value()?).ok_or("profile must be off, light or detailed")?;
            }
            "--project" => args.project = Some(PathBuf::from(value()?)),
            "--out" => args.out = Some(PathBuf::from(value()?)),
            "--iterations" => args.iterations = number(value()?)?,
            "--warmup" => args.warmup = number(value()?)?,
            "--index-runs" => args.index_runs = number(value()?)?.max(1),
            "--inputs" => args.inputs = number(value()?)?.max(1),
            // Passed by `cargo bench` to every bench binary.
            "--bench" => {}
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

/// Directory names never copied into the benchmark project.
const SKIP_DIRS: [&str; 4] = [".git", ".tokensave", "target", "node_modules"];

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let ty = entry.file_type()?;
        if ty.is_dir() {
            if !SKIP_DIRS.iter().any(|s| name == *s) {
                copy_tree(&entry.path(), &dst.join(&name))?;
            }
        } else if ty.is_file() {
            std::fs::copy(entry.path(), dst.join(&name))?;
        }
    }
    Ok(())
}

// --- Operations: each returns a fingerprint of its result -----------------

fn ids<'n>(nodes: impl Iterator<Item = &'n tokensave::types::Node>) -> String {
    let v: Vec<&str> = nodes.map(|n| n.id.as_str()).collect();
    format!("{}:{}", v.len(), v.join(","))
}

fn op_node_by_id(cg: &TokenSave, id: String) -> OpFuture<'_> {
    Box::pin(async move {
        match cg.get_node(&id).await.map_err(|e| e.to_string())? {
            Some(n) => Ok(n.id),
            None => Err(format!("node {id} not found")),
        }
    })
}

fn op_nodes_by_name(cg: &TokenSave, name: String) -> OpFuture<'_> {
    Box::pin(async move {
        let mut nodes = cg
            .get_nodes_by_name(&name)
            .await
            .map_err(|e| e.to_string())?;
        if nodes.is_empty() {
            return Err(format!("no node named {name}"));
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(ids(nodes.iter()))
    })
}

fn op_nodes_by_qualified_name(cg: &TokenSave, qn: String) -> OpFuture<'_> {
    Box::pin(async move {
        let mut nodes = cg
            .get_nodes_by_qualified_name(&qn)
            .await
            .map_err(|e| e.to_string())?;
        if nodes.is_empty() {
            return Err(format!("no node with qualified name {qn}"));
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(ids(nodes.iter()))
    })
}

/// Ranked FTS search; the fingerprint keeps result order, so ranking
/// changes are caught as mismatches.
fn op_search(cg: &TokenSave, term: String) -> OpFuture<'_> {
    Box::pin(async move {
        let hits = cg.search(&term, 20).await.map_err(|e| e.to_string())?;
        if hits.is_empty() {
            return Err(format!("search {term:?} returned nothing"));
        }
        Ok(ids(hits.iter().map(|h| &h.node)))
    })
}

fn op_callers_depth3(cg: &TokenSave, id: String) -> OpFuture<'_> {
    Box::pin(async move {
        let mut r = cg.get_callers(&id, 3).await.map_err(|e| e.to_string())?;
        r.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        Ok(ids(r.iter().map(|(n, _)| n)))
    })
}

fn op_callees_depth3(cg: &TokenSave, id: String) -> OpFuture<'_> {
    Box::pin(async move {
        let mut r = cg.get_callees(&id, 3).await.map_err(|e| e.to_string())?;
        r.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        Ok(ids(r.iter().map(|(n, _)| n)))
    })
}

fn op_impact_depth2(cg: &TokenSave, id: String) -> OpFuture<'_> {
    Box::pin(async move {
        let g = cg
            .get_impact_radius(&id, 2)
            .await
            .map_err(|e| e.to_string())?;
        let mut nodes: Vec<_> = g.nodes.iter().collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(format!(
            "{};edges={}",
            ids(nodes.into_iter()),
            g.edges.len()
        ))
    })
}

fn op_sync_noop(cg: &TokenSave, _: String) -> OpFuture<'_> {
    Box::pin(async move {
        let r = cg.sync().await.map_err(|e| e.to_string())?;
        let changed = r.files_added + r.files_modified + r.files_removed;
        if changed != 0 {
            return Err(format!("no-op sync changed {changed} files"));
        }
        Ok("unchanged".into())
    })
}

// --- Runner ---------------------------------------------------------------

struct Measured {
    name: &'static str,
    group: &'static str,
    inputs: usize,
    recorder: Recorder,
    profile: Snapshot,
    fingerprints: Vec<String>,
}

async fn run_workload(
    cg: &TokenSave,
    name: &'static str,
    group: &'static str,
    op: Op,
    inputs: &[String],
    args: &Args,
) -> Measured {
    let mut recorder = Recorder::with_capacity(args.iterations);
    let mut expected = Vec::with_capacity(inputs.len());
    for input in inputs {
        match profile::invocation(op(cg, input.clone())).await.0 {
            Ok(fp) => expected.push(fp),
            Err(e) => {
                recorder.fail(format!("validation: {e}"));
                expected.push(String::new());
            }
        }
    }
    if recorder.errors().is_empty() {
        for i in 0..args.warmup + args.iterations {
            let idx = i % inputs.len();
            let input = inputs[idx].clone();
            if i == args.warmup {
                let _ = drain_profile();
            }
            let start = Instant::now();
            let result = profile::invocation(op(cg, input)).await.0;
            let elapsed = start.elapsed();
            if i < args.warmup {
                continue;
            }
            match result {
                Ok(fp) if fp == expected[idx] => recorder.record(elapsed),
                Ok(fp) => recorder.fail(format!("input {idx}: result changed to {fp}")),
                Err(e) => recorder.fail(format!("input {idx}: {e}")),
            }
        }
    }
    Measured {
        name,
        group,
        inputs: inputs.len(),
        recorder,
        profile: drain_profile(),
        fingerprints: expected,
    }
}

/// Full index from an empty `.tokensave/`: `init` + `index_all`. Node and
/// edge counts must be identical across runs.
async fn run_full_index(dir: &Path, runs: usize) -> (Measured, Option<TokenSave>) {
    let mut recorder = Recorder::with_capacity(runs);
    let _ = drain_profile();
    let mut expected: Option<String> = None;
    let mut last = None;
    for run in 0..runs {
        drop(last.take());
        let ts_dir = dir.join(".tokensave");
        if ts_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&ts_dir) {
                recorder.fail(format!("run {run}: remove index: {e}"));
                break;
            }
        }
        let start = Instant::now();
        let result = profile::invocation(async {
            let cg = TokenSave::init(dir).await.map_err(|e| e.to_string())?;
            let r = cg.index_all().await.map_err(|e| e.to_string())?;
            Ok::<_, String>((cg, r))
        })
        .await
        .0;
        let elapsed = start.elapsed();
        match result {
            Ok((cg, r)) => {
                let fp = format!(
                    "files={} nodes={} edges={}",
                    r.file_count, r.node_count, r.edge_count
                );
                match &expected {
                    None => {
                        expected = Some(fp);
                        recorder.record(elapsed);
                    }
                    Some(e) if *e == fp => recorder.record(elapsed),
                    Some(e) => recorder.fail(format!("run {run}: {fp}, expected {e}")),
                }
                last = Some(cg);
            }
            Err(e) => recorder.fail(format!("run {run}: {e}")),
        }
    }
    let m = Measured {
        name: "index.full",
        group: "W1 full index",
        inputs: 1,
        recorder,
        profile: drain_profile(),
        fingerprints: expected.into_iter().collect(),
    };
    (m, last)
}

/// Cold-process-style open of the existing index plus its first query,
/// measured as two separate workloads.
async fn run_open(dir: &Path, term: &str, args: &Args) -> [Measured; 2] {
    let _ = drain_profile();
    let mut open_profile = Snapshot::new();
    let mut query_profile = Snapshot::new();
    let mut expected: Option<String> = None;
    let n = args.iterations;
    let mut open = Recorder::with_capacity(n);
    let mut first = Recorder::with_capacity(n);
    for i in 0..n + args.warmup {
        let start = Instant::now();
        let cg = match profile::invocation(TokenSave::open(dir)).await.0 {
            Ok(cg) => cg,
            Err(e) => {
                open.fail(e.to_string());
                continue;
            }
        };
        let opened = start.elapsed();
        let opened_profile = drain_profile();
        let q = Instant::now();
        let hits = profile::invocation(cg.search(term, 20)).await.0;
        let queried = q.elapsed();
        let queried_profile = drain_profile();
        drop(cg);
        if let Ok(hits) = &hits {
            if !hits.is_empty() && expected.is_none() {
                expected = Some(ids(hits.iter().map(|h| &h.node)));
            }
        }
        if i < args.warmup {
            continue;
        }
        profile::recorder::merge_snapshot(&mut open_profile, &opened_profile);
        profile::recorder::merge_snapshot(&mut query_profile, &queried_profile);
        open.record(opened);
        match hits {
            Ok(h)
                if !h.is_empty()
                    && expected.as_deref() == Some(ids(h.iter().map(|h| &h.node)).as_str()) =>
            {
                first.record(queried)
            }
            Ok(_) => first.fail(format!("first search {term:?} returned nothing")),
            Err(e) => first.fail(e.to_string()),
        }
    }
    [
        Measured {
            name: "open.existing",
            group: "W6 startup",
            inputs: 1,
            recorder: open,
            profile: open_profile,
            fingerprints: Vec::new(),
        },
        Measured {
            name: "open.first_search",
            group: "W6 startup",
            inputs: 1,
            recorder: first,
            profile: query_profile,
            fingerprints: expected.into_iter().collect(),
        },
    ]
}

/// Deterministic inputs: functions and methods sorted by id, evenly spaced.
async fn sample_nodes(cg: &TokenSave, count: usize) -> Result<Vec<tokensave::types::Node>, String> {
    let mut nodes: Vec<_> = cg
        .get_all_nodes()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Method))
        .collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    if nodes.is_empty() {
        return Err("indexed project has no functions or methods".into());
    }
    let step = (nodes.len() / count).max(1);
    Ok(nodes.into_iter().step_by(step).take(count).collect())
}

fn summary_json(m: &Measured) -> Value {
    let s = m.recorder.summary();
    json!({
        "fingerprints": m.fingerprints,
        "profile": m.profile,
        "name": m.name,
        "group": m.group,
        "distinct_inputs": m.inputs,
        "samples": s.samples,
        "errors": s.errors,
        "first_errors": m.recorder.errors().iter().take(5).collect::<Vec<_>>(),
        "min_us": s.min_us,
        "mean_us": s.mean_us,
        "stdev_us": s.stdev_us,
        "p50_us": s.p50_us,
        "p95_us": s.p95_us,
        "p99_us": s.p99_us,
        "max_us": s.max_us,
        "ops_per_sec": s.ops_per_sec,
        "p99_reliable": s.p99_reliable,
    })
}

fn print_table(results: &[Measured]) {
    eprintln!(
        "{:<28} {:>7} {:>4} {:>11} {:>11} {:>11} {:>11}",
        "workload", "samples", "err", "p50 µs", "p95 µs", "p99 µs", "ops/s"
    );
    for m in results {
        let s = m.recorder.summary();
        let flag = if s.p99_reliable { "" } else { "*" };
        eprintln!(
            "{:<28} {:>7} {:>4} {:>11.1} {:>11.1} {:>10.1}{flag:1} {:>11.1}",
            m.name, s.samples, s.errors, s.p50_us, s.p95_us, s.p99_us, s.ops_per_sec
        );
    }
    eprintln!(
        "* fewer than {} samples: p99 not reliable",
        stats::MIN_SAMPLES_FOR_P99
    );
}

async fn run(args: Args) -> Result<bool, String> {
    // Never start persistence/sharing or consult the user configuration.
    profile::set_mode(args.profile);
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = args
        .project
        .clone()
        .unwrap_or_else(|| repo_root.join("src"));
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let dir = tmp.path().join("project");
    copy_tree(&source, &dir).map_err(|e| format!("copy {}: {e}", source.display()))?;

    eprintln!(
        "[db_workloads] indexing {} ({} runs)…",
        source.display(),
        args.index_runs
    );
    let (index, cg) = run_full_index(&dir, args.index_runs).await;
    let cg = cg.ok_or_else(|| format!("indexing failed: {:?}", index.recorder.errors()))?;

    let stats = cg.get_stats().await.map_err(|e| e.to_string())?;
    let env = manifest::collect(&repo_root, &cg).await;
    let nodes = sample_nodes(&cg, args.inputs).await?;
    let id_inputs: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let name_inputs: Vec<String> = nodes.iter().map(|n| n.name.clone()).collect();
    let qn_inputs: Vec<String> = nodes.iter().map(|n| n.qualified_name.clone()).collect();

    let mut results = vec![index];
    let plan: [(&'static str, &'static str, Op, &[String]); 7] = [
        (
            "lookup.node_by_id",
            "W3 symbol lookup",
            op_node_by_id,
            &id_inputs,
        ),
        (
            "lookup.by_name",
            "W3 symbol lookup",
            op_nodes_by_name,
            &name_inputs,
        ),
        (
            "lookup.by_qualified_name",
            "W3 symbol lookup",
            op_nodes_by_qualified_name,
            &qn_inputs,
        ),
        (
            "search.fts_ranked",
            "W4 full-text search",
            op_search,
            &name_inputs,
        ),
        (
            "graph.callers_d3",
            "W5 graph traversal",
            op_callers_depth3,
            &id_inputs,
        ),
        (
            "graph.callees_d3",
            "W5 graph traversal",
            op_callees_depth3,
            &id_inputs,
        ),
        (
            "graph.impact_d2",
            "W5 graph traversal",
            op_impact_depth2,
            &id_inputs,
        ),
    ];
    for (name, group, op, inputs) in plan {
        eprintln!("[db_workloads] {name}");
        results.push(run_workload(&cg, name, group, op, inputs, &args).await);
    }
    // Sync and open are 10-100x slower than lookups; scale their counts down.
    let slow_args = Args {
        iterations: (args.iterations / 10).max(10),
        warmup: (args.warmup / 10).max(1),
        ..args.clone()
    };
    eprintln!("[db_workloads] sync.noop");
    results.push(
        run_workload(
            &cg,
            "sync.noop",
            "W2 incremental sync",
            op_sync_noop,
            &[String::new()],
            &slow_args,
        )
        .await,
    );
    let rss_bytes = tokensave::memstats::current_rss_bytes();
    drop(cg);

    eprintln!("[db_workloads] open.existing");
    results.extend(run_open(&dir, &name_inputs[0], &slow_args).await);

    print_table(&results);
    let failed = results.iter().any(|m| !m.recorder.errors().is_empty());
    let doc = json!({
        "schema": "tokensave-db-workloads/1",
        "timestamp_unix": SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        "environment": env,
        "dataset": {
            "source": source.display().to_string(),
            "files": stats.file_count,
            "nodes": stats.node_count,
            "edges": stats.edge_count,
            "db_size_bytes": stats.db_size_bytes,
        },
        "config": {
            "profiling": args.profile.as_str(),
            "iterations": args.iterations,
            "warmup": args.warmup,
            "slow_iterations": slow_args.iterations,
            "slow_warmup": slow_args.warmup,
            "index_runs": args.index_runs,
            "distinct_inputs": args.inputs,
            "cache": "warm",
            "clients": 1,
        },
        "profiler_resident_bytes": profile::recorder().map_or(0, profile::Recorder::resident_bytes),
        "process_rss_bytes_after_queries": rss_bytes,
        "passed": !failed,
        "workloads": results.iter().map(summary_json).collect::<Vec<_>>(),
    });
    let out = args.out.clone().unwrap_or_else(|| {
        repo_root
            .join("target")
            .join("db-bench")
            .join(format!("db_workloads-{}.json", doc["timestamp_unix"]))
    });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&out, text).map_err(|e| format!("write {}: {e}", out.display()))?;
    eprintln!("[db_workloads] wrote {}", out.display());
    Ok(!failed)
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("db_workloads: {e}");
            std::process::exit(2);
        }
    };
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("db_workloads: runtime: {e}");
            std::process::exit(2);
        }
    };
    match rt.block_on(run(args)) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("db_workloads: FAILED: some operations errored or changed result");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("db_workloads: {e}");
            std::process::exit(2);
        }
    }
}
