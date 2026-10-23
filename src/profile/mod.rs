//! Opt-in performance profiling (#754).
//!
//! Attributes tokensave's time to database operations, MCP tool phases and
//! indexing phases from real usage. Off by default; enabled by the
//! `[profiling]` table in `~/.tokensave/config.toml` or, for one run, by
//! `TOKENSAVE_PROFILE=off|light|detailed`.
//!
//! - [`ops`]: the static operation registry. Metric keys are compile-time
//!   names, never SQL, parameters, paths or other dynamic strings.
//! - [`histogram`]: fixed-bucket, mergeable latency histograms.
//! - [`recorder`]: sharded lock-free aggregation per operation.
//! - [`context`]: spans with inclusive/exclusive time and per-invocation
//!   database attribution.
//! - [`storage`]: bounded per-process hourly files, merged at report time.
//!
//! With profiling off, [`span`] costs one relaxed atomic load: no clock
//! read, no allocation and no I/O, and no file or directory is created.
//! Recording never does I/O; a background thread owned by [`Profiler`]
//! drains the recorder every [`FLUSH_INTERVAL`] and on shutdown.
//!
//! This module never touches the network.

pub mod context;
pub mod histogram;
pub mod ops;
pub mod recorder;
pub mod storage;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::OnceLock;
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use context::{InvocationSummary, Span};
pub use ops::{Category, DbOp, IndexPhase, McpPhase, Op};
pub use recorder::{Recorder, SeriesSnapshot, Snapshot};

use crate::user_config::ProfilingPreferences;

/// What the profiler collects.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Nothing is measured or persisted.
    #[default]
    Off,
    /// Counters and histograms per operation.
    Light,
    /// Everything in `light`, plus per-invocation detail as it is added.
    /// Expires back to `light` at `detailed_expires_at`.
    Detailed,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Light => "light",
            Self::Detailed => "detailed",
        }
    }

    /// Parses a `TOKENSAVE_PROFILE` value. Boolean spellings map to `off`
    /// and `light`; anything unrecognized is `None`.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "0" | "false" | "no" | "" => Some(Self::Off),
            "light" | "on" | "1" | "true" | "yes" => Some(Self::Light),
            "detailed" => Some(Self::Detailed),
            _ => None,
        }
    }

    const fn to_u8(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Light => 1,
            Self::Detailed => 2,
        }
    }

    const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Light,
            2 => Self::Detailed,
            _ => Self::Off,
        }
    }
}

/// Effective profiler settings for this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub mode: Mode,
    pub slow_threshold: Duration,
    pub retention_days: u32,
    pub max_bytes: u64,
}

impl Settings {
    /// Resolves the configured preferences against a `TOKENSAVE_PROFILE`
    /// value. A recognized environment value wins; an unrecognized one is
    /// ignored. An expired configured `detailed` mode runs as `light`.
    pub fn resolve(prefs: &ProfilingPreferences, env: Option<&str>, now_unix: i64) -> Self {
        let configured = match (prefs.mode, prefs.detailed_expires_at) {
            (Mode::Detailed, Some(expires)) if now_unix >= expires => Mode::Light,
            (mode, _) => mode,
        };
        Self {
            mode: env.and_then(Mode::parse).unwrap_or(configured),
            slow_threshold: Duration::from_millis(prefs.slow_query_ms),
            retention_days: prefs.retention_days,
            max_bytes: prefs.max_bytes,
        }
    }

