//! Bottleneck analysis: ranked, classified findings with their evidence.
//!
//! Findings are ranked by cumulative impact on wall time, not by the single
//! slowest call. To avoid counting the same time twice, phases contribute
//! their exclusive time (time not spent in nested measured operations), and
//! whole-run and per-tool totals are not findings themselves. Every
//! suggestion is labelled as a hypothesis; the evidence is what was
//! measured.

use std::fmt::Write as _;

use serde::Serialize;

use super::recorder::{SeriesSnapshot, Snapshot};
use super::report::MIN_RELIABLE_SAMPLES;
use super::tools::REPEAT_MIN_PER_CALL;

/// What kind of cost a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    DbExecutionLatency,
    ExcessiveQueryCount,
    ConnectionTransactionOverhead,
    GraphTraversal,
    FreshnessSync,
    ParsingExtraction,
    ResultProcessing,
}

impl Class {
    pub fn label(self) -> &'static str {
        match self {
            Self::DbExecutionLatency => "DB execution latency",
            Self::ExcessiveQueryCount => "excessive query count",
            Self::ConnectionTransactionOverhead => "connection/transaction overhead",
            Self::GraphTraversal => "graph traversal",
            Self::FreshnessSync => "freshness/sync",
            Self::ParsingExtraction => "parsing/extraction",
            Self::ResultProcessing => "result processing",
        }
    }
}

/// One ranked finding.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub class: Class,
    /// The operation (and tool, for query-count findings) concerned.
    pub subject: String,
    /// Cumulative wall time attributed to this finding, in microseconds.
    pub impact_us: u64,
    /// Whether `impact_us` is estimated rather than measured directly.
    pub impact_estimated: bool,
    /// What was measured.
    pub evidence: String,
    /// A suggested cause; not established by the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hypothesis: Option<String>,
}

fn ms(us: u64) -> String {
    if us >= 10_000_000 {
        format!("{:.1} s", us as f64 / 1e6)
    } else {
        format!("{:.1} ms", us as f64 / 1e3)
    }
}

fn p95(s: &SeriesSnapshot) -> String {
    match s.quantile(0.95) {
        Some(q) if s.count >= MIN_RELIABLE_SAMPLES => {
            format!("p95 {} (n={})", ms(q.upper_us), s.count)
        }
        Some(q) => format!("p95 {} (n={}, unreliable)", ms(q.upper_us), s.count),
        None => format!("n={}", s.count),
    }
}

fn classify_phase(name: &str) -> Option<Class> {
    match name {
        "mcp.graph_traversal" => Some(Class::GraphTraversal),
        "mcp.freshness" => Some(Class::FreshnessSync),
        "mcp.handler" | "mcp.total" | "mcp.ranking" | "mcp.serialization" | "mcp.dispatch" => {
            Some(Class::ResultProcessing)
        }
        "mcp.project_resolution" => Some(Class::ConnectionTransactionOverhead),
        "index.extract" | "sync.extract" => Some(Class::ParsingExtraction),
        "index.total" | "sync.total" => None,
        n if n.starts_with("index.") || n.starts_with("sync.") => Some(Class::FreshnessSync),
        _ => None,
    }
}

fn phase_note(name: &str) -> &'static str {
    match name {
        "mcp.handler" => "handler time outside DB queries and traversal",
        "mcp.total" => "validation, dispatch and response handling outside the handler",
        "mcp.project_resolution" => "graph selection and branch lease",
        "mcp.freshness" => "on-demand staleness check and resync, excluding measured DB time",
        _ => "exclusive time",
    }
}

/// Findings for one engine's snapshot, most impactful first.
pub fn findings(snapshot: &Snapshot) -> Vec<Finding> {
    let mut out = Vec::new();
    for (name, s) in snapshot {
        if s.count == 0 {
            continue;
        }
        if let Some(op) = name.strip_prefix("db.") {
            let overhead = matches!(op, "write_lock_wait" | "transaction" | "open");
            let impact_us = if overhead {
                s.exclusive_sum_us
            } else {
                s.sum_us
            };
            out.push(Finding {
                class: if overhead {
                    Class::ConnectionTransactionOverhead
                } else {
                    Class::DbExecutionLatency
                },
                subject: name.clone(),
                impact_us,
                impact_estimated: false,
                evidence: format!(
                    "{} executions, {} cumulative, {}, max {}, {} rows returned, {} errors",
                    s.count,
                    ms(impact_us),
                    p95(s),
                    ms(s.max_us),
                    s.rows_returned,
                    s.errors
                ),
                hypothesis: (s.slow > 0 && !overhead).then(|| {
                    format!(
                        "{} executions reached the slow threshold; the query plan may scan or \
                         sort more than it needs to",
                        s.slow
                    )
                }),
            });
        } else if let Some(tool) = name.strip_prefix("tool.") {
            let median_ops = s
                .db_ops_per_call
                .quantile(0.5, u64::MAX)
                .map_or(0, |q| q.upper_us);
            for (op, repeat) in &s.repeated {
                let mean_us = snapshot
                    .get(op)
                    .and_then(SeriesSnapshot::mean_us)
                    .unwrap_or(0.0);
                out.push(Finding {
                    class: Class::ExcessiveQueryCount,
                    subject: format!("{op} in {tool}"),
                    impact_us: (repeat.executions as f64 * mean_us) as u64,
                    impact_estimated: true,
                    evidence: format!(
                        "{} executions in {} of {} calls (at least {} per affected call, {:.0} on \
                         average); median {} DB ops per call (n={})",
                        repeat.executions,
                        repeat.calls,
                        s.count,
                        REPEAT_MIN_PER_CALL,
                        repeat.executions as f64 / repeat.calls.max(1) as f64,
                        median_ops,
                        s.count
                    ),
                    hypothesis: Some(
                        "per-item lookups in a loop (N+1); consider batching them into one query"
                            .to_string(),
                    ),
                });
            }
        } else if let Some(class) = classify_phase(name) {
            out.push(Finding {
                class,
                subject: name.clone(),
                impact_us: s.exclusive_sum_us,
                impact_estimated: false,
                evidence: format!(
                    "{} ({}) over {} calls, {}",
                    ms(s.exclusive_sum_us),
                    phase_note(name),
                    s.count,
                    p95(s)
                ),
                hypothesis: None,
            });
        }
    }
    out.retain(|f| f.impact_us > 0);
    out.sort_by_key(|finding| std::cmp::Reverse(finding.impact_us));
    out
}

