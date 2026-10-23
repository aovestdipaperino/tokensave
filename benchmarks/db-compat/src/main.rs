//! Runs the SQL compatibility probes against libsql (control) and turso.
//!
//! ```text
//! cargo run --release -- [--json out.json] [--existing-db path/to/tokensave.db]
//! ```
//!
//! Each probe runs in its own temporary directory on an on-disk database.
//! Outcomes: `pass`, `mismatch` (ran, wrong result), `error` (setup or query
//! failed), `panic`, `timeout`. The process exits non-zero when the libsql
//! control fails a probe, because that means the probe itself is wrong.
//!
//! `--existing-db` copies a real (checkpointed) tokensave database and runs
//! production read queries on it; the expected rows are whatever libsql
//! returns on its own copy.

mod engine;
mod probes;
mod profile;

use std::path::{Path, PathBuf};
use std::time::Duration;

use engine::{Engine, EngineKind};
use probes::{Expect, Probe, Who};
use serde_json::json;

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum Outcome {
    Pass,
    Mismatch {
        got: Vec<String>,
    },
    Error {
        stage: &'static str,
        message: String,
    },
    Panic(String),
    Timeout,
}

impl Outcome {
    fn label(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Mismatch { .. } => "mismatch",
            Self::Error { .. } => "error",
            Self::Panic(_) => "panic",
            Self::Timeout => "timeout",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Pass | Self::Timeout => String::new(),
            Self::Mismatch { got } => format!("got {got:?}"),
            Self::Error { stage, message } => format!("{stage}: {message}"),
            Self::Panic(m) => m.clone(),
        }
    }
}

fn pick(who: Who, tested: EngineKind) -> EngineKind {
    match who {
        Who::Tested => tested,
        Who::Control => EngineKind::Libsql,
    }
}

async fn run_probe(probe: &Probe, kind: EngineKind, dir: &Path) -> Outcome {
    let err = |stage, message| Outcome::Error { stage, message };
    let main_db = dir.join("main.db");
    let setup_kind = pick(probe.setup_by, kind);
    let query_kind = pick(probe.query_by, kind);

    let setup = match Engine::open(setup_kind, &main_db).await {
        Ok(e) => e,
        Err(m) => return err("open", m),
    };
    if !probe.setup.trim().is_empty() {
        let sql = probe.setup.replace("{dir}", &dir.to_string_lossy());
        if let Err(m) = setup.execute_batch(&sql).await {
            return err("setup", m);
        }
    }
    if let Some(sql) = probe.pre_query {
        if let Err(m) = setup.query(sql).await {
            return err("pre_query", m);
        }
    }

    // `_held` keeps the setup handle alive for two-handle probes.
    let (engine, _held) = if probe.reopen.is_some() || setup_kind != query_kind {
        let held = probe.keep_setup_open.then_some(setup);
        let file = dir.join(probe.reopen.unwrap_or("main.db"));
        match Engine::open(query_kind, &file).await {
            Ok(e) => (e, held),
            Err(m) => return err("reopen", m),
        }
    } else {
        (setup, None)
    };

    match &probe.expect {
        Expect::Rows(expected) => match engine.query(&probe.query).await {
            Ok(got) if &got == expected => Outcome::Pass,
            Ok(got) => Outcome::Mismatch { got },
            Err(m) => err("query", m),
        },
        Expect::Error { verify, rows } => {
            if engine.execute_batch(&probe.query).await.is_ok() {
                return Outcome::Mismatch {
                    got: vec!["statement succeeded".into()],
                };
            }
            match engine.query(verify).await {
                Ok(got) if &got == rows => Outcome::Pass,
                Ok(got) => Outcome::Mismatch { got },
                Err(m) => err("verify", m),
            }
        }
    }
}

/// Runs one probe on a worker task so a panic or hang in an engine is
/// reported as an outcome instead of aborting the whole run.
async fn run_isolated(
    probe: std::sync::Arc<Probe>,
    kind: EngineKind,
    seed: Option<PathBuf>,
    profiling: bool,
) -> (Outcome, serde_json::Value) {
    let Ok(dir) = tempfile::tempdir() else {
        return (
            Outcome::Error {
                stage: "tempdir",
                message: "could not create temporary directory".into(),
            },
            serde_json::Value::Null,
        );
    };
    if let Some(src) = seed {
        if let Err(e) = std::fs::copy(&src, dir.path().join("main.db")) {
            return (
                Outcome::Error {
                    stage: "copy",
                    message: e.to_string(),
                },
                serde_json::Value::Null,
            );
        }
    }
    let path = dir.path().to_path_buf();
    let mut task =
        tokio::spawn(
            async move { profile::scoped(profiling, run_probe(&probe, kind, &path)).await },
        );
    let outcome = match tokio::time::timeout(PROBE_TIMEOUT, &mut task).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) if e.is_panic() => {
            let payload = e.into_panic();
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "non-string panic".into());
            (Outcome::Panic(msg), serde_json::Value::Null)
        }
        Ok(Err(e)) => (Outcome::Panic(e.to_string()), serde_json::Value::Null),
        Err(_) => {
            task.abort();
            let _ = task.await;
            (Outcome::Timeout, serde_json::Value::Null)
        }
    };
    drop(dir);
    outcome
}

