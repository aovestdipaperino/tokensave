//! Human and JSON reports over merged profile data.
//!
//! Every percentile is printed with its sample count, and P95/P99 are marked
//! unreliable below [`MIN_RELIABLE_SAMPLES`], consistent with `p99_reliable`
//! in the DB workload harness. Percentiles are upper bucket bounds (see
//! [`super::histogram`]), so they over-report by at most about 1.6%.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use super::recorder::{SeriesSnapshot, Snapshot};

/// Below this many samples P95/P99 are flagged as unreliable.
pub const MIN_RELIABLE_SAMPLES: u64 = 100;

/// Which series a report covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    All,
    /// `mcp.*` phases and `tool.*` invocations.
    Mcp,
    /// `db.*` operations.
    Database,
    /// `index.*`, `sync.*` and `run.*`.
    Indexing,
}

impl Section {
    fn includes(self, name: &str) -> bool {
        let prefix = name.split('.').next().unwrap_or("");
        match self {
            Self::All => true,
            Self::Mcp => matches!(prefix, "mcp" | "tool"),
            Self::Database => prefix == "db",
            Self::Indexing => matches!(prefix, "index" | "sync" | "run"),
        }
    }
}

/// Parses a window such as `30m`, `24h` or `7d`.
pub fn parse_window(text: &str) -> Option<Duration> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit())?;
    let (number, unit) = text.split_at(split);
    let n: u64 = number.parse().ok()?;
    let secs = match unit {
        "m" | "min" => n.checked_mul(60)?,
        "h" => n.checked_mul(3600)?,
        "d" => n.checked_mul(86_400)?,
        _ => return None,
    };
    Some(Duration::from_secs(secs))
}

/// One row of a report.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub name: String,
    pub count: u64,
    pub errors: u64,
    pub total_ms: f64,
    pub exclusive_ms: f64,
    pub mean_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub max_ms: f64,
    /// Whether `count` is large enough for P95/P99 to be meaningful.
    pub tail_reliable: bool,
    pub slow: u64,
    pub rows_returned: u64,
    pub rows_affected: u64,
    /// Invocation series only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_share: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_db_ops_per_call: Option<u64>,
}

fn ms(us: u64) -> f64 {
    us as f64 / 1000.0
}

impl Row {
    fn from_series(name: &str, s: &SeriesSnapshot) -> Self {
        let q = |p| s.quantile(p).map(|q| ms(q.upper_us));
        let invocation = name.starts_with("tool.");
        Self {
            name: name.to_string(),
            count: s.count,
            errors: s.errors,
            total_ms: ms(s.sum_us),
            exclusive_ms: ms(s.exclusive_sum_us),
            mean_ms: s.mean_us().map(|us| us / 1000.0),
            p50_ms: q(0.5),
            p95_ms: q(0.95),
            p99_ms: q(0.99),
            max_ms: ms(s.max_us),
            tail_reliable: s.count >= MIN_RELIABLE_SAMPLES,
            slow: s.slow,
            rows_returned: s.rows_returned,
            rows_affected: s.rows_affected,
            db_share: (invocation && s.sum_us > 0).then(|| s.db_us as f64 / s.sum_us as f64),
            median_db_ops_per_call: if invocation {
                s.db_ops_per_call
                    .quantile(0.5, u64::MAX)
                    .map(|q| q.upper_us)
            } else {
                None
            },
        }
    }
}

/// Rows of `section`, ranked by cumulative time.
pub fn rows(snapshot: &Snapshot, section: Section) -> Vec<Row> {
    let mut rows: Vec<Row> = snapshot
        .iter()
        .filter(|(name, _)| section.includes(name))
        .map(|(name, s)| Row::from_series(name, s))
        .collect();
    rows.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
    rows
}

/// Database share of all recorded tool-call time: `(db_ms, total_ms)`.
pub fn mcp_db_share(snapshot: &Snapshot) -> Option<(f64, f64)> {
    let (db, total) = snapshot
        .iter()
        .filter(|(name, _)| name.starts_with("tool."))
        .fold((0u64, 0u64), |(db, total), (_, s)| {
            (db.saturating_add(s.db_us), total.saturating_add(s.sum_us))
        });
    (total > 0).then(|| (ms(db), ms(total)))
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_string(), |v| format!("{v:.2}"))
}

