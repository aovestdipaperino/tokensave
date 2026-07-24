// Rust guideline compliant 2026-07-24
//! Integration tests for the per-instance memory table behind
//! `tokensave memory` (#253 diagnostics). Mirrors `monitor_test.rs`:
//! writes go through the public `_to` seam against a temp directory.

use std::collections::HashSet;
use std::path::Path;

use tempfile::TempDir;
use tokensave::memstats::{record_sample_to, InstanceSlot, Sample, SlotReader};

/// A PID this large exists on no supported platform (macOS pid_max is
/// ~100k, Linux pid_max caps at 2^22), so it always reads as dead.
const DEAD_PID_BASE: u64 = 4_000_000_000;

fn sample(pid: u64, phase: &str, rss: u64, fresh: bool) -> Sample {
    Sample {
        pid,
        ppid: 1,
        kind: "serve".to_string(),
        project: "proj".to_string(),
        phase: phase.to_string(),
        rss_bytes: rss,
        graph_nodes: 0,
        fresh,
    }
}

fn write(dir: &Path, s: &Sample) {
    record_sample_to(dir, s);
}

fn reader(dir: &Path) -> SlotReader {
    SlotReader::open_at(dir).unwrap()
}

fn slot_for(dir: &Path, pid: u64) -> InstanceSlot {
    reader(dir)
        .occupied()
        .into_iter()
        .find(|s| s.pid == pid)
        .unwrap()
}

#[test]
fn test_slot_claim_and_update_in_place() {
    let dir = TempDir::new().unwrap();
    let pid = DEAD_PID_BASE;

    write(dir.path(), &sample(pid, "start", 1000, true));
    write(dir.path(), &sample(pid, "idle", 2000, false));
    write(dir.path(), &sample(pid, "tokensave_context", 1500, false));

    let r = reader(dir.path());
    // Three samples from one PID occupy exactly one slot.
    assert_eq!(r.occupied().len(), 1);

    let slot = slot_for(dir.path(), pid);
    assert_eq!(slot.samples, 3);
    assert_eq!(slot.current_rss, 1500);
    assert_eq!(slot.phase, "tokensave_context");
    assert_eq!(slot.kind, "serve");
    assert_eq!(slot.project, "proj");
    assert!(slot.start_timestamp > 0);
    assert!(slot.last_update_timestamp >= slot.start_timestamp);
}

#[test]
fn test_peak_and_peak_phase_tracking() {
    let dir = TempDir::new().unwrap();
    let pid = DEAD_PID_BASE + 1;

    write(dir.path(), &sample(pid, "start", 100, true));
    write(
        dir.path(),
        &sample(pid, "sync:resolve:build_caches", 900, false),
    );
    write(dir.path(), &sample(pid, "sync:done", 300, false));

    let slot = slot_for(dir.path(), pid);
    // The peak survives the later, lower sample — and remembers which
    // phase it was observed in.
    assert_eq!(slot.peak_rss, 900);
    assert_eq!(slot.peak_phase, "sync:resolve:build_caches");
    assert_eq!(slot.current_rss, 300);
    assert_eq!(slot.phase, "sync:done");

    // A new high-water mark moves both peak and peak_phase.
    write(dir.path(), &sample(pid, "tokensave_health", 1200, false));
    let slot = slot_for(dir.path(), pid);
    assert_eq!(slot.peak_rss, 1200);
    assert_eq!(slot.peak_phase, "tokensave_health");
}

#[test]
fn test_fresh_sample_resets_stale_slot_with_reused_pid() {
    let dir = TempDir::new().unwrap();
    let pid = DEAD_PID_BASE + 2;

    // Previous incarnation of this PID left a big peak behind.
    write(dir.path(), &sample(pid, "start", 100, true));
    write(dir.path(), &sample(pid, "sync:extract", 5000, false));
    let old = slot_for(dir.path(), pid);
    assert_eq!(old.peak_rss, 5000);
    assert_eq!(old.samples, 2);

    // A new process reusing the PID starts with fresh=true: peak,
    // peak_phase, and samples must not leak across incarnations.
    write(dir.path(), &sample(pid, "start", 200, true));
    let slot = slot_for(dir.path(), pid);
    assert_eq!(slot.samples, 1);
    assert_eq!(slot.peak_rss, 200);
    assert_eq!(slot.peak_phase, "start");
    assert_eq!(reader(dir.path()).occupied().len(), 1);
}

#[test]
fn test_full_table_reclaims_dead_slot() {
    let dir = TempDir::new().unwrap();

    // Fill every slot with a distinct, definitely-dead PID.
    write(
        dir.path(),
        &sample(DEAD_PID_BASE + 10, "sync:extract", 100, true),
    );
    let capacity = reader(dir.path()).capacity();
    for i in 1..capacity as u64 {
        write(
            dir.path(),
            &sample(DEAD_PID_BASE + 10 + i, "sync:extract", 100 + i, true),
        );
    }
    assert_eq!(reader(dir.path()).occupied().len(), capacity);

    // Our own (live) PID arrives with the table full: it must reclaim
    // a dead slot instead of being dropped, without growing the table.
    let me = u64::from(std::process::id());
    write(dir.path(), &sample(me, "start", 777, true));

    let occupied = reader(dir.path()).occupied();
    assert_eq!(occupied.len(), capacity);
    let mine = occupied.iter().find(|s| s.pid == me).unwrap();
    assert_eq!(mine.current_rss, 777);
    assert_eq!(mine.samples, 1);
    // The reclaimed slot's previous owner is gone.
    let pids: HashSet<u64> = occupied.iter().map(|s| s.pid).collect();
    assert_eq!(pids.len(), capacity, "no duplicate PIDs after reclaim");
}

