//! Project memory policy for extraction scratch space and parser processes.
//!
//! This bounds simultaneous parsers, not total RSS. Completed extraction data
//! and the resolver graph remain input-proportional and owned by the indexing
//! request. Memory is sampled once before extraction; there is no background
//! task, persistent probe cache or process-global allocator setting.
use serde::{Deserialize, Serialize};

/// Parser-memory tradeoff selected at project open, independently of CPU usage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryUsage {
    /// Parsers budgeted from detected memory capacity (default); see
    /// [`crate::resources::Plan`].
    #[default]
    Auto,
    /// At most one live parser or extraction subprocess.
    Low,
    /// At most two live parsers or extraction subprocesses.
    Balanced,
    /// No additional limit beyond the CPU policy and pending files.
    Performance,
}

impl MemoryUsage {
    /// Reduce a CPU worker ceiling without ever raising it.
    pub fn workers(self, cpu_workers: usize) -> usize {
        cpu_workers.min(match self {
            Self::Auto => crate::resources::Plan::current().parser_workers,
            Self::Low => 1,
            Self::Balanced => 2,
            Self::Performance => usize::MAX,
        })
    }
}

/// Estimated parser scratch/process allowance, not a per-parser allocation cap.
const PARSER_ALLOWANCE: u64 = 128 * 1024 * 1024;

/// Sample physical RAM only; no process scan, CPU refresh, swap or sleeps.
/// sysinfo exposes visible cgroup v1/v2 limits on Linux when readable.
pub(crate) fn available_memory_bytes() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
    // sysinfo's cgroup reader requires a successful host-memory refresh.
    if system.total_memory() == 0 {
        return None;
    }
    headroom(
        system.total_memory(),
        system.available_memory(),
        system.cgroup_limits().map(|limits| limits.free_memory),
    )
}

fn headroom(total: u64, available: u64, cgroup_free: Option<u64>) -> Option<u64> {
    (total > 0).then(|| {
        cgroup_free
            .map_or(available, |free| free.min(available))
            .min(total)
    })
}

/// Reserve half the reported headroom for parent/other activity; budget the
/// remaining half in estimated 128 MiB parser slots. Always allow one parser
/// for progress, even at zero headroom. Unknown probes preserve the ceiling.
/// This snapshot cannot prevent later pressure, large-file or graph OOMs.
pub(crate) fn workers_for_headroom(requested: usize, available: Option<u64>) -> usize {
    let Some(available) = available else {
        return requested;
    };
    let affordable = usize::try_from(available / 2 / PARSER_ALLOWANCE).unwrap_or(usize::MAX);
    requested.min(affordable.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_limits_only_lower_cpu_limits() {
        for cpu in [0, 1, 2, 3, 8, usize::MAX] {
            assert_eq!(MemoryUsage::Low.workers(cpu), cpu.min(1));
            assert_eq!(MemoryUsage::Balanced.workers(cpu), cpu.min(2));
            assert_eq!(MemoryUsage::Performance.workers(cpu), cpu);
            assert!(MemoryUsage::Auto.workers(cpu) <= cpu);
        }
    }

    #[test]
    fn pressure_budget_boundaries_preserve_forward_progress() {
        let slot = 2 * PARSER_ALLOWANCE;
        for requested in [0, 1, 2, 8, usize::MAX] {
            assert_eq!(workers_for_headroom(requested, None), requested);
            for available in [0, 1, slot - 1, slot, 2 * slot - 1] {
                assert_eq!(
                    workers_for_headroom(requested, Some(available)),
                    requested.min(1)
                );
            }
            assert_eq!(
                workers_for_headroom(requested, Some(2 * slot)),
                requested.min(2)
            );
            assert!(workers_for_headroom(requested, Some(u64::MAX)) <= requested);
        }
        assert_eq!(workers_for_headroom(8, Some(8 * slot)), 8);
    }

    #[test]
    fn pressure_never_raises_profile_or_cpu_ceiling() {
        for profile in [
            MemoryUsage::Auto,
            MemoryUsage::Low,
            MemoryUsage::Balanced,
            MemoryUsage::Performance,
        ] {
            for cpu in [0, 1, 2, 8] {
                let requested = profile.workers(cpu);
                for available in [None, Some(0), Some(4 * PARSER_ALLOWANCE), Some(u64::MAX)] {
                    let result = workers_for_headroom(requested, available);
                    assert!(result <= requested);
                    assert_eq!(result == 0, requested == 0);
                }
            }
        }
    }

    #[test]
    fn host_and_container_headroom_use_the_stricter_bound() {
        assert_eq!(headroom(1000, 800, None), Some(800));
        assert_eq!(headroom(1000, 800, Some(200)), Some(200));
        assert_eq!(headroom(1000, 800, Some(900)), Some(800));
        assert_eq!(headroom(1000, 800, Some(0)), Some(0));
        assert_eq!(headroom(1000, 1200, None), Some(1000));
        assert_eq!(headroom(0, 0, None), None);
    }
}
