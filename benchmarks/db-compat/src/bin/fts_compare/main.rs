//! Functional and performance comparison of tokensave's full-text search on
//! libsql + SQLite FTS5 (production) and turso + native Tantivy FTS.
//!
//! ```text
//! fts-compare export <tokensave.db> <out.jsonl>
//! fts-compare functional [--adapters a,b] [--json out.json]
//! fts-compare relevance <corpus.jsonl>... [--adapters a,b] [--json out.json]
//! fts-compare perf <corpus.jsonl>... [--adapters a,b] [--reps N] [--json out.json]
//! fts-compare env [--json out.json]
//! ```
//!
//! This is a separate binary from the SQL compatibility probes
//! (`tokensave-db-compat`), whose results it does not change.

mod adapter;
mod corpus;
mod db;
mod functional;
mod perf;
mod query;
mod relevance;
mod stats;
mod sysinfo;

use std::path::PathBuf;

use adapter::AdapterKind;
use serde_json::json;

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

/// Positional arguments after the subcommand, skipping `--flag value` pairs.
fn positionals(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 2;
    while i < args.len() {
        if args[i].starts_with("--") {
            i += 2;
        } else {
            out.push(args[i].clone());
            i += 1;
        }
    }
    out
}

fn adapters(args: &[String], default: &[AdapterKind]) -> Vec<AdapterKind> {
    match arg_value(args, "--adapters") {
        None => default.to_vec(),
        Some(list) => list
            .split(',')
            .map(|s| {
                AdapterKind::parse(s.trim()).unwrap_or_else(|| {
                    eprintln!("unknown adapter {s}");
                    std::process::exit(2);
                })
            })
            .collect(),
    }
}

fn write_json(args: &[String], doc: &serde_json::Value) {
    if let Some(path) = arg_value(args, "--json") {
        let text = serde_json::to_string_pretty(doc).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("write {path}: {e}");
            std::process::exit(2);
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("");
    match cmd {
        "export" => {
            let pos = positionals(&args);
            let (Some(src), Some(out)) = (pos.first(), pos.get(1)) else {
                eprintln!("usage: fts-compare export <tokensave.db> <out.jsonl>");
                std::process::exit(2);
            };
            match corpus::export(&PathBuf::from(src), &PathBuf::from(out)).await {
                Ok((n, b)) => println!("{out}: {n} nodes, {b} bodies"),
                Err(e) => {
                    eprintln!("export: {e}");
                    std::process::exit(1);
                }
            }
        }
        "functional" => {
            let kinds = adapters(&args, &AdapterKind::ALL);
            let (rows, control_ok) = functional::run(&kinds).await;
            write_json(
                &args,
                &json!({ "environment": sysinfo::collect(), "results": rows }),
            );
            if !control_ok {
                eprintln!("the libsql control failed a scenario; fix the scenario first");
                std::process::exit(1);
            }
        }
        "relevance" => {
            let kinds = adapters(
                &args,
                &[
                    AdapterKind::LibsqlFts5,
                    AdapterKind::TursoTantivy,
                    AdapterKind::TursoTantivyNgram,
                ],
            );
            let corpora: Vec<PathBuf> = positionals(&args).into_iter().map(PathBuf::from).collect();
            let doc = relevance::run(&corpora, &kinds).await;
            write_json(&args, &doc);
        }
        "perf" => {
            let kinds = adapters(&args, &[AdapterKind::LibsqlFts5, AdapterKind::TursoTantivy]);
            let corpora: Vec<PathBuf> = positionals(&args).into_iter().map(PathBuf::from).collect();
            let reps = arg_value(&args, "--reps")
                .and_then(|r| r.parse().ok())
                .unwrap_or(5);
            let doc = perf::run(&corpora, &kinds, reps).await;
            write_json(&args, &doc);
        }
        // Internal: one perf phase in a fresh process (see perf.rs).
        "perf-child" => perf::child(&args).await,
        "env" => {
            let env = sysinfo::collect();
            println!("{}", serde_json::to_string_pretty(&env).unwrap_or_default());
            write_json(&args, &env);
        }
        _ => {
            eprintln!(
                "usage: fts-compare <export|functional|relevance|perf|env> …  (see src/bin/fts_compare/main.rs)"
            );
            std::process::exit(2);
        }
    }
}
