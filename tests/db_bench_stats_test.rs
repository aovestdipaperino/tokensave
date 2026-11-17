//! Percentile and summary math of the database workload bench
//! (`benches/db_workloads`), which itself does not run under `cargo test`.

#[path = "../benches/db_workloads/stats.rs"]
#[allow(dead_code)]
mod stats;

use std::time::Duration;

use stats::{percentile, Recorder, Summary, MIN_SAMPLES_FOR_P99};

#[test]
fn nearest_rank_percentiles() {
    let v: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(percentile(&v, 50.0), 50.0);
    assert_eq!(percentile(&v, 95.0), 95.0);
    assert_eq!(percentile(&v, 99.0), 99.0);
    assert_eq!(percentile(&v, 100.0), 100.0);
    assert_eq!(percentile(&v, 0.0), 1.0);
    assert_eq!(percentile(&[7.0], 99.0), 7.0);
    assert_eq!(percentile(&[], 50.0), 0.0);
}

#[test]
fn summary_sorts_and_derives_throughput() {
    let samples: Vec<Duration> = [30, 10, 20, 40].map(Duration::from_millis).to_vec();
    let s = Summary::from_samples(&samples, 0);
    assert_eq!(s.samples, 4);
    assert_eq!(s.min_us, 10_000.0);
    assert_eq!(s.max_us, 40_000.0);
    assert_eq!(s.p50_us, 20_000.0);
    assert_eq!(s.mean_us, 25_000.0);
    // 4 operations in 100 ms of summed operation time.
    assert!((s.ops_per_sec - 40.0).abs() < 1e-9);
    assert!(!s.p99_reliable);
}

#[test]
fn failures_are_counted_and_not_timed() {
    let mut r = Recorder::with_capacity(MIN_SAMPLES_FOR_P99);
    for _ in 0..MIN_SAMPLES_FOR_P99 {
        r.record(Duration::from_micros(5));
    }
    r.fail("lookup returned no node".into());
    let s = r.summary();
    assert_eq!(s.samples, MIN_SAMPLES_FOR_P99);
    assert_eq!(s.errors, 1);
    assert!(s.p99_reliable);
    assert_eq!(r.errors().len(), 1);
}

#[test]
fn empty_summary_is_zero() {
    let s = Summary::from_samples(&[], 3);
    assert_eq!(s.samples, 0);
    assert_eq!(s.errors, 3);
    assert_eq!(s.ops_per_sec, 0.0);
}