#[test]
fn test_graph_nodes_zero_keeps_last_known_value() {
    let dir = TempDir::new().unwrap();
    let pid = DEAD_PID_BASE + 3;

    let mut s = sample(pid, "sync:resolve:build_caches", 100, true);
    s.graph_nodes = 4_300_000;
    write(dir.path(), &s);
    // Later samples that don't know the node count (graph_nodes == 0)
    // must not clobber the last known value.
    write(dir.path(), &sample(pid, "idle", 100, false));

    let slot = slot_for(dir.path(), pid);
    assert_eq!(slot.graph_nodes, 4_300_000);
}

#[test]
fn test_long_strings_are_truncated_not_rejected() {
    let dir = TempDir::new().unwrap();
    let pid = DEAD_PID_BASE + 4;

    let mut s = sample(pid, &"p".repeat(100), 10, true);
    s.project = "x".repeat(100);
    write(dir.path(), &s);

    let slot = slot_for(dir.path(), pid);
    // 31 chars + null terminator per 32-byte field.
    assert_eq!(slot.phase.len(), 31);
    assert_eq!(slot.project.len(), 31);
}

fn instance(pid: u64, ppid: u64, peak: u64) -> InstanceSlot {
    InstanceSlot {
        pid,
        ppid,
        start_timestamp: 1_000,
        last_update_timestamp: 4_600,
        current_rss: 512 * 1024 * 1024,
        peak_rss: peak,
        samples: 42,
        graph_nodes: 4_300_000,
        kind: "serve".to_string(),
        project: "bigrepo".to_string(),
        phase: "idle".to_string(),
        peak_phase: "sync:resolve:build_caches".to_string(),
    }
}

#[test]
fn test_render_report_smoke() {
    // pid 1234 has a live parent (999); pid 5678's process is dead.
    let slots = vec![
        instance(1234, 999, 9 * 1024 * 1024 * 1024),
        instance(5678, 999, 2 * 1024 * 1024),
    ];
    let alive: HashSet<u64> = [1234u64, 999].into_iter().collect();

    let report = tokensave::memstats::render_report(&slots, &alive, 10_000);

    // Header names every diagnosable column.
    for col in ["PID", "PPID", "STATE", "PEAK PHASE", "SAMPLES", "NODES"] {
        assert!(report.contains(col), "missing column {col}: {report}");
    }
    assert!(report.contains("alive"));
    // Dead instances stay visible — their peak is the forensic data.
    assert!(report.contains("dead"));
    assert!(report.contains("9.0 GiB"));
    assert!(report.contains("512.0 MiB"));
    assert!(report.contains("sync:resolve:build_caches"));
    assert!(report.contains("4300000"));
}

#[test]
fn test_render_report_flags_orphans() {
    // pid 1000 reparented to init (ppid 1); pid 2000's parent (3000)
    // is dead; pid 4000 has a live parent (5000). All three processes
    // are themselves alive.
    let slots = vec![
        instance(1000, 1, 1024),
        instance(2000, 3000, 1024),
        instance(4000, 5000, 1024),
    ];
    let alive: HashSet<u64> = [1000u64, 2000, 4000, 5000].into_iter().collect();

    let report = tokensave::memstats::render_report(&slots, &alive, 10_000);

    // Two of the three lines must be flagged orphan; the third alive.
    assert_eq!(report.matches("orphan").count(), 2, "report: {report}");
    let alive_line = report
        .lines()
        .find(|l| l.starts_with("4000 "))
        .unwrap_or("");
    assert!(alive_line.contains("alive"), "line: {alive_line}");
    assert!(!alive_line.contains("orphan"), "line: {alive_line}");
}

#[test]
fn test_run_at_without_table_is_ok() {
    let dir = TempDir::new().unwrap();
    // Diagnostic command must succeed even when nothing was recorded.
    tokensave::memstats::run_at(dir.path(), false).unwrap();
    tokensave::memstats::run_at(dir.path(), true).unwrap();
}

#[test]
fn test_run_at_clean_purges_dead_slots() {
    let dir = TempDir::new().unwrap();
    let me = u64::from(std::process::id());

    write(
        dir.path(),
        &sample(DEAD_PID_BASE + 5, "sync:extract", 100, true),
    );
    write(dir.path(), &sample(me, "idle", 200, true));
    assert_eq!(reader(dir.path()).occupied().len(), 2);

    tokensave::memstats::run_at(dir.path(), true).unwrap();

    let remaining = reader(dir.path()).occupied();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].pid, me);
}
