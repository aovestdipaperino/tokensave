//! Background daemon that watches all tracked tokensave projects for file
//! changes and runs incremental syncs automatically.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use daemon_kit::{Daemon, DaemonConfig};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};

use crate::errors::{Result, TokenSaveError};

/// Parse a human-readable duration string like "15s" or "1m" into a Duration.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.trim().parse::<u64>().ok().map(Duration::from_secs)
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.trim().parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
    } else {
        s.parse::<u64>().ok().map(Duration::from_secs)
    }
}

/// Returns the `~/.tokensave` directory used for PID/log files.
fn daemon_pid_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".tokensave")
}

/// Build the daemon-kit Daemon instance with tokensave paths.
pub fn build_daemon() -> std::result::Result<Daemon, TokenSaveError> {
    let ts_dir = daemon_pid_dir();
    let bin = crate::agents::which_tokensave().unwrap_or_else(|| "tokensave".to_string());

    let config = DaemonConfig::new("tokensave-daemon")
        .pid_dir(&ts_dir)
        .log_file(ts_dir.join("daemon.log"))
        .executable(PathBuf::from(bin))
        .service_args(vec!["daemon".to_string(), "--foreground".to_string()])
        .description("tokensave file watcher daemon");

    Ok(Daemon::new(config))
}

/// Returns the PID of the running daemon, or None.
pub fn running_daemon_pid() -> Option<u32> {
    build_daemon().ok()?.running_pid()
}

/// Returns true if an autostart service is installed.
pub fn is_autostart_enabled() -> bool {
    build_daemon().ok().is_some_and(|d| d.is_service_installed())
}

/// Directories to ignore inside watched projects.
const IGNORED_DIRS: &[&str] = &[
    ".tokensave", ".git", "node_modules", "target", ".build",
    "__pycache__", ".next", "dist", "build", ".cache",
];

/// The core daemon event loop. Watches projects, debounces changes, syncs.
async fn run_loop(debounce: Duration) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<PathBuf>(256);

    let mut watchers: HashMap<PathBuf, RecommendedWatcher> = HashMap::new();
    let mut dirty: HashMap<PathBuf, Instant> = HashMap::new();

    // Initial project discovery
    let project_paths = discover_projects().await;
    for path in &project_paths {
        if let Some(w) = create_watcher(path, tx.clone()) {
            watchers.insert(path.clone(), w);
        }
    }

    daemon_log(&format!("started, watching {} projects", watchers.len()));

    let mut discovery_interval = time::interval(Duration::from_secs(60));
    discovery_interval.tick().await; // consume first immediate tick

    // Set up ctrl-c handler
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        shutdown_tx.send(()).await.ok();
    });

    loop {
        // Find the next debounce deadline
        let next_deadline = dirty.values().copied().min();
        let sleep_dur = match next_deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => Duration::from_secs(3600),
        };

        tokio::select! {
            _ = shutdown_rx.recv() => {
                daemon_log("shutting down (signal)");
                break;
            }
            Some(project_root) = rx.recv() => {
                dirty.insert(project_root, Instant::now() + debounce);
            }
            _ = tokio::time::sleep(sleep_dur), if next_deadline.is_some() => {
                let now = Instant::now();
                let ready: Vec<PathBuf> = dirty
                    .iter()
                    .filter(|(_, deadline)| **deadline <= now)
                    .map(|(path, _)| path.clone())
                    .collect();
                for path in ready {
                    dirty.remove(&path);
                    sync_project(&path).await;
                }
            }
            _ = discovery_interval.tick() => {
                let current = discover_projects().await;
                let current_set: HashSet<PathBuf> = current.into_iter().collect();
                let watched_set: HashSet<PathBuf> = watchers.keys().cloned().collect();

                for path in current_set.difference(&watched_set) {
                    if let Some(w) = create_watcher(path, tx.clone()) {
                        daemon_log(&format!("discovered new project: {}", path.display()));
                        watchers.insert(path.clone(), w);
                    }
                }
                let stale: Vec<PathBuf> = watched_set.difference(&current_set).cloned().collect();
                for path in stale {
                    watchers.remove(&path);
                    dirty.remove(&path);
                }
            }
        }
    }

    Ok(())
}

/// Query the global DB for all tracked project paths.
async fn discover_projects() -> Vec<PathBuf> {
    let Some(gdb) = crate::global_db::GlobalDb::open().await else {
        return Vec::new();
    };
    gdb.list_project_paths()
        .await
        .into_iter()
        .filter_map(|s| {
            let p = PathBuf::from(&s);
            if p.is_dir() && crate::tokensave::TokenSave::is_initialized(&p) {
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

/// Create a notify watcher for a project root.
fn create_watcher(project_root: &Path, tx: mpsc::Sender<PathBuf>) -> Option<RecommendedWatcher> {
    let root = project_root.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
        let Ok(event) = res else { return };
        if !matches!(
            event.kind,
            notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_)
        ) {
            return;
        }
        let dominated_by_ignored = event.paths.iter().all(|p| {
            p.components().any(|c| {
                IGNORED_DIRS.contains(&c.as_os_str().to_str().unwrap_or(""))
            })
        });
        if dominated_by_ignored {
            return;
        }
        let _ = tx.blocking_send(root.clone());
    })
    .ok()?;
    watcher.watch(project_root, RecursiveMode::Recursive).ok()?;
    Some(watcher)
}

/// Run an incremental sync on a single project. Best-effort.
///
/// Catches panics (e.g. from extractor bugs on malformed files) so one
/// bad project doesn't kill the entire daemon.
async fn sync_project(project_root: &Path) {
    let root = project_root.to_path_buf();
    let result = tokio::task::spawn(async move {
        sync_project_inner(&root).await;
    })
    .await;

    if let Err(e) = result {
        let msg = if e.is_panic() {
            let panic = e.into_panic();
            if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "unknown panic".to_string()
            }
        } else {
            format!("task error: {e}")
        };
        daemon_log(&format!(
            "sync panicked for {}: {msg}",
            project_root.display()
        ));
    }
}

