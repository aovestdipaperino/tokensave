//! A registry of running `tokensave serve` processes (#421).
//!
//! `serve` holds an exclusive handle on `.tokensave/tokensave.db` for the life
//! of the process — file watching is why the process is long-lived, and the
//! handle comes with it. On Windows that makes an indexed directory
//! undeletable while any client has a server up, which is routine rather than
//! exotic: a git worktree per task, each one indexed, each one cleaned up
//! afterwards.
//!
//! The problem is not the lock, it is that the lock has no owner you can name.
//! Before v6.0.0 the long-lived process was the daemon, and the daemon had
//! `--status` to print its PID; `serve` inherited none of that, and an MCP
//! client that supplies the project some other way leaves no `--path` in argv
//! either. So v6.0.0 removed a partly-broken stop path and left no path at
//! all, and the workaround in the field is to correlate the `SQLite` `-shm`
//! sidecar's mtime against process start times — inference from an
//! implementation detail, which fails outright when two servers start in the
//! same second.
//!
//! This module makes both directions answerable from files:
//!
//! - **index → process**, by matching `db_path` — what you need to delete a
//!   checkout that will not go away;
//! - **enumerate all**, by listing the directory — what a wrapper needs to
//!   offer a list/stop UI.
//!
//! ## Why `~/.tokensave/servers/`, not `.tokensave/`
//!
//! A PID file beside the index would put the registry in the one directory
//! whose defining problem is that it cannot be deleted: it lives on the locked
//! filesystem, and it vanishes with the checkout being cleaned up. The
//! registry goes in the global directory instead, alongside `config.toml`,
//! `state.toml` and `global.db`.
//!
//! ## Liveness
//!
//! Entries are keyed by PID so a server can remove its own without scanning,
//! and `started_at` records the *process* start time rather than the write
//! time. That turns the field into a PID-reuse check: a live process at a
//! recorded PID whose start time disagrees is a different process, and the
//! entry is stale. Stale entries are reaped on any `serve` startup and on
//! every read, so a hard kill that skips the clean-exit path self-heals.
//!
//! There is deliberately no `stop` here. MCP clients restart their servers, so
//! a stop the host immediately undoes is a trap that looks like a fix;
//! terminating is safe only for a caller who knows whether the host is running
//! and can stop it first, which tokensave cannot know. Identification is the
//! part tokensave owes; the kill stays with whoever has that context.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One running server, as recorded in `~/.tokensave/servers/<pid>.json`.
///
/// This shape is a documented contract: wrappers are expected to read these
/// files directly rather than shell out. Fields may be added; existing ones
/// keep their meaning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    /// OS process id of the server. Also the file's stem.
    pub pid: u32,
    /// Process start time, Unix epoch seconds, as the OS reports it. Together
    /// with `pid` this identifies the process across PID reuse.
    pub started_at: u64,
    /// The project root the server resolved and is serving.
    pub project_path: String,
    /// The project root as given on the command line, when `serve --path` was
    /// used. `None` when the host supplied the project some other way — which
    /// is the common case, and the reason argv alone cannot answer this.
    pub argv_path: Option<String>,
    /// The database file this server holds open. This is the field to match
    /// when the starting point is a directory that will not delete; it accounts
    /// for per-branch databases, so it is not always
    /// `<project_path>/.tokensave/tokensave.db`.
    pub db_path: String,
    /// The tokensave version serving, so a stale binary is visible in a list.
    pub version: String,
}

/// `~/.tokensave/servers/`.
fn registry_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave").join("servers"))
}

fn entry_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("{pid}.json"))
}

/// Process start times for the given PIDs, Unix epoch seconds. A PID absent
/// from the map is not running.
///
/// `memstats::alive_pids` answers the liveness half of this, but not the start
/// time, and the start time is what distinguishes a live server from an
/// unrelated process that inherited its PID.
fn start_times(pids: &[u32]) -> HashMap<u32, u64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let sys_pids: Vec<Pid> = pids
        .iter()
        .filter(|&&p| p != 0)
        .map(|&p| Pid::from_u32(p))
        .collect();
    if sys_pids.is_empty() {
        return HashMap::new();
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&sys_pids),
        true,
        ProcessRefreshKind::new(),
    );
    pids.iter()
        .filter_map(|&pid| {
            let proc = sys.process(Pid::from_u32(pid))?;
            Some((pid, proc.start_time()))
        })
        .collect()
}

