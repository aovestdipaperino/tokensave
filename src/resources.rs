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
//!
//! `auto` never exceeds the `balanced` profiles: measurements showed no
//! consistent gain from more workers or parsers (see the user guide), so the
//! policy only lowers limits on constrained hosts.
use std::sync::OnceLock;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Share of memory capacity budgeted for concurrent parsers.
const PARSER_BUDGET_PERCENT: u64 = 15;
/// Estimated memory per live parser or extraction subprocess.
const PARSER_ESTIMATE: u64 = 128 * MIB;
/// The `balanced` parser limit: the ceiling, and the value for unknown memory.
const BALANCED_PARSERS: usize = 2;

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
    // sysinfo reads only the root cgroup's limit (enough inside a container
    // namespace); a nested cgroup such as a systemd scope needs the walk.
    let cgroup = [
        system.cgroup_limits().map(|limits| limits.total_memory),
        nested_cgroup_memory_limit(),
    ]
    .into_iter()
    .flatten()
    .filter(|&l| l > 0)
    .min();
    capacity(system.total_memory(), cgroup)
}

/// Strictest cgroup v2 `memory.max` from this process's cgroup up to the root.
#[cfg(target_os = "linux")]
fn nested_cgroup_memory_limit() -> Option<u64> {
    let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    strictest_memory_max(&membership, |dir| {
        std::fs::read_to_string(format!("/sys/fs/cgroup{dir}/memory.max")).ok()
    })
}

#[cfg(not(target_os = "linux"))]
fn nested_cgroup_memory_limit() -> Option<u64> {
    None
}

/// Parses the unified (`0::`) entry of `/proc/self/cgroup` and returns the
/// smallest numeric `memory.max` along the path; `max` means unlimited.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn strictest_memory_max(membership: &str, read: impl Fn(&str) -> Option<String>) -> Option<u64> {
    let path = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?
        .trim_end_matches('/');
    let mut dir = path;
    let mut strictest: Option<u64> = None;
    loop {
        if let Some(limit) = read(dir).and_then(|v| v.trim().parse::<u64>().ok()) {
            strictest = Some(strictest.map_or(limit, |s| s.min(limit)));
        }
        match dir.rfind('/') {
            Some(i) if !dir.is_empty() => dir = &dir[..i],
            _ => break,
        }
    }
    strictest
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
        // `balanced` is half the CPUs; constrained tiers cap it further.
        let half = cpus.div_ceil(2);
        let cpu_workers = match tier {
            Tier::Minimal => 1,
            Tier::Low => half.min(2),
            Tier::Balanced => half.min(4),
            Tier::High | Tier::Performance => half,
        };
        let parser_workers = hardware.memory_bytes.map_or(BALANCED_PARSERS, |bytes| {
            let budget = bytes / 100 * PARSER_BUDGET_PERCENT;
            usize::try_from(budget / PARSER_ESTIMATE)
                .unwrap_or(usize::MAX)
                .clamp(1, BALANCED_PARSERS)
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
            ("two-core VM", Some(2), mib(4096), Tier::Minimal, 1, 2),
            ("Raspberry Pi 4 1GB", Some(4), mib(906), Tier::Minimal, 1, 1),
            ("Raspberry Pi 4 4GB", Some(4), mib(3796), Tier::Low, 2, 2),
            ("4-core laptop 8GB", Some(4), mib(7782), Tier::Low, 2, 2),
            (
                "8-core dev box 16GB",
                Some(8),
                mib(15_884),
                Tier::High,
                4,
                2,
            ),
            (
                "12 CPUs, 32GB",
                Some(12),
                mib(32_000),
                Tier::Performance,
                6,
                2,
            ),
            (
                "64-core workstation",
                Some(64),
                mib(262_144),
                Tier::Performance,
                32,
                2,
            ),
            ("32 CPUs, 6GB", Some(32), mib(6144), Tier::Balanced, 4, 2),
            // Containers: quotas and limits arrive already applied.
            (
                "container 2 CPU/512MiB",
                Some(2),
                mib(512),
                Tier::Minimal,
                1,
                1,
            ),
            (
                "container 16 CPU/3GiB",
                Some(16),
                mib(3072),
                Tier::Low,
                2,
                2,
            ),
            (
                "affinity 3 of 32 CPUs",
                Some(3),
                mib(65_536),
                Tier::Low,
                2,
                2,
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
    fn nested_cgroup_limits_use_the_strictest_ancestor() {
        let files = |dir: &str| match dir {
            "/user.slice/app.scope" => Some("1073741824\n".to_string()),
            "/user.slice" => Some("max\n".to_string()),
            "" => None,
            _ => Some("4294967296".to_string()),
        };
        let membership = "1:name=systemd:/x\n0::/user.slice/app.scope\n";
        assert_eq!(strictest_memory_max(membership, files), Some(GIB));
        // Parent stricter than the child.
        let parent = |dir: &str| (dir == "/a").then(|| "536870912".to_string());
        assert_eq!(strictest_memory_max("0::/a/b/c\n", parent), Some(512 * MIB));
        // Unlimited everywhere, root cgroup, cgroup v1 only.
        assert_eq!(
            strictest_memory_max("0::/a\n", |_| Some("max".into())),
            None
        );
        assert_eq!(strictest_memory_max("0::/\n", |_| None), None);
        assert_eq!(
            strictest_memory_max("3:memory:/a\n", |_| Some("1".into())),
            None
        );
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
                // Never above the `balanced` profiles.
                assert!((1..=cpus.div_ceil(2)).contains(&plan.cpu_workers));
                assert!((1..=BALANCED_PARSERS).contains(&plan.parser_workers));
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
        assert_eq!(unknown.parser_workers, BALANCED_PARSERS);
        assert_eq!(unknown.sqlite_cache_bytes, None);
        let cpus_only = Plan::for_hardware(Hardware {
            cpus: Some(8),
            memory_bytes: None,
        });
        assert_eq!(cpus_only.tier, Tier::High);
        assert_eq!(cpus_only.cpu_workers, 4);
    }

    #[test]
    fn large_hosts_match_the_balanced_profiles() {
        use crate::{cpu::CpuUsage, memory::MemoryUsage};
        for cpus in [8, 12, 64, 256] {
            let plan = Plan::for_hardware(hw(cpus, 4096));
            assert_eq!(plan.cpu_workers, CpuUsage::Balanced.workers_for(cpus));
            assert_eq!(
                plan.parser_workers,
                MemoryUsage::Balanced.workers(plan.cpu_workers)
            );
        }
    }
}
