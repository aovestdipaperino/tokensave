// Rust guideline compliant 2026-07-24
//! Global per-instance memory telemetry for diagnosing RSS spikes (#253).
//!
//! Long-lived `tokensave serve` processes have been observed reaching
//! 8-10 GiB RSS on very large repos. To tell a *transient peak during
//! sync/reference-resolution* apart from *unbounded growth per tool
//! call*, every tokensave process records its own resident-set size
//! into a machine-global, fixed-size mmap table at
//! `~/.tokensave/memory.mmap`, and `tokensave memory` renders a report
//! covering all instances — including dead ones, whose recorded
//! `peak_rss`/`peak_phase` are exactly the forensic data an OOM-killed
//! server leaves behind.
//!
//! Architecture mirrors [`crate::monitor`]: a 32-byte header followed
//! by fixed-width little-endian slots with null-padded string fields,
//! written best-effort under an `fs2` exclusive file lock. Telemetry
//! must never break the primary workload, so every failure is silently
//! swallowed (`let _ = ...`).
//!
//! Unlike the monitor's ring buffer, this file is a *slot-per-instance
//! table*: [`SLOT_COUNT`] slots keyed by PID. A process claims the
//! first slot whose PID is zero, dead, or its own, then updates it in
//! place on every [`record`] call. Each sample refreshes the current
//! RSS and parent PID, bumps a `samples` counter, names the current
//! `phase` (a tool name, or a sync phase such as
//! `sync:resolve:build_caches`), and — when a new process-lifetime peak
//! is observed — freezes that phase into `peak_phase`.
//!
//! The report distinguishes three states, which map to the three
//! failure modes on #253: a **dead** slot (its recorded PID no longer
//! exists) is the forensic record an OOM-killed or exited process left
//! behind; an **orphan** is a still-running process whose parent is
//! init (ppid 1) or already dead — the abandoned `serve` daemons with
//! no MCP client attached; and a plain **alive** process. Orphan
//! detection is the reason each sample records `ppid` alongside RSS.
//!
//! RSS sampling uses the already-present `sysinfo` dependency scoped
//! to the calling process only (one `/proc` read on Linux, one
//! `proc_pidinfo` on macOS), so a sample is cheap enough to take once
//! per tool call and once per sync phase. On platforms where sampling
//! fails the recorded RSS degrades to zero rather than erroring.
//! Liveness checks may report another user's process as dead on macOS,
//! where unprivileged process inspection is restricted; the slot data
//! itself is still shown.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

use fs2::FileExt;

// ── Layout constants ────────────────────────────────────────────────
const HEADER_SIZE: usize = 32;
const SLOT_SIZE: usize = 192;
/// Number of instance slots. 64 concurrent tokensave processes per
/// machine is far above anything observed in practice; the table stays
/// a fixed 12.3 KB so it can be mmapped safely from every process.
const SLOT_COUNT: usize = 64;
const FILE_SIZE: usize = HEADER_SIZE + SLOT_SIZE * SLOT_COUNT;

const FIELD_LEN: usize = 32; // null-padded UTF-8 per string field

// Header offsets
const OFF_VERSION: usize = 0;
/// Bumped if the slot layout ever changes; a writer seeing an unknown
/// version leaves the file untouched instead of corrupting it.
const LAYOUT_VERSION: u64 = 1;
// bytes 8..32 reserved

// Slot field offsets (relative to slot start)
const SOFF_PID: usize = 0;
const SOFF_START_TS: usize = 8;
const SOFF_LAST_TS: usize = 16;
const SOFF_RSS: usize = 24;
const SOFF_PEAK_RSS: usize = 32;
const SOFF_SAMPLES: usize = 40;
const SOFF_GRAPH_NODES: usize = 48;
const SOFF_KIND: usize = 56;
const SOFF_PROJECT: usize = 88;
const SOFF_PHASE: usize = 120;
const SOFF_PEAK_PHASE: usize = 152;
/// Parent PID at the most recent sample. The orphan diagnosis for #253
/// hinges on this: a `serve` process reparented to init (ppid 1) or
/// whose recorded parent is no longer alive is an abandoned server.
const SOFF_PPID: usize = 184;
// bytes 192..192 — slot is now fully packed

