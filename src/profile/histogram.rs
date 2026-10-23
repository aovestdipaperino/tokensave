//! Fixed-bucket log-linear latency histograms.
//!
//! Values are microseconds. Below 128 µs every value has its own bucket;
//! above it each power-of-two range is split into 64 equal buckets, so a
//! bucket's width is at most 1/64 (about 1.6%) of its lower bound. Values
//! above [`MAX_TRACKABLE_US`] (60 s) land in the last bucket; the exact
//! maximum is kept separately by the series. The layout is fixed, so
//! histograms from different threads, processes and time windows merge
//! exactly by adding bucket counts.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Largest value with its own bucket range; larger values saturate.
pub const MAX_TRACKABLE_US: u64 = 60_000_000;

const LINEAR: u64 = 128;
const SUB_BITS: u32 = 6;
const SUB: u64 = 1 << SUB_BITS;
const FIRST_OCTAVE: u32 = 7; // log2(LINEAR)

/// Number of buckets in a histogram: up to and including the bucket that
/// holds `MAX_TRACKABLE_US`.
pub const BUCKETS: usize = bucket_index(MAX_TRACKABLE_US) + 1;

/// Bucket holding `us`.
pub const fn bucket_index(us: u64) -> usize {
    let v = if us < MAX_TRACKABLE_US {
        us
    } else {
        MAX_TRACKABLE_US
    };
    if v < LINEAR {
        return v as usize;
    }
    let octave = 63 - v.leading_zeros();
    let shift = octave - SUB_BITS;
    let sub = (v >> shift) - SUB;
    (LINEAR + (octave - FIRST_OCTAVE) as u64 * SUB + sub) as usize
}

/// Half-open value range `[lower, upper)` covered by bucket `index`.
pub fn bucket_bounds(index: usize) -> (u64, u64) {
    let index = index as u64;
    if index < LINEAR {
        return (index, index + 1);
    }
    let rel = index - LINEAR;
    let octave = (rel / SUB) as u32 + FIRST_OCTAVE;
    let shift = octave - SUB_BITS;
    let lower = (SUB + rel % SUB) << shift;
    (lower, lower + (1 << shift))
}

/// Lock-free histogram that many threads record into concurrently.
pub(crate) struct AtomicHistogram {
    buckets: Box<[AtomicU64]>,
}

impl AtomicHistogram {
    pub(crate) fn new() -> Self {
        Self {
            buckets: (0..BUCKETS).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    pub(crate) fn record(&self, us: u64) {
        self.buckets[bucket_index(us)].fetch_add(1, Ordering::Relaxed);
    }

    /// Moves every count into `into` and resets this histogram. A record
    /// racing the drain lands either in this drain or in the next one, never
    /// in both and never lost.
    pub(crate) fn drain_into(&self, into: &mut Histogram) {
        for (index, bucket) in self.buckets.iter().enumerate() {
            if bucket.load(Ordering::Relaxed) == 0 {
                continue;
            }
            let n = bucket.swap(0, Ordering::Relaxed);
            if n > 0 {
                into.record_n_at(index, n);
            }
        }
    }

    pub(crate) const fn resident_bytes() -> usize {
        BUCKETS * std::mem::size_of::<AtomicU64>()
    }
}

/// A quantile estimate with its bucket error bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quantile {
    /// Number of samples the estimate is based on.
    pub samples: u64,
    /// The true nearest-rank value is at least this.
    pub lower_us: u64,
    /// The true nearest-rank value is at most this; also the reported value.
    pub upper_us: u64,
}

/// Plain, mergeable histogram snapshot, stored sparsely.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Histogram {
    buckets: BTreeMap<u16, u64>,
}

impl Histogram {
    pub fn record(&mut self, us: u64) {
        self.record_n_at(bucket_index(us), 1);
    }

    fn record_n_at(&mut self, index: usize, n: u64) {
        // Bucket indexes are < BUCKETS (1331), well within u16.
        let entry = self.buckets.entry(index as u16).or_insert(0);
        *entry = entry.saturating_add(n);
    }

    pub fn merge(&mut self, other: &Self) {
        for (&index, &n) in &other.buckets {
            if usize::from(index) < BUCKETS {
                self.record_n_at(usize::from(index), n);
            }
        }
    }

    pub fn count(&self) -> u64 {
        self.buckets
            .values()
            .fold(0, |acc, n| acc.saturating_add(*n))
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.values().all(|n| *n == 0)
    }

