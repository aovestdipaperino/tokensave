//! `tokensave init` uses the machine-level hardware benchmark to pick a CPU
//! usage profile (#731, #752), and `tokensave tune` re-runs that benchmark
//! later. Driven through the real binary because the selection lives in the
//! CLI path. Each run gets its own home directory, so the global benchmark
//! cache never touches the developer's `~/.tokensave` (on Unix; Windows
//! resolves the profile directory without consulting the environment).

use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;
use tokensave::config::{configured_cpu_usage, load_config, save_config};
use tokensave::cpu::CpuUsage;

fn tokensave_in(home: &Path, args: &[&str], dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .arg(dir)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("TOKENSAVE_CPU_USAGE")
        .stdin(Stdio::null())
        .output()
        .expect("failed to run tokensave")
}

/// Runs in a project-private home (the project's own parent temp dir is
/// unique per test).
fn tokensave(args: &[&str], dir: &Path) -> Output {
    let home = dir.join(".test-home");
    std::fs::create_dir_all(&home).unwrap();
    tokensave_in(&home, args, dir)
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
fn init_benchmarks_globally_and_pins_nothing_in_the_project() {
    let dir = sample_project();
    let output = tokensave(&["init"], dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("hardware benchmark"), "{text}");
    assert!(text.contains("CPU usage profile: "), "{text}");
    // The machine-level result lives in the global cache, not the project.
    assert_eq!(configured_cpu_usage(dir.path()).unwrap(), None);
    if cfg!(not(windows)) {
        let cache = dir
            .path()
            .join(".test-home")
            .join(".tokensave")
            .join("benchmark.toml");
        assert!(cache.is_file(), "{text}");
    }
}

#[test]
fn init_flag_skips_the_benchmark() {
    let dir = sample_project();
    let output = tokensave(&["init", "--cpu-usage", "performance"], dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(!text.contains("benchmark"), "{text}");
    assert!(text.contains("CPU usage profile: performance (from --cpu-usage)"));
    assert_eq!(
        load_config(dir.path()).unwrap().cpu_usage,
        Some(CpuUsage::Performance)
    );
}

#[test]
fn init_keeps_a_profile_already_in_the_config() {
    let dir = sample_project();
    let mut config = load_config(dir.path()).unwrap();
    config.cpu_usage = Some(CpuUsage::Low);
    save_config(dir.path(), &config).unwrap();

    let output = tokensave(&["init"], dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(!text.contains("benchmark"), "{text}");
    assert!(text.contains("kept from .tokensave/config.json"), "{text}");
    assert_eq!(
        load_config(dir.path()).unwrap().cpu_usage,
        Some(CpuUsage::Low)
    );
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
    assert_eq!(
        load_config(dir.path()).unwrap().cpu_usage,
        Some(recommended)
    );
    assert_eq!(report["global_saved"], true);
}

#[test]
fn tune_leaves_an_unpinned_project_following_the_global_benchmark() {
    let dir = sample_project();
    assert!(tokensave(&["init"], dir.path()).status.success());
    let output = tokensave(&["tune", "--json"], dir.path());
    assert!(output.status.success(), "{}", combined(&output));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["previous"], serde_json::Value::Null);
    assert_eq!(report["written"], false);
    assert_eq!(report["global_saved"], true);
    assert_eq!(configured_cpu_usage(dir.path()).unwrap(), None);
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
    assert!(
        !dir.path()
            .join(".test-home")
            .join(".tokensave")
            .join("benchmark.toml")
            .exists(),
        "a dry run must not save the global benchmark"
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