const MMAP_FILENAME: &str = "memory.mmap";

/// Resolve the global `~/.tokensave/` directory.
fn global_tokensave_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave"))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Process-local context ───────────────────────────────────────────

/// (command kind, project folder name) — set once per process.
static CONTEXT: OnceLock<(String, String)> = OnceLock::new();
/// Last known graph node count, folded into the next sample.
static GRAPH_NODES: AtomicU64 = AtomicU64::new(0);
/// True until the first `record` of this process. The first sample
/// reclaims a stale slot even if it carries our (reused) PID.
static FIRST_SAMPLE: AtomicBool = AtomicBool::new(true);

/// Set the command kind (`"serve"`, `"sync"`, ...) and project for this process.
///
/// Call once early (before the first [`record`]); later calls are
/// ignored. `project_root` is reduced to its folder name.
pub fn init(kind: &str, project_root: &Path) {
    let project = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let _ = CONTEXT.set((kind.to_string(), project));
}

/// Publish the latest known graph node count for peak-vs-size context.
///
/// Stored process-locally and written out with the next [`record`].
pub fn set_graph_nodes(count: u64) {
    GRAPH_NODES.store(count, Ordering::Relaxed);
}

/// Sample current RSS and update this process's slot. Best-effort.
///
/// `phase` names what the process is doing right now (a tool name, a
/// sync phase, `"start"`, `"idle"`, ...). If the sampled RSS exceeds
/// the recorded process-lifetime peak, `peak_phase` is set to `phase`.
pub fn record(phase: &str) {
    let Some(dir) = global_tokensave_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let (kind, project) = CONTEXT
        .get()
        .map_or(("unknown", ""), |(k, p)| (k.as_str(), p.as_str()));
    let (rss_bytes, ppid) = sample_self();
    let sample = Sample {
        pid: u64::from(std::process::id()),
        ppid,
        kind: kind.to_string(),
        project: project.to_string(),
        phase: phase.to_string(),
        rss_bytes,
        graph_nodes: GRAPH_NODES.load(Ordering::Relaxed),
        fresh: FIRST_SAMPLE.swap(false, Ordering::Relaxed),
    };
    let _ = write_sample_inner(&dir.join(MMAP_FILENAME), &sample, now_secs());
}

// ── Writer ──────────────────────────────────────────────────────────

/// One instrumentation sample destined for an instance slot.
#[derive(Debug, Clone)]
pub struct Sample {
    pub pid: u64,
    /// Parent PID at sample time; `0` when it could not be determined.
    pub ppid: u64,
    pub kind: String,
    pub project: String,
    pub phase: String,
    pub rss_bytes: u64,
    /// Last known node count; `0` leaves the slot's stored value untouched.
    pub graph_nodes: u64,
    /// First sample from this process: reset the slot (start timestamp,
    /// peak, samples) even when a stale slot carries the same reused PID.
    pub fresh: bool,
}

/// Write a sample to the memory table in a specific directory (test seam).
///
/// Mirrors `monitor::write_entry_to`: creates the directory, then
/// silently swallows any failure.
pub fn record_sample_to(dir: &Path, sample: &Sample) {
    let _ = std::fs::create_dir_all(dir);
    let _ = write_sample_inner(&dir.join(MMAP_FILENAME), sample, now_secs());
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap_or([0; 8]))
}

