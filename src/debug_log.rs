//! Opt-in stderr diagnostics. Call sites supply allowlisted fields only.
use std::io::Write;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[repr(u8)]
pub enum Level {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Off as u8);
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

tokio::task_local! {
    static REQUEST: u64;
}

pub fn configure(debug: bool, cli_level: Option<Level>) {
    use clap::ValueEnum;
    let (level, source) = if let Some(level) = cli_level {
        (level, "flag")
    } else if debug {
        (Level::Debug, "flag")
    } else {
        match std::env::var_os("TOKENSAVE_LOG") {
            Some(value) => match value.to_str().and_then(|s| Level::from_str(s, true).ok()) {
                Some(level) => (level, "environment"),
                None => {
                    let _ = writeln!(
                        std::io::stderr().lock(),
                        "[tokensave] invalid TOKENSAVE_LOG; expected off, error, warn, info, debug or trace"
                    );
                    (Level::Off, "default")
                }
            },
            None => (Level::Off, "default"),
        }
    };
    LEVEL.store(level as u8, Ordering::Relaxed);
    event(
        Level::Info,
        "config",
        "log_level",
        || serde_json::json!({ "level": level.name(), "source": source }),
    );
}

pub fn enabled(level: Level) -> bool {
    level != Level::Off && LEVEL.load(Ordering::Relaxed) >= level as u8
}

pub fn event(
    level: Level,
    subsystem: &'static str,
    operation: &'static str,
    fields: impl FnOnce() -> Value,
) {
    if !enabled(level) {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let event = serde_json::json!({
        "timestamp_unix_ms": timestamp,
        "level": level.name(),
        "subsystem": subsystem,
        "operation": operation,
        "pid": std::process::id(),
        "request_id": REQUEST.try_with(|id| *id).unwrap_or(0),
        "fields": fields(),
    });
    // A closed stderr pipe must never change the operation's result.
    let _ = writeln!(std::io::stderr().lock(), "{event}");
}

pub(crate) async fn request<T>(
    task: impl std::future::Future<Output = T>,
    report: impl FnOnce(&T, u128),
) -> T {
    if !enabled(Level::Error) {
        return task.await;
    }
    let id = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed);
    REQUEST
        .scope(id, async {
            let start = Instant::now();
            let result = task.await;
            report(&result, start.elapsed().as_micros());
            result
        })
        .await
}
