// Rust guideline compliant 2026-05-25
//! Runtime telemetry snapshot for diagnosing CPU/RAM regressions
//! (issue #80).
//!
//! Captures process-level resource use (RSS, virtual size, CPU%, thread
//! count) via [`sysinfo`] and database-level signals (sqlite + WAL + SHM
//! sizes, journal mode) so users hitting unexpected resource pressure
//! can attach a structured snapshot to a bug report.
//!
//! `cpu_percent` requires a refresh interval to be meaningful — sysinfo
//! reports CPU% as a delta between two refreshes. [`collect`] performs
//! the refresh, sleeps for [`CPU_SAMPLE_WINDOW`], then refreshes again.
//! Callers therefore pay ~200 ms latency per snapshot.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

use crate::errors::{Result, TokenSaveError};

/// Window over which `cpu_percent` is sampled.
const CPU_SAMPLE_WINDOW: Duration = Duration::from_millis(200);

/// Captured process + database telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    /// Captured at (Unix epoch seconds).
    pub captured_at: u64,
    /// `tokensave` build version (e.g. `6.0.0`).
    pub tokensave_version: &'static str,
    /// Host OS short name (`macos`, `linux`, `windows`, …).
    pub host_os: &'static str,
    pub process: ProcessSnapshot,
    pub database: DatabaseSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub rss_bytes: u64,
    pub virtual_bytes: u64,
    /// Sustained CPU% across [`CPU_SAMPLE_WINDOW`] (0-100 per core, may
    /// exceed 100 on multi-threaded workloads).
    pub cpu_percent: f32,
    pub uptime_secs: u64,
    /// Number of CPUs the kernel reports — useful for interpreting
    /// `cpu_percent > 100`.
    pub system_cpu_count: usize,
    /// Total system memory in bytes (for ratio reporting).
    pub system_total_memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSnapshot {
    pub project_root: PathBuf,
    /// `<root>/.tokensave/<branch>.db` or whichever DB is being served.
    pub db_path: PathBuf,
    pub db_size_bytes: u64,
    /// Size of the WAL (`-wal`) file alongside the DB, when present.
    pub wal_size_bytes: u64,
    /// Size of the shared-memory file (`-shm`).
    pub shm_size_bytes: u64,
    /// `journal_mode` PRAGMA (`wal`, `delete`, `truncate`, …).
    pub journal_mode: Option<String>,
    /// Total source size we've indexed, from the `files` table sum, in
    /// bytes — useful to compute the "DB / source" ratio.
    pub source_total_bytes: u64,
    /// Total node + edge counts. Lets the user compare DB bloat to
    /// graph size — a 25× ratio with a tiny graph is suspicious.
    pub node_count: u64,
    pub edge_count: u64,
}

/// Capture a runtime snapshot for the given project.
///
/// Two responsibilities: (a) sample our own process via `sysinfo`,
/// (b) `stat` the SQLite files and ask the connection for its journal
/// mode. Both are best-effort — failures degrade to zeroes / `None`
/// rather than failing the whole snapshot, because the value of this
/// tool is recording *what's available* during a spike.
pub async fn collect(cg: &crate::tokensave::TokenSave) -> Result<RuntimeSnapshot> {
    let process = sample_process();
    let database = sample_database(cg).await?;
    let captured_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(RuntimeSnapshot {
        captured_at,
        tokensave_version: env!("CARGO_PKG_VERSION"),
        host_os: std::env::consts::OS,
        process,
        database,
    })
}

/// Render a `RuntimeSnapshot` as the JSON wire shape used by both the
/// CLI (`--json` flag) and the MCP tool result.
pub fn to_pretty_json(snap: &RuntimeSnapshot) -> String {
    serde_json::to_string_pretty(snap).unwrap_or_default()
}