fn write_u64(mmap: &mut memmap2::MmapMut, offset: usize, value: u64) {
    mmap[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_str(mmap: &mut memmap2::MmapMut, offset: usize, value: &str) {
    let bytes = value.as_bytes();
    let copy_len = bytes.len().min(FIELD_LEN - 1);
    mmap[offset..offset + FIELD_LEN].fill(0);
    mmap[offset..offset + copy_len].copy_from_slice(&bytes[..copy_len]);
}

fn read_str(bytes: &[u8], offset: usize) -> String {
    let field = &bytes[offset..offset + FIELD_LEN];
    let end = field.iter().position(|&b| b == 0).unwrap_or(FIELD_LEN);
    String::from_utf8_lossy(&field[..end]).to_string()
}

/// Pick the slot for `pid`: its own, else the first free, else the
/// first dead. Returns `(index, needs_reset)`; `None` when all slots
/// belong to other live processes (the sample is then dropped).
fn choose_slot(mmap: &memmap2::MmapMut, pid: u64, fresh: bool) -> Option<(usize, bool)> {
    let mut first_free = None;
    let mut pids = [0u64; SLOT_COUNT];
    for (i, slot_pid) in pids.iter_mut().enumerate() {
        *slot_pid = read_u64(mmap, HEADER_SIZE + i * SLOT_SIZE + SOFF_PID);
        if *slot_pid == pid {
            return Some((i, fresh));
        }
        if *slot_pid == 0 && first_free.is_none() {
            first_free = Some(i);
        }
    }
    if let Some(i) = first_free {
        return Some((i, true));
    }
    // Table full: reclaim the first slot whose owner is gone. The
    // liveness syscall only runs on this rare path, never per sample.
    let alive = alive_pids(&pids);
    pids.iter()
        .position(|p| !alive.contains(p))
        .map(|i| (i, true))
}

fn write_sample_inner(mmap_path: &Path, s: &Sample, now: u64) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(mmap_path)?;

    // Exclusive lock for concurrent writer safety.
    file.lock_exclusive()?;

    let len = file.metadata()?.len() as usize;
    if len < FILE_SIZE {
        file.set_len(FILE_SIZE as u64)?;
    }

    let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };

    let version = read_u64(&mmap, OFF_VERSION);
    if version == 0 {
        write_u64(&mut mmap, OFF_VERSION, LAYOUT_VERSION);
    } else if version != LAYOUT_VERSION {
        // A future layout owns this file; leave it alone.
        let _ = file.unlock();
        return Ok(());
    }

    let Some((idx, reset)) = choose_slot(&mmap, s.pid, s.fresh) else {
        let _ = file.unlock();
        return Ok(());
    };
    let off = HEADER_SIZE + idx * SLOT_SIZE;

    if reset {
        mmap[off..off + SLOT_SIZE].fill(0);
        write_u64(&mut mmap, off + SOFF_PID, s.pid);
        write_u64(&mut mmap, off + SOFF_START_TS, now);
    }

    write_u64(&mut mmap, off + SOFF_LAST_TS, now);
    // Re-written every sample, not just on reset: a process reparented
    // to init after its MCP client dies must show ppid 1 on its next
    // sample so the report can flag it as an orphan.
    write_u64(&mut mmap, off + SOFF_PPID, s.ppid);
    write_u64(&mut mmap, off + SOFF_RSS, s.rss_bytes);
    let samples = read_u64(&mmap, off + SOFF_SAMPLES).saturating_add(1);
    write_u64(&mut mmap, off + SOFF_SAMPLES, samples);
    if s.graph_nodes > 0 {
        write_u64(&mut mmap, off + SOFF_GRAPH_NODES, s.graph_nodes);
    }
    write_str(&mut mmap, off + SOFF_KIND, &s.kind);
    write_str(&mut mmap, off + SOFF_PROJECT, &s.project);
    write_str(&mut mmap, off + SOFF_PHASE, &s.phase);
    if s.rss_bytes > read_u64(&mmap, off + SOFF_PEAK_RSS) {
        write_u64(&mut mmap, off + SOFF_PEAK_RSS, s.rss_bytes);
        write_str(&mut mmap, off + SOFF_PEAK_PHASE, &s.phase);
    }

    mmap.flush()?;
    file.unlock()?;
    Ok(())
}

// ── Self-sampling and liveness (sysinfo, already a dependency) ──────

/// Sample this process's `(resident-set size in bytes, parent PID)` in a
/// single `sysinfo` refresh; either component degrades to `0` on failure.
///
/// Mirrors the self-sampling already done in
/// [`crate::runtime_telemetry`]: one `/proc` read on Linux, one
/// `proc_pidinfo` on macOS, scoped to this PID only, so it is cheap
/// enough to call once per tool call and once per sync phase.
fn sample_self() -> (u64, u64) {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = Pid::from_u32(std::process::id());
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::new().with_memory(),
    );
    sys.process(pid).map_or((0, 0), |p| {
        let ppid = p.parent().map_or(0, |pp| u64::from(pp.as_u32()));
        (p.memory(), ppid)
    })
}

