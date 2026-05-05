//! Background daemon that watches all tracked tokensave projects for file
//! changes and runs incremental syncs automatically.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use daemon_kit::{Daemon, DaemonConfig};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};

use crate::errors::{Result, TokenSaveError};

/// Path to the file where the daemon writes its API port.
fn api_port_path() -> PathBuf {
    daemon_pid_dir().join("daemon.port")
}

/// Response from the daemon's status API.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DaemonInfo {
    pub version: String,
    pub pid: u32,
    pub uptime_secs: u64,
    pub projects_watched: usize,
}

/// Query the running daemon's status API. Returns None if unreachable.
pub fn query_daemon_info() -> Option<DaemonInfo> {
    let port_str = std::fs::read_to_string(api_port_path()).ok()?;
    let port: u16 = port_str.trim().parse().ok()?;

    let mut stream = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.write_all(b"GET /status HTTP/1.0\r\n\r\n").ok()?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).ok()?;

    // Skip HTTP headers — find the blank line then parse JSON body.
    let body = buf.split("\r\n\r\n").nth(1)?;
    serde_json::from_str(body).ok()
}

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
        mins.trim()
            .parse::<u64>()
            .ok()
            .map(|m| Duration::from_secs(m * 60))
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
    build_daemon()
        .ok()
        .is_some_and(|d| d.is_service_installed())
}

/// Directories to ignore inside watched projects.
const IGNORED_DIRS: &[&str] = &[
    ".tokensave",
    ".git",
    "node_modules",
    "target",
    ".build",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".cache",
];