/// This process's start time as the OS reports it, so a recorded `started_at`
/// is comparable with what a later `start_times` lookup returns. Falls back to
/// wall-clock now if the process cannot see itself, which only weakens the
/// PID-reuse check rather than breaking the entry.
fn own_start_time(pid: u32) -> u64 {
    start_times(&[pid]).get(&pid).copied().unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    })
}

/// Every entry file in `dir`, paired with what it parsed to. A `None` payload
/// is a file that is corrupt or from a shape this binary does not understand;
/// the path is kept so the caller can remove it.
fn read_all_with_paths(dir: &Path) -> Vec<(PathBuf, Option<ServerEntry>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|path| {
            let parsed = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str::<ServerEntry>(&t).ok());
            (path, parsed)
        })
        .collect()
}

/// Read every entry file, without any liveness filtering. Tests use this to
/// assert on the on-disk shape without depending on which PIDs happen to be
/// alive on the machine running them.
#[cfg(test)]
fn read_all(dir: &Path) -> Vec<ServerEntry> {
    read_all_with_paths(dir)
        .into_iter()
        .filter_map(|(_, e)| e)
        .collect()
}

/// Drop entries whose process is gone, or whose PID now belongs to a different
/// process. Returns the entries that survived.
///
/// A file that cannot be parsed is removed too: it is either corrupt or from a
/// future shape this binary does not understand, and in both cases leaving it
/// to accumulate is worse than dropping a row that names no reachable process.
pub fn reap() -> Vec<ServerEntry> {
    registry_dir().map(|d| reap_at(&d)).unwrap_or_default()
}

