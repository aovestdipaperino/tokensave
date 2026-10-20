//! One-shot host resource benchmark that selects a [`CpuUsage`] profile.
//!
//! Runs once, on demand (`tokensave init`, `tokensave tune`), never in the
//! background. It probes CPUs and memory, then times a fixed CPU-bound
//! workload on one thread and on several scoped threads to measure the
//! parallel speedup the host actually delivers. [`recommend`] maps the result
//! to one of the existing profiles; the caller persists that profile, so
//! normal runs read the config and never re-measure.
use std::hint::black_box;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::cpu::CpuUsage;

const GIB: u64 = 1 << 30;

/// Single-thread time the calibration aims for. Short enough that the whole
/// benchmark normally finishes in well under a second.
const TARGET_SAMPLE: Duration = Duration::from_millis(30);
/// Calibration stops doubling here even on an extremely slow host.
const MAX_UNITS: u32 = 1 << 16;
/// Threads used for the parallel phase. Bounds the worst case (every thread
/// serialized onto one CPU) to `MAX_BENCH_THREADS * TARGET_SAMPLE` per round.
const MAX_BENCH_THREADS: usize = 16;
/// Parallel rounds; the fastest is kept to filter out transient load.
const PARALLEL_ROUNDS: usize = 2;
/// Iterations of the integer kernel in one work unit.
const UNIT_ITERS: u32 = 4096;

/// Below this total (or cgroup-limited) memory, `low` is selected. A "4 GB"
/// device reports about 3.7 GiB, so it stays above the line.
const LOW_TOTAL_MEMORY: u64 = 3 * GIB;
/// Below this currently available memory, `low` is selected.
const LOW_AVAILABLE_MEMORY: u64 = GIB;
/// Hosts with at most this many CPUs get `low`; `balanced` would also use one worker.
const LOW_MAX_CPUS: usize = 2;
/// Below this measured parallel throughput (in CPUs), `low` is selected.
const LOW_EFFECTIVE_CPUS: f64 = 2.5;
/// `performance` needs at least this measured parallel throughput (in CPUs)…
const PERFORMANCE_EFFECTIVE_CPUS: f64 = 6.0;
/// …this parallel efficiency (speedup / threads; turbo boost keeps it below 1)…
const PERFORMANCE_EFFICIENCY: f64 = 0.6;
/// …this much total memory…
const PERFORMANCE_TOTAL_MEMORY: u64 = 16 * GIB;
/// …and this much currently available memory.
const PERFORMANCE_AVAILABLE_MEMORY: u64 = 8 * GIB;

/// Static host capacity. `None` means the probe failed or is unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HostResources {
    /// CPUs this process may use (affinity and cgroup quota aware).
    pub cpus: Option<usize>,
    /// Total memory in bytes, or the cgroup limit when it is stricter.
    pub memory_total: Option<u64>,
    /// Currently available memory in bytes, capped by the cgroup's free memory.
    pub memory_available: Option<u64>,
}

/// Result of the parallel scaling benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ParallelBenchmark {
    /// Threads used in the parallel phase.
    pub threads: usize,
    /// Throughput with `threads` threads relative to one thread.
    pub speedup: f64,
    /// Wall-clock time spent benchmarking, in milliseconds.
    pub elapsed_ms: u64,
}

impl ParallelBenchmark {
    /// Speedup per thread, in `0.0..=1.0` (clamped; noise can push it above 1).
    pub fn efficiency(&self) -> f64 {
        (self.speedup / self.threads.max(1) as f64).clamp(0.0, 1.0)
    }
}

/// Everything [`recommend`] decides on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Measurement {
    pub resources: HostResources,
    /// `None` when the CPU count is unknown or a benchmark thread failed to start.
    pub benchmark: Option<ParallelBenchmark>,
}

impl Measurement {
    /// Parallel throughput in CPUs: available CPUs scaled by measured efficiency.
    pub fn effective_cpus(&self) -> Option<f64> {
        let cpus = self.resources.cpus?;
        Some(cpus as f64 * self.benchmark?.efficiency())
    }

