//! Bounded on-disk storage under `~/.tokensave/profile/`.
//!
//! Each process accumulates drained snapshots for the current hour and
//! rewrites one file per hour, `<hour>-<pid>-<nonce>.json`, by atomic temp
//! file plus rename. Reports read and merge every file, so aggregates from
//! the CLI and several MCP servers combine exactly. Data drained during a
//! flush is attributed to the hour the flush runs in, so a window boundary
//! is accurate to one flush interval.
//!
//! Retention is enforced at flush time: files older than the retention
//! period are deleted, then the oldest files are deleted until the
//! directory fits in the size limit. The file being written is never
//! deleted. Nothing is written while nothing has been recorded.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::recorder::{merge_snapshot, Snapshot};

/// Format version of an aggregate file.
pub const FILE_SCHEMA: u32 = 1;

/// Database engine every series in this build is measured against.
pub const ENGINE: &str = "libsql";

const SECS_PER_HOUR: u64 = 3600;

/// Smallest accepted size limit: below it a single hour's file could not be
/// kept.
const MIN_MAX_BYTES: u64 = 1024 * 1024;

/// One process's aggregates for one hour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowFile {
    pub schema: u32,
    pub tokensave_version: String,
    pub engine: String,
    /// Hours since the UNIX epoch.
    pub hour: u64,
    pub series: Snapshot,
}

/// The profiler's own cost, for `profile status`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Overhead {
    pub flushes: u64,
    pub flush_us: u64,
    pub bytes_written: u64,
    pub files_removed: u64,
}

static FLUSHES: AtomicU64 = AtomicU64::new(0);
static FLUSH_US: AtomicU64 = AtomicU64::new(0);
static BYTES_WRITTEN: AtomicU64 = AtomicU64::new(0);
static FILES_REMOVED: AtomicU64 = AtomicU64::new(0);

/// Cumulative flush overhead of this process.
pub fn overhead() -> Overhead {
    Overhead {
        flushes: FLUSHES.load(Ordering::Relaxed),
        flush_us: FLUSH_US.load(Ordering::Relaxed),
        bytes_written: BYTES_WRITTEN.load(Ordering::Relaxed),
        files_removed: FILES_REMOVED.load(Ordering::Relaxed),
    }
}

/// Owns this process's current-hour accumulator and its file.
pub struct Store {
    dir: PathBuf,
    retention_hours: u64,
    max_bytes: u64,
    /// `<pid>-<nonce>`: distinguishes processes, including a reused PID.
    process_tag: String,
    hour: u64,
    current: Snapshot,
}

impl Store {
    pub fn new(dir: PathBuf, retention_days: u32, max_bytes: u64) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Self {
            dir,
            retention_hours: u64::from(retention_days.max(1)) * 24,
            max_bytes: max_bytes.max(MIN_MAX_BYTES),
            process_tag: format!("{}-{nonce:08x}", std::process::id()),
            hour: 0,
            current: Snapshot::new(),
        }
    }

    fn file_path(&self, hour: u64) -> PathBuf {
        self.dir.join(format!("{hour}-{}.json", self.process_tag))
    }

    /// Merges `drained` into the current hour and rewrites its file, then
    /// enforces retention. Does no I/O when nothing has been recorded this
    /// hour.
    pub fn flush(&mut self, drained: &Snapshot, now_unix: u64) -> std::io::Result<()> {
        let started = Instant::now();
        let hour = now_unix / SECS_PER_HOUR;
        if hour != self.hour {
            self.hour = hour;
            self.current.clear();
        }
        merge_snapshot(&mut self.current, drained);
        if self.current.is_empty() {
            return Ok(());
        }

        let file = WindowFile {
            schema: FILE_SCHEMA,
            tokensave_version: env!("CARGO_PKG_VERSION").to_string(),
            engine: ENGINE.to_string(),
            hour,
            series: std::mem::take(&mut self.current),
        };
        let json = serde_json::to_string(&file).map_err(std::io::Error::other);
        self.current = file.series;
        let json = json?;

        std::fs::create_dir_all(&self.dir)?;
        let path = self.file_path(hour);
        if !crate::user_config::write_atomic(&path, &json) {
            return Err(std::io::Error::other("profile write failed"));
        }
        BYTES_WRITTEN.fetch_add(json.len() as u64, Ordering::Relaxed);
        self.enforce_retention(&path);

        FLUSHES.fetch_add(1, Ordering::Relaxed);
        FLUSH_US.fetch_add(
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        Ok(())
    }

    fn enforce_retention(&self, keep: &Path) {
        let oldest_kept = self.hour.saturating_sub(self.retention_hours - 1);
        let mut files: Vec<(u64, PathBuf, u64)> = Vec::new();
        for (hour, path) in list_files(&self.dir) {
            if path == keep {
                continue;
            }
            if hour < oldest_kept {
                remove(&path);
            } else {
                let size = std::fs::metadata(&path).map_or(0, |m| m.len());
                files.push((hour, path, size));
            }
        }
        let keep_size = std::fs::metadata(keep).map_or(0, |m| m.len());
        let mut total = keep_size + files.iter().map(|f| f.2).sum::<u64>();
        files.sort_unstable();
        for (_, path, size) in files {
            if total <= self.max_bytes {
                break;
            }
            remove(&path);
            total = total.saturating_sub(size);
        }
    }
}

