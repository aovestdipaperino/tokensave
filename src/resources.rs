//! Automatic resource policy derived from the host's effective capacity.
//!
//! `auto` CPU and memory profiles resolve to limits computed here when they are
//! first needed in a process. Nothing machine-specific is written to project
//! config, so a project moved to another machine adapts on its next run.
//!
//! Capacity is read once per process: effective CPUs from the standard library
//! (Linux affinity and cgroup CPU quotas included) and memory capacity from
//! sysinfo (host RAM, or a stricter visible cgroup limit). Short-term pressure
//! is handled separately by the per-extraction headroom check in
//! [`crate::memory`]. There is no background monitor or polling.
use std::sync::OnceLock;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Share of memory capacity budgeted for concurrent parsers.
const PARSER_BUDGET_PERCENT: u64 = 15;
/// Absolute ceiling for the parser budget, however large the host.
const MAX_PARSER_BUDGET: u64 = 4 * GIB;
/// Estimated memory per live parser or extraction subprocess.
const PARSER_ESTIMATE: u64 = 128 * MIB;
/// Parser limit when memory capacity is unknown (the fixed `balanced` limit).
const UNKNOWN_MEMORY_PARSERS: usize = 2;
/// Hard ceiling on automatic CPU workers, however many CPUs are visible.
const MAX_AUTO_WORKERS: usize = 16;

/// Effective capacity visible to this process. `None` means the probe failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hardware {
    /// CPUs this process may use (affinity and container quotas applied).
    pub cpus: Option<usize>,
    /// Physical RAM, or a stricter visible container memory limit, in bytes.
    pub memory_bytes: Option<u64>,
}

impl Hardware {
    /// Probe the host. Never fails; unavailable values become `None`.
    pub fn detect() -> Self {
        Self {
            cpus: std::thread::available_parallelism()
                .ok()
                .map(std::num::NonZeroUsize::get),
            memory_bytes: memory_capacity(),
        }
    }

    /// The process-wide snapshot, probed on first use.
    pub fn current() -> Self {
        static CURRENT: OnceLock<Hardware> = OnceLock::new();
        *CURRENT.get_or_init(Self::detect)
    }
}

/// Reads physical RAM only; no process scan, CPU refresh or swap.
fn memory_capacity() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
    capacity(
        system.total_memory(),
        system.cgroup_limits().map(|limits| limits.total_memory),
    )
}

/// The stricter of host RAM and a nonzero container limit.
fn capacity(total: u64, cgroup_total: Option<u64>) -> Option<u64> {
    (total > 0).then(|| {
        cgroup_total
            .filter(|&l| l > 0)
            .map_or(total, |l| l.min(total))
    })
}

/// Capacity class. Ordering runs from most to least restrictive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Minimal,
    Low,
    Balanced,
    High,
    Performance,
}

impl Tier {
    fn from_cpus(cpus: usize) -> Self {
        match cpus {
            0..=2 => Self::Minimal,
            3..=4 => Self::Low,
            5..=7 => Self::Balanced,
            8..=11 => Self::High,
            _ => Self::Performance,
        }
    }

    fn from_memory(bytes: u64) -> Self {
        match bytes {
            b if b < 2 * GIB => Self::Minimal,
            b if b < 4 * GIB => Self::Low,
            b if b < 8 * GIB => Self::Balanced,
            b if b <= 16 * GIB => Self::High,
            _ => Self::Performance,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Balanced => "balanced",
            Self::High => "high",
            Self::Performance => "performance",
        }
    }
}

/// Limits for `auto` profiles. Explicit profiles ignore this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub tier: Tier,
    /// Ceiling for parallel indexing phases (hashing, extraction, resolution).
    pub cpu_workers: usize,
    /// Ceiling for simultaneous parsers, from the memory budget.
    pub parser_workers: usize,
    /// Upper bound for one `SQLite` connection's page cache, in bytes.
    pub sqlite_cache_bytes: Option<u64>,
    /// Upper bound for one `SQLite` connection's memory map, in bytes.
    pub sqlite_mmap_bytes: Option<u64>,
}