/// Current resident-set size of this process in bytes; `0` on failure.
pub fn current_rss_bytes() -> u64 {
    sample_self().0
}

/// Return the subset of `pids` that belong to live processes.
fn alive_pids(pids: &[u64]) -> HashSet<u64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let sys_pids: Vec<Pid> = pids
        .iter()
        .filter(|&&p| p != 0)
        .filter_map(|&p| u32::try_from(p).ok())
        .map(Pid::from_u32)
        .collect();
    if sys_pids.is_empty() {
        return HashSet::new();
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&sys_pids),
        true,
        ProcessRefreshKind::new(),
    );
    pids.iter()
        .copied()
        .filter(|&p| {
            u32::try_from(p)
                .ok()
                .is_some_and(|p32| p32 != 0 && sys.process(Pid::from_u32(p32)).is_some())
        })
        .collect()
}

// ── Reader ──────────────────────────────────────────────────────────

/// One instance's slot, decoded from the memory table.
#[derive(Debug, Clone)]
pub struct InstanceSlot {
    pub pid: u64,
    pub ppid: u64,
    pub start_timestamp: u64,
    pub last_update_timestamp: u64,
    pub current_rss: u64,
    pub peak_rss: u64,
    pub samples: u64,
    pub graph_nodes: u64,
    pub kind: String,
    pub project: String,
    pub phase: String,
    pub peak_phase: String,
}

/// Read-only view of the global memory table.
#[derive(Debug)]
pub struct SlotReader {
    mmap: memmap2::Mmap,
}

impl SlotReader {
    /// Open the global memory table for reading.
    pub fn open() -> std::io::Result<Self> {
        let dir = global_tokensave_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "cannot resolve home directory",
            )
        })?;
        Self::open_at(&dir)
    }

    /// Open a memory table at an explicit directory (for testing).
    pub fn open_at(dir: &Path) -> std::io::Result<Self> {
        let mmap_path = dir.join(MMAP_FILENAME);
        let file = std::fs::OpenOptions::new().read(true).open(&mmap_path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self { mmap })
    }

    /// The number of slots in the table.
    pub fn capacity(&self) -> usize {
        SLOT_COUNT
    }

    /// Decode the slot at `idx`; `None` if out of range or unmapped.
    pub fn slot(&self, idx: usize) -> Option<InstanceSlot> {
        if idx >= SLOT_COUNT {
            return None;
        }
        let off = HEADER_SIZE + idx * SLOT_SIZE;
        if self.mmap.len() < off + SLOT_SIZE {
            return None;
        }
        Some(InstanceSlot {
            pid: read_u64(&self.mmap, off + SOFF_PID),
            ppid: read_u64(&self.mmap, off + SOFF_PPID),
            start_timestamp: read_u64(&self.mmap, off + SOFF_START_TS),
            last_update_timestamp: read_u64(&self.mmap, off + SOFF_LAST_TS),
            current_rss: read_u64(&self.mmap, off + SOFF_RSS),
            peak_rss: read_u64(&self.mmap, off + SOFF_PEAK_RSS),
            samples: read_u64(&self.mmap, off + SOFF_SAMPLES),
            graph_nodes: read_u64(&self.mmap, off + SOFF_GRAPH_NODES),
            kind: read_str(&self.mmap, off + SOFF_KIND),
            project: read_str(&self.mmap, off + SOFF_PROJECT),
            phase: read_str(&self.mmap, off + SOFF_PHASE),
            peak_phase: read_str(&self.mmap, off + SOFF_PEAK_PHASE),
        })
    }

    /// All claimed slots (PID != 0), in table order.
    pub fn occupied(&self) -> Vec<InstanceSlot> {
        (0..SLOT_COUNT)
            .filter_map(|i| self.slot(i))
            .filter(|s| s.pid != 0)
            .collect()
    }
}

// ── Report (tokensave memory command) ───────────────────────────────

