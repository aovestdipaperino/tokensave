//! Doctor command: comprehensive health check of the tokensave installation.
//!
//! Checks the binary, project index, global DB, user config, agent
//! integrations, and network connectivity.

use std::path::{Path, PathBuf};

use crate::agents::{self, DoctorCounters, HealthcheckContext};
use crate::display::format_token_count;
use crate::tokensave::TokenSave;

/// Runs a comprehensive health check of the tokensave installation.
pub async fn run_doctor(agent_filter: Option<&str>) {
    debug_assert!(!env!("CARGO_PKG_VERSION").is_empty(), "CARGO_PKG_VERSION must not be empty");
    let mut dc = DoctorCounters::new();

    eprintln!("\n\x1b[1mtokensave doctor v{}\x1b[0m\n", env!("CARGO_PKG_VERSION"));

    check_binary(&mut dc);

    eprintln!("\n\x1b[1mCurrent project\x1b[0m");
    let project_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if TokenSave::is_initialized(&project_path) {
        dc.pass(&format!("Index found: {}/.tokensave/", project_path.display()));
        check_database(&mut dc, &project_path).await;
    } else {
        dc.warn(&format!("No index at {}/.tokensave/ — run `tokensave sync`", project_path.display()));
    }

    check_global_db(&mut dc);
    check_user_config(&mut dc);

    // Agent-specific health checks
    if let Some(ref home) = agents::home_dir() {
        let hctx = HealthcheckContext {
            home: home.clone(),
            project_path: project_path.clone(),
        };
        let agents_to_check: Vec<Box<dyn agents::AgentIntegration>> = match agent_filter {
            Some(id) => match agents::get_integration(id) {
                Ok(ag) => vec![ag],
                Err(e) => {
                    dc.fail(&format!("{e}"));
                    vec![]
                }
            },
            None => agents::all_integrations(),
        };
        for ag in &agents_to_check {
            ag.healthcheck(&mut dc, &hctx);
        }
    } else {
        dc.fail("Could not determine home directory");
    }

    check_daemon(&mut dc);
    check_network(&mut dc);
    print_summary(&dc);
}

/// Check database health: report size and run VACUUM to reclaim space.
async fn check_database(dc: &mut DoctorCounters, project_path: &Path) {
    let db_path = crate::config::get_tokensave_dir(project_path).join("tokensave.db");
    let size_before = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let ts = match TokenSave::open(project_path).await {
        Ok(ts) => ts,
        Err(e) => {
            dc.fail(&format!("Could not open database: {e}"));
            return;
        }
    };

    dc.pass(&format!("DB size: {}", format_bytes(size_before)));

    eprintln!("    Compacting database (VACUUM)…");
    match ts.optimize().await {
        Ok(()) => {
            let size_after = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(size_before);
            if size_before > size_after {
                let reclaimed = size_before - size_after;
                dc.pass(&format!(
                    "Compacted: {} → {} (reclaimed {})",
                    format_bytes(size_before),
                    format_bytes(size_after),
                    format_bytes(reclaimed),
                ));
            } else {
                dc.pass("Database already compact");
            }
        }
        Err(e) => {
            dc.warn(&format!("VACUUM failed: {e}"));
        }
    }
}

/// Format a byte count as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Check binary location and version.
fn check_binary(dc: &mut DoctorCounters) {
    eprintln!("\x1b[1mBinary\x1b[0m");
    if let Ok(exe) = std::env::current_exe() {
        dc.pass(&format!("Binary: {}", exe.display()));
    } else {
        dc.fail("Could not determine binary path");
    }
    dc.pass(&format!("Version: {}", env!("CARGO_PKG_VERSION")));
}

/// Check global database exists.
fn check_global_db(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mGlobal database\x1b[0m");
    if let Some(db_path) = crate::global_db::global_db_path() {
        if db_path.exists() {
            dc.pass(&format!("Global DB: {}", db_path.display()));
        } else {
            dc.warn("Global DB not yet created (created on first sync)");
        }
    } else {
        dc.fail("Could not determine home directory for global DB");
    }
}

/// Check user config file.
fn check_user_config(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mUser config\x1b[0m");
    if let Some(config_path) = crate::user_config::config_path() {
        if config_path.exists() {
            let config = crate::user_config::UserConfig::load();
            dc.pass(&format!("Config: {}", config_path.display()));
            if config.upload_enabled {
                dc.pass("Upload enabled");
            } else {
                dc.info("Upload disabled (opt-out)");
            }
            if config.pending_upload > 0 {
                dc.info(&format!("Pending upload: {} tokens", config.pending_upload));
            }
        } else {
            dc.warn("Config not yet created (created on first sync)");
        }
    } else {
        dc.fail("Could not determine home directory for config");
    }
}

/// Check daemon status and autostart configuration.
fn check_daemon(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mDaemon\x1b[0m");
    match crate::daemon::running_daemon_pid() {
        Some(pid) => dc.pass(&format!("Daemon is running (PID: {pid})")),
        None => dc.warn("Daemon is not running — run `tokensave daemon` to start"),
    }
    if crate::daemon::is_autostart_enabled() {
        dc.pass("Autostart enabled");
    } else {
        dc.warn("Autostart not configured — run `tokensave daemon --enable-autostart`");
    }
}

/// Check network connectivity.
fn check_network(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mNetwork\x1b[0m");
    if let Some(total) = crate::cloud::fetch_worldwide_total() {
        dc.pass(&format!("Worldwide counter reachable (total: {})", format_token_count(total)));
    } else {
        dc.warn("Worldwide counter unreachable (offline or timeout)");
    }
    if crate::cloud::fetch_latest_version().is_some() {
        dc.pass("GitHub releases API reachable");
    } else {
        dc.warn("GitHub releases API unreachable (offline or timeout)");
    }
}

/// Print final summary.
fn print_summary(dc: &DoctorCounters) {
    eprintln!();
    if dc.issues == 0 && dc.warnings == 0 {
        eprintln!("\x1b[32mAll checks passed.\x1b[0m");
    } else if dc.issues == 0 {
        eprintln!("\x1b[33m{} warning(s), no issues.\x1b[0m", dc.warnings);
    } else {
        eprintln!("\x1b[31m{} issue(s), {} warning(s).\x1b[0m", dc.issues, dc.warnings);
        eprintln!("Run \x1b[1mtokensave install\x1b[0m to fix most issues.");
    }
    eprintln!();
}