impl Plan {
    /// Pure policy, deterministic for a given capacity.
    ///
    /// The tier is the stricter of the CPU and memory classes; unknown memory
    /// does not restrict the tier. An unknown CPU count is treated as one CPU,
    /// matching the profiles' existing fallback.
    pub fn for_hardware(hardware: Hardware) -> Self {
        let cpus = hardware.cpus.unwrap_or(1).max(1);
        let cpu_tier = Tier::from_cpus(cpus);
        let tier = hardware
            .memory_bytes
            .map_or(cpu_tier, |bytes| cpu_tier.min(Tier::from_memory(bytes)));
        let half = cpus.div_ceil(2);
        // Leave at least half the CPUs (a quarter at the top tier) for others.
        let cpu_workers = match tier {
            Tier::Minimal => 1,
            Tier::Low => half.min(2),
            Tier::Balanced => half.min(4),
            Tier::High => half.min(8),
            Tier::Performance => (cpus * 3 / 4).max(half),
        }
        .clamp(1, MAX_AUTO_WORKERS);
        let parser_workers = hardware
            .memory_bytes
            .map_or(UNKNOWN_MEMORY_PARSERS, |bytes| {
                let budget = (bytes / 100 * PARSER_BUDGET_PERCENT).min(MAX_PARSER_BUDGET);
                usize::try_from(budget / PARSER_ESTIMATE)
                    .unwrap_or(usize::MAX)
                    .max(1)
            });
        // Below 4 GiB, cap SQLite's per-connection cache at 1/64 and its
        // memory map at 1/16 of capacity; larger hosts keep the DB-size policy.
        let sqlite_cache_bytes = hardware.memory_bytes.map(|bytes| (bytes / 64).max(2 * MIB));
        let sqlite_mmap_bytes = hardware.memory_bytes.map(|bytes| bytes / 16);
        Self {
            tier,
            cpu_workers,
            parser_workers,
            sqlite_cache_bytes,
            sqlite_mmap_bytes,
        }
    }

    /// The plan for this process's capacity snapshot.
    pub fn current() -> Self {
        Self::for_hardware(Hardware::current())
    }
}

/// Concise multi-line description of the effective limits for a project's
/// profiles, shown after `init`. Nothing here is persisted.
pub fn summary(cpu: crate::cpu::CpuUsage, memory: crate::memory::MemoryUsage) -> String {
    use std::fmt::Write as _;
    let hardware = Hardware::current();
    let plan = Plan::for_hardware(hardware);
    let workers = cpu.workers();
    let parsers = memory.workers(workers);
    let (cache_kb, _) = crate::db::capped_cache_sizes(u64::MAX, &plan);
    let cpus = hardware
        .cpus
        .map_or_else(|| "unknown".to_string(), |n| n.to_string());
    let ram = hardware
        .memory_bytes
        .map_or_else(|| "unknown".to_string(), format_bytes);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Resources: {cpus} CPUs, {ram} RAM (capacity tier: {})",
        plan.tier.name()
    );
    let _ = writeln!(
        out,
        "  index workers: {workers} (cpu_usage: {})",
        profile_name(&cpu)
    );
    let _ = writeln!(
        out,
        "  parsers:       {parsers} (memory_usage: {})",
        profile_name(&memory)
    );
    let _ = write!(
        out,
        "  SQLite cache:  up to {} per connection",
        format_bytes(cache_kb * 1024)
    );
    out
}

