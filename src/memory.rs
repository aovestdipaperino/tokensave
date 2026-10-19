//! Project memory policy for extraction scratch space and parser processes.
//!
//! This bounds simultaneous parsers, not total RSS. Completed extraction data
//! and the resolver graph remain input-proportional and owned by the indexing
//! request. No memory probe, background task or process-global allocator setting.
use serde::{Deserialize, Serialize};

/// Parser-memory tradeoff selected at project open, independently of CPU usage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryUsage {
    /// At most one live parser or extraction subprocess.
    Low,
    /// At most two live parsers or extraction subprocesses.
    #[default]
    Balanced,
    /// No additional limit beyond the CPU policy and pending files.
    Performance,
}

impl MemoryUsage {
    /// Reduce a CPU worker ceiling without ever raising it.
    pub fn workers(self, cpu_workers: usize) -> usize {
        cpu_workers.min(match self {
            Self::Low => 1,
            Self::Balanced => 2,
            Self::Performance => usize::MAX,
        })
    }
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
        }
    }
}