/// `reap` against an explicit directory, so tests do not touch the real home
/// and do not race each other over one shared registry.
pub fn reap_at(dir: &Path) -> Vec<ServerEntry> {
    let candidates = read_all_with_paths(dir);

    let pids: Vec<u32> = candidates
        .iter()
        .filter_map(|(_, e)| e.as_ref().map(|e| e.pid))
        .collect();
    let live = start_times(&pids);

    let mut alive = Vec::new();
    for (path, parsed) in candidates {
        let keep = parsed.as_ref().is_some_and(|e| {
            // A one-second disagreement is not PID reuse. Some platforms report
            // a process's start time with a granularity coarser than the clock
            // used when the entry was written, and rejecting on that would reap
            // live servers — a far worse failure than keeping a stale row until
            // the next read.
            live.get(&e.pid)
                .is_some_and(|&now| now.abs_diff(e.started_at) <= 1)
        });
        if keep {
            if let Some(e) = parsed {
                alive.push(e);
            }
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
    alive.sort_by_key(|e| e.pid);
    alive
}

/// Record this process as serving `project_path` out of `db_path`.
///
/// Called once the database is open, so an entry never advertises a lock the
/// process does not yet hold. Best-effort throughout: a server that cannot
/// write its entry still serves, because failing to start over a diagnostic
/// file would trade a working index for a listing.
pub fn register(project_path: &Path, db_path: &Path, argv_path: Option<&str>) {
    if let Some(dir) = registry_dir() {
        register_at(&dir, std::process::id(), project_path, db_path, argv_path);
    }
}

/// `register` against an explicit directory and PID. The PID is a parameter so
/// a test can record an entry for a process other than itself — the reported
/// case is seven servers at once, which one test process cannot be.
pub fn register_at(
    dir: &Path,
    pid: u32,
    project_path: &Path,
    db_path: &Path,
    argv_path: Option<&str>,
) {
    // Reaping here as well as on read means a machine that only ever starts
    // servers still converges, rather than accumulating rows until something
    // happens to list them.
    reap_at(dir);

    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let entry = ServerEntry {
        pid,
        started_at: own_start_time(pid),
        project_path: project_path.to_string_lossy().into_owned(),
        argv_path: argv_path.map(str::to_string),
        db_path: db_path.to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let Ok(json) = serde_json::to_string_pretty(&entry) else {
        return;
    };
    let _ = std::fs::write(entry_path(dir, pid), json);
}

/// Remove this process's entry. Called from the graceful shutdown path; a hard
/// kill skips it, which is what reaping is for.
pub fn unregister() {
    if let Some(dir) = registry_dir() {
        let _ = std::fs::remove_file(entry_path(&dir, std::process::id()));
    }
}

/// Every live server, stale entries reaped first.
pub fn list() -> Vec<ServerEntry> {
    reap()
}

/// `list` against an explicit directory.
pub fn list_at(dir: &Path) -> Vec<ServerEntry> {
    reap_at(dir)
}

/// Render the listing for humans.
pub fn render(entries: &[ServerEntry]) -> String {
    use std::fmt::Write as _;

    if entries.is_empty() {
        return "No tokensave servers running.\n".to_string();
    }

    let pid_w = entries
        .iter()
        .map(|e| e.pid.to_string().len())
        .max()
        .unwrap_or(3)
        .max(3);
    let ver_w = entries
        .iter()
        .map(|e| e.version.len())
        .max()
        .unwrap_or(7)
        .max(7);

    let mut out = format!(
        "{:>pid_w$}  {:>ver_w$}  {}\n",
        "PID",
        "VERSION",
        "PROJECT",
        pid_w = pid_w,
        ver_w = ver_w
    );
    for e in entries {
        let _ = writeln!(
            out,
            "{:>pid_w$}  {:>ver_w$}  {}",
            e.pid,
            e.version,
            e.project_path,
            pid_w = pid_w,
            ver_w = ver_w
        );
        // The database is the thing actually held open, and with per-branch
        // databases it is not always derivable from the project root — so show
        // it whenever it is not the default the reader would have guessed.
        let default_db = Path::new(&e.project_path)
            .join(".tokensave")
            .join("tokensave.db");
        if Path::new(&e.db_path) != default_db {
            let _ = writeln!(
                out,
                "{:>pid_w$}  {:>ver_w$}  └─ db: {}",
                "",
                "",
                e.db_path,
                pid_w = pid_w,
                ver_w = ver_w
            );
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn entry(pid: u32, started_at: u64, project: &str) -> ServerEntry {
        ServerEntry {
            pid,
            started_at,
            project_path: project.to_string(),
            argv_path: None,
            db_path: format!("{project}/.tokensave/tokensave.db"),
            version: "9.9.9".to_string(),
        }
    }

    fn write(dir: &Path, e: &ServerEntry) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            entry_path(dir, e.pid),
            serde_json::to_string_pretty(e).unwrap(),
        )
        .unwrap();
    }

    /// The whole point of the registry: a `db_path` on disk resolves to the
    /// process holding it, which is the direction argv cannot answer.
    #[test]
    fn an_entry_names_the_process_holding_a_database() {
        let dir = tempfile::tempdir().unwrap();
        let e = entry(18012, 1_700_000_000, "/proj/a");
        write(dir.path(), &e);

        let found: Vec<_> = read_all(dir.path())
            .into_iter()
            .filter(|x| x.db_path == "/proj/a/.tokensave/tokensave.db")
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pid, 18012);
    }

    /// The reported case: seven servers, five with no project in argv. The
    /// registry records the resolved path either way, and `argv_path`
    /// distinguishes what the host actually launched.
    #[test]
    fn a_server_launched_without_a_path_still_records_its_project() {
        let dir = tempfile::tempdir().unwrap();
        let mut bare = entry(46000, 1_700_000_000, "/proj/bare");
        let mut explicit = entry(44224, 1_700_000_000, "/proj/explicit");
        explicit.argv_path = Some("/proj/explicit".to_string());
        write(dir.path(), &bare);
        write(dir.path(), &explicit);

        let all = read_all(dir.path());
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|e| !e.project_path.is_empty()));
        bare.argv_path = None;
        assert!(all.iter().any(|e| e.pid == 46000 && e.argv_path.is_none()));
        assert!(all
            .iter()
            .any(|e| e.pid == 44224 && e.argv_path.as_deref() == Some("/proj/explicit")));
    }

    /// A live process at a recorded PID is not necessarily the recorded
    /// process. This is the ambiguity the field workaround could not resolve:
    /// correlating `-shm` mtime against start times fails when two servers
    /// start in the same second, and PID reuse defeats it outright.
    #[test]
    fn a_reused_pid_does_not_pass_for_the_recorded_process() {
        let own = std::process::id();
        let real = own_start_time(own);
        let stale = entry(own, real.saturating_sub(10_000), "/proj/gone");
        let live = start_times(&[own]);

        assert!(
            live.contains_key(&own),
            "this process must see itself as running"
        );
        assert!(
            live[&own].abs_diff(stale.started_at) > 1,
            "a start time 10,000s off must not be accepted as the same process"
        );
    }

    /// A server that exits without running its clean-exit path leaves a row
    /// behind. The reporter's environment kills servers at worktree teardown,
    /// so this is the normal case rather than the exceptional one.
    #[test]
    fn a_dead_process_is_reaped_on_read() {
        let dir = tempfile::tempdir().unwrap();
        // PID 1 exists everywhere but did not start when we claim it did, so
        // it stands in for a PID that has been reused since the entry was
        // written. A never-allocated PID covers the plain-dead case.
        write(dir.path(), &entry(1, 42, "/proj/reused"));
        write(dir.path(), &entry(4_294_967_294, 42, "/proj/gone"));
        assert_eq!(read_all(dir.path()).len(), 2, "both are on disk to start");

        let alive = reap_at(dir.path());
        assert!(alive.is_empty(), "neither names a live server: {alive:?}");
        assert!(
            read_all(dir.path()).is_empty(),
            "reaping removes the files, not just the returned rows"
        );
    }

    /// The live entry must survive a reap that clears the dead ones around it.
    #[test]
    fn a_live_server_survives_a_reap() {
        let dir = tempfile::tempdir().unwrap();
        let own = std::process::id();
        register_at(
            dir.path(),
            own,
            Path::new("/proj/live"),
            Path::new("/proj/live/.tokensave/tokensave.db"),
            None,
        );
        write(dir.path(), &entry(4_294_967_294, 42, "/proj/gone"));

        let alive = list_at(dir.path());
        assert_eq!(alive.len(), 1, "got {alive:?}");
        assert_eq!(alive[0].pid, own);
        assert_eq!(alive[0].project_path, "/proj/live");
        assert_eq!(alive[0].version, env!("CARGO_PKG_VERSION"));
    }

    /// Registering twice for the same PID replaces the row rather than
    /// accumulating: a restarted server reuses its own file.
    #[test]
    fn re_registering_replaces_rather_than_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let own = std::process::id();
        for project in ["/proj/first", "/proj/second"] {
            register_at(
                dir.path(),
                own,
                Path::new(project),
                Path::new("/db"),
                Some(project),
            );
        }
        let all = read_all(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].project_path, "/proj/second");
    }

    #[test]
    fn an_unparseable_entry_is_not_mistaken_for_a_server() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(entry_path(dir.path(), 1), b"{ not json").unwrap();
        assert!(read_all(dir.path()).is_empty());
        assert!(reap_at(dir.path()).is_empty());
        assert!(
            !entry_path(dir.path(), 1).exists(),
            "a file that cannot be parsed is removed rather than left to accumulate"
        );
    }

    #[test]
    fn an_empty_registry_says_so_rather_than_printing_a_bare_header() {
        assert_eq!(render(&[]), "No tokensave servers running.\n");
    }

    /// A per-branch database is not derivable from the project root, so the
    /// listing has to show it or the reverse lookup silently misses.
    #[test]
    fn a_non_default_database_path_is_shown() {
        let plain = entry(1, 0, "/proj/a");
        let mut branched = entry(2, 0, "/proj/b");
        branched.db_path = "/proj/b/.tokensave/branches/feature.db".to_string();

        let out = render(&[plain, branched]);
        assert!(
            !out.contains("/proj/a/.tokensave/tokensave.db"),
            "the default path is already implied by the project column"
        );
        assert!(out.contains("/proj/b/.tokensave/branches/feature.db"));
    }
}