fn profile_name(profile: &impl serde::Serialize) -> String {
    serde_json::to_value(profile)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Human-readable byte count for summaries (`512 MiB`, `7.6 GiB`).
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= GIB {
        #[allow(clippy::cast_precision_loss)]
        let gib = bytes as f64 / GIB as f64;
        format!("{gib:.1} GiB")
    } else {
        format!("{} MiB", bytes / MIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(cpus: usize, gib: u64) -> Hardware {
        Hardware {
            cpus: Some(cpus),
            memory_bytes: Some(gib * GIB),
        }
    }

    #[test]
    fn representative_hosts() {
        let mib = |m: u64| Some(m * MIB);
        // (name, cpus, memory, tier, cpu workers, parsers)
        let cases = [
            ("single core VM", Some(1), mib(1024), Tier::Minimal, 1, 1),
            ("two-core VM", Some(2), mib(4096), Tier::Minimal, 1, 4),
            (
                "Raspberry Pi 4 (1 GB)",
                Some(4),
                mib(906),
                Tier::Minimal,
                1,
                1,
            ),
            ("Raspberry Pi 4 (4 GB)", Some(4), mib(3796), Tier::Low, 2, 4),
            ("4-core laptop, 8 GB", Some(4), mib(7782), Tier::Low, 2, 9),
            (
                "8-core dev box, 16 GB",
                Some(8),
                mib(15_884),
                Tier::High,
                4,
                18,
            ),
            (
                "12-core, 32 GB",
                Some(12),
                mib(32_000),
                Tier::Performance,
                9,
                32,
            ),
            (
                "64-core workstation",
                Some(64),
                mib(262_144),
                Tier::Performance,
                16,
                32,
            ),
            // Containers: quotas and limits arrive already applied.
            (
                "container 2 CPU / 512 MiB",
                Some(2),
                mib(512),
                Tier::Minimal,
                1,
                1,
            ),
            (
                "container 16 CPU / 3 GiB",
                Some(16),
                mib(3072),
                Tier::Low,
                2,
                3,
            ),
            (
                "affinity 3 of 32 CPUs, 64 GB",
                Some(3),
                mib(65_536),
                Tier::Low,
                2,
                32,
            ),
            ("probes unavailable", None, None, Tier::Minimal, 1, 2),
        ];
        for (name, cpus, memory_bytes, tier, workers, parsers) in cases {
            let plan = Plan::for_hardware(Hardware { cpus, memory_bytes });
            assert_eq!(
                (plan.tier, plan.cpu_workers, plan.parser_workers),
                (tier, workers, parsers),
                "{name}"
            );
        }
    }

    #[test]
    fn container_limit_is_used_only_when_stricter_and_nonzero() {
        assert_eq!(capacity(8 * GIB, None), Some(8 * GIB));
        assert_eq!(capacity(8 * GIB, Some(GIB)), Some(GIB));
        assert_eq!(capacity(8 * GIB, Some(u64::MAX)), Some(8 * GIB));
        assert_eq!(capacity(8 * GIB, Some(0)), Some(8 * GIB));
        assert_eq!(capacity(0, Some(GIB)), None);
    }

    #[test]
    fn stricter_dimension_selects_the_tier() {
        assert_eq!(Plan::for_hardware(hw(32, 1)).tier, Tier::Minimal);
        assert_eq!(Plan::for_hardware(hw(2, 64)).tier, Tier::Minimal);
        assert_eq!(Plan::for_hardware(hw(16, 3)).tier, Tier::Low);
        assert_eq!(Plan::for_hardware(hw(8, 16)).tier, Tier::High);
        assert_eq!(Plan::for_hardware(hw(16, 64)).tier, Tier::Performance);
    }

    #[test]
    fn limits_are_monotonic_and_bounded() {
        for cpus in 1..=256 {
            for gib in [0, 1, 2, 4, 8, 16, 32, 1024] {
                let plan = Plan::for_hardware(hw(cpus, gib));
                assert!((1..=MAX_AUTO_WORKERS.min(cpus)).contains(&plan.cpu_workers));
                assert!(plan.parser_workers >= 1);
                let more_cpus = Plan::for_hardware(hw(cpus + 1, gib));
                assert!(more_cpus.cpu_workers >= plan.cpu_workers);
                let more_ram = Plan::for_hardware(hw(cpus, gib * 2 + 1));
                assert!(more_ram.cpu_workers >= plan.cpu_workers);
                assert!(more_ram.parser_workers >= plan.parser_workers);
            }
        }
    }

    #[test]
    fn unknown_probes_degrade_to_previous_defaults() {
        let unknown = Plan::for_hardware(Hardware {
            cpus: None,
            memory_bytes: None,
        });
        assert_eq!(unknown.cpu_workers, 1);
        assert_eq!(unknown.parser_workers, UNKNOWN_MEMORY_PARSERS);
        assert_eq!(unknown.sqlite_cache_bytes, None);
        let cpus_only = Plan::for_hardware(Hardware {
            cpus: Some(8),
            memory_bytes: None,
        });
        assert_eq!(cpus_only.tier, Tier::High);
        assert_eq!(cpus_only.cpu_workers, 4);
    }

    #[test]
    fn parser_budget_has_an_absolute_cap() {
        let huge = Plan::for_hardware(hw(64, 4096));
        assert_eq!(
            huge.parser_workers,
            usize::try_from(MAX_PARSER_BUDGET / PARSER_ESTIMATE).unwrap_or(0)
        );
        assert_eq!(huge.cpu_workers, MAX_AUTO_WORKERS);
    }
}