/// Print the memory report for all recorded instances. Diagnostic
/// tool: prints a message instead of failing when there is no data.
pub fn run(clean: bool) -> std::io::Result<()> {
    let Some(dir) = global_tokensave_dir() else {
        println!("cannot resolve home directory; no memory report available");
        return Ok(());
    };
    run_at(&dir, clean)
}

/// Like [`run`], against an explicit table directory (for testing).
pub fn run_at(dir: &Path, clean: bool) -> std::io::Result<()> {
    let Ok(reader) = SlotReader::open_at(dir) else {
        println!(
            "No memory telemetry recorded yet. Run a tokensave command \
             (serve, sync) to populate {}.",
            dir.join(MMAP_FILENAME).display()
        );
        return Ok(());
    };
    let mut slots = reader.occupied();
    if slots.is_empty() {
        println!("No tokensave instances have recorded memory samples yet.");
        return Ok(());
    }

    // Probe both PIDs and parent PIDs in one pass: the report needs
    // parent liveness to tell an orphan from a normally-parented server.
    let mut probe: Vec<u64> = slots.iter().map(|s| s.pid).collect();
    probe.extend(slots.iter().map(|s| s.ppid));
    let alive = alive_pids(&probe);

    if clean {
        let purged = purge_dead_slots(dir, &alive)?;
        println!("Purged {purged} dead slot(s).");
        slots.retain(|s| alive.contains(&s.pid));
        if slots.is_empty() {
            println!("No live tokensave instances remain.");
            return Ok(());
        }
    }

    print!("{}", render_report(&slots, &alive, now_secs()));
    Ok(())
}

/// Zero every slot whose PID is dead; returns how many were purged.
fn purge_dead_slots(dir: &Path, alive: &HashSet<u64>) -> std::io::Result<usize> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join(MMAP_FILENAME))?;
    file.lock_exclusive()?;
    let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let mut purged = 0;
    for i in 0..SLOT_COUNT {
        let off = HEADER_SIZE + i * SLOT_SIZE;
        if mmap.len() < off + SLOT_SIZE {
            break;
        }
        let pid = read_u64(&mmap, off + SOFF_PID);
        if pid != 0 && !alive.contains(&pid) {
            mmap[off..off + SLOT_SIZE].fill(0);
            purged += 1;
        }
    }
    mmap.flush()?;
    file.unlock()?;
    Ok(purged)
}

/// Lifecycle state of an instance, derived from PID/parent liveness.
///
/// `alive` must contain the live subset of every slot's `pid` *and*
/// `ppid` (see [`run_at`]), so parent liveness can be judged here.
fn instance_state<S: std::hash::BuildHasher>(
    s: &InstanceSlot,
    alive: &HashSet<u64, S>,
) -> &'static str {
    if !alive.contains(&s.pid) {
        // The process itself is gone; peak_rss/peak_phase are its
        // forensic legacy (OOM-killed or exited).
        "dead"
    } else if s.ppid == 1 || (s.ppid != 0 && !alive.contains(&s.ppid)) {
        // Reparented to init, or its recorded parent has since died:
        // an abandoned server with no client attached (#253).
        "orphan"
    } else {
        "alive"
    }
}

