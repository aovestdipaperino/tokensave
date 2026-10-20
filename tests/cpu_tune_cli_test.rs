//! `tokensave init` benchmarks the host to pick a CPU usage profile, and
//! `tokensave tune` re-runs that benchmark later (#731). Driven through the
//! real binary because the selection lives in the CLI path.

use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;
use tokensave::config::{configured_cpu_usage, load_config, save_config};
use tokensave::cpu::CpuUsage;

fn tokensave(args: &[&str], dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .arg(dir)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run tokensave")
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn sample_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn hello() {}\n").unwrap();
    dir
}

#[test]
fn init_benchmarks_and_stores_an_existing_profile() {
    let dir = sample_project();
    let output = tokensave(&["init"], dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("Benchmarking this machine"), "{text}");
    assert!(text.contains("CPU usage profile: "), "{text}");
    // Written explicitly, so later runs and `init` never re-benchmark silently.
    let stored = configured_cpu_usage(dir.path()).unwrap();
    assert!(
        matches!(
            stored,
            Some(CpuUsage::Low | CpuUsage::Balanced | CpuUsage::Performance)
        ),
        "{stored:?}"
    );
}

#[test]
fn init_flag_skips_the_benchmark() {
    let dir = sample_project();
    let output = tokensave(&["init", "--cpu-usage", "performance"], dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(!text.contains("Benchmarking"), "{text}");
    assert!(text.contains("CPU usage profile: performance (from --cpu-usage)"));
    assert_eq!(
        load_config(dir.path()).unwrap().cpu_usage,
        CpuUsage::Performance
    );
}

#[test]
fn init_keeps_a_profile_already_in_the_config() {
    let dir = sample_project();
    let mut config = load_config(dir.path()).unwrap();
    config.cpu_usage = CpuUsage::Low;
    save_config(dir.path(), &config).unwrap();

    let output = tokensave(&["init"], dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(!text.contains("Benchmarking"), "{text}");
    assert!(text.contains("kept from .tokensave/config.json"), "{text}");
    assert_eq!(load_config(dir.path()).unwrap().cpu_usage, CpuUsage::Low);
}

#[test]
fn tune_updates_an_initialized_project() {
    let dir = sample_project();
    assert!(tokensave(&["init", "--cpu-usage", "low"], dir.path())
        .status
        .success());

    let output = tokensave(&["tune", "--json"], dir.path());
    assert!(output.status.success(), "{}", combined(&output));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let recommended: CpuUsage = serde_json::from_value(report["recommended"].clone()).unwrap();
    assert_eq!(report["previous"], "low");
    assert_eq!(report["written"], recommended != CpuUsage::Low);
    assert!(report["measurement"]["resources"]["cpus"].as_u64() >= Some(1));
    assert_eq!(load_config(dir.path()).unwrap().cpu_usage, recommended);
}

#[test]
fn tune_dry_run_never_writes() {
    let dir = sample_project();
    let output = tokensave(&["tune", "--dry-run"], dir.path());
    assert!(output.status.success(), "{}", combined(&output));
    assert!(
        !dir.path().join(".tokensave").exists(),
        "a dry run on an uninitialized project must not create .tokensave"
    );

    assert!(tokensave(&["init", "--cpu-usage", "low"], dir.path())
        .status
        .success());
    let before = std::fs::read(dir.path().join(".tokensave").join("config.json")).unwrap();
    let output = tokensave(&["tune", "--dry-run", "--json"], dir.path());
    assert!(output.status.success(), "{}", combined(&output));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["written"], false);
    assert_eq!(
        std::fs::read(dir.path().join(".tokensave").join("config.json")).unwrap(),
        before
    );
}

#[test]
fn tune_refuses_an_uninitialized_project() {
    let dir = sample_project();
    let output = tokensave(&["tune"], dir.path());
    assert!(!output.status.success());
    assert!(combined(&output).contains("tokensave init"));
    assert!(!dir.path().join(".tokensave").exists());
}

/// `--json` is a machine-readable mode, so it has to produce a document on
/// the error path too — not a human message on stderr and no output at all.
#[test]
fn tune_json_reports_an_uninitialized_project_as_a_document() {
    let dir = sample_project();
    let output = tokensave(&["tune", "--json"], dir.path());
    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("stdout is JSON ({e}): {stdout}"));
    assert!(
        report["error"]
            .as_str()
            .unwrap_or_default()
            .contains("tokensave init"),
        "{report}"
    );
    assert_eq!(report["written"].as_bool(), Some(false));
    assert!(!dir.path().join(".tokensave").exists());
}
