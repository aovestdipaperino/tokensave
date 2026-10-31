use std::{
    path::Path,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokensave::{
    db::Database,
    integrity::{self, Check},
};

fn executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_tokensave"))
}
async fn database() -> (TempDir, std::path::PathBuf, Database) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let (db, _) = Database::initialize(&path).await.unwrap();
    (dir, path, db)
}
async fn finish(check: &mut Check) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check.poll().await.unwrap() {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
/// A child forked concurrently by another test can briefly inherit the lock
/// descriptor until it execs, so a just-released lock may still read as held.
async fn start_after_release(path: &Path) -> Check {
    start_after_release_with(path, false).await
}
async fn start_after_release_with(path: &Path, full: bool) -> Check {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(check) = Check::start(path, executable(), full, None).unwrap() {
            return check;
        }
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
#[tokio::test]
async fn healthy_checks_preserve_database_and_persist_results() {
    let (_dir, path, db) = database().await;
    db.checkpoint().await.unwrap();
    drop(db);
    let bytes = std::fs::read(&path).unwrap();
    for full in [false, true] {
        let mut check = start_after_release_with(&path, full).await;
        finish(&mut check).await;
        let report = check.report().unwrap();
        assert_eq!(report.outcome, "healthy", "{report:?}");
        assert_eq!(report.full, full);
        assert!(report.finished_at.is_some());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    assert!(Check::start(&path, executable(), false, Some(86400))
        .unwrap()
        .is_none());
    assert!(Check::start(&path, executable(), false, Some(0))
        .unwrap()
        .is_none());
}
#[tokio::test]
async fn corrupt_and_missing_files_are_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("corrupt.db");
    std::fs::write(&path, b"not a database").unwrap();
    let mut check = Check::start(&path, executable(), false, None)
        .unwrap()
        .unwrap();
    finish(&mut check).await;
    let report = check.report().unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(!report.diagnostics.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), b"not a database");
    let missing = dir.path().join("missing.db");
    assert!(Check::start(&missing, executable(), false, None).is_err());
    assert!(!missing.exists());
}
#[tokio::test]
async fn concurrent_writer_and_rollback_do_not_block_or_report_corruption() {
    let (_dir, path, db) = database().await;
    db.conn()
        .execute_batch("BEGIN IMMEDIATE; INSERT INTO metadata(key,value) VALUES('pending','1');")
        .await
        .unwrap();
    let mut check = Check::start(&path, executable(), true, None)
        .unwrap()
        .unwrap();
    finish(&mut check).await;
    assert_eq!(check.report().unwrap().outcome, "healthy");
    drop(check);
    db.conn().execute_batch("ROLLBACK").await.unwrap();
    assert_eq!(integrity::inspect(&path, true).await.outcome, "healthy");
}
#[tokio::test]
async fn process_lock_and_cancellation_release_resources() {
    let (_dir, path, _db) = database().await;
    for _ in 0..5 {
        let mut check = start_after_release(&path).await;
        assert!(Check::start(&path, executable(), false, None)
            .unwrap()
            .is_none());
        check.stop("deferred", "test cancellation").await.unwrap();
        let outcome = check.report().unwrap().outcome;
        assert!(outcome == "deferred" || outcome == "healthy", "{outcome}");
        drop(check);
    }
    let mut check = start_after_release(&path).await;
    finish(&mut check).await;
    assert_eq!(check.report().unwrap().outcome, "healthy");
}
#[tokio::test]
async fn logical_inconsistency_is_not_file_corruption() {
    let (_dir, path, db) = database().await;
    db.conn().execute_batch("PRAGMA foreign_keys=OFF; INSERT INTO edges(source,target,kind) VALUES ('missing','also_missing','calls');").await.unwrap();
    assert_eq!(integrity::inspect(&path, true).await.outcome, "healthy");
}
#[tokio::test]
async fn scheduler_defers_when_busy_and_disabled_without_launching_work() {
    let (_dir, path, _db) = database().await;
    let mut scheduler = integrity::Scheduler::new(path.clone(), 1);
    scheduler.tick(true).await.unwrap();
    assert!(integrity::load(&path).is_err());
    let mut scheduler = integrity::Scheduler::new(path.clone(), 0);
    for _ in 0..100 {
        scheduler.tick(false).await.unwrap();
    }
    assert!(integrity::load(&path).is_err());
}
#[test]
fn config_defaults_to_daily_checks_and_accepts_disable() {
    let mut value = serde_json::to_value(tokensave::config::TokenSaveConfig::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("integrity_interval_secs");
    let config: tokensave::config::TokenSaveConfig = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(config.integrity_interval_secs, 86400);
    value["integrity_interval_secs"] = 0.into();
    let config: tokensave::config::TokenSaveConfig = serde_json::from_value(value).unwrap();
    assert_eq!(config.integrity_interval_secs, 0);
}

#[tokio::test]
async fn damaged_page_is_reported_without_recovery() {
    use std::io::{Seek, Write};
    let (_dir, path, db) = database().await;
    db.checkpoint().await.unwrap();
    drop(db);
    let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(std::io::SeekFrom::Start(8192)).unwrap();
    file.write_all(&[0xde, 0xad, 0xbe, 0xef].repeat(64))
        .unwrap();
    drop(file);
    let bytes = std::fs::read(&path).unwrap();
    let report = integrity::inspect(&path, false).await;
    assert_ne!(report.outcome, "healthy", "{report:?}");
    assert!(!report.diagnostics.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_kills_and_reaps_a_stalled_child() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, path, _db) = database().await;
    let worker = dir.path().join("stalled-worker");
    std::fs::write(&worker, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut check = Check::start(&path, &worker, false, None).unwrap().unwrap();
    finish(&mut check).await;
    let report = check.report().unwrap();
    assert_eq!(report.outcome, "interrupted");
    assert!(report.diagnostics[0].contains("budget"));
    drop(check);
    assert!(Check::start(&path, executable(), false, Some(86400))
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn recent_attempt_gate_is_cheap_and_does_not_spawn() {
    let (_dir, path, _db) = database().await;
    let mut check = Check::start(&path, executable(), false, None)
        .unwrap()
        .unwrap();
    finish(&mut check).await;
    drop(check);
    let before = integrity::load(&path).unwrap();
    let start = Instant::now();
    for _ in 0..100 {
        // A nonexistent executable proves due-check gating never spawns work.
        assert!(
            Check::start(&path, Path::new("nonexistent-checker"), false, Some(86400))
                .unwrap()
                .is_none()
        );
    }
    eprintln!("100 persisted interval gates: {:?}", start.elapsed());
    assert_eq!(
        integrity::load(&path).unwrap().started_at,
        before.started_at
    );
}

#[cfg(unix)]
#[tokio::test]
async fn saving_diagnostics_does_not_follow_a_sidecar_symlink() {
    let (_dir, path, db) = database().await;
    db.checkpoint().await.unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(".integrity.json");
    std::os::unix::fs::symlink(&path, &sidecar).unwrap();
    let mut check = Check::start(&path, executable(), false, None)
        .unwrap()
        .unwrap();
    finish(&mut check).await;
    assert_eq!(check.report().unwrap().outcome, "healthy");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

fn sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.canonicalize().unwrap().into_os_string();
    name.push(suffix);
    name.into()
}
fn write_report(path: &Path, outcome: &str, age_secs: i64) {
    let report = integrity::Report {
        started_at: tokensave::tokensave::current_timestamp() - age_secs,
        finished_at: None,
        outcome: outcome.into(),
        full: false,
        diagnostics: Vec::new(),
    };
    std::fs::write(
        sidecar(path, ".integrity.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn deferred_attempts_retry_sooner_than_failed_ones() {
    let (_dir, path, _db) = database().await;
    let retry = i64::try_from(integrity::DEFERRED_RETRY_SECS).unwrap();
    // Nonexistent executable: a gated attempt returns None, a due one fails to spawn.
    let gated = |path: &Path| {
        Check::start(path, Path::new("nonexistent-checker"), false, Some(86400))
            .is_ok_and(|check| check.is_none())
    };
    write_report(&path, "deferred", retry - 60);
    assert!(gated(&path));
    write_report(&path, "interrupted", retry + 60);
    assert!(gated(&path));
    write_report(&path, "deferred", retry + 60);
    assert!(!gated(&path));
    // The retry delay never exceeds a shorter configured interval.
    write_report(&path, "deferred", 120);
    let mut check = Check::start(&path, executable(), false, Some(60))
        .unwrap()
        .unwrap();
    finish(&mut check).await;
    assert_eq!(check.report().unwrap().outcome, "healthy");
}

#[tokio::test]
async fn gated_ticks_do_not_create_the_lockfile() {
    let (_dir, path, _db) = database().await;
    write_report(&path, "healthy", 0);
    assert!(Check::start(&path, executable(), false, Some(86400))
        .unwrap()
        .is_none());
    assert!(!sidecar(&path, ".integrity.lock").exists());
}

#[tokio::test]
async fn status_reports_an_orphaned_running_record_as_interrupted() {
    let (_dir, path, _db) = database().await;
    write_report(&path, "running", 0);
    let report = integrity::status(&path).unwrap();
    assert_eq!(report.outcome, "interrupted");
    assert_eq!(report.exit_code(), 2);
    let mut check = start_after_release(&path).await;
    assert_eq!(integrity::status(&path).unwrap().outcome, "running");
    finish(&mut check).await;
}
