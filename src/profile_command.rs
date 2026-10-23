//! `tokensave profile ...`: opt-in performance profiling (#754).
//!
//! Subcommands read or change local state only. Uploads, when approved and
//! separately consented, run on the profiler flush thread.

use std::path::Path;
use std::time::Duration;

use tokensave::errors::{Result, TokenSaveError};
use tokensave::profile::local::Scope;
use tokensave::profile::{self, analysis, export, report, storage, Mode, Settings};
use tokensave::user_config;

use crate::cli::{ProfileAction, ProfileShareAction};
use tokensave::profile::sharing;

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

pub(crate) fn handle_profile_action(action: ProfileAction, local: bool) -> Result<()> {
    let scope = if local {
        Scope::from_cwd()?
    } else {
        Scope::Global
    };
    dispatch(action, &scope)
}

fn dispatch(action: ProfileAction, scope: &Scope) -> Result<()> {
    match action {
        ProfileAction::Share { action } => share(action, scope),
        ProfileAction::Status => status(scope),
        ProfileAction::Enable { detailed, duration } => {
            enable(detailed, duration.as_deref(), scope)
        }
        ProfileAction::Disable => set_mode(Mode::Off, None, scope),
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
            let mut merged = storage::load(&scope.profile_dir()?, since_hour(window(&since)?));
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
        ProfileAction::Analyze { since, top, json } => {
            let merged = storage::load(&scope.profile_dir()?, since_hour(window(&since)?));
            if json {
                let findings: serde_json::Map<String, serde_json::Value> = merged
                    .iter()
                    .map(|(engine, snapshot)| {
                        (
                            engine.clone(),
                            serde_json::to_value(analysis::findings(snapshot)).unwrap_or_default(),
                        )
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "window": since,
                        "engines": findings,
                    }))
                    .unwrap_or_default()
                );
            } else if merged.is_empty() {
                println!("No profile data recorded (window: {since}).");
            } else {
                for (engine, snapshot) in &merged {
                    print!("{}", analysis::render_text(engine, snapshot, &since, top));
                }
            }
            Ok(())
        }
        ProfileAction::SlowQueries { since } => {
            let merged = storage::load(&scope.profile_dir()?, since_hour(window(&since)?));
            let threshold = scope.load()?.profiling.slow_query_ms;
            print!("{}", report::render_slow(&merged, threshold, &since));
            Ok(())
        }
        ProfileAction::Export { since, output } => {
            let window = window(&since)?;
            let merged = storage::load(&scope.profile_dir()?, since_hour(window));
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
            let removed = storage::reset(&scope.profile_dir()?);
            println!("Removed {removed} profile file(s).");
            Ok(())
        }
    }
}

fn enable(detailed: bool, duration: Option<&str>, scope: &Scope) -> Result<()> {
    if !detailed {
        set_mode(Mode::Light, None, scope)?;
        return invite_sharing(scope);
    }
    let lasts = match duration {
        Some(text) => window(text)?,
        None => DEFAULT_DETAILED_FOR,
    };
    let expires = unix_now().saturating_add(lasts.as_secs());
    set_mode(Mode::Detailed, Some(expires as i64), scope)?;
    invite_sharing(scope)
}

fn set_mode(mode: Mode, detailed_expires_at: Option<i64>, scope: &Scope) -> Result<()> {
    scope.save_mode(mode, detailed_expires_at)?;
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
            "Profiles are stored under {}. Sharing requires separate consent. New tokensave processes pick \
             this up; restart running MCP servers to include them.", scope.profile_dir()?.display()
        );
    }
    Ok(())
}

fn share(action: ProfileShareAction, scope: &Scope) -> Result<()> {
    match action {
        ProfileShareAction::Preview => {
            print!("{}", sharing::payload(&scope.profile_dir()?, unix_now()));
        }
        ProfileShareAction::Disable => {
            scope.save_sharing(false)?;
            println!("Performance sharing disabled. Local profile data is unchanged.");
        }
        ProfileShareAction::Enable => {
            let destination = sharing::DESTINATION.filter(|d| d.ready()).ok_or_else(||
                config_error("Performance sharing is unavailable: the maintainer has not approved the endpoint and data policy. Consent remains unchanged."))?;
            if scope.settings().mode == Mode::Off {
                return Err(config_error(
                    "Enable profiling first with `tokensave profile enable`.",
                ));
            }
            show_destination(destination);
            print!("{}", sharing::payload(&scope.profile_dir()?, unix_now()));
            scope.save_sharing(true)?;
            println!(
                "Performance sharing enabled by explicit request. Uploads run at most once a day."
            );
            if !sharing::environment_allows() {
                println!("Uploads remain blocked for this run by TOKENSAVE_PROFILE_SHARE.");
            }
        }
        ProfileShareAction::Status => {
            let enabled = scope.sharing_consent();
            println!(
                "Performance sharing consent: {}",
                if enabled { "enabled" } else { "disabled" }
            );
            match sharing::DESTINATION.filter(|d| d.ready()) {
                Some(destination) => show_destination(destination),
                None => println!("Uploads unavailable: endpoint, access, retention and deletion policy await maintainer approval."),
            }
            println!(
                "Per-run sharing override: {}",
                if sharing::environment_allows() {
                    "allows configured consent"
                } else {
                    "blocks uploads"
                }
            );
            let state = sharing::state(&scope.profile_dir()?);
            match state {
                Some(state) => {
                    println!(
                        "Last upload (UNIX seconds, 0 = never): {}",
                        state.last_upload
                    );
                    if enabled
                        && sharing::DESTINATION.is_some()
                        && sharing::environment_allows()
                        && scope.settings().mode != Mode::Off
                    {
                        println!(
                            "Next due (UNIX seconds): {}",
                            state.last_attempt.saturating_add(sharing::INTERVAL_SECS)
                        );
                    } else {
                        println!("Next upload: inactive");
                    }
                }
                None => println!("Upload state unreadable; uploads are blocked."),
            }
        }
    }
    Ok(())
}

fn show_destination(destination: sharing::Destination) {
    println!(
        "Destination: {}\nAccess: {}\nRetention: {}\nDeletion: {}",
        destination.url, destination.access, destination.retention, destination.deletion
    );
    println!("Fields: schema, tokensave version, OS, architecture, logical CPU count, window hours, engine and static metric aggregates. No stable identifier.");
}

fn invite_sharing(scope: &Scope) -> Result<()> {
    use std::io::{IsTerminal, Write};
    let config = scope.load()?;
    if config.profiling_share_asked
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        return Ok(());
    }
    let Some(destination) = sharing::DESTINATION.filter(|d| d.ready()) else {
        println!("Performance sharing is unavailable pending the maintainer's endpoint and policy decisions.");
        return Ok(());
    };
    show_destination(destination);
    print!("{}", sharing::payload(&scope.profile_dir()?, unix_now()));
    print!("Share these sanitized performance aggregates? [y/N] ");
    std::io::stdout()
        .flush()
        .map_err(|e| config_error(e.to_string()))?;
    let mut answer = String::new();
    // Read a bounded answer; only an explicit yes grants consent.
    use std::io::BufRead;
    use std::io::Read;
    std::io::BufReader::new(std::io::stdin().lock().take(64))
        .read_line(&mut answer)
        .map_err(|e| config_error(e.to_string()))?;
    scope.save_sharing(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn status(scope: &Scope) -> Result<()> {
    let prefs = scope.load()?.profiling;
    let effective = scope.settings();
    let dir = scope.profile_dir()?;
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