    /// Settings from `config.toml` and `TOKENSAVE_PROFILE`.
    pub fn from_environment() -> Self {
        let env = std::env::var("TOKENSAVE_PROFILE").ok();
        Self::resolve(
            &crate::user_config::profiling_preferences(),
            env.as_deref(),
            unix_now() as i64,
        )
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

static MODE: AtomicU8 = AtomicU8::new(0);
static RECORDER: OnceLock<Recorder> = OnceLock::new();

/// The current mode.
pub fn mode() -> Mode {
    Mode::from_u8(MODE.load(Ordering::Relaxed))
}

/// Whether anything is being measured.
#[inline]
pub fn enabled() -> bool {
    MODE.load(Ordering::Relaxed) != 0
}

/// Switches the mode for this process. The recorder (a fixed allocation,
/// see [`Recorder::resident_bytes`]) is created the first time profiling
/// is enabled and kept for the life of the process.
pub fn set_mode(mode: Mode) {
    if mode != Mode::Off {
        RECORDER.get_or_init(Recorder::new);
    }
    MODE.store(mode.to_u8(), Ordering::Relaxed);
}

/// The process recorder, if profiling has ever been enabled.
pub fn recorder() -> Option<&'static Recorder> {
    RECORDER.get()
}

/// Starts measuring `op`. Inert when profiling is off.
#[inline]
pub fn span(op: impl Into<Op>) -> Span {
    if !enabled() {
        return Span::inert();
    }
    match RECORDER.get() {
        Some(recorder) => Span::start(recorder, op.into()),
        None => Span::inert(),
    }
}

/// Runs `fut` as one invocation (an MCP call, a sync run): spans inside it
/// form their own context, and the returned summary holds its database
/// attribution. With profiling off, `fut` runs unwrapped.
pub async fn invocation<F: std::future::Future>(fut: F) -> (F::Output, InvocationSummary) {
    if enabled() {
        context::scoped(fut).await
    } else {
        (fut.await, InvocationSummary::default())
    }
}

/// Synchronous counterpart of [`invocation`] on the current thread.
pub fn invocation_sync<R>(f: impl FnOnce() -> R) -> (R, InvocationSummary) {
    if enabled() {
        context::scoped_sync(f)
    } else {
        (f(), InvocationSummary::default())
    }
}

/// Serializes tests, across modules, that change the process-wide mode.
#[cfg(test)]
pub(crate) static TEST_MODE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// How often the background thread flushes aggregates to disk.
pub const FLUSH_INTERVAL: Duration = Duration::from_secs(60);

/// Owns the background flush thread. Call [`Profiler::shutdown`] for the
/// final flush; dropping the handle does the same as a fallback.
pub struct Profiler {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Profiler {
    /// Applies `settings` and, unless the mode is off, starts the flush
    /// thread writing to `dir`. Returns `None` (and creates nothing) when
    /// profiling is off.
    pub fn start(settings: &Settings, dir: PathBuf, interval: Duration) -> Option<Self> {
        set_mode(settings.mode);
        if settings.mode == Mode::Off {
            return None;
        }
        let recorder = RECORDER.get_or_init(Recorder::new);
        recorder.set_slow_threshold_us(
            u64::try_from(settings.slow_threshold.as_micros()).unwrap_or(u64::MAX),
        );
        let mut store = storage::Store::new(dir, settings.retention_days, settings.max_bytes);
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("tokensave-profile-flush".into())
            .spawn(move || loop {
                let last = !matches!(
                    stopped.recv_timeout(interval),
                    Err(RecvTimeoutError::Timeout)
                );
                // Best-effort: profiling must never break the workload.
                let _ = store.flush(&recorder.drain(), unix_now());
                if last {
                    break;
                }
            })
            .ok()?;
        Some(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// [`Profiler::start`] with settings from `config.toml` and
    /// `TOKENSAVE_PROFILE`, writing to `~/.tokensave/profile/`.
    pub fn start_from_environment() -> Option<Self> {
        let settings = Settings::from_environment();
        if settings.mode == Mode::Off {
            set_mode(Mode::Off);
            return None;
        }
        Self::start(
            &settings,
            crate::user_config::profile_dir()?,
            FLUSH_INTERVAL,
        )
    }

    /// Stops the flush thread after a final flush and waits for it.
    pub fn shutdown(mut self) {
        self.stop_and_join();
    }

    fn stop_and_join(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn prefs(mode: Mode) -> ProfilingPreferences {
        ProfilingPreferences {
            mode,
            ..ProfilingPreferences::default()
        }
    }

    #[test]
    fn default_is_off() {
        let s = Settings::resolve(&ProfilingPreferences::default(), None, 0);
        assert_eq!(s.mode, Mode::Off);
        assert_eq!(s.slow_threshold, Duration::from_millis(50));
    }

    #[test]
    fn env_overrides_config_in_both_directions() {
        let light = prefs(Mode::Light);
        assert_eq!(Settings::resolve(&light, Some("off"), 0).mode, Mode::Off);
        assert_eq!(Settings::resolve(&light, Some("0"), 0).mode, Mode::Off);
        let off = prefs(Mode::Off);
        assert_eq!(Settings::resolve(&off, Some("1"), 0).mode, Mode::Light);
        assert_eq!(
            Settings::resolve(&off, Some(" Detailed "), 0).mode,
            Mode::Detailed
        );
        assert_eq!(
            Settings::resolve(&light, Some("turbo"), 0).mode,
            Mode::Light,
            "an unrecognized value falls back to the config"
        );
    }

    #[test]
    fn detailed_expires_to_light() {
        let mut p = prefs(Mode::Detailed);
        p.detailed_expires_at = Some(100);
        assert_eq!(Settings::resolve(&p, None, 99).mode, Mode::Detailed);
        assert_eq!(Settings::resolve(&p, None, 100).mode, Mode::Light);
        assert_eq!(
            Settings::resolve(&p, Some("detailed"), 100).mode,
            Mode::Detailed,
            "an explicit per-run choice is not subject to the config expiry"
        );
    }

    #[test]
    fn off_creates_nothing_and_spans_are_inert() {
        let _g = TEST_MODE_LOCK.blocking_lock();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let settings = Settings::resolve(&ProfilingPreferences::default(), None, 0);
        assert!(Profiler::start(&settings, dir.clone(), Duration::from_millis(1)).is_none());
        assert!(!enabled());
        let s = span(DbOp::Open);
        assert!(!s.is_active());
        s.finish();
        let (out, summary) = invocation_sync(|| 7);
        assert_eq!((out, summary), (7, InvocationSummary::default()));
        assert!(!dir.exists());
    }

    #[test]
    fn shutdown_flushes_recorded_spans() {
        let _g = TEST_MODE_LOCK.blocking_lock();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let settings = Settings::resolve(&prefs(Mode::Light), None, 0);
        // A long interval: only the shutdown flush can write the file.
        let profiler = Profiler::start(&settings, dir.clone(), Duration::from_secs(3600)).unwrap();
        assert_eq!(mode(), Mode::Light);
        let (_, summary) = invocation_sync(|| {
            let mut s = span(DbOp::Transaction);
            s.add_rows(0, 4);
            s.finish();
        });
        assert_eq!(summary.db_ops, 1);
        profiler.shutdown();
        set_mode(Mode::Off);
        let merged = storage::load(&dir, 0);
        let s = &merged[storage::ENGINE]["db.transaction"];
        assert!(s.count >= 1);
        assert!(s.rows_affected >= 4);
        assert!(storage::overhead().flushes >= 1);
    }

    #[test]
    fn profile_sources_do_not_reach_the_network() {
        let sources = [
            include_str!("mod.rs"),
            include_str!("ops.rs"),
            include_str!("histogram.rs"),
            include_str!("recorder.rs"),
            include_str!("context.rs"),
            include_str!("storage.rs"),
        ];
        // Built with concat! so this test's own source does not match.
        let forbidden = [
            concat!("ur", "eq"),
            concat!("crate::", "cloud"),
            concat!("std::net", "::"),
            concat!("Tcp", "Stream"),
        ];
        for source in sources {
            for needle in forbidden {
                assert!(!source.contains(needle), "profile code uses {needle}");
            }
        }
    }
}
