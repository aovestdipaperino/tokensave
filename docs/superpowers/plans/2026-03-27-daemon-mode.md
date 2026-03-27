# Daemon Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A background daemon that watches all tracked tokensave projects for file changes and automatically runs incremental syncs.

**Architecture:** A new `tokensave daemon` subcommand backed by `src/daemon.rs`. Uses the `notify` crate for filesystem watching and tokio timers for per-project debounce. Discovers projects from the global DB, re-polls every 60s for new ones. Self-daemonizes via Unix fork, with PID file management and optional launchd/systemd service generation.

**Tech Stack:** Rust, `notify` v7 (file watcher), `nix` crate (fork/setsid/signals), tokio (async runtime, timers), existing `TokenSave::sync()`, existing `GlobalDb`.

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add `notify` and `nix` dependencies |
| Modify | `src/user_config.rs` | Add `daemon_debounce: String` field |
| Modify | `src/global_db.rs` | Add `list_project_paths()` method |
| Create | `src/daemon.rs` | Core daemon: watcher, debounce, sync loop, PID management, service installer |
| Modify | `src/lib.rs` | Add `pub mod daemon;` |
| Modify | `src/main.rs` | Add `Commands::Daemon` variant and handler |
| Modify | `src/doctor.rs` | Add daemon running/autostart checks |
| Modify | `tests/user_config_test.rs` | Add `daemon_debounce` to round-trip test |

---

### Task 1: Add dependencies and `daemon_debounce` config field

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/user_config.rs`
- Modify: `tests/user_config_test.rs`

- [ ] **Step 1: Add notify and nix to Cargo.toml**

In `[dependencies]`, add:
```toml
notify = { version = "7", default-features = false, features = ["macos_fsevent"] }
nix = { version = "0.29", features = ["signal", "process"] }
```

- [ ] **Step 2: Add `daemon_debounce` to UserConfig**

In `src/user_config.rs`, add after the `installed_agents` field:
```rust
/// Debounce duration for the daemon file watcher (e.g. "15s", "1m").
#[serde(default = "default_daemon_debounce")]
pub daemon_debounce: String,
```

Add the default function:
```rust
fn default_daemon_debounce() -> String {
    "15s".to_string()
}
```

In the `Default` impl, add:
```rust
daemon_debounce: default_daemon_debounce(),
```

- [ ] **Step 3: Update round-trip test**

In `tests/user_config_test.rs`, add `daemon_debounce: "30s".to_string()` to the test struct.

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test user_config`

- [ ] **Step 5: Commit**
```
feat: add notify/nix deps and daemon_debounce config field
```

---

### Task 2: Add `list_project_paths()` to GlobalDb

**Files:**
- Modify: `src/global_db.rs`

- [ ] **Step 1: Add the method**

Add to `impl GlobalDb`:
```rust
/// Returns all tracked project paths.
pub async fn list_project_paths(&self) -> Vec<String> {
    let mut rows = match self
        .conn
        .query("SELECT path FROM projects", ())
        .await
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut paths = Vec::new();
    loop {
        match rows.next().await {
            Ok(Some(row)) => {
                if let Ok(path) = row.get::<String>(0) {
                    paths.push(path);
                }
            }
            _ => break,
        }
    }
    paths
}
```

- [ ] **Step 2: Build**

Run: `cargo build`

- [ ] **Step 3: Commit**
```
feat: add GlobalDb::list_project_paths()
```

---

### Task 3: Create `src/daemon.rs` — duration parser and PID management

**Files:**
- Create: `src/daemon.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create daemon.rs with module doc, imports, and duration parser**

```rust
//! Background daemon that watches all tracked tokensave projects for file
//! changes and runs incremental syncs automatically.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::errors::{Result, TokenSaveError};

/// Parse a human-readable duration string like "15s" or "1m" into a Duration.
/// Returns None if the format is unrecognized.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.trim().parse::<u64>().ok().map(Duration::from_secs)
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.trim().parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
    } else {
        // Try bare number as seconds
        s.parse::<u64>().ok().map(Duration::from_secs)
    }
}

/// Path to the PID file: `~/.tokensave/daemon.pid`.
fn pid_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave").join("daemon.pid"))
}

