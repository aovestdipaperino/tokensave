//! Background daemon that watches all tracked tokensave projects for file
//! changes and runs incremental syncs automatically.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use daemon_kit::{Daemon, DaemonConfig};
use tokio::sync::mpsc;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::project_watcher::ProjectWatcher;

use crate::errors::{Result, TokenSaveError};

/// Snapshot of the on-disk binary for upgrade detection.
struct BinarySnapshot {
    path: PathBuf,
    modified: std::time::SystemTime,
    size: u64,
}

impl BinarySnapshot {
    /// Take a snapshot of the current binary's metadata.
    fn capture() -> Option<Self> {
        let path = std::env::current_exe().ok()?;
        let meta = std::fs::metadata(&path).ok()?;
        Some(Self {
            path,
            modified: meta.modified().unwrap_or(std::time::UNIX_EPOCH),
            size: meta.len(),
        })
    }

    /// Returns true if the on-disk binary has changed since this snapshot.
    fn has_changed(&self) -> bool {
        let Ok(meta) = std::fs::metadata(&self.path) else {
            // File disappeared (mid-upgrade race) — treat as changed.
            return true;
        };
        let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        modified != self.modified || meta.len() != self.size
    }
}

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

/// The core daemon event loop. Manages per-project watchers, discovers
/// new projects, and detects binary upgrades.
///
/// Returns `true` if the loop exited because the on-disk binary was updated
/// (upgrade detected), signalling the caller to exit with a non-zero code so
/// the service manager restarts with the new version.
async fn run_loop(debounce: Duration) -> Result<bool> {
    let mut watchers: HashMap<PathBuf, (CancellationToken, tokio::task::JoinHandle<()>)> =
        HashMap::new();

    // Snapshot the current binary for upgrade detection.
    let binary_snapshot = BinarySnapshot::capture();

    // Initial project discovery
    let project_paths = discover_projects().await;
    for path in project_paths {
        spawn_watcher(&mut watchers, path, debounce);
    }

    daemon_log(&format!(
        "v{} started, watching {} projects",
        env!("CARGO_PKG_VERSION"),
        watchers.len(),
    ));

    let mut discovery_interval = time::interval(Duration::from_secs(60));
    discovery_interval.tick().await; // consume first immediate tick

    // Set up ctrl-c handler
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        shutdown_tx.send(()).await.ok();
    });

    let mut upgraded = false;

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                daemon_log("shutting down (signal)");
                break;
            }
            _ = discovery_interval.tick() => {
                // Check for binary upgrade.
                if let Some(ref snapshot) = binary_snapshot {
                    if snapshot.has_changed() {
                        daemon_log("binary updated on disk, restarting to pick up new version");
                        upgraded = true;
                        break;
                    }
                }

                let current = discover_projects().await;
                let current_set: HashSet<PathBuf> = current.into_iter().collect();
                let watched_set: HashSet<PathBuf> = watchers.keys().cloned().collect();

                for path in current_set.difference(&watched_set) {
                    daemon_log(&format!("discovered new project: {}", path.display()));
                    spawn_watcher(&mut watchers, path.clone(), debounce);
                }
                let stale: Vec<PathBuf> = watched_set.difference(&current_set).cloned().collect();
                for path in &stale {
                    cancel_watcher(&mut watchers, path).await;
                }
            }
        }
    }

    // Cancel all watchers — each flushes its pending sync before returning.
    let all_paths: Vec<PathBuf> = watchers.keys().cloned().collect();
    for path in &all_paths {
        cancel_watcher(&mut watchers, path).await;
    }

    Ok(upgraded)
}

/// Spawn a [`ProjectWatcher`] for a single project directory.
fn spawn_watcher(
    watchers: &mut HashMap<PathBuf, (CancellationToken, tokio::task::JoinHandle<()>)>,
    path: PathBuf,
    debounce: Duration,
) {
    if let Some(pw) = ProjectWatcher::new(path.clone(), debounce) {
        let token = CancellationToken::new();
        let handle = tokio::spawn(pw.run(token.clone()));
        watchers.insert(path, (token, handle));
    }
}

/// Cancel a running [`ProjectWatcher`] and wait for it to finish.
async fn cancel_watcher(
    watchers: &mut HashMap<PathBuf, (CancellationToken, tokio::task::JoinHandle<()>)>,
    path: &Path,
) {
    if let Some((token, handle)) = watchers.remove(path) {
        token.cancel();
        handle.await.ok();
    }
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
///
/// Returns `true` if the daemon exited due to an upgrade (binary changed on
/// disk). The caller should exit with a non-zero code so the service manager
/// restarts with the new version.
pub async fn run(foreground: bool) -> Result<bool> {
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
        // The daemon-kit closure returns Result<()>, so the child handles
        // the upgrade exit code internally via std::process::exit.
        daemon
            .start(false, move || {
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    daemon_kit::DaemonError::Daemonize(format!("failed to create runtime: {e}"))
                })?;
                rt.block_on(async {
                    match run_loop(debounce).await {
                        Ok(true) => {
                            // Upgrade detected — exit non-zero for service manager restart.
                            std::process::exit(1);
                        }
                        Ok(false) => Ok(()),
                        Err(e) => Err(daemon_kit::DaemonError::Daemonize(e.to_string())),
                    }
                })
            })
            .map_err(|e| TokenSaveError::Config {
                message: format!("daemon error: {e}"),
            })?;
        // Parent returns immediately after fork.
        Ok(false)
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
///
/// On Windows, installing a service requires administrator privileges.
/// If the current process is not elevated, this spawns an elevated child
/// process via UAC to perform only the service installation step.
pub fn enable_autostart() -> Result<()> {
    #[cfg(target_os = "windows")]
    if !win_elevated::is_elevated() {
        return win_elevated::run_elevated_autostart();
    }

    let daemon = build_daemon()?;
    daemon.install_service().map_err(|e| TokenSaveError::Config {
        message: format!("{e}"),
    })?;
    eprintln!("\x1b[32m✔\x1b[0m Autostart service installed");
    Ok(())
}