fn remove(path: &Path) {
    if std::fs::remove_file(path).is_ok() {
        FILES_REMOVED.fetch_add(1, Ordering::Relaxed);
    }
}

/// Files in `dir` written by the profiler (including leftover temp files
/// from an interrupted write), with the hour encoded in their name.
fn list_files(dir: &Path) -> Vec<(u64, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let hour = name.to_str()?.split_once('-')?.0.parse().ok()?;
            Some((hour, entry.path()))
        })
        .collect()
}

/// Reads and merges every aggregate file in `dir` from `since_hour` on, per
/// engine. Unreadable or foreign-schema files are skipped.
pub fn load(dir: &Path, since_hour: u64) -> BTreeMap<String, Snapshot> {
    let mut merged: BTreeMap<String, Snapshot> = BTreeMap::new();
    for (hour, path) in list_files(dir) {
        if hour < since_hour || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(file) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<WindowFile>(&s).ok())
        else {
            continue;
        };
        if file.schema != FILE_SCHEMA || file.hour < since_hour {
            continue;
        }
        merge_snapshot(merged.entry(file.engine).or_default(), &file.series);
    }
    merged
}

/// Total size of the files in `dir`, for `profile status`.
pub fn disk_usage(dir: &Path) -> u64 {
    list_files(dir)
        .iter()
        .filter_map(|(_, p)| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::recorder::SeriesSnapshot;

    const HOUR: u64 = SECS_PER_HOUR;

    fn snapshot(count: u64) -> Snapshot {
        let mut s = SeriesSnapshot {
            count,
            sum_us: count * 10,
            max_us: 10,
            ..SeriesSnapshot::default()
        };
        for _ in 0..count {
            s.histogram.record(10);
        }
        let mut snap = Snapshot::new();
        snap.insert("db.open".into(), s);
        snap
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn nothing_recorded_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let mut store = Store::new(dir.clone(), 14, 0);
        store.flush(&Snapshot::new(), 1000 * HOUR).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn flushes_accumulate_within_the_hour_and_merge_across_processes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut a = Store::new(dir.clone(), 14, 0);
        let mut b = Store::new(dir.clone(), 14, 0);
        b.process_tag = "other-1".into();
        a.flush(&snapshot(2), 1000 * HOUR).unwrap();
        a.flush(&snapshot(3), 1000 * HOUR + 60).unwrap();
        b.flush(&snapshot(4), 1000 * HOUR + 120).unwrap();
        assert_eq!(files(&dir).len(), 2, "one file per process per hour");
        let merged = load(&dir, 0);
        let s = &merged[ENGINE]["db.open"];
        assert_eq!(s.count, 9);
        assert_eq!(s.histogram.count(), 9);
    }

    #[test]
    fn a_new_hour_starts_a_new_file_and_since_filters_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut store = Store::new(dir.clone(), 14, 0);
        store.flush(&snapshot(1), 1000 * HOUR).unwrap();
        store.flush(&snapshot(5), 1001 * HOUR).unwrap();
        assert_eq!(files(&dir).len(), 2);
        assert_eq!(load(&dir, 0)[ENGINE]["db.open"].count, 6);
        assert_eq!(load(&dir, 1001)[ENGINE]["db.open"].count, 5);
    }

    #[test]
    fn retention_deletes_expired_files_and_ignores_foreign_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        std::fs::write(dir.join("10-1-old.json"), "{}").unwrap();
        std::fs::write(dir.join("10-1-old.tmp.1.0"), "partial").unwrap();
        std::fs::write(dir.join("notes.txt"), "keep me").unwrap();
        let now_hour = 10 + 24 * 2;
        std::fs::write(dir.join(format!("{}-1-recent.json", now_hour - 3)), "{}").unwrap();
        let mut store = Store::new(dir.clone(), 1, 0);
        store.flush(&snapshot(1), now_hour * HOUR).unwrap();
        let names = files(&dir);
        assert!(!names.iter().any(|n| n.starts_with("10-")), "{names:?}");
        assert!(names.contains(&"notes.txt".to_string()));
        assert!(names.iter().any(|n| n.contains("recent")));
    }

    #[test]
    fn size_limit_deletes_oldest_first_but_never_the_current_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let big = "x".repeat(400 * 1024);
        for hour in [100, 101, 102] {
            std::fs::write(dir.join(format!("{hour}-9-old.json")), &big).unwrap();
        }
        let mut store = Store::new(dir.clone(), 30, MIN_MAX_BYTES);
        store.flush(&snapshot(1), 103 * HOUR).unwrap();
        let names = files(&dir);
        assert!(!names.iter().any(|n| n.starts_with("100-")), "{names:?}");
        assert!(names.iter().any(|n| n.starts_with("101-")));
        assert!(names.iter().any(|n| n.starts_with("103-")));
        assert!(disk_usage(&dir) <= MIN_MAX_BYTES);
    }

    #[test]
    fn corrupt_and_future_schema_files_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        std::fs::write(dir.join("5-1-bad.json"), "not json").unwrap();
        std::fs::write(
            dir.join("5-1-future.json"),
            r#"{"schema":99,"tokensave_version":"x","engine":"libsql","hour":5,"series":{}}"#,
        )
        .unwrap();
        let mut store = Store::new(dir.clone(), 30, 0);
        store.flush(&snapshot(2), 6 * HOUR).unwrap();
        assert_eq!(load(&dir, 0)[ENGINE]["db.open"].count, 2);
    }
}