/// Path to the daemon log file: `~/.tokensave/daemon.log`.
fn log_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave").join("daemon.log"))
}

/// Write the current PID to the PID file.
fn write_pid_file() -> Result<()> {
    let path = pid_file_path().ok_or_else(|| TokenSaveError::Config {
        message: "cannot determine home directory".to_string(),
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, std::process::id().to_string()).map_err(|e| TokenSaveError::Config {
        message: format!("failed to write PID file: {e}"),
    })
}

/// Remove the PID file.
fn remove_pid_file() {
    if let Some(path) = pid_file_path() {
        std::fs::remove_file(path).ok();
    }
}

/// Read the PID from the PID file. Returns None if missing or unreadable.
fn read_pid() -> Option<u32> {
    let path = pid_file_path()?;
    let contents = std::fs::read_to_string(path).ok()?;
    contents.trim().parse().ok()
}

/// Check if a process with the given PID is alive.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid as i32),
        None, // signal 0: check existence
    )
    .is_ok()
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    false
}

/// Returns the PID of the running daemon, or None if not running.
pub fn running_daemon_pid() -> Option<u32> {
    let pid = read_pid()?;
    if is_process_alive(pid) {
        Some(pid)
    } else {
        // Stale PID file
        remove_pid_file();
        None
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
```

- [ ] **Step 2: Add `pub mod daemon;` to lib.rs**

In `src/lib.rs`, add:
```rust
pub mod daemon;
```

- [ ] **Step 3: Build and test**

Run: `cargo build && cargo test daemon`

- [ ] **Step 4: Commit**
```
feat: daemon duration parser and PID file management
```

---

### Task 4: Daemon core event loop

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Add the main daemon run function**

Append to `src/daemon.rs` — this is the core loop. It:
1. Opens the global DB
2. Reads all project paths
3. Sets up `notify` watchers for each
4. Runs a tokio select loop: file events → mark dirty + reset debounce timer; 60s ticker → re-poll global DB for new projects; debounce timer fires → sync the dirty project; SIGTERM/SIGINT → shutdown.

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Instant};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};

/// Directories to ignore inside watched projects.
const IGNORED_DIRS: &[&str] = &[
    ".tokensave", ".git", "node_modules", "target", ".build",
    "__pycache__", ".next", "dist", "build", ".cache",
];

/// Run the daemon event loop. This function does not return until
/// a shutdown signal is received.
pub async fn run(foreground: bool) -> Result<()> {
    if let Some(pid) = running_daemon_pid() {
        return Err(TokenSaveError::Config {
            message: format!("daemon already running (PID: {pid})"),
        });
    }

    if !foreground {
        daemonize()?;
    }

    write_pid_file()?;

    // Set up graceful shutdown on SIGTERM/SIGINT
    let shutdown = tokio::signal::ctrl_c();

    let config = crate::user_config::UserConfig::load();
    let debounce = parse_duration(&config.daemon_debounce)
        .unwrap_or(Duration::from_secs(15));

    let result = run_loop(debounce, shutdown).await;

    remove_pid_file();
    result
}

async fn run_loop(
    debounce: Duration,
    shutdown: impl std::future::Future<Output = std::io::Result<()>>,
) -> Result<()> {
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

    tokio::pin!(shutdown);

    loop {
        // Find the next debounce deadline
        let next_deadline = dirty.values().copied().min();
        let sleep = match next_deadline {
            Some(deadline) => tokio::time::sleep_until(deadline),
            None => tokio::time::sleep(Duration::from_secs(3600)), // park
        };
        tokio::pin!(sleep);

        tokio::select! {
            _ = &mut shutdown => {
                daemon_log("shutting down (signal)");
                break;
            }
            Some(project_root) = rx.recv() => {
                // File change in a project — mark dirty, reset timer
                dirty.insert(project_root, Instant::now() + debounce);
            }
            _ = &mut sleep, if next_deadline.is_some() => {
                // A debounce timer fired — sync all projects past their deadline
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
                // Re-discover projects
                let current = discover_projects().await;
                let current_set: HashSet<&PathBuf> = current.iter().collect();
                let watched_set: HashSet<&PathBuf> = watchers.keys().collect();

                // Add new projects
                for path in current_set.difference(&watched_set) {
                    if let Some(w) = create_watcher(path, tx.clone()) {
                        daemon_log(&format!("discovered new project: {}", path.display()));
                        watchers.insert((*path).clone(), w);
                    }
                }
                // Remove stale projects
                let stale: Vec<PathBuf> = watched_set
                    .difference(&current_set)
                    .map(|p| (*p).clone())
                    .collect();
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

/// Create a notify watcher for a project root. Sends the project root
/// path to `tx` on any relevant file event.
fn create_watcher(project_root: &Path, tx: mpsc::Sender<PathBuf>) -> Option<RecommendedWatcher> {
    let root = project_root.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
        let Ok(event) = res else { return };
        // Only care about create/modify/remove
        if !matches!(
            event.kind,
            notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_)
        ) {
            return;
        }
        // Check if any path in the event should be ignored
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
async fn sync_project(project_root: &Path) {
    let start = std::time::Instant::now();
    let Ok(cg) = crate::tokensave::TokenSave::open(project_root).await else {
        daemon_log(&format!("failed to open {}", project_root.display()));
        return;
    };
    match cg.sync().await {
        Ok(result) => {
            let ms = start.elapsed().as_millis();
            daemon_log(&format!(
                "synced {} — {} added, {} modified, {} removed ({}ms)",
                project_root.display(),
                result.files_added,
                result.files_modified,
                result.files_removed,
                ms
            ));
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

/// Append a timestamped line to the daemon log file.
fn daemon_log(msg: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{now}] {msg}\n");
    // Also print to stderr if running in foreground
    eprint!("{line}");
    if let Some(log_path) = log_file_path() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
```

Wait — this uses `chrono`. Let me avoid adding a new dep and just use `std::time` for the log timestamp. Replace the `daemon_log` function:

```rust
/// Append a timestamped line to the daemon log file.
fn daemon_log(msg: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{secs}] {msg}\n");
    eprint!("{line}");
    if let Some(log_path) = log_file_path() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build`

Note: `notify` v7 will be pulled in from Cargo.toml. If there are API differences (v7 uses `Event` directly, not `DebouncedEvent`), adjust the `create_watcher` call accordingly.

- [ ] **Step 3: Commit**
```
feat: daemon core event loop with file watching and debounced sync
```

---

### Task 5: Daemonize (fork) and stop/status commands

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Add daemonize function**

```rust
/// Fork to background, detach from terminal, redirect stdio to log.
#[cfg(unix)]
fn daemonize() -> Result<()> {
    use nix::unistd::{fork, setsid, ForkResult};

    // First fork
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // Parent exits immediately
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {}
        Err(e) => {
            return Err(TokenSaveError::Config {
                message: format!("fork failed: {e}"),
            });
        }
    }

    // Create new session
    setsid().map_err(|e| TokenSaveError::Config {
        message: format!("setsid failed: {e}"),
    })?;

    // Redirect stdout/stderr to log file
    if let Some(log_path) = log_file_path() {
        use std::os::unix::io::AsRawFd;
        if let Ok(f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let fd = f.as_raw_fd();
            unsafe {
                libc::dup2(fd, libc::STDOUT_FILENO);
                libc::dup2(fd, libc::STDERR_FILENO);
            }
        }
        // Close stdin
        unsafe { libc::close(libc::STDIN_FILENO); }
    }

    Ok(())
}

#[cfg(not(unix))]
fn daemonize() -> Result<()> {
    Err(TokenSaveError::Config {
        message: "daemon mode requires Unix (use --foreground on Windows)".to_string(),
    })
}
```

- [ ] **Step 2: Add stop command**

```rust
/// Stop the running daemon.
pub fn stop() -> Result<()> {
    let Some(pid) = running_daemon_pid() else {
        eprintln!("tokensave daemon is not running");
        return Ok(());
    };

    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        let nix_pid = Pid::from_raw(pid as i32);
        signal::kill(nix_pid, Signal::SIGTERM).ok();

        // Wait up to 5 seconds
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if !is_process_alive(pid) {
                remove_pid_file();
                eprintln!("tokensave daemon stopped (PID: {pid})");
                return Ok(());
            }
        }

        // Force kill
        signal::kill(nix_pid, Signal::SIGKILL).ok();
        remove_pid_file();
        eprintln!("tokensave daemon killed (PID: {pid})");
    }

    #[cfg(not(unix))]
    {
        eprintln!("stop not supported on this platform");
    }

    Ok(())
}
```

- [ ] **Step 3: Add status command**

```rust
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
```

- [ ] **Step 4: Build**

Run: `cargo build`

- [ ] **Step 5: Commit**
```
feat: daemon daemonize, stop, and status commands
```

---

### Task 6: Service installer (--enable-autostart / --disable-autostart)

**Files:**
- Modify: `src/daemon.rs`

- [ ] **Step 1: Add enable_autostart**

```rust
/// Install a system service for automatic daemon startup.
pub fn enable_autostart() -> Result<()> {
    let tokensave_bin = crate::agents::which_tokensave().ok_or_else(|| TokenSaveError::Config {
        message: "tokensave not found on PATH".to_string(),
    })?;

    #[cfg(target_os = "macos")]
    {
        let plist_dir = dirs::home_dir()
            .ok_or_else(|| TokenSaveError::Config { message: "no home dir".to_string() })?
            .join("Library/LaunchAgents");
        std::fs::create_dir_all(&plist_dir).ok();
        let plist_path = plist_dir.join("com.tokensave.daemon.plist");

        let log_path = log_file_path().unwrap_or_else(|| PathBuf::from("/tmp/tokensave-daemon.log"));

        let plist = format!(
r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.tokensave.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{tokensave_bin}</string>
        <string>daemon</string>
        <string>--foreground</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>"#,
            log = log_path.display()
        );

        std::fs::write(&plist_path, plist).map_err(|e| TokenSaveError::Config {
            message: format!("failed to write plist: {e}"),
        })?;
        eprintln!("\x1b[32m✔\x1b[0m Wrote {}", plist_path.display());

        let output = std::process::Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                eprintln!("\x1b[32m✔\x1b[0m Loaded launchd service");
            }
            _ => {
                eprintln!("\x1b[33m!\x1b[0m Could not load service — run: launchctl load {}", plist_path.display());
            }
        }
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let unit_dir = dirs::home_dir()
            .ok_or_else(|| TokenSaveError::Config { message: "no home dir".to_string() })?
            .join(".config/systemd/user");
        std::fs::create_dir_all(&unit_dir).ok();
        let unit_path = unit_dir.join("tokensave-daemon.service");

        let unit = format!(
r#"[Unit]
Description=tokensave file watcher daemon

[Service]
ExecStart={tokensave_bin} daemon --foreground
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#);

        std::fs::write(&unit_path, unit).map_err(|e| TokenSaveError::Config {
            message: format!("failed to write unit file: {e}"),
        })?;
        eprintln!("\x1b[32m✔\x1b[0m Wrote {}", unit_path.display());

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let output = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", "tokensave-daemon.service"])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                eprintln!("\x1b[32m✔\x1b[0m Enabled and started systemd service");
            }
            _ => {
                eprintln!("\x1b[33m!\x1b[0m Could not enable service — run: systemctl --user enable --now tokensave-daemon.service");
            }
        }
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(TokenSaveError::Config {
            message: "autostart not supported on this platform".to_string(),
        });
    }
}
```

- [ ] **Step 2: Add disable_autostart**

```rust
/// Remove the autostart service.
pub fn disable_autostart() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist_path = dirs::home_dir()
            .ok_or_else(|| TokenSaveError::Config { message: "no home dir".to_string() })?
            .join("Library/LaunchAgents/com.tokensave.daemon.plist");
        if plist_path.exists() {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()])
                .status();
            std::fs::remove_file(&plist_path).ok();
            eprintln!("\x1b[32m✔\x1b[0m Removed launchd service");
        } else {
            eprintln!("No launchd service found");
        }
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let unit_path = dirs::home_dir()
            .ok_or_else(|| TokenSaveError::Config { message: "no home dir".to_string() })?
            .join(".config/systemd/user/tokensave-daemon.service");
        if unit_path.exists() {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", "tokensave-daemon.service"])
                .status();
            std::fs::remove_file(&unit_path).ok();
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .status();
            eprintln!("\x1b[32m✔\x1b[0m Removed systemd service");
        } else {
            eprintln!("No systemd service found");
        }
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(TokenSaveError::Config {
            message: "autostart not supported on this platform".to_string(),
        });
    }
}

/// Returns true if an autostart service is installed.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "macos")]
    {
        return dirs::home_dir()
            .map(|h| h.join("Library/LaunchAgents/com.tokensave.daemon.plist").exists())
            .unwrap_or(false);
    }
    #[cfg(target_os = "linux")]
    {
        return dirs::home_dir()
            .map(|h| h.join(".config/systemd/user/tokensave-daemon.service").exists())
            .unwrap_or(false);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        false
    }
}
```

- [ ] **Step 3: Build**

Run: `cargo build`

- [ ] **Step 4: Commit**
```
feat: daemon autostart service installer (launchd/systemd)
```

---

### Task 7: CLI integration and doctor checks

**Files:**
- Modify: `src/main.rs`
- Modify: `src/doctor.rs`

- [ ] **Step 1: Add Commands::Daemon to main.rs**

In the `Commands` enum, add:
```rust
/// Background file watcher daemon
Daemon {
    /// Run in foreground (don't fork)
    #[arg(long)]
    foreground: bool,
    /// Stop the running daemon
    #[arg(long)]
    stop: bool,
    /// Show daemon status
    #[arg(long)]
    status: bool,
    /// Install autostart service (launchd/systemd)
    #[arg(long)]
    enable_autostart: bool,
    /// Remove autostart service
    #[arg(long)]
    disable_autostart: bool,
},
```

- [ ] **Step 2: Add handler in the match block**

In the `match command { ... }` block, add:
```rust
Commands::Daemon { foreground, stop, status, enable_autostart, disable_autostart } => {
    if stop {
        tokensave::daemon::stop()?;
    } else if status {
        let code = tokensave::daemon::status();
        std::process::exit(code);
    } else if enable_autostart {
        tokensave::daemon::enable_autostart()?;
    } else if disable_autostart {
        tokensave::daemon::disable_autostart()?;
    } else {
        tokensave::daemon::run(foreground).await?;
    }
}
```

- [ ] **Step 3: Add daemon checks to doctor.rs**

Add a new function `check_daemon` and call it from `run_doctor`:

```rust
fn check_daemon(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mDaemon\x1b[0m");
    match crate::daemon::running_daemon_pid() {
        Some(pid) => dc.pass(&format!("Daemon is running (PID: {pid})")),
        None => dc.warn("Daemon is not running — run `tokensave daemon` to start"),
    }
    if crate::daemon::is_autostart_enabled() {
        #[cfg(target_os = "macos")]
        dc.pass("Autostart enabled (launchd)");
        #[cfg(target_os = "linux")]
        dc.pass("Autostart enabled (systemd)");
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        dc.pass("Autostart enabled");
    } else {
        dc.warn("Autostart not configured — run `tokensave daemon --enable-autostart`");
    }
}
```

In `run_doctor`, add `check_daemon(&mut dc);` before the network checks.

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`

- [ ] **Step 5: Manual test**

```bash
./target/debug/tokensave daemon --status
./target/debug/tokensave daemon --foreground &
./target/debug/tokensave daemon --status
./target/debug/tokensave daemon --stop
./target/debug/tokensave doctor | grep -A2 Daemon
```

- [ ] **Step 6: Commit**
```
feat: daemon CLI subcommand and doctor integration
```

---

### Task 8: CHANGELOG and final verification

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add CHANGELOG entry**

Add a new `## [Unreleased]` or version section with:
```markdown
### Added
- **Daemon mode** — `tokensave daemon` watches all tracked projects for file changes and runs incremental syncs automatically; debounce configurable via `daemon_debounce` in `~/.tokensave/config.toml` (default `"15s"`)
- **Daemon management** — `--stop`, `--status`, `--foreground` flags for process control; PID file at `~/.tokensave/daemon.pid`
- **Autostart service** — `--enable-autostart` / `--disable-autostart` generates and manages a launchd plist (macOS) or systemd user unit (Linux)
- **Doctor daemon checks** — `tokensave doctor` now reports daemon running status and autostart configuration
```

- [ ] **Step 2: Full test suite**

Run: `cargo test`

- [ ] **Step 3: Release build**

Run: `cargo build --release`

- [ ] **Step 4: Commit**
```
feat: daemon mode — background file watcher with auto-sync
```