/// Production read queries replayed on a copy of a real tokensave database.
fn existing_db_queries() -> Vec<(&'static str, &'static str)> {
    vec![
        ("existing.integrity_check", "PRAGMA integrity_check"),
        ("existing.user_version", "PRAGMA user_version"),
        ("existing.count_nodes", "SELECT count(*) FROM nodes"),
        ("existing.count_edges", "SELECT count(*) FROM edges"),
        (
            "existing.fts_search_bm25",
            "SELECT n.id FROM nodes_fts f JOIN nodes n ON n.rowid = f.rowid \
             WHERE nodes_fts MATCH '\"parse\"* OR \"file\"*' \
             ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0, 2.0, 3.0), n.id LIMIT 20",
        ),
        (
            "existing.edges_by_target_kind",
            "SELECT source, kind FROM edges WHERE target = (SELECT target FROM edges \
             GROUP BY target ORDER BY count(*) DESC, target LIMIT 1) ORDER BY source, kind, line",
        ),
        (
            "existing.lower_name_lookup",
            "SELECT id FROM nodes WHERE lower(name) = (SELECT lower(name) FROM nodes \
             ORDER BY id LIMIT 1) ORDER BY id",
        ),
    ]
}

async fn existing_db_probes(path: &Path) -> Result<Vec<Probe>, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let copy = dir.path().join("main.db");
    std::fs::copy(path, &copy).map_err(|e| format!("copy {}: {e}", path.display()))?;
    let control = Engine::open(EngineKind::Libsql, &copy).await?;
    let mut out = Vec::new();
    for (id, sql) in existing_db_queries() {
        let rows = control.query(sql).await.map_err(|e| format!("{id}: {e}"))?;
        out.push(Probe {
            id,
            gate: "Existing tokensave DB",
            setup: String::new(),
            setup_by: Who::Tested,
            reopen: None,
            keep_setup_open: false,
            pre_query: None,
            query_by: Who::Tested,
            query: sql.to_string(),
            expect: Expect::Rows(rows),
        });
    }
    Ok(out)
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let profiling = args.iter().any(|arg| arg == "--profile");
    let json_out = arg_value(&args, "--json");
    let existing = arg_value(&args, "--existing-db").map(PathBuf::from);

    let mut catalog = probes::catalog();
    if let Some(path) = &existing {
        match existing_db_probes(path).await {
            Ok(mut extra) => catalog.append(&mut extra),
            Err(e) => {
                eprintln!("--existing-db: {e}");
                std::process::exit(2);
            }
        }
    }
    let catalog: Vec<_> = catalog.into_iter().map(std::sync::Arc::new).collect();

    let mut results = Vec::new();
    let mut control_failures = 0;
    println!(
        "| Probe | Gate | {} |",
        EngineKind::ALL.map(EngineKind::label).join(" | ")
    );
    println!("|---|---|{}", "---|".repeat(EngineKind::ALL.len()));
    for probe in &catalog {
        let seed = (probe.gate == "Existing tokensave DB")
            .then(|| existing.clone())
            .flatten();
        let mut cells = Vec::new();
        for kind in EngineKind::ALL {
            let (outcome, metrics) =
                run_isolated(probe.clone(), kind, seed.clone(), profiling).await;
            if kind == EngineKind::Libsql && !matches!(outcome, Outcome::Pass) {
                control_failures += 1;
            }
            cells.push(outcome.label().to_string());
            results.push(json!({
                "profile": metrics,
                "probe": probe.id,
                "gate": probe.gate,
                "engine": kind.label(),
                "outcome": outcome.label(),
                "detail": outcome.detail(),
            }));
        }
        println!(
            "| `{}` | {} | {} |",
            probe.id,
            probe.gate,
            cells.join(" | ")
        );
    }

    if let Some(path) = json_out {
        let doc = json!({
            "engines": EngineKind::ALL.map(EngineKind::label),
            "existing_db": existing.as_ref().map(|p| p.display().to_string()),
            "results": results,
        });
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("write {path}: {e}");
            std::process::exit(2);
        }
    }

    if control_failures > 0 {
        eprintln!(
            "{control_failures} probe(s) failed on the libsql control; fix the probe(s) first"
        );
        std::process::exit(1);
    }
}