/// Renders the top `top` findings of one engine.
pub fn render_text(engine: &str, snapshot: &Snapshot, window: &str, top: usize) -> String {
    let mut out = String::new();
    let calls: u64 = snapshot
        .iter()
        .filter(|(name, _)| name.starts_with("tool."))
        .map(|(_, s)| s.count)
        .sum();
    let _ = writeln!(
        out,
        "Recorded MCP calls: {calls} (window: {window}, engine: {engine})\n"
    );
    let all = findings(snapshot);
    if all.is_empty() {
        let _ = writeln!(
            out,
            "No findings: nothing with measurable impact was recorded."
        );
    }
    for (i, f) in all.iter().take(top).enumerate() {
        let _ = writeln!(out, "{}. [{}] {}", i + 1, f.class.label(), f.subject);
        let _ = writeln!(
            out,
            "   Impact: {}{}",
            ms(f.impact_us),
            if f.impact_estimated {
                " (estimated: executions x mean latency)"
            } else {
                ""
            }
        );
        let _ = writeln!(out, "   Measured: {}", f.evidence);
        if let Some(h) = &f.hypothesis {
            let _ = writeln!(out, "   Hypothesis: {h}");
        }
    }
    if let Some((db, total)) = super::report::mcp_db_share(snapshot) {
        let _ = writeln!(
            out,
            "\nDatabase share of measured MCP time: {:.0}% (outside DB: {:.0}%)",
            100.0 * db / total,
            100.0 * (total - db) / total
        );
    }
    let _ = writeln!(
        out,
        "Memory and lock contention outside the write lock are not measured here; see `tokensave memory`."
    );
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::recorder::Repeat;

    fn series(count: u64, mean_us: u64) -> SeriesSnapshot {
        let mut s = SeriesSnapshot {
            count,
            sum_us: count * mean_us,
            exclusive_sum_us: count * mean_us,
            max_us: mean_us,
            ..SeriesSnapshot::default()
        };
        for _ in 0..count {
            s.histogram.record(mean_us);
        }
        s
    }

    fn snapshot() -> Snapshot {
        let mut snap = Snapshot::new();
        snap.insert("db.get_outgoing_edges".into(), series(42_000, 400));
        snap.insert("db.search_nodes_fts".into(), series(500, 2_000));
        let mut lock = series(10, 100);
        lock.exclusive_sum_us = 1_000;
        snap.insert("db.write_lock_wait".into(), lock);
        let mut handler = series(8_000, 5_000);
        handler.exclusive_sum_us = 300_000;
        snap.insert("mcp.handler".into(), handler);
        snap.insert("index.total".into(), series(1, 90_000_000));
        let mut tool = series(8_000, 4_000);
        tool.repeated.insert(
            "db.get_outgoing_edges".into(),
            Repeat {
                calls: 1_500,
                executions: 40_000,
            },
        );
        tool.db_ops_per_call.record(5);
        snap.insert("tool.tokensave_context".into(), tool);
        snap
    }

    #[test]
    fn ranks_by_cumulative_impact_and_classifies() {
        let f = findings(&snapshot());
        assert_eq!(f[0].class, Class::DbExecutionLatency);
        assert_eq!(f[0].subject, "db.get_outgoing_edges");
        let n_plus_one = f
            .iter()
            .find(|f| f.class == Class::ExcessiveQueryCount)
            .unwrap();
        assert_eq!(
            n_plus_one.subject,
            "db.get_outgoing_edges in tokensave_context"
        );
        assert_eq!(n_plus_one.impact_us, 16_000_000);
        assert!(n_plus_one.impact_estimated);
        assert!(n_plus_one.hypothesis.is_some());
        assert!(f.windows(2).all(|w| w[0].impact_us >= w[1].impact_us));
        let handler = f.iter().find(|f| f.subject == "mcp.handler").unwrap();
        assert_eq!(handler.impact_us, 300_000, "phases count exclusive time");
        assert_eq!(
            f.iter()
                .find(|f| f.subject == "db.write_lock_wait")
                .unwrap()
                .class,
            Class::ConnectionTransactionOverhead
        );
        assert!(
            !f.iter()
                .any(|f| f.subject == "index.total" || f.subject.starts_with("tool.")),
            "totals are not findings"
        );
    }

    #[test]
    fn text_labels_hypotheses_and_estimates() {
        let text = render_text("libsql", &snapshot(), "7d", 10);
        assert!(text.starts_with("Recorded MCP calls: 8000 (window: 7d, engine: libsql)"));
        assert!(text.contains("[excessive query count] db.get_outgoing_edges in tokensave_context"));
        assert!(text.contains("Hypothesis: per-item lookups"));
        assert!(text.contains("(estimated: executions x mean latency)"));
    }

    #[test]
    fn empty_snapshot_has_no_findings() {
        assert!(findings(&Snapshot::new()).is_empty());
        assert!(render_text("libsql", &Snapshot::new(), "24h", 10).contains("No findings"));
    }
}
