//! Project CPU policy. Limits parallel work, not CPU percentages.
//!
//! Pools are lazy and shared by profile across projects, so opening another
//! project does not create another set of indexing threads. Interactive work
//! continues on its existing runtime; no process-global Rayon pool is changed.
use std::sync::OnceLock;

use rayon::{ThreadPool, ThreadPoolBuilder};
use serde::{Deserialize, Serialize};

use crate::errors::{Result, TokenSaveError};

/// Concurrency policy selected in `.tokensave/config.json` at project open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum CpuUsage {
    /// One worker, leaving parallel capacity to other workloads.
    Low,
    /// Half the available CPUs, rounded up.
    #[default]
    Balanced,
    /// All available CPUs, without artificial delays.
    Performance,
}

static LOW: OnceLock<std::result::Result<ThreadPool, String>> = OnceLock::new();
static BALANCED: OnceLock<std::result::Result<ThreadPool, String>> = OnceLock::new();
static PERFORMANCE: OnceLock<std::result::Result<ThreadPool, String>> = OnceLock::new();

impl CpuUsage {
    /// Name as written in `.tokensave/config.json`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }

    /// Pure policy calculation, also defined for a failed/zero CPU probe.
    pub fn workers_for(self, available: usize) -> usize {
        let available = available.max(1);
        match self {
            Self::Low => 1,
            Self::Balanced => available.div_ceil(2),
            Self::Performance => available,
        }
    }

    /// Worker limit using the OS's available parallelism, with a safe fallback.
    pub fn workers(self) -> usize {
        self.workers_for(
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        )
    }

    /// Execute a synchronous parallel phase in the shared profile pool.
    /// Nested Rayon operations inherit this pool. Never hold its context across
    /// an async suspension or change the global Rayon pool used by callers.
    pub(crate) fn run<T: Send>(self, work: impl FnOnce() -> T + Send) -> Result<T> {
        let slot = match self {
            Self::Low => &LOW,
            Self::Balanced => &BALANCED,
            Self::Performance => &PERFORMANCE,
        };
        let pool = slot.get_or_init(|| {
            ThreadPoolBuilder::new()
                .num_threads(self.workers())
                .thread_name(|i| format!("tokensave-cpu-{i}"))
                .build()
                .map_err(|e| e.to_string())
        });
        match pool {
            Ok(pool) => Ok(pool.install(work)),
            Err(message) => Err(TokenSaveError::Config {
                message: format!("could not create {self:?} CPU pool: {message}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_adapt_to_available_parallelism() {
        for available in [0, 1, 2, 3, 8, 64, usize::MAX] {
            let low = CpuUsage::Low.workers_for(available);
            let balanced = CpuUsage::Balanced.workers_for(available);
            let performance = CpuUsage::Performance.workers_for(available);
            assert_eq!(low, 1);
            assert!(low <= balanced && balanced <= performance);
            assert_eq!(performance, available.max(1));
        }
        assert_eq!(CpuUsage::Balanced.workers_for(8), 4);
        assert_eq!(CpuUsage::Balanced.workers_for(3), 2);
    }

    #[test]
    fn nested_parallel_work_uses_selected_pool() -> Result<()> {
        use rayon::iter::{IntoParallelIterator, ParallelIterator};
        for profile in [CpuUsage::Low, CpuUsage::Balanced, CpuUsage::Performance] {
            let counts = profile.run(|| {
                (0..128)
                    .into_par_iter()
                    .map(|_| rayon::current_num_threads())
                    .collect::<Vec<_>>()
            })?;
            assert!(counts.iter().all(|&n| n == profile.workers()));
        }
        Ok(())
    }
}