/// The core daemon event loop. Watches projects, debounces changes, syncs.
///
/// Returns `true` if the loop exited because the on-disk binary was updated
/// (upgrade detected), signalling the caller to exit with a non-zero code so
/// the service manager restarts with the new version.
async fn run_loop(debounce: Duration) -> Result<bool> {
    let (tx, mut rx) = mpsc::channel::<PathBuf>(256);

    let mut watchers: HashMap<PathBuf, RecommendedWatcher> = HashMap::new();
    let mut dirty: HashMap<PathBuf, Instant> = HashMap::new();

    // Snapshot the current binary for upgrade detection.
    let binary_snapshot = BinarySnapshot::capture();

    // Initial project discovery
    let project_paths = discover_projects().await;
    for path in &project_paths {
        if let Some(w) = create_watcher(path, tx.clone()) {
            watchers.insert(path.clone(), w);
        }
    }

    // Shared project count for the status API.
    let project_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(watchers.len()));
    let start_time = std::time::SystemTime::now();

    // Start the status API listener.
    let api_handle = start_status_api(project_count.clone(), start_time);

    daemon_log(&format!(
        "v{} started, watching {} projects",
        env!("CARGO_PKG_VERSION"),
        watchers.len(),
    ));

    let mut discovery_interval = time::interval(Duration::from_mins(1));
    discovery_interval.tick().await; // consume first immediate tick

    // Set up ctrl-c handler
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        shutdown_tx.send(()).await.ok();
    });

    let mut upgraded = false;

    loop {
        // Find the next debounce deadline
        let next_deadline = dirty.values().copied().min();
        let sleep_dur = match next_deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => Duration::from_hours(1),
        };

        tokio::select! {
            _ = shutdown_rx.recv() => {
                daemon_log("shutting down (signal)");
                break;
            }
            Some(project_root) = rx.recv() => {
                dirty.insert(project_root, Instant::now() + debounce);
            }
            () = tokio::time::sleep(sleep_dur), if next_deadline.is_some() => {
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
                // Check for binary upgrade.
                if let Some(ref snapshot) = binary_snapshot {
                    if snapshot.has_changed() {
                        daemon_log("binary updated on disk, restarting to pick up new version");
                        // Flush pending syncs before exiting.
                        let pending: Vec<PathBuf> = dirty.keys().cloned().collect();
                        for path in pending {
                            dirty.remove(&path);
                            sync_project(&path).await;
                        }
                        upgraded = true;
                        break;
                    }
                }

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
                project_count.store(watchers.len(), std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    // Clean up status API.
    drop(api_handle);
    let _ = std::fs::remove_file(api_port_path());

    Ok(upgraded)
}

/// Starts a tiny TCP server on an ephemeral port that responds to
/// `GET /status` with a JSON `DaemonInfo`. Writes the port to
/// `~/.tokensave/daemon.port` so `--status` can connect.
fn start_status_api(
    project_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    start_time: std::time::SystemTime,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(e) => {
                daemon_log(&format!("status API bind failed: {e}"));
                return;
            }
        };
        let port = match listener.local_addr() {
            Ok(a) => a.port(),
            Err(_) => return,
        };
        // Write port file.
        let port_path = api_port_path();
        if let Some(parent) = port_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if std::fs::write(&port_path, port.to_string()).is_err() {
            daemon_log("failed to write daemon.port");
            return;
        }
        daemon_log(&format!("status API listening on port {port}"));

        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let count = project_count.load(std::sync::atomic::Ordering::Relaxed);
            let uptime = start_time.elapsed().map_or(0, |d| d.as_secs());
            let info = DaemonInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                pid: std::process::id(),
                uptime_secs: uptime,
                projects_watched: count,
            };
            let body = serde_json::to_string(&info).unwrap_or_default();
            let response = format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body,
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    })
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
    let mut watcher =
        notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
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
                p.components()
                    .any(|c| IGNORED_DIRS.contains(&c.as_os_str().to_str().unwrap_or("")))
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
            // Best-effort update branch metadata sync timestamp
            if let Some(branch) = cg.active_branch() {
                let tokensave_dir = crate::config::get_tokensave_dir(project_root);
                if let Some(mut meta) = crate::branch_meta::load_branch_meta(&tokensave_dir) {
                    meta.touch_synced(branch);
                    let _ = crate::branch_meta::save_branch_meta(&tokensave_dir, &meta);
                }
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
///
/// Returns `true` if the daemon exited due to an upgrade (binary changed on
/// disk). The caller should exit with a non-zero code so the service manager
/// restarts with the new version.
pub async fn run(foreground: bool, debounce_override: Option<String>) -> Result<bool> {
    let daemon = build_daemon()?;

    let config = crate::user_config::UserConfig::load();
    let debounce = if let Some(ref override_str) = debounce_override {
        parse_duration(override_str).unwrap_or(Duration::from_secs(2))
    } else {
        parse_duration(&config.daemon_debounce).unwrap_or(Duration::from_secs(2))
    };

    if foreground {
        // Already inside a tokio runtime — call run_loop directly.
        // daemon.start(foreground=true) would invoke a FnOnce closure
        // that creates a nested runtime, which panics.
        let pid_file = daemon_kit::PidFile::new(daemon_pid_dir().join("tokensave-daemon.pid"));
        pid_file.write().ok();
        let result = run_loop(debounce)
            .await
            .map_err(|e| TokenSaveError::Config {
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
                let future = async {
                    match run_loop(debounce).await {
                        Ok(true) => {
                            // Upgrade detected — exit non-zero for service manager restart.
                            std::process::exit(1);
                        }
                        Ok(false) => Ok(()),
                        Err(e) => Err(daemon_kit::DaemonError::Daemonize(e.to_string())),
                    }
                };
                // On Unix, daemon-kit forks — the child has no tokio runtime,
                // so we must create one. On Windows, the closure runs inline in
                // the existing #[tokio::main] runtime, so we use block_in_place.
                #[cfg(windows)]
                {
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(future)
                    })
                }
                #[cfg(not(windows))]
                {
                    let rt = tokio::runtime::Runtime::new().map_err(|e| {
                        daemon_kit::DaemonError::Daemonize(format!("failed to create runtime: {e}"))
                    })?;
                    rt.block_on(future)
                }
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
    if let Some(pid) = running_daemon_pid() {
        if let Some(info) = query_daemon_info() {
            eprintln!(
                "tokensave daemon v{} is running (PID: {}, uptime: {}, watching {} projects)",
                info.version,
                info.pid,
                format_uptime(info.uptime_secs),
                info.projects_watched,
            );
            check_daemon_version_mismatch(&info.version);
        } else {
            eprintln!("tokensave daemon is running (PID: {pid})");
        }
        0
    } else {
        eprintln!("tokensave daemon is not running");
        1
    }
}

/// Warns if the running daemon version differs from the CLI version and
/// suggests the appropriate corrective action.
fn check_daemon_version_mismatch(daemon_version: &str) {
    let cli_version = env!("CARGO_PKG_VERSION");
    if daemon_version == cli_version {
        return;
    }

    let daemon_is_beta = daemon_version.contains('-');
    let cli_is_beta = cli_version.contains('-');

    let advice = if daemon_is_beta && !cli_is_beta {
        // Daemon is beta, CLI is stable — daemon should be restarted to pick up stable
        "The daemon is running a beta version. Restart it to use the stable release:\n  \
         tokensave daemon --stop && tokensave daemon --enable-autostart"
    } else if !daemon_is_beta && cli_is_beta {
        // Daemon is stable, CLI is beta — daemon should be restarted to pick up beta
        "The daemon is running a stable version. Restart it to use the beta release:\n  \
         tokensave daemon --stop && tokensave daemon --enable-autostart"
    } else if crate::cloud::is_newer_version(daemon_version, cli_version) {
        // CLI is newer — daemon hasn't picked up the upgrade yet
        "The daemon hasn't restarted after the upgrade. It should auto-restart within 60s.\n  \
         To restart now: tokensave daemon --stop && tokensave daemon --enable-autostart"
    } else {
        // CLI is older than daemon — unusual, maybe downgrade
        "The CLI is older than the running daemon. Restart to align versions:\n  \
         tokensave daemon --stop && tokensave daemon --enable-autostart"
    };

    eprintln!(
        "\n\x1b[33mWarning: version mismatch — CLI is v{cli_version}, daemon is v{daemon_version}\x1b[0m\n  {advice}"
    );
}

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
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
    daemon
        .install_service()
        .map_err(|e| TokenSaveError::Config {
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
            ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
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
                    message: format!("elevated process exited with code {exit_code}"),
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
    daemon
        .uninstall_service()
        .map_err(|e| TokenSaveError::Config {
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::duration_suboptimal_units,
)]
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
        assert!(
            !snapshot.has_changed(),
            "binary should not have changed immediately"
        );
    }

    #[test]
    fn binary_snapshot_detects_missing_file() {
        let snapshot = BinarySnapshot {
            path: PathBuf::from("/nonexistent/binary/path"),
            modified: std::time::UNIX_EPOCH,
            size: 100,
        };
        assert!(
            snapshot.has_changed(),
            "missing file should count as changed"
        );
    }

    #[test]
    fn binary_snapshot_detects_size_change() {
        let snapshot = BinarySnapshot::capture().unwrap();
        let tampered = BinarySnapshot {
            path: snapshot.path,
            modified: snapshot.modified,
            size: snapshot.size + 1, // fake a size difference
        };
        assert!(
            tampered.has_changed(),
            "different size should count as changed"
        );
    }

    #[test]
    fn binary_snapshot_detects_mtime_change() {
        let snapshot = BinarySnapshot::capture().unwrap();
        let tampered = BinarySnapshot {
            path: snapshot.path,
            modified: std::time::UNIX_EPOCH, // fake a different mtime
            size: snapshot.size,
        };
        assert!(
            tampered.has_changed(),
            "different mtime should count as changed"
        );
    }

    /// Regression test for the Windows daemon nested-runtime panic.
    ///
    /// On Windows, daemon-kit runs the closure inline (no fork), so creating
    /// `Runtime::new()` inside an existing `#[tokio::main]` runtime panics
    /// with "Cannot start a runtime from within a runtime."
    ///
    /// The fix uses `block_in_place` + `Handle::current().block_on()` on
    /// Windows, which is safe inside a multi-thread runtime.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nested_runtime_block_in_place_does_not_panic() {
        // Simulate what the daemon closure does on Windows: run an async
        // block from inside an already-running tokio runtime using
        // block_in_place + Handle::current().
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { 42 })
        });
        assert_eq!(result, 42);
    }

    /// Verify that creating a nested `Runtime::new()` inside an existing
    /// multi-thread runtime panics — this is the bug the Windows fix prevents.
    /// The exact message varies by tokio version ("Cannot start a runtime…"
    /// vs "Cannot drop a runtime…"), so we just assert it panics.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[should_panic(expected = "runtime")]
    async fn nested_runtime_new_panics() {
        let _rt = tokio::runtime::Runtime::new().unwrap();
    }
}
