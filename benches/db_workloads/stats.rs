//! Per-operation latency summaries for the database workload bench.
//!
//! Kept free of tokensave types so `tests/db_bench_stats_test.rs` can include
//! it with `#[path]` and the percentile math is covered by the normal test run.

use std::time::Duration;

/// Below this many successful samples a p99 is reported but flagged: it is
/// then just one of the slowest few observations, not a stable estimate.
pub const MIN_SAMPLES_FOR_P99: usize = 100;

/// Latency samples of one workload, in insertion order.
#[derive(Default)]
pub struct Recorder {
    samples: Vec<Duration>,
    errors: Vec<String>,
}

impl Recorder {
    pub fn with_capacity(n: usize) -> Self {
        Self {
            samples: Vec::with_capacity(n),
            errors: Vec::new(),
        }
    }

    pub fn record(&mut self, elapsed: Duration) {
        self.samples.push(elapsed);
    }

    /// A failed or incorrect operation. Its latency is not recorded, and any
    /// failure makes the whole run fail rather than look fast.
    pub fn fail(&mut self, reason: String) {
        self.errors.push(reason);
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn summary(&self) -> Summary {
        Summary::from_samples(&self.samples, self.errors.len())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub samples: usize,
    pub errors: usize,
    pub min_us: f64,
    pub mean_us: f64,
    pub stdev_us: f64,
    pub p50_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
    pub max_us: f64,
    /// Successful operations per second of summed operation time
    /// (sequential, single client).
    pub ops_per_sec: f64,
    pub p99_reliable: bool,
}

impl Summary {
    pub fn from_samples(samples: &[Duration], errors: usize) -> Self {
        let mut us: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1e6).collect();
        us.sort_by(f64::total_cmp);
        let n = us.len();
        let total: f64 = us.iter().sum();
        let mean = if n == 0 { 0.0 } else { total / n as f64 };
        let var = if n < 2 {
            0.0
        } else {
            us.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        };
        Self {
            samples: n,
            errors,
            min_us: us.first().copied().unwrap_or(0.0),
            mean_us: mean,
            stdev_us: var.sqrt(),
            p50_us: percentile(&us, 50.0),
            p95_us: percentile(&us, 95.0),
            p99_us: percentile(&us, 99.0),
            max_us: us.last().copied().unwrap_or(0.0),
            ops_per_sec: if total > 0.0 {
                n as f64 / (total / 1e6)
            } else {
                0.0
            },
            p99_reliable: n >= MIN_SAMPLES_FOR_P99,
        }
    }
}

/// Nearest-rank percentile of an ascending slice (0 when empty).
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}