    /// Nearest-rank quantile (`q` in `0.0..=1.0`), bounded by `max_us`, the
    /// exact maximum observed. `None` for an empty histogram.
    pub fn quantile(&self, q: f64, max_us: u64) -> Option<Quantile> {
        let samples = self.count();
        if samples == 0 {
            return None;
        }
        let rank = ((q.clamp(0.0, 1.0) * samples as f64).ceil() as u64).clamp(1, samples);
        let mut seen = 0u64;
        for (&index, &n) in &self.buckets {
            seen = seen.saturating_add(n);
            if seen >= rank {
                let index = usize::from(index);
                let (lower, upper) = bucket_bounds(index);
                let upper_us = if index + 1 == BUCKETS {
                    max_us.max(lower)
                } else {
                    (upper - 1).min(max_us.max(lower))
                };
                return Some(Quantile {
                    samples,
                    lower_us: lower.min(upper_us),
                    upper_us,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_covers_one_microsecond_to_sixty_seconds() {
        assert_eq!(BUCKETS, 1331);
        assert_eq!(bucket_index(0), 0);
        assert_eq!(bucket_index(1), 1);
        assert_eq!(bucket_index(127), 127);
        assert_eq!(bucket_index(128), 128);
        assert_eq!(bucket_index(MAX_TRACKABLE_US), BUCKETS - 1);
        assert_eq!(bucket_index(u64::MAX), BUCKETS - 1);
    }

    #[test]
    fn every_value_falls_inside_its_bucket_bounds() {
        let mut v = 0u64;
        while v <= MAX_TRACKABLE_US {
            let (lower, upper) = bucket_bounds(bucket_index(v));
            assert!(lower <= v && v < upper, "{v} not in [{lower}, {upper})");
            v = v + 1 + v / 97;
        }
    }

    #[test]
    fn buckets_are_contiguous_and_relative_error_is_bounded() {
        for index in 1..BUCKETS {
            let (prev_lower, prev_upper) = bucket_bounds(index - 1);
            let (lower, upper) = bucket_bounds(index);
            assert_eq!(prev_upper, lower, "gap before bucket {index}");
            assert!(prev_lower < prev_upper);
            if lower >= LINEAR {
                assert!((upper - lower) * 64 <= lower, "bucket {index} too wide");
            }
        }
    }

    fn exact_nearest_rank(sorted: &[u64], q: f64) -> u64 {
        let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
        sorted[rank - 1]
    }

    #[test]
    fn quantiles_bound_the_exact_nearest_rank() {
        let cases: Vec<Vec<u64>> = vec![
            vec![5],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            (1..=1000).map(|i| i * 37).collect(),
            (0..500u32)
                .map(|i| 10u64.pow(i % 7) + u64::from(i))
                .collect(),
            vec![70_000_000, 1, 2],
        ];
        for values in cases {
            let mut h = Histogram::default();
            for &v in &values {
                h.record(v);
            }
            let mut sorted = values.clone();
            sorted.sort_unstable();
            let max = *sorted.last().unwrap();
            for q in [0.0, 0.01, 0.5, 0.9, 0.95, 0.99, 1.0] {
                let exact = exact_nearest_rank(&sorted, q);
                let est = h.quantile(q, max).unwrap();
                assert_eq!(est.samples, values.len() as u64);
                assert!(
                    est.lower_us <= exact && exact <= est.upper_us,
                    "q={q}: exact {exact} outside [{}, {}]",
                    est.lower_us,
                    est.upper_us
                );
            }
        }
    }

    #[test]
    fn merge_equals_recording_everything_in_one_histogram() {
        let mut a = Histogram::default();
        let mut b = Histogram::default();
        let mut all = Histogram::default();
        for v in 0..2000u64 {
            let v = v * v;
            if v % 3 == 0 { &mut a } else { &mut b }.record(v);
            all.record(v);
        }
        a.merge(&b);
        assert_eq!(a, all);
        assert_eq!(a.count(), 2000);
    }

    #[test]
    fn empty_histogram_has_no_quantile() {
        assert!(Histogram::default().quantile(0.5, 0).is_none());
        assert!(Histogram::default().is_empty());
    }

    #[test]
    fn atomic_drain_moves_counts_and_resets() {
        let h = AtomicHistogram::new();
        h.record(10);
        h.record(10);
        h.record(5_000);
        let mut out = Histogram::default();
        h.drain_into(&mut out);
        assert_eq!(out.count(), 3);
        let mut again = Histogram::default();
        h.drain_into(&mut again);
        assert!(again.is_empty());
    }

    #[test]
    fn serializes_sparsely() {
        let mut h = Histogram::default();
        h.record(3);
        h.record(3);
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, r#"{"3":2}"#);
        assert_eq!(serde_json::from_str::<Histogram>(&json).unwrap(), h);
    }
}