/// Render a `RuntimeSnapshot` as a human-readable status block for
/// terminals. Mirrors the structure of `tokensave status` so it's
/// familiar to users running the CLI manually.
pub fn to_text_report(snap: &RuntimeSnapshot) -> String {
    let p = &snap.process;
    let d = &snap.database;
    let pct_of_system_mem = if p.system_total_memory_bytes > 0 {
        (p.rss_bytes as f64 / p.system_total_memory_bytes as f64) * 100.0
    } else {
        0.0
    };
    let bloat_ratio = if d.source_total_bytes > 0 {
        d.db_size_bytes as f64 / d.source_total_bytes as f64
    } else {
        0.0
    };
    format!(
        "tokensave {ver} runtime snapshot ({os})\n\
         ────────────────────────────────────────\n\
           pid              {pid}\n\
           rss              {rss}  ({rss_pct:.2}% of system)\n\
           virtual          {vsz}\n\
           cpu              {cpu:.1}% (sampled over {win}ms, {ncpu} CPUs)\n\
           uptime           {up}s\n\
           system memory    {sysmem}\n\
         \n\
           db file          {db}\n\
           db size          {dbsz}\n\
           wal size         {wal}\n\
           shm size         {shm}\n\
           journal mode     {jm}\n\
           source indexed   {src}\n\
           db / source      {ratio:.1}×\n\
           nodes / edges    {nodes} / {edges}\n\
        ",
        ver = snap.tokensave_version,
        os = snap.host_os,
        pid = p.pid,
        rss = bytes_human(p.rss_bytes),
        rss_pct = pct_of_system_mem,
        vsz = bytes_human(p.virtual_bytes),
        cpu = p.cpu_percent,
        win = CPU_SAMPLE_WINDOW.as_millis(),
        ncpu = p.system_cpu_count,
        up = p.uptime_secs,
        sysmem = bytes_human(p.system_total_memory_bytes),
        db = d.db_path.display(),
        dbsz = bytes_human(d.db_size_bytes),
        wal = bytes_human(d.wal_size_bytes),
        shm = bytes_human(d.shm_size_bytes),
        jm = d.journal_mode.as_deref().unwrap_or("(unknown)"),
        src = bytes_human(d.source_total_bytes),
        ratio = bloat_ratio,
        nodes = d.node_count,
        edges = d.edge_count,
    )
}

// ---------------------------------------------------------------------------
// Process sampling
// ---------------------------------------------------------------------------

fn sample_process() -> ProcessSnapshot {
    let pid = Pid::from_u32(std::process::id());

    // First refresh: takes the baseline reading.
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_processes(ProcessRefreshKind::new().with_cpu().with_memory())
            .with_memory(sysinfo::MemoryRefreshKind::new().with_ram())
            .with_cpu(sysinfo::CpuRefreshKind::new()),
    );
    // Two reads bracketing a sleep are required: sysinfo reports
    // `cpu_usage()` as the delta between successive refreshes.
    std::thread::sleep(CPU_SAMPLE_WINDOW);
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::new().with_cpu().with_memory(),
    );

    let proc = sys.process(pid);
    let rss_bytes = proc.map(sysinfo::Process::memory).unwrap_or(0);
    let virtual_bytes = proc.map(sysinfo::Process::virtual_memory).unwrap_or(0);
    let cpu_percent = proc.map(sysinfo::Process::cpu_usage).unwrap_or(0.0);
    let uptime_secs = proc.map(sysinfo::Process::run_time).unwrap_or(0);

    ProcessSnapshot {
        pid: std::process::id(),
        rss_bytes,
        virtual_bytes,
        cpu_percent,
        uptime_secs,
        system_cpu_count: sys.cpus().len(),
        system_total_memory_bytes: sys.total_memory(),
    }
}

// ---------------------------------------------------------------------------
// Database sampling
// ---------------------------------------------------------------------------

