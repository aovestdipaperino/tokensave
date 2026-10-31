//! Bounded, read-only SQLite file checks, isolated from the serving connection.
use crate::errors::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

pub const DEFAULT_INTERVAL_SECS: u64 = 86_400;
/// Retry delay after a check deferred for activity, capped by the interval.
pub const DEFERRED_RETRY_SECS: u64 = 900;
const BUDGET: Duration = Duration::from_secs(5);

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub outcome: String,
    pub full: bool,
    pub diagnostics: Vec<String>,
}
impl Report {
    fn new(full: bool) -> Self {
        Self {
            started_at: crate::tokensave::current_timestamp(),
            finished_at: None,
            outcome: "running".into(),
            full,
            diagnostics: Vec::new(),
        }
    }
    pub fn exit_code(&self) -> i32 {
        match self.outcome.as_str() {
            "healthy" => 0,
            "corrupt" => 1,
            _ => 2,
        }
    }
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
pub fn load(path: &Path) -> Result<Report> {
    let mut bytes = Vec::new();
    let path = sidecar(path, ".integrity.json");
    if !std::fs::metadata(&path)?.is_file() {
        return Err(std::io::Error::other("integrity report is not a regular file").into());
    }
    File::open(path)?
        .take(1024 * 1024)
        .read_to_end(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
/// Reads the saved report for `--status`. A `running` record whose checker
/// lock is free is reported as interrupted: its worker died without a result.
pub fn status(path: &Path) -> Result<Report> {
    let mut report = load(path)?;
    if report.outcome == "running" {
        let held = File::open(sidecar(path, ".integrity.lock"))
            .is_ok_and(|lock| lock.try_lock_exclusive().is_err());
        if !held {
            report.outcome = "interrupted".into();
            report
                .diagnostics
                .push("no checker is running; the previous check ended without a result".into());
        }
    }
    Ok(report)
}
/// Whether the persisted interval allows a new attempt. Checks deferred for
/// activity retry sooner, so a busy machine still completes them eventually.
fn due(path: &Path, interval: u64) -> bool {
    let Ok(previous) = load(path) else {
        return true;
    };
    let wait = if previous.outcome == "deferred" {
        interval.min(DEFERRED_RETRY_SECS)
    } else {
        interval
    };
    let elapsed = crate::tokensave::current_timestamp().saturating_sub(previous.started_at);
    elapsed < 0 || elapsed as u64 >= wait
}
fn save(path: &Path, report: &Report) -> Result<()> {
    use std::io::Write;
    let target = sidecar(path, ".integrity.json");
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("report has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec(report)?)?;
    temporary.as_file().sync_all()?;
    // Atomic replacement does not follow a preexisting sidecar symlink.
    temporary.persist(target).map_err(|error| error.error)?;
    Ok(())
}

/// Worker entry point. Never opens the database through recovery or migrations.
pub async fn inspect(path: &Path, full: bool) -> Report {
    let mut report = Report::new(full);
    let result: Result<()> = async {
        if !std::fs::metadata(path)?.is_file() {
            return Err(std::io::Error::other("database is not a regular file").into());
        }
        let db = libsql::Builder::new_local(path)
            .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY).build().await?;
        let conn = db.connect()?;
        conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=0; PRAGMA cache_size=-2048; PRAGMA mmap_size=0;").await?;
        crate::db::compatibility::check(&conn, u32::MAX).await?;
        let sql = if full { "PRAGMA integrity_check(100)" } else { "PRAGMA quick_check(100)" };
        let mut rows = conn.query(sql, ()).await?;
        let mut saw_ok = false;
        while let Some(row) = rows.next().await? {
            let message: String = row.get(0)?;
            if message == "ok" { saw_ok = true; }
            else if report.diagnostics.len() < 100 {
                report.diagnostics.push(crate::text::utf8_prefix_at_or_before(&message, 1024).into());
            }
        }
        report.outcome = if saw_ok && report.diagnostics.is_empty() { "healthy" } else { "corrupt" }.into();
        Ok(())
    }.await;
    if let Err(error) = result {
        // Lock, compatibility and I/O failures are incomplete checks, not proof of corruption.
        report.outcome = "error".into();
        report
            .diagnostics
            .push(crate::text::utf8_prefix_at_or_before(&error.to_string(), 1024).into());
    }
    report.finished_at = Some(crate::tokensave::current_timestamp());
    report
}

pub async fn worker(path: &Path, full: bool) -> Result<()> {
    // A native watchdog also bounds a worker whose parent is killed, even while
    // SQLite is executing synchronously and cannot yield to a Tokio timer.
    let (done, receiver) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if receiver.recv_timeout(BUDGET).is_err() {
            std::process::exit(2);
        }
    });
    let result = save(path, &inspect(path, full).await);
    let _ = done.send(());
    let _ = watchdog.join();
    result
}

/// One child and one OS lock per database. Normal exits explicitly kill/reap;
/// `kill_on_drop` also prevents a cancelled owner from leaving a running checker.
pub struct Check {
    child: tokio::process::Child,
    path: PathBuf,
    started: Instant,
    report: Report,
    _lock: File,
}
impl Check {
    pub fn start(
        path: &Path,
        executable: &Path,
        full: bool,
        interval: Option<u64>,
    ) -> Result<Option<Self>> {
        if interval == Some(0) {
            return Ok(None);
        }
        let path = path.canonicalize()?;
        // Gate before touching the lockfile so idle ticks within the interval do no I/O
        // beyond reading the report; re-checked under the lock against a racing checker.
        if interval.is_some_and(|interval| !due(&path, interval)) {
            return Ok(None);
        }
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(sidecar(&path, ".integrity.lock"))?;
        if let Err(error) = lock.try_lock_exclusive() {
            // Windows reports contention as ERROR_LOCK_VIOLATION, not WouldBlock.
            if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() {
                return Ok(None);
            }
            return Err(error.into());
        }
        if interval.is_some_and(|interval| !due(&path, interval)) {
            return Ok(None);
        }
        let report = Report::new(full);
        save(&path, &report)?;
        let mut command = tokio::process::Command::new(executable);
        command
            .arg("integrity-check")
            .arg(&path)
            .arg("--worker")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if full {
            command.arg("--full");
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let mut failed = report;
                failed.outcome = "error".into();
                failed.finished_at = Some(crate::tokensave::current_timestamp());
                failed.diagnostics.push(error.to_string());
                save(&path, &failed)?;
                return Err(error.into());
            }
        };
        Ok(Some(Self {
            child,
            path,
            started: Instant::now(),
            report,
            _lock: lock,
        }))
    }
    pub async fn poll(&mut self) -> Result<bool> {
        if let Some(status) = self.child.try_wait()? {
            if !status.success() || load(&self.path)?.outcome == "running" {
                self.finish("interrupted", "checker exited without a completed result")?;
            }
            return Ok(true);
        }
        if self.started.elapsed() >= BUDGET {
            self.stop("interrupted", "check exceeded the five-second budget")
                .await?;
            return Ok(true);
        }
        Ok(false)
    }
    fn finish(&mut self, outcome: &str, reason: &str) -> Result<()> {
        self.report.outcome = outcome.into();
        self.report.finished_at = Some(crate::tokensave::current_timestamp());
        self.report.diagnostics = vec![reason.into()];
        save(&self.path, &self.report)
    }
    /// Kills a running checker and records `outcome`: `interrupted` for a
    /// failed attempt, `deferred` for one yielded to request activity.
    pub async fn stop(&mut self, outcome: &str, reason: &str) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().await?; // Tokio kills and waits, reaping the child.
            self.finish(outcome, reason)?;
        }
        Ok(())
    }
    pub fn report(&self) -> Result<Report> {
        load(&self.path)
    }
}

