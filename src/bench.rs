//! Reproducible benchmark harness for `tokensave bench`.
//!
//! Loads a query file (TOML), runs each query through `cg.build_context(...)`,
//! and reports retrieval savings: how many tokens an agent would have spent
//! reading the full content of every file referenced, vs the tokens in the
//! actual context response.

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context::format_context_as_markdown;
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::{BuildContextOptions, OutputFormat as ContextFormat};

#[derive(Debug, Deserialize)]
struct QueryFile {
    query: Vec<Query>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Query {
    pub task: String,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub task: String,
    pub baseline_tokens: u64,
    pub context_tokens: u64,
    pub savings_pct: f64,
    pub files_referenced: usize,
    pub nodes_returned: usize,
}

#[derive(Debug, Serialize)]
pub struct AggregateReport {
    pub queries: usize,
    pub total_baseline_tokens: u64,
    pub total_context_tokens: u64,
    pub mean_savings_pct: f64,
}

#[derive(Debug, Serialize)]
pub struct BenchReport {
    pub results: Vec<QueryResult>,
    pub aggregate: AggregateReport,
}

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Markdown,
    Json,
}

#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub format: OutputFormat,
    pub max_nodes: usize,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Markdown,
            max_nodes: 20,
        }
    }
}

/// The embedded default query set. Compiled into the binary so `tokensave bench`
/// works without any external file dependency.
pub const DEFAULT_QUERIES_TOML: &str = include_str!("../benchmarks/queries/default.toml");

/// Run the bench from a TOML query file on disk.
pub async fn run_bench(
    cg: &TokenSave,
    queries_path: &Path,
    opts: BenchOptions,
) -> Result<BenchReport> {
    let raw = std::fs::read_to_string(queries_path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read query file {}: {e}", queries_path.display()),
    })?;
    run_bench_with_toml(cg, &raw, opts).await
}

/// Run the bench from an in-memory TOML string. Used by the CLI's default path
/// (avoids a filesystem dependency on the embedded query set).
pub async fn run_bench_with_toml(
    cg: &TokenSave,
    toml_str: &str,
    opts: BenchOptions,
) -> Result<BenchReport> {
    let parsed: QueryFile = toml::from_str(toml_str).map_err(|e| TokenSaveError::Config {
        message: format!("failed to parse query file as TOML: {e}"),
    })?;

    let mut results = Vec::with_capacity(parsed.query.len());
    for q in &parsed.query {
        let options = BuildContextOptions {
            max_nodes: opts.max_nodes,
            format: ContextFormat::Markdown,
            ..Default::default()
        };
        let ctx = cg.build_context(&q.task, &options).await?;
        let markdown = format_context_as_markdown(&ctx);
        let context_tokens = (markdown.len() / 4) as u64;

        // `related_files` is the deduplicated set of files referenced by the context.
        let referenced_files = &ctx.related_files;
        let mut baseline = 0u64;
        for path in referenced_files {
            let full = cg.project_root().join(path);
            if let Ok(bytes) = std::fs::read(&full) {
                baseline += (bytes.len() / 4) as u64;
            }
        }
        // Safety floor: at least the context itself; prevents 100% savings when no files match.
        if baseline < context_tokens {
            baseline = context_tokens;
        }

        let savings_pct = if baseline == 0 {
            0.0
        } else {
            (baseline.saturating_sub(context_tokens) as f64 / baseline as f64) * 100.0
        };

        let nodes_returned = ctx.subgraph.nodes.len();

        results.push(QueryResult {
            task: q.task.clone(),
            baseline_tokens: baseline,
            context_tokens,
            savings_pct,
            files_referenced: referenced_files.len(),
            nodes_returned,
        });
    }

    let total_baseline: u64 = results.iter().map(|r| r.baseline_tokens).sum();
    let total_context: u64 = results.iter().map(|r| r.context_tokens).sum();
    let mean_savings_pct = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.savings_pct).sum::<f64>() / results.len() as f64
    };

    let report = BenchReport {
        aggregate: AggregateReport {
            queries: results.len(),
            total_baseline_tokens: total_baseline,
            total_context_tokens: total_context,
            mean_savings_pct,
        },
        results,
    };
    Ok(report)
}

/// Convenience: format a report for terminal output.
pub fn format_report_markdown(report: &BenchReport) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# tokensave bench — {} queries\n",
        report.aggregate.queries
    );
    s.push_str("| # | Query | Baseline | Context | Savings | Files | Nodes |\n");
    s.push_str("|---|---|---:|---:|---:|---:|---:|\n");
    for (i, r) in report.results.iter().enumerate() {
        let task = if r.task.len() > 60 {
            format!("{}…", &r.task[..59])
        } else {
            r.task.clone()
        };
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {:.0}% | {} | {} |",
            i + 1,
            task,
            r.baseline_tokens,
            r.context_tokens,
            r.savings_pct,
            r.files_referenced,
            r.nodes_returned,
        );
    }
    let _ = writeln!(
        s,
        "\n**Aggregate:** {:.0}% mean retrieval savings ({} → {} tokens across {} queries).",
        report.aggregate.mean_savings_pct,
        report.aggregate.total_baseline_tokens,
        report.aggregate.total_context_tokens,
        report.aggregate.queries,
    );
    s
}

pub fn format_report_json(report: &BenchReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}
