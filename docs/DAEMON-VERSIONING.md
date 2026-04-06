# Daemon Versioning & Auto-Upgrade

## How it works

The tokensave daemon runs as a background service (launchd on macOS, systemd on
Linux, Windows Service on Windows). It watches project directories for file
changes and runs incremental syncs automatically.

When the user upgrades tokensave (`brew upgrade`, `cargo install`, `scoop update`),
the binary on disk changes but the running daemon is still the old version. The
daemon handles this with a two-part mechanism:

### 1. Binary snapshot polling

At startup, the daemon captures a snapshot of its own binary's **mtime** and
**size** via `std::env::current_exe()`. Every 60 seconds (on the discovery
interval tick), it re-checks the binary metadata. If the mtime or size differs,
the daemon:

1. Logs `"binary updated on disk, restarting to pick up new version"`
2. Flushes all pending project syncs
3. Exits with a non-zero exit code

### 2. Service manager restart

The service manager is configured to automatically restart the daemon on
non-zero exit:

| Platform | Mechanism | Config |
|----------|-----------|--------|
| macOS | launchd `KeepAlive: true` | Restarts on any exit |
| Linux | systemd `Restart=on-failure` | Restarts on non-zero exit |
| Windows | SCM failure actions | Restart after 5s, then 10s |

When the daemon restarts, it picks up the new binary, logs its new version
(`v3.4.0 started, watching N projects`), and resumes watching.

### Status API

The daemon runs a TCP status API on an ephemeral port (written to
`~/.tokensave/daemon.port`). `tokensave daemon --status` queries this API
to show:

```
tokensave daemon v3.4.0 is running (PID: 1234, uptime: 2h 15m, watching 5 projects)
```

After an upgrade restart, the version and uptime reset to reflect the new
binary.

### 3. Version mismatch detection

After printing the daemon status, `tokensave daemon --status` compares the
daemon's reported version against the CLI binary's version. If they differ,
it prints a yellow warning with context-aware advice:

| Scenario | Warning |
|----------|---------|
| CLI is newer than daemon | "The daemon hasn't restarted after the upgrade. It should auto-restart within 60s." |
| Daemon is beta, CLI is stable | "The daemon is running a beta version. Restart it to use the stable release." |
| Daemon is stable, CLI is beta | "The daemon is running a stable version. Restart it to use the beta release." |
| CLI is older than daemon | "The CLI is older than the running daemon. Restart to align versions." |

All warnings include the corrective command:

```
tokensave daemon --stop && tokensave daemon --enable-autostart
```

This catches the window between an upgrade and the daemon's 60-second polling
cycle, as well as cross-channel mismatches (beta daemon running with a stable
CLI or vice versa).

---

## Manual Testing

### Prerequisites

- tokensave built and installed on PATH
- The daemon autostart service is installed (`tokensave daemon --enable-autostart`)
- At least one project has been indexed (`tokensave sync` in a project directory)

### Test 1: Verify daemon is running

```bash
tokensave daemon --status
```

**Expected**: Output shows the daemon is running with current version, PID,
uptime, and project count. Example:

```
tokensave daemon v3.3.2 is running (PID: 12345, uptime: 1h 30m, watching 3 projects)
```

Record the **PID** and **version** for later comparison.

---

### Test 2: Verify daemon log exists

```bash
cat ~/.tokensave/daemon.log | tail -5
```

**Expected**: Recent log entries with timestamps. The first line after a start
should show the version: `[1712345678] v3.3.2 started, watching 3 projects`.

---

### Test 3: Verify file-change sync works

In an indexed project, modify a source file:

```bash
echo "// daemon test" >> src/lib.rs
sleep 20
cat ~/.tokensave/daemon.log | tail -5
```

**Expected**: Within the debounce window (default 15s), the daemon log shows a
sync line like:

```
[1712345699] synced /path/to/project — 0 added, 1 modified, 0 removed (120ms)
```

Clean up:

```bash
git checkout -- src/lib.rs
```

---

### Test 4: Simulate an upgrade

Record the current daemon state:

```bash
tokensave daemon --status
tokensave --version
```

Now rebuild and reinstall tokensave to simulate an upgrade. The binary on disk
changes, which the daemon will detect.

**Option A — cargo install (from source):**

```bash
cargo install --path . --force
```

**Option B — touch the binary (simulates any package manager):**

```bash
touch "$(which tokensave)"
```

Both change the binary's mtime, which triggers the daemon's upgrade detection.

---

### Test 5: Verify daemon detects the upgrade

Wait up to 60 seconds (the discovery interval), then check the log:

```bash
sleep 65
cat ~/.tokensave/daemon.log | tail -10
```

**Expected**: The log shows the upgrade detection sequence:

```
[1712345780] binary updated on disk, restarting to pick up new version
[1712345781] v3.4.0 started, watching 3 projects
```

The first line is the old daemon exiting. The second line is the new daemon
starting (the service manager restarted it).

---

### Test 6: Verify new version is running

```bash
tokensave daemon --status
```

**Expected**:
- **Version** matches the newly installed version (e.g. `v3.4.0`)
- **PID** is different from the one recorded in Test 1
- **Uptime** is low (seconds or minutes, not hours)
- **Projects watched** count is the same as before

---

### Test 7: Verify syncing still works after upgrade

```bash
echo "// post-upgrade test" >> src/lib.rs
sleep 20
cat ~/.tokensave/daemon.log | tail -5
```

**Expected**: Sync line appears, confirming the new daemon version is fully
operational.

Clean up:

```bash
git checkout -- src/lib.rs
```

---

### Test 8: Verify pending syncs are flushed before restart