/// The idle loop owns this scheduler; no detached tasks or queues.
pub struct Scheduler {
    path: PathBuf,
    interval: u64,
    check: Option<Check>,
}
impl Scheduler {
    pub fn new(path: PathBuf, interval: u64) -> Self {
        Self {
            path,
            interval,
            check: None,
        }
    }
    pub fn delay(&self) -> Duration {
        Duration::from_secs(if self.check.is_some() { 1 } else { 60 })
    }
    pub async fn tick(&mut self, busy: bool) -> Result<()> {
        if busy {
            return self.stop().await;
        }
        if let Some(check) = self.check.as_mut() {
            if check.poll().await? {
                let report = check.report();
                self.check = None;
                log_report(&report?);
            }
        } else if self.interval != 0 {
            self.check = Check::start(
                &self.path,
                &std::env::current_exe()?,
                false,
                Some(self.interval),
            )?;
        }
        Ok(())
    }
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut check) = self.check.take() {
            check
                .stop(
                    "deferred",
                    "deferred for request activity or server shutdown",
                )
                .await?;
            log_report(&check.report()?);
        }
        Ok(())
    }
}

fn log_report(report: &Report) {
    // A deferral is routine scheduling, not a finding worth a warning.
    if report.outcome != "healthy" && report.outcome != "deferred" {
        eprintln!("[tokensave] database integrity {}: {:?}. Preserve the database and WAL before recovery; run integrity-check for diagnostics.", report.outcome, report.diagnostics);
    }
}