    /// One-line human summary, e.g.
    /// `8 CPUs, 15.5 GiB memory (9.8 GiB available), 6.1x speedup on 8 threads`.
    pub fn summary(&self) -> String {
        let r = &self.resources;
        let mut parts = vec![match r.cpus {
            Some(1) => "1 CPU".to_string(),
            Some(n) => format!("{n} CPUs"),
            None => "unknown CPUs".to_string(),
        }];
        parts.push(match (r.memory_total, r.memory_available) {
            (Some(t), Some(a)) => format!("{} memory ({} available)", gib(t), gib(a)),
            (Some(t), None) => format!("{} memory", gib(t)),
            (None, _) => "unknown memory".to_string(),
        });
        if let Some(b) = self.benchmark {
            parts.push(format!(
                "{:.1}x speedup on {} thread{}",
                b.speedup,
                b.threads,
                if b.threads == 1 { "" } else { "s" }
            ));
        }
        parts.join(", ")
    }
}

/// Selected profile and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recommendation {
    pub profile: CpuUsage,
    pub reason: String,
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / GIB as f64)
}

/// Probes the host and runs the benchmark. Blocks for roughly 0.1–1 s.
pub fn measure() -> Measurement {
    let resources = probe_resources();
    let benchmark = resources.cpus.and_then(run_benchmark);
    Measurement {
        resources,
        benchmark,
    }
}

/// Reads CPU and memory capacity once. Never fails; unknown values are `None`.
pub fn probe_resources() -> HostResources {
    let cpus = std::thread::available_parallelism()
        .ok()
        .map(std::num::NonZeroUsize::get);

    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let nonzero = |v: u64| (v > 0).then_some(v);
    let mut memory_total = nonzero(system.total_memory());
    let mut memory_available = nonzero(system.available_memory());
    if let Some(limits) = system.cgroup_limits() {
        memory_total = min_known(memory_total, nonzero(limits.total_memory));
        memory_available = min_known(memory_available, nonzero(limits.free_memory));
    }

    HostResources {
        cpus,
        memory_total,
        memory_available,
    }
}

