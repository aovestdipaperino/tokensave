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

/// Format the report for the terminal: a fixed-width colored table.
/// Numbers use compact units (`k`, `M`); savings percentages are colored by
/// tier (green ≥80%, yellow ≥50%, red <50%). Matches the ANSI style used
/// elsewhere in `tokensave status`.
pub fn format_report_console(report: &BenchReport) -> String {
    use crate::display::{format_number, format_token_count};

    // Column widths (display columns, not bytes). Tuned so the typical
    // terminal (≥100 cols) shows the table without wrapping.
    const W_NUM: usize = 4;
    const W_QUERY: usize = 56;
    const W_BASELINE: usize = 10;
    const W_CONTEXT: usize = 10;
    const W_SAVINGS: usize = 9;
    const W_FILES: usize = 7;
    const W_NODES: usize = 7;

    // ANSI escape codes — kept here, like the equivalents in src/display.rs,
    // so we don't pull in a colour crate just for this surface.
    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";
    const DIM: &str = "\x1b[2m";
    const CYAN: &str = "\x1b[36m";
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const RED: &str = "\x1b[31m";

    let mut s = String::new();

    let _ = writeln!(
        s,
        "{BOLD}{CYAN}tokensave bench{RESET}{BOLD} — {} {}{RESET}\n",
        report.aggregate.queries,
        if report.aggregate.queries == 1 {
            "query"
        } else {
            "queries"
        }
    );

    let _ = writeln!(
        s,
        " {BOLD}{:>w_num$}  {:<w_query$}  {:>w_base$}  {:>w_ctx$}  {:>w_sav$}  {:>w_files$}  {:>w_nodes$}{RESET}",
        "#",
        "Query",
        "Baseline",
        "Context",
        "Savings",
        "Files",
        "Nodes",
        w_num = W_NUM,
        w_query = W_QUERY,
        w_base = W_BASELINE,
        w_ctx = W_CONTEXT,
        w_sav = W_SAVINGS,
        w_files = W_FILES,
        w_nodes = W_NODES,
    );

    let separator = format!(
        " {DIM}{}  {}  {}  {}  {}  {}  {}{RESET}",
        "─".repeat(W_NUM),
        "─".repeat(W_QUERY),
        "─".repeat(W_BASELINE),
        "─".repeat(W_CONTEXT),
        "─".repeat(W_SAVINGS),
        "─".repeat(W_FILES),
        "─".repeat(W_NODES),
    );
    let _ = writeln!(s, "{separator}");

    for (i, r) in report.results.iter().enumerate() {
        let task = truncate_display(&r.task, W_QUERY);
        let savings_color = if r.savings_pct >= 80.0 {
            GREEN
        } else if r.savings_pct >= 50.0 {
            YELLOW
        } else {
            RED
        };
        let savings_str = format!("{:.0}%", r.savings_pct);
        let _ = writeln!(
            s,
            " {:>w_num$}  {:<w_query$}  {:>w_base$}  {:>w_ctx$}  {savings_color}{:>w_sav$}{RESET}  {:>w_files$}  {:>w_nodes$}",
            i + 1,
            task,
            format_token_count(r.baseline_tokens),
            format_token_count(r.context_tokens),
            savings_str,
            format_number(r.files_referenced as u64),
            format_number(r.nodes_returned as u64),
            w_num = W_NUM,
            w_query = W_QUERY,
            w_base = W_BASELINE,
            w_ctx = W_CONTEXT,
            w_sav = W_SAVINGS,
            w_files = W_FILES,
            w_nodes = W_NODES,
        );
    }

    let _ = writeln!(s, "{separator}");

    let agg_color = if report.aggregate.mean_savings_pct >= 80.0 {
        GREEN
    } else if report.aggregate.mean_savings_pct >= 50.0 {
        YELLOW
    } else {
        RED
    };
    let _ = writeln!(
        s,
        " {BOLD}Aggregate:{RESET} {agg_color}{BOLD}{:.0}%{RESET} mean savings — {} → {} tokens across {} {}.",
        report.aggregate.mean_savings_pct,
        format_token_count(report.aggregate.total_baseline_tokens),
        format_token_count(report.aggregate.total_context_tokens),
        report.aggregate.queries,
        if report.aggregate.queries == 1 {
            "query"
        } else {
            "queries"
        },
    );
    s
}

/// Truncate `s` to fit within `max` display columns, appending `…` when
/// truncation happens. Operates on character boundaries, not bytes, so
/// multi-byte UTF-8 input does not produce invalid slices.
fn truncate_display(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{truncated}…")
}

pub fn format_report_json(report: &BenchReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}
