//! Per-tool MCP invocation aggregates.
//!
//! Tool names are registered once from the server's static tool registry
//! (#740), so the table is bounded by the registry, not by what clients
//! send: a name that was not registered is recorded under [`OTHER`]. Each
//! tool keeps end-to-end latency, database time and operation count per
//! invocation, response size, and a histogram of database operations per
//! invocation (the distribution N+1 detection reads).
//!
//! The table is allocated only when profiling is enabled at registration
//! time: about `(registered tools + 1) × 21 KiB`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use super::context::InvocationSummary;
use super::histogram::AtomicHistogram;
use super::recorder::{SeriesSnapshot, Snapshot};

/// Series name for calls to tools that were not registered.
pub const OTHER: &str = "other";

/// Upper bound on registered tools; names past it fold into [`OTHER`].
const MAX_TOOLS: usize = 256;

#[derive(Default)]
struct Counters {
    count: AtomicU64,
    errors: AtomicU64,
    sum_us: AtomicU64,
    max_us: AtomicU64,
    db_us: AtomicU64,
    db_ops: AtomicU64,
    response_bytes: AtomicU64,
}

struct ToolSeries {
    counters: Counters,
    latency: AtomicHistogram,
    db_ops_per_call: AtomicHistogram,
}

impl ToolSeries {
    fn new() -> Self {
        Self {
            counters: Counters::default(),
            latency: AtomicHistogram::new(),
            db_ops_per_call: AtomicHistogram::new(),
        }
    }
}

struct Table {
    /// Sorted, deduplicated; the last series (index `names.len()`) is
    /// [`OTHER`].
    names: Box<[Box<str>]>,
    series: Box<[ToolSeries]>,
}

static TABLE: OnceLock<Table> = OnceLock::new();

/// Registers the server's tool names. Only the first call takes effect, and
/// only while profiling is enabled, so a server with profiling off
/// allocates nothing.
pub fn register<'a>(names: impl IntoIterator<Item = &'a str>) {
    if !super::enabled() || TABLE.get().is_some() {
        return;
    }
    let mut names: Vec<Box<str>> = names
        .into_iter()
        .filter(|n| *n != OTHER)
        .map(Box::from)
        .collect();
    names.sort_unstable();
    names.dedup();
    names.truncate(MAX_TOOLS);
    let series = (0..=names.len()).map(|_| ToolSeries::new()).collect();
    let _ = TABLE.set(Table {
        names: names.into_boxed_slice(),
        series,
    });
}

/// One finished `tools/call`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolCall {
    pub elapsed_us: u64,
    pub ok: bool,
    pub invocation: InvocationSummary,
    pub response_bytes: u64,
}

/// Records one call to `tool`. Does nothing before registration.
pub fn record(tool: &str, call: ToolCall) {
    let Some(table) = TABLE.get() else {
        return;
    };
    let index = table
        .names
        .binary_search_by(|n| (**n).cmp(tool))
        .unwrap_or(table.names.len());
    let Some(series) = table.series.get(index) else {
        return;
    };
    let c = &series.counters;
    if !call.ok {
        c.errors.fetch_add(1, Ordering::Relaxed);
        return;
    }
    c.count.fetch_add(1, Ordering::Relaxed);
    c.sum_us.fetch_add(call.elapsed_us, Ordering::Relaxed);
    c.max_us.fetch_max(call.elapsed_us, Ordering::Relaxed);
    c.db_us.fetch_add(call.invocation.db_us, Ordering::Relaxed);
    c.db_ops
        .fetch_add(call.invocation.db_ops, Ordering::Relaxed);
    c.response_bytes
        .fetch_add(call.response_bytes, Ordering::Relaxed);
    series.latency.record(call.elapsed_us);
    series.db_ops_per_call.record(call.invocation.db_ops);
}

/// Moves every tool's aggregates into `into` as `tool.<name>` series.
pub(crate) fn drain_into(into: &mut Snapshot) {
    let Some(table) = TABLE.get() else {
        return;
    };
    for (index, series) in table.series.iter().enumerate() {
        let c = &series.counters;
        let mut s = SeriesSnapshot {
            count: c.count.swap(0, Ordering::Relaxed),
            errors: c.errors.swap(0, Ordering::Relaxed),
            sum_us: c.sum_us.swap(0, Ordering::Relaxed),
            max_us: c.max_us.swap(0, Ordering::Relaxed),
            db_us: c.db_us.swap(0, Ordering::Relaxed),
            db_ops: c.db_ops.swap(0, Ordering::Relaxed),
            response_bytes: c.response_bytes.swap(0, Ordering::Relaxed),
            ..SeriesSnapshot::default()
        };
        // Time outside the database, the counterpart of `db_us`.
        s.exclusive_sum_us = s.sum_us.saturating_sub(s.db_us);
        series.latency.drain_into(&mut s.histogram);
        series.db_ops_per_call.drain_into(&mut s.db_ops_per_call);
        if s.is_empty() {
            continue;
        }
        let name = table.names.get(index).map_or(OTHER, |n| &**n);
        into.insert(format!("tool.{name}"), s);
    }
}

/// Whether `name` is a registered tool (for tests and reports).
pub fn is_registered(name: &str) -> bool {
    TABLE
        .get()
        .is_some_and(|t| t.names.binary_search_by(|n| (**n).cmp(name)).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{self, Mode};

    #[test]
    fn records_registered_tools_and_folds_unknown_names_into_other() {
        let _g = profile::TEST_MODE_LOCK.blocking_lock();
        profile::set_mode(Mode::Light);
        // Real tool names: a server built concurrently while profiling is on
        // may register the table first, and it registers these too.
        register(["tokensave_search", "tokensave_context"]);
        let mut before = Snapshot::new();
        drain_into(&mut before);

        let invocation = InvocationSummary {
            db_us: 300,
            db_ops: 25,
        };
        let ok = |elapsed_us| ToolCall {
            elapsed_us,
            ok: true,
            invocation,
            response_bytes: 10,
        };
        record("tokensave_search", ok(1_000));
        record("tokensave_search", ok(3_000));
        record("tokensave_search", ToolCall { ok: false, ..ok(5) });
        record("not_a_tool", ok(50));

        let mut snap = Snapshot::new();
        drain_into(&mut snap);
        profile::set_mode(Mode::Off);

        assert!(is_registered("tokensave_search"));
        let s = &snap["tool.tokensave_search"];
        assert!(s.count >= 2 && s.errors >= 1);
        assert!(s.db_ops >= 50 && s.db_us >= 600);
        assert_eq!(s.exclusive_sum_us, s.sum_us - s.db_us);
        assert!(s.db_ops_per_call.count() >= 2);
        assert!(s.response_bytes >= 20);
        assert!(snap[&format!("tool.{OTHER}")].count >= 1);
        assert!(!snap.contains_key("tool.not_a_tool"));
    }
}