/// Windows-only helpers for UAC elevation.
#[cfg(target_os = "windows")]
mod win_elevated {
    use crate::errors::{Result, TokenSaveError};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    /// Check whether the current process is running with administrator privileges.
    pub fn is_elevated() -> bool {
        use std::mem;
        use std::ptr;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }

            let mut elevation: TOKEN_ELEVATION = mem::zeroed();
            let mut size: u32 = 0;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut _,
                mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            );
            CloseHandle(token);

            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    /// Spawn an elevated child process via UAC, wait for it to exit, and
    /// check its exit code.
    fn run_elevated(args: &str, success_msg: &str) -> Result<()> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, WaitForSingleObject, INFINITE,
        };
        use windows_sys::Win32::UI::Shell::{
            ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_NOCLOSEPROCESS,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let exe = std::env::current_exe().map_err(|e| TokenSaveError::Config {
            message: format!("cannot determine executable path: {e}"),
        })?;

        let verb: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
        let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let params: Vec<u16> = OsStr::new(args).encode_wide().chain(Some(0)).collect();

        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_SHOWNORMAL;

        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok == 0 || info.hProcess.is_null() {
            return Err(TokenSaveError::Config {
                message: "UAC elevation was cancelled or failed".to_string(),
            });
        }

        // Wait for the elevated child to finish and check its exit code.
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut exit_code: u32 = 1;
            GetExitCodeProcess(info.hProcess, &mut exit_code);
            CloseHandle(info.hProcess);

            if exit_code != 0 {
                return Err(TokenSaveError::Config {
                    message: format!(
                        "elevated process exited with code {exit_code}"
                    ),
                });
            }
        }

        eprintln!("{success_msg}");
        Ok(())
    }

    /// Spawn an elevated child to install the autostart service.
    pub fn run_elevated_autostart() -> Result<()> {
        run_elevated(
            "daemon --enable-autostart",
            "\x1b[32m✔\x1b[0m Autostart service installed (via elevated process)",
        )
    }

    /// Spawn an elevated child to remove the autostart service.
    pub fn run_elevated_disable_autostart() -> Result<()> {
        run_elevated(
            "daemon --disable-autostart",
            "\x1b[32m✔\x1b[0m Autostart service removed (via elevated process)",
        )
    }
}

/// Remove autostart service.
///
/// On Windows, this may require elevation to access the SCM.
pub fn disable_autostart() -> Result<()> {
    #[cfg(target_os = "windows")]
    if !win_elevated::is_elevated() {
        return win_elevated::run_elevated_disable_autostart();
    }

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

    #[test]
    fn binary_snapshot_captures_current_exe() {
        let snapshot = BinarySnapshot::capture();
        assert!(snapshot.is_some(), "should capture current test binary");
        let snapshot = snapshot.unwrap();
        assert!(snapshot.path.exists());
        assert!(snapshot.size > 0);
    }

    #[test]
    fn binary_snapshot_unchanged() {
        let snapshot = BinarySnapshot::capture().unwrap();
        assert!(!snapshot.has_changed(), "binary should not have changed immediately");
    }

    #[test]
    fn binary_snapshot_detects_missing_file() {
        let snapshot = BinarySnapshot {
            path: PathBuf::from("/nonexistent/binary/path"),
            modified: std::time::UNIX_EPOCH,
            size: 100,
        };
        assert!(snapshot.has_changed(), "missing file should count as changed");
    }

    #[test]
    fn binary_snapshot_detects_size_change() {
        let snapshot = BinarySnapshot::capture().unwrap();
        let tampered = BinarySnapshot {
            path: snapshot.path,
            modified: snapshot.modified,
            size: snapshot.size + 1, // fake a size difference
        };
        assert!(tampered.has_changed(), "different size should count as changed");
    }

    #[test]
    fn binary_snapshot_detects_mtime_change() {
        let snapshot = BinarySnapshot::capture().unwrap();
        let tampered = BinarySnapshot {
            path: snapshot.path,
            modified: std::time::UNIX_EPOCH, // fake a different mtime
            size: snapshot.size,
        };
        assert!(tampered.has_changed(), "different mtime should count as changed");
    }
}
