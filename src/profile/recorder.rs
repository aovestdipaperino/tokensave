//! In-memory aggregation: one series per [`Op`].
//!
//! Plain counters are striped across [`SHARDS`] cache-line-aligned shards,
//! and each thread always writes to the same shard, so concurrent recorders
//! (rayon extraction, concurrent MCP reads) do not contend on one counter.
//! Histogram buckets are shared atomics: concurrent samples of the same op
//! rarely hit the same bucket, and striping them too would multiply the
//! largest allocation by the shard count.
//!
//! Memory is fixed at construction: `Op::COUNT` series of
//! `SHARDS * 64` bytes of counters plus one histogram each (see
//! [`Recorder::resident_bytes`]). Recording never allocates, locks or does
//! I/O.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use super::histogram::{AtomicHistogram, Histogram, Quantile};
use super::ops::Op;

/// Counter stripes per series.
pub(crate) const SHARDS: usize = 8;

static NEXT_SHARD: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static SHARD: usize = NEXT_SHARD.fetch_add(1, Ordering::Relaxed) % SHARDS;
}

fn shard() -> usize {
    SHARD.try_with(|s| *s).unwrap_or(0)
}

#[derive(Default)]
#[repr(align(64))]
struct Counters {
    count: AtomicU64,
    errors: AtomicU64,
    sum_us: AtomicU64,
    exclusive_sum_us: AtomicU64,
    max_us: AtomicU64,
    slow: AtomicU64,
    rows_returned: AtomicU64,
    rows_affected: AtomicU64,
}

struct Series {
    shards: [Counters; SHARDS],
    histogram: AtomicHistogram,
}

/// One completed measurement.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    /// Wall time including child operations.
    pub inclusive_us: u64,
    /// Wall time minus child operations.
    pub exclusive_us: u64,
    pub rows_returned: u64,
    pub rows_affected: u64,
}

/// Lock-free aggregate store for every [`Op`].
pub struct Recorder {
    series: Box<[Series]>,
    slow_threshold_us: AtomicU64,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            series: (0..Op::COUNT)
                .map(|_| Series {
                    shards: Default::default(),
                    histogram: AtomicHistogram::new(),
                })
                .collect(),
            slow_threshold_us: AtomicU64::new(u64::MAX),
        }
    }

    /// Successful samples at least this slow are also counted as slow.
    pub fn set_slow_threshold_us(&self, us: u64) {
        self.slow_threshold_us.store(us, Ordering::Relaxed);
    }

    /// Records a successful operation.
    pub fn record(&self, op: Op, sample: Sample) {
        let series = &self.series[op.index()];
        let c = &series.shards[shard()];
        c.count.fetch_add(1, Ordering::Relaxed);
        c.sum_us.fetch_add(sample.inclusive_us, Ordering::Relaxed);
        c.exclusive_sum_us
            .fetch_add(sample.exclusive_us, Ordering::Relaxed);
        c.max_us.fetch_max(sample.inclusive_us, Ordering::Relaxed);
        if sample.rows_returned > 0 {
            c.rows_returned
                .fetch_add(sample.rows_returned, Ordering::Relaxed);
        }
        if sample.rows_affected > 0 {
            c.rows_affected
                .fetch_add(sample.rows_affected, Ordering::Relaxed);
        }
        if sample.inclusive_us >= self.slow_threshold_us.load(Ordering::Relaxed) {
            c.slow.fetch_add(1, Ordering::Relaxed);
        }
        series.histogram.record(sample.inclusive_us);
    }

    /// Records a failed operation. Its latency stays out of the histogram so
    /// fast failures cannot hide slow successes.
    pub fn record_error(&self, op: Op) {
        self.series[op.index()].shards[shard()]
            .errors
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Moves everything recorded so far into a snapshot and resets the
    /// recorder. Ops with nothing recorded are omitted.
    pub fn drain(&self) -> Snapshot {
        let mut out = Snapshot::new();
        for (index, series) in self.series.iter().enumerate() {
            let mut s = SeriesSnapshot::default();
            for c in &series.shards {
                s.count += c.count.swap(0, Ordering::Relaxed);
                s.errors += c.errors.swap(0, Ordering::Relaxed);
                s.sum_us += c.sum_us.swap(0, Ordering::Relaxed);
                s.exclusive_sum_us += c.exclusive_sum_us.swap(0, Ordering::Relaxed);
                s.max_us = s.max_us.max(c.max_us.swap(0, Ordering::Relaxed));
                s.slow += c.slow.swap(0, Ordering::Relaxed);
                s.rows_returned += c.rows_returned.swap(0, Ordering::Relaxed);
                s.rows_affected += c.rows_affected.swap(0, Ordering::Relaxed);
            }
            series.histogram.drain_into(&mut s.histogram);
            if !s.is_empty() {
                if let Some(op) = Op::from_index(index) {
                    out.insert(op.name().to_string(), s);
                }
            }
        }
        out
    }

    /// Fixed heap and inline size of this recorder.
    pub fn resident_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.series.len()
                * (std::mem::size_of::<Series>() + AtomicHistogram::resident_bytes())
    }
}

/// Aggregates for one op over some window, mergeable across threads,
/// processes and windows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesSnapshot {
    /// Successful samples (the histogram's population).
    pub count: u64,
    pub errors: u64,
    pub sum_us: u64,
    pub exclusive_sum_us: u64,
    pub max_us: u64,
    /// Successful samples at or above the slow threshold in force when they
    /// were recorded.
    pub slow: u64,
    pub rows_returned: u64,
    pub rows_affected: u64,
    pub histogram: Histogram,
    /// Invocation series only (`tool.*`): database time and operations
    /// attributed to the invocations, response size, and the distribution
    /// of database operations per invocation.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub db_us: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub db_ops: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub response_bytes: u64,
    #[serde(default, skip_serializing_if = "Histogram::is_empty")]
    pub db_ops_per_call: Histogram,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