fn min_known(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Fixed integer workload (xorshift + multiply). `black_box` keeps the
/// optimizer from folding it away.
fn work_unit(seed: u64) -> u64 {
    let mut x = seed | 1;
    for _ in 0..UNIT_ITERS {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
    }
    black_box(x)
}

fn run_units(units: u32, seed: u64) -> Duration {
    let start = Instant::now();
    let mut acc = 0u64;
    for i in 0..units {
        acc = acc.wrapping_add(work_unit(seed ^ u64::from(i)));
    }
    black_box(acc);
    start.elapsed()
}

/// Times `units` on one thread, then the same per-thread work on
/// `min(cpus, MAX_BENCH_THREADS)` scoped threads. Every thread is joined
/// before returning. Returns `None` if a thread cannot be spawned.
fn run_benchmark(cpus: usize) -> Option<ParallelBenchmark> {
    let start = Instant::now();

    // Calibrate: double the unit count until one thread takes TARGET_SAMPLE.
    let mut units = 1u32;
    let mut single = run_units(units, 0);
    while single < TARGET_SAMPLE && units < MAX_UNITS {
        units *= 2;
        single = run_units(units, u64::from(units));
    }
    let single = single.max(Duration::from_nanos(1));

    let threads = cpus.clamp(1, MAX_BENCH_THREADS);
    if threads == 1 {
        return Some(ParallelBenchmark {
            threads,
            speedup: 1.0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }

    let mut best: Option<Duration> = None;
    for round in 0..PARALLEL_ROUNDS {
        let round_start = Instant::now();
        let spawned = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    std::thread::Builder::new()
                        .name(format!("tokensave-bench-{t}"))
                        .spawn_scoped(scope, move || {
                            run_units(units, ((round * threads + t) as u64) << 32)
                        })
                })
                .collect();
            // Join explicitly so a spawn failure is reported, not just dropped.
            handles
                .into_iter()
                .map(|h| h.ok().map(|h| h.join().is_ok()))
                .all(|ok| ok == Some(true))
        });
        if !spawned {
            return None;
        }
        let elapsed = round_start.elapsed().max(Duration::from_nanos(1));
        best = Some(best.map_or(elapsed, |b| b.min(elapsed)));
    }

    let parallel = best?;
    Some(ParallelBenchmark {
        threads,
        speedup: threads as f64 * single.as_secs_f64() / parallel.as_secs_f64(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// Maps a measurement to an existing profile. Pure and deterministic.
///
/// Memory and CPU are each checked for `low` first; `performance` requires
/// every signal to be known and ample; anything else (including a failed
/// probe) keeps the `balanced` default.
pub fn recommend(m: &Measurement) -> Recommendation {
    let r = &m.resources;
    let pick = |profile, reason: String| Recommendation { profile, reason };

    if let Some(total) = r.memory_total {
        if total < LOW_TOTAL_MEMORY {
            return pick(
                CpuUsage::Low,
                format!(
                    "{} total memory is below {}",
                    gib(total),
                    gib(LOW_TOTAL_MEMORY)
                ),
            );
        }
    }
    if let Some(available) = r.memory_available {
        if available < LOW_AVAILABLE_MEMORY {
            return pick(
                CpuUsage::Low,
                format!(
                    "{} available memory is below {}",
                    gib(available),
                    gib(LOW_AVAILABLE_MEMORY)
                ),
            );
        }
    }
    let Some(cpus) = r.cpus else {
        return pick(
            CpuUsage::Balanced,
            "the CPU count is unavailable, keeping the default".to_string(),
        );
    };
    if cpus <= LOW_MAX_CPUS {
        return pick(
            CpuUsage::Low,
            format!("{cpus} CPU{} available", if cpus == 1 { "" } else { "s" }),
        );
    }
    let Some(effective) = m.effective_cpus() else {
        return pick(
            CpuUsage::Balanced,
            "the parallel benchmark did not run, keeping the default".to_string(),
        );
    };
    if effective < LOW_EFFECTIVE_CPUS {
        return pick(
            CpuUsage::Low,
            format!("only {effective:.1} CPUs of parallel throughput measured"),
        );
    }

    let efficiency = m.benchmark.map_or(0.0, |b| b.efficiency());
    let ample_memory = r
        .memory_total
        .is_some_and(|t| t >= PERFORMANCE_TOTAL_MEMORY)
        && r.memory_available
            .is_some_and(|a| a >= PERFORMANCE_AVAILABLE_MEMORY);
    if effective >= PERFORMANCE_EFFECTIVE_CPUS
        && efficiency >= PERFORMANCE_EFFICIENCY
        && ample_memory
    {
        return pick(
            CpuUsage::Performance,
            format!(
                "{effective:.1} CPUs of parallel throughput ({:.0}% efficiency) and ample memory",
                efficiency * 100.0
            ),
        );
    }
    pick(
        CpuUsage::Balanced,
        format!("{effective:.1} CPUs of parallel throughput; resources are moderate or shared"),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn host(
        cpus: Option<usize>,
        total_gib: Option<f64>,
        avail_gib: Option<f64>,
        speedup: Option<f64>,
    ) -> Measurement {
        let bytes = |g: f64| (g * GIB as f64) as u64;
        Measurement {
            resources: HostResources {
                cpus,
                memory_total: total_gib.map(bytes),
                memory_available: avail_gib.map(bytes),
            },
            benchmark: cpus.zip(speedup).map(|(c, s)| ParallelBenchmark {
                threads: c.min(MAX_BENCH_THREADS),
                speedup: s,
                elapsed_ms: 100,
            }),
        }
    }

    #[test]
    fn representative_hosts() {
        use CpuUsage::{Balanced, Low, Performance};
        let cases = [
            // (cpus, total GiB, available GiB, speedup, expected)
            (Some(1), Some(1.0), Some(0.5), Some(1.0), Low), // Pi 1 GB
            (Some(4), Some(1.0), Some(0.6), Some(3.8), Low), // Pi 3, memory bound
            (Some(4), Some(3.7), Some(3.0), Some(3.6), Balanced), // Pi 4 GB
            (Some(2), Some(8.0), Some(6.0), Some(1.9), Low), // 2-vCPU VM
            (Some(8), Some(0.5), Some(0.4), Some(7.0), Low), // 512 MiB container
            (Some(8), Some(16.0), Some(0.5), Some(7.0), Low), // memory exhausted
            (Some(8), Some(7.6), Some(5.0), Some(6.5), Balanced), // laptop
            (Some(8), Some(32.0), Some(20.0), Some(7.0), Performance), // idle 8-core
            (Some(16), Some(64.0), Some(40.0), Some(13.0), Performance),
            (Some(64), Some(256.0), Some(200.0), Some(14.0), Performance), // 16 bench threads
            (Some(16), Some(64.0), Some(40.0), Some(6.0), Balanced),       // busy or shared
            (Some(8), Some(32.0), Some(20.0), Some(1.5), Low),             // over-reported quota
            (Some(12), Some(16.0), Some(6.0), Some(10.0), Balanced),       // memory in use
        ];
        for (cpus, total, avail, speedup, expected) in cases {
            let m = host(cpus, total, avail, speedup);
            assert_eq!(
                recommend(&m).profile,
                expected,
                "{cpus:?} CPUs, {total:?}/{avail:?} GiB, {speedup:?}x"
            );
        }
    }

    #[test]
    fn failed_probes_keep_default_unless_a_known_signal_says_low() {
        assert_eq!(
            recommend(&host(None, None, None, None)).profile,
            CpuUsage::Balanced
        );
        assert_eq!(
            recommend(&host(Some(8), None, None, None)).profile,
            CpuUsage::Balanced
        );
        assert_eq!(
            recommend(&host(Some(32), None, None, Some(15.0))).profile,
            CpuUsage::Balanced,
            "performance needs known, ample memory"
        );
        assert_eq!(
            recommend(&host(None, Some(1.0), None, None)).profile,
            CpuUsage::Low
        );
    }

    #[test]
    fn more_resources_never_select_a_lower_profile() {
        fn rank(p: CpuUsage) -> u8 {
            match p {
                CpuUsage::Low => 0,
                CpuUsage::Balanced => 1,
                CpuUsage::Performance => 2,
            }
        }
        let mut previous = 0;
        for (cpus, mem) in [
            (1, 1.0),
            (2, 2.0),
            (4, 4.0),
            (8, 8.0),
            (16, 32.0),
            (64, 256.0),
        ] {
            let m = host(
                Some(cpus),
                Some(mem),
                Some(mem * 0.75),
                Some(cpus as f64 * 0.9),
            );
            let r = rank(recommend(&m).profile);
            assert!(r >= previous, "{cpus} CPUs / {mem} GiB regressed");
            previous = r;
        }
    }

    #[test]
    fn benchmark_is_bounded_and_sane() {
        let cpus = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        let b = run_benchmark(cpus).expect("benchmark threads should start");
        assert_eq!(b.threads, cpus.clamp(1, MAX_BENCH_THREADS));
        assert!(b.speedup.is_finite() && b.speedup > 0.0);
        assert!((0.0..=1.0).contains(&b.efficiency()));
        // Worst case is calibration plus every round fully serialized;
        // generous slack for debug builds on loaded CI runners.
        assert!(b.elapsed_ms < 30_000, "benchmark took {} ms", b.elapsed_ms);
    }

    #[test]
    fn summary_mentions_measurements() {
        let s = host(Some(8), Some(16.0), Some(8.0), Some(6.0)).summary();
        assert_eq!(
            s,
            "8 CPUs, 16.0 GiB memory (8.0 GiB available), 6.0x speedup on 8 threads"
        );
        assert_eq!(
            host(None, None, None, None).summary(),
            "unknown CPUs, unknown memory"
        );
    }
}