This tests that the daemon doesn't drop in-flight work during an upgrade.

Trigger a file change and immediately simulate an upgrade:

```bash
echo "// flush test" >> src/lib.rs
sleep 2
touch "$(which tokensave)"
sleep 65
cat ~/.tokensave/daemon.log | tail -15
```

**Expected**: The log shows:
1. A sync line for the modified file (flushed before exit)
2. The upgrade detection message
3. The new version startup

The sync should appear **before** the `binary updated on disk` line, confirming
pending work was flushed.

Clean up:

```bash
git checkout -- src/lib.rs
```

---

### Test 9: Manual foreground restart

Stop the service and run in foreground to observe upgrade behavior directly:

```bash
tokensave daemon --stop
tokensave daemon --foreground &
DAEMON_PID=$!
sleep 5
tokensave daemon --status
```

**Expected**: Status shows the daemon running with the current version.

Now simulate upgrade:

```bash
touch "$(which tokensave)"
sleep 65
```

**Expected**: The foreground process exits. The shell shows the process
terminated. Check exit code:

```bash
wait $DAEMON_PID
echo "Exit code: $?"
```

**Expected**: Exit code is `1` (non-zero, signalling upgrade to the service
manager).

Restart the service:

```bash
tokensave daemon --enable-autostart
```

---

### Test 10: Daemon survives unchanged binary

Verify the daemon does NOT restart when the binary hasn't changed:

```bash
tokensave daemon --status
```

Record the PID. Wait 2+ minutes:

```bash
sleep 130
tokensave daemon --status
```

**Expected**: Same PID, uptime increased by ~2 minutes. No spurious restarts.

---

### Test 11: Service manager restart on crash

Kill the daemon process directly to verify the service manager restarts it:

```bash
tokensave daemon --status
```

Record the PID, then:

```bash
kill -9 $(cat ~/.tokensave/tokensave-daemon.pid)
sleep 10
tokensave daemon --status
```

**Expected**: The daemon is running again with a new PID and low uptime.
The service manager detected the abnormal exit and restarted it.

---

### Test 12: Status API port file cleanup

```bash
cat ~/.tokensave/daemon.port
```

**Expected**: Contains a port number (e.g. `54321`).

```bash
tokensave daemon --stop
cat ~/.tokensave/daemon.port 2>/dev/null
```

**Expected**: The port file is removed on clean shutdown.

Restart:

```bash
tokensave daemon --enable-autostart
sleep 5
cat ~/.tokensave/daemon.port
```

**Expected**: A new port file exists with a (potentially different) port number.

---

### Test 13: Version mismatch warning — upgrade pending

Simulate the window between an upgrade and the daemon's auto-restart. The
daemon is still running the old version while the CLI is the new version.

First, confirm versions match:

```bash
tokensave daemon --status
```

**Expected**: No warning. Versions match.

Now rebuild the CLI with a bumped version to simulate an upgrade, without
restarting the daemon:

```bash
tokensave daemon --stop
# Start daemon with the current version
tokensave daemon --foreground &
sleep 3

# Rebuild CLI with a different version (simulates upgrade)
# Option A: actually rebuild with a patch bump
# Option B: use the daemon status API directly to verify the logic
tokensave daemon --status
```

If the daemon hasn't auto-restarted yet (within the 60s window), the output
should show:

```
tokensave daemon v3.3.2 is running (PID: 12345, uptime: 3s, watching 3 projects)

Warning: version mismatch — CLI is v3.4.0, daemon is v3.3.2
  The daemon hasn't restarted after the upgrade. It should auto-restart within 60s.
  To restart now: tokensave daemon --stop && tokensave daemon --enable-autostart
```

Clean up:

```bash
tokensave daemon --stop
tokensave daemon --enable-autostart
```

---

### Test 14: Version mismatch warning — beta/stable cross

This tests the scenario where the daemon was started from a beta build but the
user has since switched to a stable release (or vice versa).

To simulate without actually installing two versions, you can modify the
daemon's status API response or compare against the logic directly. The key
behavior to verify:

**Daemon is beta, CLI is stable:**

```
tokensave daemon v3.4.0-beta.1 is running (PID: 12345, ...)

Warning: version mismatch — CLI is v3.4.0, daemon is v3.4.0-beta.1
  The daemon is running a beta version. Restart it to use the stable release:
  tokensave daemon --stop && tokensave daemon --enable-autostart
```

**Daemon is stable, CLI is beta:**

```
tokensave daemon v3.3.2 is running (PID: 12345, ...)

Warning: version mismatch — CLI is v3.4.0-beta.1, daemon is v3.3.2
  The daemon is running a stable version. Restart it to use the beta release:
  tokensave daemon --stop && tokensave daemon --enable-autostart
```

---

### Test 15: No warning when versions match

```bash
tokensave daemon --status
```

**Expected**: Status line only, no warning. This confirms the version check
doesn't produce false positives.

---

## Platform-specific notes

### macOS (launchd)

The plist is installed at `~/Library/LaunchAgents/com.tokensave.daemon.plist`
with `KeepAlive: true`. To inspect:

```bash
launchctl list | grep tokensave
cat ~/Library/LaunchAgents/com.tokensave.daemon.plist
```

### Linux (systemd)

The unit is installed at `~/.config/systemd/user/tokensave-daemon.service`
with `Restart=on-failure`. To inspect:

```bash
systemctl --user status tokensave-daemon
journalctl --user -u tokensave-daemon --since "5 minutes ago"
```

### Windows (SCM)

The service is registered as `tokensave-daemon` with failure recovery actions.
To inspect:

```powershell
Get-Service tokensave-daemon
sc.exe qfailure tokensave-daemon
```