fn is_zero(v: &u64) -> bool {
    *v == 0
}

impl SeriesSnapshot {
    pub fn is_empty(&self) -> bool {
        self.count == 0 && self.errors == 0
    }

    pub fn merge(&mut self, other: &Self) {
        self.count = self.count.saturating_add(other.count);
        self.errors = self.errors.saturating_add(other.errors);
        self.sum_us = self.sum_us.saturating_add(other.sum_us);
        self.exclusive_sum_us = self.exclusive_sum_us.saturating_add(other.exclusive_sum_us);
        self.max_us = self.max_us.max(other.max_us);
        self.slow = self.slow.saturating_add(other.slow);
        self.rows_returned = self.rows_returned.saturating_add(other.rows_returned);
        self.rows_affected = self.rows_affected.saturating_add(other.rows_affected);
        self.histogram.merge(&other.histogram);
        self.db_us = self.db_us.saturating_add(other.db_us);
        self.db_ops = self.db_ops.saturating_add(other.db_ops);
        self.response_bytes = self.response_bytes.saturating_add(other.response_bytes);
        self.db_ops_per_call.merge(&other.db_ops_per_call);
    }

    pub fn mean_us(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum_us as f64 / self.count as f64)
    }

    pub fn quantile(&self, q: f64) -> Option<Quantile> {
        self.histogram.quantile(q, self.max_us)
    }
}

/// Series keyed by their static op name. Names are kept as strings so files
/// written by other versions (with ops this build does not know) still merge.
pub type Snapshot = BTreeMap<String, SeriesSnapshot>;

/// Merges `from` into `into`, series by series.
pub fn merge_snapshot(into: &mut Snapshot, from: &Snapshot) {
    for (name, series) in from {
        into.entry(name.clone()).or_default().merge(series);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::ops::{DbOp, IndexPhase};
    use std::sync::Arc;

    fn ok(us: u64) -> Sample {
        Sample {
            inclusive_us: us,
            exclusive_us: us,
            ..Sample::default()
        }
    }

    #[test]
    fn records_counts_sums_max_rows_and_slow() {
        let r = Recorder::new();
        r.set_slow_threshold_us(50_000);
        let op = Op::Db(DbOp::Transaction);
        r.record(op, ok(100));
        r.record(
            op,
            Sample {
                inclusive_us: 60_000,
                exclusive_us: 10,
                rows_returned: 3,
                rows_affected: 2,
            },
        );
        r.record_error(op);
        let snap = r.drain();
        let s = &snap["db.transaction"];
        assert_eq!(s.count, 2);
        assert_eq!(s.errors, 1);
        assert_eq!(s.sum_us, 60_100);
        assert_eq!(s.exclusive_sum_us, 110);
        assert_eq!(s.max_us, 60_000);
        assert_eq!(s.slow, 1);
        assert_eq!((s.rows_returned, s.rows_affected), (3, 2));
        assert_eq!(s.histogram.count(), 2, "errors stay out of the histogram");
        assert_eq!(snap.len(), 1, "untouched ops are omitted");
    }

    #[test]
    fn slow_threshold_boundary_is_inclusive() {
        let r = Recorder::new();
        r.set_slow_threshold_us(50_000);
        let op = Op::Index(IndexPhase::Scan);
        r.record(op, ok(49_999));
        r.record(op, ok(50_000));
        assert_eq!(r.drain()["index.scan"].slow, 1);
    }

    #[test]
    fn drain_resets() {
        let r = Recorder::new();
        r.record(Op::Db(DbOp::Open), ok(1));
        assert_eq!(r.drain().len(), 1);
        assert!(r.drain().is_empty());
    }

    #[test]
    fn concurrent_records_are_exact() {
        let r = Arc::new(Recorder::new());
        let threads: Vec<_> = (0..16)
            .map(|t| {
                let r = Arc::clone(&r);
                std::thread::spawn(move || {
                    for i in 0..5_000u64 {
                        r.record(Op::Db(DbOp::Transaction), ok(i % 1000 + t));
                        if i % 10 == 0 {
                            r.record_error(Op::Db(DbOp::Transaction));
                        }
                    }
                })
            })
            .collect();
        // Drain concurrently too: nothing may be lost or counted twice.
        let mut total = Snapshot::new();
        for _ in 0..50 {
            merge_snapshot(&mut total, &r.drain());
        }
        for t in threads {
            t.join().unwrap();
        }
        merge_snapshot(&mut total, &r.drain());
        let s = &total["db.transaction"];
        assert_eq!(s.count, 16 * 5_000);
        assert_eq!(s.errors, 16 * 500);
        assert_eq!(s.histogram.count(), 16 * 5_000);
    }

    #[test]
    fn snapshot_merge_adds_series() {
        let mut a = Snapshot::new();
        let mut s = SeriesSnapshot {
            count: 1,
            sum_us: 5,
            max_us: 5,
            ..SeriesSnapshot::default()
        };
        s.histogram.record(5);
        a.insert("x".into(), s.clone());
        let mut b = a.clone();
        b.insert("y".into(), s.clone());
        merge_snapshot(&mut a, &b);
        assert_eq!(a["x"].count, 2);
        assert_eq!(a["x"].histogram.count(), 2);
        assert_eq!(a["y"].count, 1);
    }

    #[test]
    fn resident_size_is_fixed_and_small() {
        let bytes = Recorder::new().resident_bytes();
        assert!(bytes < 1024 * 1024, "{bytes} bytes");
    }
}