/// Renders the per-engine report as text.
pub fn render_text(merged: &BTreeMap<String, Snapshot>, section: Section, window: &str) -> String {
    let mut out = String::new();
    if merged.values().all(BTreeMap::is_empty) {
        let _ = writeln!(
            out,
            "No profile data recorded (window: {window}). Enable profiling with `tokensave profile enable`."
        );
        return out;
    }
    for (engine, snapshot) in merged {
        let rows = rows(snapshot, section);
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(out, "Engine: {engine} (window: {window})");
        let _ = writeln!(
            out,
            "{:<44} {:>9} {:>6} {:>11} {:>9} {:>9} {:>10} {:>10} {:>9}",
            "OPERATION", "COUNT", "ERR", "TOTAL ms", "MEAN", "P50", "P95", "P99", "MAX"
        );
        for row in &rows {
            let mark = if row.tail_reliable { "" } else { "*" };
            let _ = writeln!(
                out,
                "{:<44} {:>9} {:>6} {:>11.1} {:>9} {:>9} {:>10} {:>10} {:>9.2}",
                row.name,
                row.count,
                row.errors,
                row.total_ms,
                fmt_opt(row.mean_ms),
                fmt_opt(row.p50_ms),
                format!("{}{mark}", fmt_opt(row.p95_ms)),
                format!("{}{mark}", fmt_opt(row.p99_ms)),
                row.max_ms,
            );
        }
        if let Some((db, total)) = mcp_db_share(snapshot) {
            if matches!(section, Section::All | Section::Mcp) {
                let _ = writeln!(
                    out,
                    "\nDatabase share of measured MCP time: {:.0}% (outside DB: {:.0}%)",
                    100.0 * db / total,
                    100.0 * (total - db) / total
                );
            }
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(
        out,
        "Times in ms. Percentiles are bucket upper bounds (<= 1.6% high). \
         * = fewer than {MIN_RELIABLE_SAMPLES} samples, P95/P99 unreliable."
    );
    out
}

/// The per-engine report as JSON.
pub fn render_json(merged: &BTreeMap<String, Snapshot>, section: Section, window: &str) -> Value {
    let engines: serde_json::Map<String, Value> = merged
        .iter()
        .map(|(engine, snapshot)| {
            let mut body = json!({ "rows": rows(snapshot, section) });
            if let Some((db, total)) = mcp_db_share(snapshot) {
                body["mcp_db_ms"] = json!(db);
                body["mcp_total_ms"] = json!(total);
            }
            (engine.clone(), body)
        })
        .collect();
    json!({
        "window": window,
        "min_reliable_samples": MIN_RELIABLE_SAMPLES,
        "engines": engines,
    })
}

/// Operations with slow executions, most first.
pub fn render_slow(merged: &BTreeMap<String, Snapshot>, threshold_ms: u64, window: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Operations at or above {threshold_ms} ms (window: {window}):"
    );
    let mut any = false;
    for (engine, snapshot) in merged {
        let mut slow: Vec<(&String, &SeriesSnapshot)> =
            snapshot.iter().filter(|(_, s)| s.slow > 0).collect();
        slow.sort_by(|a, b| b.1.slow.cmp(&a.1.slow));
        for (name, s) in slow {
            any = true;
            let _ = writeln!(
                out,
                "  [{engine}] {name}: {} slow of {} ({:.1}%), max {:.2} ms",
                s.slow,
                s.count,
                100.0 * s.slow as f64 / s.count.max(1) as f64,
                ms(s.max_us)
            );
        }
    }
    if !any {
        let _ = writeln!(out, "  none");
    }
    let _ = writeln!(
        out,
        "The threshold is applied when a sample is recorded; changing it does not reclassify recorded data."
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(values: &[u64]) -> SeriesSnapshot {
        let mut s = SeriesSnapshot::default();
        for &v in values {
            s.count += 1;
            s.sum_us += v;
            s.max_us = s.max_us.max(v);
            s.histogram.record(v);
        }
        s
    }

    fn merged() -> BTreeMap<String, Snapshot> {
        let mut snap = Snapshot::new();
        snap.insert("db.get_node_by_id".into(), series(&[1_000; 150]));
        snap.insert("index.scan".into(), series(&[50_000]));
        let mut tool = series(&[10_000, 20_000]);
        tool.db_us = 12_000;
        tool.db_ops = 6;
        tool.db_ops_per_call.record(2);
        tool.db_ops_per_call.record(4);
        snap.insert("tool.tokensave_search".into(), tool);
        BTreeMap::from([("libsql".to_string(), snap)])
    }

    #[test]
    fn parses_windows() {
        assert_eq!(parse_window("30m"), Some(Duration::from_secs(1800)));
        assert_eq!(parse_window("24h"), Some(Duration::from_secs(86_400)));
        assert_eq!(parse_window("7d"), Some(Duration::from_secs(604_800)));
        assert_eq!(parse_window("7"), None);
        assert_eq!(parse_window("h"), None);
        assert_eq!(parse_window("3w"), None);
    }

    #[test]
    fn sections_filter_by_prefix_and_rank_by_total() {
        let m = merged();
        let snap = &m["libsql"];
        let all = rows(snap, Section::All);
        assert_eq!(all[0].name, "db.get_node_by_id", "150 ms total ranks first");
        assert_eq!(rows(snap, Section::Database).len(), 1);
        assert_eq!(rows(snap, Section::Indexing)[0].name, "index.scan");
        let mcp = rows(snap, Section::Mcp);
        assert_eq!(mcp[0].name, "tool.tokensave_search");
        assert_eq!(mcp[0].db_share, Some(0.4));
        assert!(mcp[0].median_db_ops_per_call.is_some());
    }

    #[test]
    fn tails_are_flagged_below_the_sample_minimum() {
        let m = merged();
        let rows = rows(&m["libsql"], Section::All);
        let by_name = |n: &str| rows.iter().find(|r| r.name == n).unwrap();
        assert!(by_name("db.get_node_by_id").tail_reliable);
        assert!(!by_name("index.scan").tail_reliable);
        let text = render_text(&m, Section::All, "24h");
        assert!(text.contains('*'));
        assert!(text.contains("Database share of measured MCP time: 40%"));
    }

    #[test]
    fn empty_data_says_so() {
        let text = render_text(&BTreeMap::new(), Section::All, "24h");
        assert!(text.contains("No profile data"));
    }

    #[test]
    fn json_report_carries_rows_and_db_share() {
        let v = render_json(&merged(), Section::Mcp, "7d");
        assert_eq!(v["window"], "7d");
        assert_eq!(
            v["engines"]["libsql"]["rows"][0]["name"],
            "tool.tokensave_search"
        );
        assert_eq!(v["engines"]["libsql"]["mcp_total_ms"], 30.0);
    }
}
