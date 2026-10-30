//! `tokensave profile ...`: opt-in performance profiling (#754).
//!
//! Every subcommand reads or changes local state only: `config.toml` and
//! `~/.tokensave/profile/`. Nothing here touches the network.

use std::path::Path;
use std::time::Duration;

use tokensave::errors::{Result, TokenSaveError};
use tokensave::profile::{self, export, report, storage, Mode, Settings};
use tokensave::user_config::{self, UserConfig};

use crate::cli::ProfileAction;

/// Detailed mode lasts this long unless `--for` says otherwise.
const DEFAULT_DETAILED_FOR: Duration = Duration::from_secs(24 * 3600);

fn config_error(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Config {
        message: message.into(),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn window(text: &str) -> Result<Duration> {
    report::parse_window(text)
        .ok_or_else(|| config_error(format!("invalid window '{text}'; use e.g. 30m, 24h or 7d")))
}

fn since_hour(window: Duration) -> u64 {
    unix_now().saturating_sub(window.as_secs()) / 3600
}

fn profile_dir() -> Result<std::path::PathBuf> {
    user_config::profile_dir().ok_or_else(|| config_error("could not locate the home directory"))
}

pub(crate) fn handle_profile_action(action: ProfileAction) -> Result<()> {
    match action {
        ProfileAction::Status => status(),
        ProfileAction::Enable { detailed, duration } => enable(detailed, duration.as_deref()),
        ProfileAction::Disable => set_mode(Mode::Off, None),
        ProfileAction::Report {
            mcp,
            database,
            indexing,
            since,
            engine,
            json,
        } => {
            let section = if mcp {
                report::Section::Mcp
            } else if database {
                report::Section::Database
            } else if indexing {
                report::Section::Indexing
            } else {
                report::Section::All
            };
            let mut merged = storage::load(&profile_dir()?, since_hour(window(&since)?));
            if let Some(engine) = engine {
                merged.retain(|name, _| *name == engine);
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report::render_json(&merged, section, &since))
                        .unwrap_or_default()
                );
            } else {
                print!("{}", report::render_text(&merged, section, &since));
            }
            Ok(())
        }
        ProfileAction::SlowQueries { since } => {
            let merged = storage::load(&profile_dir()?, since_hour(window(&since)?));
            let threshold = user_config::profiling_preferences().slow_query_ms;
            print!("{}", report::render_slow(&merged, threshold, &since));
            Ok(())
        }
        ProfileAction::Export { since, output } => {
            let window = window(&since)?;
            let merged = storage::load(&profile_dir()?, since_hour(window));
            let export = export::sanitized(
                &merged,
                window.as_secs().div_ceil(3600),
                &export::known_tools(),
            );
            let json = export::to_json(&export);
            match output {
                Some(path) => {
                    std::fs::write(&path, format!("{json}\n")).map_err(|e| {
                        config_error(format!("failed to write {}: {e}", path.display()))
                    })?;
                    eprintln!("Wrote sanitized profile export to {}", path.display());
                }
                None => println!("{json}"),
            }
            Ok(())
        }
        ProfileAction::Reset => {
            let removed = storage::reset(&profile_dir()?);
            println!("Removed {removed} profile file(s).");
            Ok(())
        }
    }
}

fn enable(detailed: bool, duration: Option<&str>) -> Result<()> {
    if !detailed {
        return set_mode(Mode::Light, None);
    }
    let lasts = match duration {
        Some(text) => window(text)?,
        None => DEFAULT_DETAILED_FOR,
    };
    let expires = unix_now().saturating_add(lasts.as_secs());
    set_mode(Mode::Detailed, Some(expires as i64))
}

