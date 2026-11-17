//! Summary statistics for repeated timings.

use serde_json::{json, Value};

/// Nearest-rank percentile of an unsorted sample (`p` in 0..=100).
pub fn percentile(samples: &[f64], p: f64) -> f64 {
    if samples.is_empty() {
        return f64::NAN;
    }
    let mut v = samples.to_vec();
    v.sort_by(f64::total_cmp);
    let rank = ((p / 100.0) * v.len() as f64).ceil().max(1.0) as usize;
    v[rank.min(v.len()) - 1]
}

pub fn mean(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return f64::NAN;
    }
    samples.iter().sum::<f64>() / samples.len() as f64
}

/// Sample standard deviation.
pub fn stddev(samples: &[f64]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    let m = mean(samples);
    (samples.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (samples.len() - 1) as f64).sqrt()
}

/// Latency summary in the unit of the samples.
pub fn summary(samples: &[f64]) -> Value {
    json!({
        "n": samples.len(),
        "mean": mean(samples),
        "stddev": stddev(samples),
        "cv": if mean(samples) > 0.0 { stddev(samples) / mean(samples) } else { 0.0 },
        "min": percentile(samples, 0.0),
        "p50": percentile(samples, 50.0),
        "p95": percentile(samples, 95.0),
        "p99": percentile(samples, 99.0),
        "max": percentile(samples, 100.0),
    })
}
