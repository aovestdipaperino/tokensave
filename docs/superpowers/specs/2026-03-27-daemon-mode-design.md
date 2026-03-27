# Daemon Mode Design Spec

## Goal

A background daemon that watches all tracked tokensave projects for file changes and automatically runs incremental syncs, keeping the code graph up-to-date without manual `tokensave sync` invocations.

## CLI Surface

```
tokensave daemon              # start (forks to background, writes PID file)
tokensave daemon --foreground # stay in foreground (for debugging / service managers)
tokensave daemon --stop       # kill running daemon via PID file
tokensave daemon --status     # check if running
tokensave daemon --enable-autostart   # install launchd/systemd service
tokensave daemon --disable-autostart  # remove service
```

All flags are mutually exclusive. `--foreground` is useful for debugging and when the daemon is managed by an external service manager.

## Config

Add `daemon_debounce` to `~/.tokensave/config.toml`:

```toml
daemon_debounce = "15s"
```

A simple duration parser understands `s` (seconds) and `m` (minutes). Examples: `"15s"`, `"30s"`, `"1m"`, `"2m"`. Stored as `String` in `UserConfig`, parsed at runtime with `parse_duration()`. Default is `"15s"` if absent or unparseable.

## Architecture

### Components

| Component | File | Responsibility |
|-----------|------|----------------|
| DaemonRunner | `src/daemon.rs` | Core event loop: project discovery, file watching, debounce, sync dispatch |
| PID management | `src/daemon.rs` | Write/read/check `~/.tokensave/daemon.pid` |
| Duration parser | `src/daemon.rs` | Parse `"15s"` / `"1m"` strings into `Duration` |
| Service installer | `src/daemon.rs` | Generate launchd plist (macOS) / systemd user unit (Linux) |
| CLI integration | `src/main.rs` | `Commands::Daemon` enum variant with flags |
| Doctor integration | `src/doctor.rs` | Check if daemon is running |

### Data Flow

1. **Startup:** Open global DB, read all project paths from `projects` table, set up a `notify::RecommendedWatcher` watching each project root recursively.

2. **Project discovery (every 60s):** Re-read the global DB projects table. Add watchers for new projects. Remove watchers for projects no longer in the DB.

3. **File change event:** When `notify` fires an event, determine which project it belongs to (by matching the event path against watched project roots). Mark that project as "dirty" and start/reset its per-project debounce timer.

4. **Debounce fires (default 15s after last change):** Open `TokenSave::open()` for the dirty project, call `sync()`, log the result (files added/modified/removed, duration). Update global DB token count.

5. **Filtering:** Ignore change events inside `.tokensave/`, `.git/`, `node_modules/`, `target/`, `.build/`, and other common build output directories. Also ignore events for files that don't match any supported language extension.

### Self-Daemonizing

On `tokensave daemon` (without `--foreground`):

1. Fork the process using `fork()` (Unix) or equivalent
2. Detach from terminal (setsid, close stdin/stdout/stderr, redirect to log file `~/.tokensave/daemon.log`)
3. Write PID to `~/.tokensave/daemon.pid`
4. Enter the main event loop

On `--foreground`: skip the fork, keep stderr/stdout attached, still write PID file.

### PID File Management

- **Path:** `~/.tokensave/daemon.pid`
- **Write:** On daemon start, write the PID as a plain integer
- **Read:** On `--stop` and `--status`, read the PID and check if the process is alive (`kill(pid, 0)` on Unix)
- **Stale detection:** If PID file exists but the process is dead, treat as not running. On start, overwrite stale PID files.
- **Cleanup:** On graceful shutdown (SIGTERM/SIGINT), delete the PID file

### `--stop`

Read PID file, check process alive, send SIGTERM. Wait up to 5 seconds for process to exit. If still alive after 5s, send SIGKILL. Remove PID file.

### `--status`

Read PID file, check process alive. Print:
- "tokensave daemon is running (PID: 12345)" or
- "tokensave daemon is not running"

Exit code 0 if running, 1 if not.

### `--enable-autostart`

**macOS (launchd):**

Write `~/Library/LaunchAgents/com.tokensave.daemon.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.tokensave.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/tokensave</string>
        <string>daemon</string>
        <string>--foreground</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>~/.tokensave/daemon.log</string>
    <key>StandardErrorPath</key>
    <string>~/.tokensave/daemon.log</string>
</dict>
</plist>
```

Then run `launchctl load <plist>`.

**Linux (systemd):**

Write `~/.config/systemd/user/tokensave-daemon.service`:
```ini
[Unit]
Description=tokensave file watcher daemon

[Service]
ExecStart=/path/to/tokensave daemon --foreground
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

Then run `systemctl --user enable --now tokensave-daemon.service`.

### `--disable-autostart`

**macOS:** `launchctl unload <plist>`, then delete the plist file.

**Linux:** `systemctl --user disable --now tokensave-daemon.service`, then delete the unit file.

## Error Handling

- **Project path gone:** Skip it on next discovery cycle, don't crash. Log a warning once.
- **sync() failure:** Log the error, continue watching. Don't retry immediately — wait for next file change.
- **Global DB unreachable:** Retry on next 60s poll cycle. Keep existing watchers running.
- **Watcher limit exhaustion:** If the OS file watch limit is hit, log a warning and suggest increasing `fs.inotify.max_user_watches` (Linux) or similar. Continue watching already-registered projects.
- **Permission errors:** Log and skip the project.

## Doctor Integration

Add a "Daemon" section to `tokensave doctor` output:

```
Daemon
  ✔ Daemon is running (PID: 12345)
```
or
```
Daemon
  ! Daemon is not running — run `tokensave daemon` to start
```

Also check if autostart is enabled:
```
  ✔ Autostart enabled (launchd)
```
or
```
  ! Autostart not configured — run `tokensave daemon --enable-autostart`
```

## Dependencies

- `notify` crate (v7) — cross-platform file system watcher
- `nix` crate — Unix fork/setsid/signal handling (or use `libc` directly)

## Logging

The daemon writes to `~/.tokensave/daemon.log`. Log format:

```
[2026-03-27 14:32:01] started, watching 3 projects
[2026-03-27 14:32:16] synced /Users/foo/myproject — 2 added, 1 modified, 0 removed (45ms)
[2026-03-27 14:33:01] discovered new project: /Users/foo/another
[2026-03-27 14:33:05] synced /Users/foo/another — 0 added, 0 modified, 0 removed (12ms)
[2026-03-27 14:35:00] shutting down (SIGTERM)
```

## Out of Scope

- Windows service support (can be added later)
- Remote/network file system watching
- Per-project debounce overrides (global config only)
- Web UI or dashboard