fn set_mode(mode: Mode, detailed_expires_at: Option<i64>) -> Result<()> {
    let mut config = UserConfig::load();
    config.profiling.mode = mode;
    config.profiling.detailed_expires_at = detailed_expires_at;
    if !config.save() {
        return Err(config_error("failed to save ~/.tokensave/config.toml"));
    }
    match mode {
        Mode::Off => println!(
            "Profiling disabled. Recorded data is kept; `tokensave profile reset` deletes it."
        ),
        Mode::Light => println!("Profiling enabled (light)."),
        Mode::Detailed => println!(
            "Profiling enabled (detailed) until {}; it then falls back to light.",
            detailed_expires_at.map_or_else(|| "-".to_string(), |t| format!("UNIX time {t}"))
        ),
    }
    if mode != Mode::Off {
        println!(
            "Data stays on this machine under ~/.tokensave/profile/. New tokensave processes pick \
             this up; restart running MCP servers to include them."
        );
    }
    Ok(())
}

fn status() -> Result<()> {
    let prefs = user_config::profiling_preferences();
    let effective = Settings::from_environment();
    let dir = profile_dir()?;
    print!(
        "{}",
        render_status(
            &prefs,
            &effective,
            std::env::var("TOKENSAVE_PROFILE").is_ok(),
            &dir
        )
    );
    Ok(())
}

fn render_status(
    prefs: &user_config::ProfilingPreferences,
    effective: &Settings,
    env_override: bool,
    dir: &Path,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = write!(out, "Profiling: {}", effective.mode.as_str());
    if env_override {
        let _ = write!(
            out,
            " (configured: {}; overridden by TOKENSAVE_PROFILE)",
            prefs.mode.as_str()
        );
    } else if effective.mode != prefs.mode {
        let _ = write!(
            out,
            " (configured: {}; detailed mode expired)",
            prefs.mode.as_str()
        );
    }
    let _ = writeln!(out);
    if let Some(expires) = prefs.detailed_expires_at {
        let remaining = expires - unix_now() as i64;
        if remaining > 0 {
            let _ = writeln!(
                out,
                "Detailed mode expires in {}h {}m",
                remaining / 3600,
                remaining % 3600 / 60
            );
        }
    }
    let _ = writeln!(out, "Slow threshold: {} ms", prefs.slow_query_ms);

    let summary = storage::summarize(dir);
    let _ = writeln!(
        out,
        "Storage: {} — {} file(s), {:.1} KiB of {:.1} MiB limit, retention {} day(s)",
        dir.display(),
        summary.files,
        summary.bytes as f64 / 1024.0,
        prefs.max_bytes as f64 / (1024.0 * 1024.0),
        prefs.retention_days
    );
    let merged = storage::load(dir, 0);
    let (series, samples) = merged
        .values()
        .flat_map(|snapshot| snapshot.values())
        .fold((0usize, 0u64), |(n, total), s| {
            (n + 1, total.saturating_add(s.count + s.errors))
        });
    let _ = writeln!(
        out,
        "Recorded: {samples} sample(s) across {series} series from {} process(es)",
        summary.processes
    );
    let o = summary.overhead;
    let _ = writeln!(
        out,
        "Profiler overhead: {} flush(es), {:.1} ms flushing, {:.1} KiB written",
        o.flushes,
        o.flush_us as f64 / 1000.0,
        o.bytes_written as f64 / 1024.0
    );
    let _ = writeln!(
        out,
        "In-memory aggregates while enabled: {:.0} KiB per process (fixed), plus about \
         21 KiB per registered tool in MCP servers",
        profile::Recorder::new().resident_bytes() as f64 / 1024.0
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_override_without_the_value() {
        let tmp = tempfile::tempdir().unwrap();
        let prefs = user_config::ProfilingPreferences::default();
        let effective = Settings::resolve(&prefs, Some("light"), 0);
        let text = render_status(&prefs, &effective, true, tmp.path());
        assert!(
            text.starts_with("Profiling: light (configured: off; overridden by TOKENSAVE_PROFILE)")
        );
        assert!(text.contains("0 file(s)"));
        assert!(text.contains("Recorded: 0 sample(s)"));
    }
}