async fn sync_project_inner(project_root: &Path) {
    let start = std::time::Instant::now();
    let Ok(cg) = crate::tokensave::TokenSave::open(project_root).await else {
        daemon_log(&format!("failed to open {}", project_root.display()));
        return;
    };
    match cg.sync().await {
        Ok(result) => {
            let ms = start.elapsed().as_millis();
            if result.files_added > 0 || result.files_modified > 0 || result.files_removed > 0 {
                daemon_log(&format!(
                    "synced {} — {} added, {} modified, {} removed ({}ms)",
                    project_root.display(),
                    result.files_added,
                    result.files_modified,
                    result.files_removed,
                    ms
                ));
            }
            // Best-effort update global DB
            if let Some(gdb) = crate::global_db::GlobalDb::open().await {
                let tokens = cg.get_tokens_saved().await.unwrap_or(0);
                gdb.upsert(project_root, tokens).await;
            }
        }
        Err(e) => {
            daemon_log(&format!("sync failed for {}: {e}", project_root.display()));
        }
    }
}

/// Log a timestamped daemon message to stderr.
///
/// When running under launchd/systemd, stderr is redirected to the daemon
/// log file automatically. Writing directly to the file as well would
/// duplicate every line.
fn daemon_log(msg: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    eprintln!("[{secs}] {msg}");
}

/// Start the daemon. Forks to background on Unix unless `foreground` is true.
pub async fn run(foreground: bool) -> Result<()> {
    let daemon = build_daemon()?;

    let config = crate::user_config::UserConfig::load();
    let debounce = parse_duration(&config.daemon_debounce)
        .unwrap_or(Duration::from_secs(15));

    if foreground {
        // Already inside a tokio runtime — call run_loop directly.
        // daemon.start(foreground=true) would invoke a FnOnce closure
        // that creates a nested runtime, which panics.
        let pid_file = daemon_kit::PidFile::new(
            daemon_pid_dir().join("tokensave-daemon.pid"),
        );
        pid_file.write().ok();
        let result = run_loop(debounce).await.map_err(|e| TokenSaveError::Config {
            message: format!("daemon error: {e}"),
        });
        pid_file.remove();
        result
    } else {
        // Fork to background — the child needs its own tokio runtime.
        daemon
            .start(false, move || {
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    daemon_kit::DaemonError::Daemonize(format!("failed to create runtime: {e}"))
                })?;
                rt.block_on(async {
                    run_loop(debounce).await.map_err(|e| {
                        daemon_kit::DaemonError::Daemonize(e.to_string())
                    })
                })
            })
            .map_err(|e| TokenSaveError::Config {
                message: format!("daemon error: {e}"),
            })
    }
}

/// Stop the running daemon.
pub fn stop() -> Result<()> {
    let daemon = build_daemon()?;
    daemon.stop().map_err(|e| TokenSaveError::Config {
        message: format!("{e}"),
    })?;
    eprintln!("tokensave daemon stopped");
    Ok(())
}

/// Print daemon status and return exit code (0 = running, 1 = not running).
pub fn status() -> i32 {
    match running_daemon_pid() {
        Some(pid) => {
            eprintln!("tokensave daemon is running (PID: {pid})");
            0
        }
        None => {
            eprintln!("tokensave daemon is not running");
            1
        }
    }
}

/// Install autostart service (launchd/systemd/Windows Service).
pub fn enable_autostart() -> Result<()> {
    let daemon = build_daemon()?;
    daemon.install_service().map_err(|e| TokenSaveError::Config {
        message: format!("{e}"),
    })?;
    eprintln!("\x1b[32m✔\x1b[0m Autostart service installed");
    Ok(())
}

/// Remove autostart service.
pub fn disable_autostart() -> Result<()> {
    let daemon = build_daemon()?;
    daemon.uninstall_service().map_err(|e| TokenSaveError::Config {
        message: format!("{e}"),
    })?;
    eprintln!("\x1b[32m✔\x1b[0m Autostart service removed");
    Ok(())
}

/// Offer to install the daemon autostart service during `tokensave install`.
///
/// Skips silently when:
/// - stdin is not a terminal (non-interactive)
/// - the autostart service is already installed
/// - the daemon is already running
pub fn offer_daemon_autostart() {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return;
    }

    if is_autostart_enabled() {
        eprintln!("  Daemon autostart service already installed, skipping");
        return;
    }

    if running_daemon_pid().is_some() {
        eprintln!("  Daemon already running (no autostart service), skipping");
        return;
    }

    eprintln!();
    eprint!(
        "Install the \x1b[1mtokensave daemon\x1b[0m as a background service (auto-syncs on file changes)? [y/N] "
    );

    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return;
    }
    if !matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
        eprintln!("  Skipped daemon service");
        eprintln!("  tip: you can install it later with \x1b[1mtokensave daemon --enable-autostart\x1b[0m");
        return;
    }

    match enable_autostart() {
        Ok(()) => {}
        Err(e) => eprintln!("  \x1b[31m✘\x1b[0m Failed to install daemon service: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("15s"), Some(Duration::from_secs(15)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration(" 5s "), Some(Duration::from_secs(5)));
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration("1m"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration("2m"), Some(Duration::from_secs(120)));
    }

    #[test]
    fn parse_duration_bare_number() {
        assert_eq!(parse_duration("10"), Some(Duration::from_secs(10)));
    }

    #[test]
    fn parse_duration_invalid() {
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("1h"), None);
    }
}