/// Render the plain-text report table (pure, for testing).
pub fn render_report<S: std::hash::BuildHasher>(
    slots: &[InstanceSlot],
    alive: &HashSet<u64, S>,
    now: u64,
) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    // Writing to a String is infallible; the results are ignored.
    let _ = writeln!(
        out,
        "{:<8} {:<8} {:<6} {:<20} {:<8} {:>8} {:>8} {:>10} {:>10}  {:<26} {:<26} {:>8} {:>10}",
        "PID",
        "PPID",
        "STATE",
        "PROJECT",
        "KIND",
        "UPTIME",
        "AGE",
        "RSS",
        "PEAK",
        "PEAK PHASE",
        "PHASE",
        "SAMPLES",
        "NODES"
    );
    for s in slots {
        let state = instance_state(s, alive);
        // A dead process stopped aging at its last sample; a running one
        // (alive or orphan) is still aging now.
        let uptime = if state == "dead" {
            s.last_update_timestamp.saturating_sub(s.start_timestamp)
        } else {
            now.saturating_sub(s.start_timestamp)
        };
        let age = now.saturating_sub(s.last_update_timestamp);
        let _ = writeln!(
            out,
            "{:<8} {:<8} {:<6} {:<20} {:<8} {:>8} {:>8} {:>10} {:>10}  {:<26} {:<26} {:>8} {:>10}",
            s.pid,
            s.ppid,
            state,
            truncate(&s.project, 20),
            truncate(&s.kind, 8),
            format_duration(uptime),
            format_duration(age),
            format_bytes(s.current_rss),
            format_bytes(s.peak_rss),
            truncate(&s.peak_phase, 26),
            truncate(&s.phase, 26),
            s.samples,
            s.graph_nodes,
        );
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Format a byte count using binary units (`1.2 GiB`, `353.4 MiB`).
fn format_bytes(n: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    if n >= GIB {
        format!("{:.1} GiB", n as f64 / GIB as f64)
    } else if n >= MIB {
        format!("{:.1} MiB", n as f64 / MIB as f64)
    } else if n >= KIB {
        format!("{:.1} KiB", n as f64 / KIB as f64)
    } else {
        format!("{n} B")
    }
}

/// Compact duration: `42s`, `5m03s`, `2h05m`, `3d04h`.
fn format_duration(secs: u64) -> String {
    if secs >= 86_400 {
        format!("{}d{:02}h", secs / 86_400, (secs % 86_400) / 3600)
    } else if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_uses_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2 * 1024), "2.0 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(format_bytes(10 * 1024 * 1024 * 1024), "10.0 GiB");
    }

    #[test]
    fn format_duration_scales() {
        assert_eq!(format_duration(42), "42s");
        assert_eq!(format_duration(5 * 60 + 3), "5m03s");
        assert_eq!(format_duration(2 * 3600 + 5 * 60), "2h05m");
        assert_eq!(format_duration(3 * 86_400 + 4 * 3600), "3d04h");
    }

    #[test]
    fn truncate_preserves_short_and_marks_long() {
        assert_eq!(truncate("short", 10), "short");
        let t = truncate("a-very-long-phase-name-indeed", 10);
        assert_eq!(t.chars().count(), 10);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn rss_sampler_returns_positive_on_host() {
        // The test process itself certainly has resident memory.
        assert!(current_rss_bytes() > 0);
    }

    fn slot_with(pid: u64, ppid: u64) -> InstanceSlot {
        InstanceSlot {
            pid,
            ppid,
            start_timestamp: 0,
            last_update_timestamp: 0,
            current_rss: 0,
            peak_rss: 0,
            samples: 1,
            graph_nodes: 0,
            kind: "serve".to_string(),
            project: "p".to_string(),
            phase: "idle".to_string(),
            peak_phase: "idle".to_string(),
        }
    }

    #[test]
    fn instance_state_classifies_dead_orphan_alive() {
        let dead_pid = 4_000_000_000u64;
        let live_parent = 10u64;
        let alive: HashSet<u64> = [100u64, 200, 300, 400, live_parent].into_iter().collect();

        // Process gone -> dead, regardless of parent.
        assert_eq!(
            instance_state(&slot_with(dead_pid, live_parent), &alive),
            "dead"
        );
        // Alive process reparented to init (ppid 1) -> orphan.
        assert_eq!(instance_state(&slot_with(100, 1), &alive), "orphan");
        // Alive process whose recorded parent is dead -> orphan.
        assert_eq!(instance_state(&slot_with(200, dead_pid), &alive), "orphan");
        // Alive process with a live parent -> alive.
        assert_eq!(
            instance_state(&slot_with(300, live_parent), &alive),
            "alive"
        );
        // Unknown parent (0) on a live process is not treated as orphan.
        assert_eq!(instance_state(&slot_with(400, 0), &alive), "alive");
    }

    #[test]
    fn alive_pids_sees_self_and_not_bogus_pid() {
        let me = u64::from(std::process::id());
        // A PID this large exists on no supported platform.
        let bogus = 4_000_000_000u64;
        let alive = alive_pids(&[me, bogus, 0]);
        assert!(alive.contains(&me));
        assert!(!alive.contains(&bogus));
        assert!(!alive.contains(&0));
    }
}