async fn sample_database(cg: &crate::tokensave::TokenSave) -> Result<DatabaseSnapshot> {
    let project_root = cg.project_root().to_path_buf();
    let db_path = cg.db_path().to_path_buf();
    let db_size_bytes = file_size(&db_path);
    let wal_size_bytes = file_size(&with_suffix(&db_path, "-wal"));
    let shm_size_bytes = file_size(&with_suffix(&db_path, "-shm"));
    let journal_mode = read_journal_mode(cg).await.ok();
    let source_total_bytes = read_source_total_bytes(cg).await.unwrap_or(0);
    let (node_count, edge_count) = read_graph_counts(cg).await.unwrap_or((0, 0));
    Ok(DatabaseSnapshot {
        project_root,
        db_path,
        db_size_bytes,
        wal_size_bytes,
        shm_size_bytes,
        journal_mode,
        source_total_bytes,
        node_count,
        edge_count,
    })
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s: std::ffi::OsString = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

async fn read_journal_mode(cg: &crate::tokensave::TokenSave) -> Result<String> {
    let mut rows = cg
        .db()
        .conn()
        .query("PRAGMA journal_mode", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read journal_mode: {e}"),
            operation: "read_journal_mode".to_string(),
        })?;
    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read journal_mode row: {e}"),
            operation: "read_journal_mode".to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no journal_mode row returned".to_string(),
            operation: "read_journal_mode".to_string(),
        })?;
    row.get::<String>(0).map_err(|e| TokenSaveError::Database {
        message: format!("failed to decode journal_mode: {e}"),
        operation: "read_journal_mode".to_string(),
    })
}

async fn read_source_total_bytes(cg: &crate::tokensave::TokenSave) -> Result<u64> {
    let mut rows = cg
        .db()
        .conn()
        .query("SELECT COALESCE(SUM(size), 0) FROM files", ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to sum source bytes: {e}"),
            operation: "read_source_total_bytes".to_string(),
        })?;
    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("failed to read source-sum row: {e}"),
            operation: "read_source_total_bytes".to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no source-sum row returned".to_string(),
            operation: "read_source_total_bytes".to_string(),
        })?;
    let v: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
        message: format!("failed to decode source-sum: {e}"),
        operation: "read_source_total_bytes".to_string(),
    })?;
    Ok(u64::try_from(v).unwrap_or(0))
}

async fn read_graph_counts(cg: &crate::tokensave::TokenSave) -> Result<(u64, u64)> {
    let nodes = scalar_count(cg, "SELECT COUNT(*) FROM nodes").await?;
    let edges = scalar_count(cg, "SELECT COUNT(*) FROM edges").await?;
    Ok((nodes, edges))
}

async fn scalar_count(cg: &crate::tokensave::TokenSave, sql: &str) -> Result<u64> {
    let mut rows = cg
        .db()
        .conn()
        .query(sql, ())
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("scalar query failed: {e}"),
            operation: "scalar_count".to_string(),
        })?;
    let row = rows
        .next()
        .await
        .map_err(|e| TokenSaveError::Database {
            message: format!("scalar row read failed: {e}"),
            operation: "scalar_count".to_string(),
        })?
        .ok_or_else(|| TokenSaveError::Database {
            message: "no scalar row".to_string(),
            operation: "scalar_count".to_string(),
        })?;
    let v: i64 = row.get(0).map_err(|e| TokenSaveError::Database {
        message: format!("scalar decode failed: {e}"),
        operation: "scalar_count".to_string(),
    })?;
    Ok(u64::try_from(v).unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a byte count as a short human-readable string (`353.2 MB`).
fn bytes_human(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if n >= GB {
        format!("{:.1} GB", n as f64 / GB as f64)
    } else if n >= MB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else {
        format!("{n} B")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bytes_human_formats_units() {
        assert_eq!(bytes_human(0), "0 B");
        assert_eq!(bytes_human(512), "512 B");
        assert_eq!(bytes_human(2 * 1024), "2.0 KB");
        assert_eq!(bytes_human(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(bytes_human(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn with_suffix_appends_to_path() {
        let p = Path::new("/tmp/x.db");
        assert_eq!(with_suffix(p, "-wal"), Path::new("/tmp/x.db-wal"));
        assert_eq!(with_suffix(p, "-shm"), Path::new("/tmp/x.db-shm"));
    }
}
