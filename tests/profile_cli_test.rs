//! `tokensave profile ...` through the real binary (#754, #885): the parts
//! the inline unit tests cannot reach. `enable`/`disable` writing
//! `~/.tokensave/config.toml`, `--for` expiry as the CLI records and reports
//! it, and the end-to-end path from enabling to a recorded run in `report`.
//!
//! Every run gets its own home directory. Unix only: on Windows the home
//! directory is resolved through the known-folder API rather than
//! `HOME`/`USERPROFILE`, so these commands would change the developer's real
//! profiling configuration and data.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

fn tokensave(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("TOKENSAVE_UPDATE_CHECK", "off")
        .env_remove("TOKENSAVE_PROFILE")
        .env_remove("TOKENSAVE_CPU_USAGE")
        .env_remove("TOKENSAVE_AUTO_TRACK")
        .stdin(Stdio::null())
        .output()
        .expect("failed to run tokensave")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn config_path(home: &Path) -> std::path::PathBuf {
    home.join(".tokensave").join("config.toml")
}

/// The `[profiling]` table of the home's `config.toml`, if any.
fn profiling_table(home: &Path) -> Option<toml::Table> {
    let text = std::fs::read_to_string(config_path(home)).ok()?;
    let config: toml::Table = toml::from_str(&text).expect("config.toml parses");
    config.get("profiling")?.as_table().cloned()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[test]
fn enable_and_disable_write_the_config_table() {
    let home = TempDir::new().unwrap();

    let output = tokensave(home.path(), &["profile", "enable"]);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(stdout(&output).contains("Profiling enabled (light)."));
    let table = profiling_table(home.path()).expect("[profiling] written");
    assert_eq!(table["mode"].as_str(), Some("light"));
    assert!(!table.contains_key("detailed_expires_at"));

    let output = tokensave(home.path(), &["profile", "disable"]);
    assert!(output.status.success(), "{}", combined(&output));
    // An all-default table may be omitted entirely; either way it is off.
    let mode = profiling_table(home.path())
        .and_then(|t| t.get("mode").and_then(|m| m.as_str()).map(str::to_string));
    assert!(
        matches!(mode.as_deref(), None | Some("off")),
        "mode after disable: {mode:?}"
    );

    let output = tokensave(home.path(), &["profile", "status"]);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(
        stdout(&output).starts_with("Profiling: off"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn detailed_for_records_an_expiry_that_status_reports_and_honours() {
    let home = TempDir::new().unwrap();

    let before = unix_now();
    let output = tokensave(
        home.path(),
        &["profile", "enable", "--detailed", "--for", "2h"],
    );
    let after = unix_now();
    let text = stdout(&output);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(
        text.contains("Profiling enabled (detailed) until "),
        "{text}"
    );
    assert!(!text.contains("UNIX time"), "{text}");
    assert!(
        text.contains("Z (in 2h 0m)") || text.contains("Z (in 1h 59m)"),
        "{text}"
    );

    let table = profiling_table(home.path()).expect("[profiling] written");
    assert_eq!(table["mode"].as_str(), Some("detailed"));
    let expires = table["detailed_expires_at"].as_integer().unwrap();
    assert!(
        (before + 7_200..=after + 7_200).contains(&expires),
        "expiry {expires} not 2h after {before}..{after}"
    );

    let output = tokensave(home.path(), &["profile", "status"]);
    let text = stdout(&output);
    assert!(text.starts_with("Profiling: detailed\n"), "{text}");
    assert!(text.contains("Detailed mode expires in 1h"), "{text}");

    // Move the recorded expiry into the past: the configured mode stays
    // `detailed`, but it now runs as `light`.
    let config = std::fs::read_to_string(config_path(home.path())).unwrap();
    let expired = config.replace(
        &format!("detailed_expires_at = {expires}"),
        &format!("detailed_expires_at = {}", unix_now() - 60),
    );
    assert_ne!(config, expired, "expiry line not found in:\n{config}");
    std::fs::write(config_path(home.path()), expired).unwrap();

    let output = tokensave(home.path(), &["profile", "status"]);
    let text = stdout(&output);
    assert!(
        text.starts_with("Profiling: light (configured: detailed; detailed mode expired)"),
        "{text}"
    );
    assert!(!text.contains("Detailed mode expires"), "{text}");
}

#[test]
fn enabled_profiling_records_a_run_that_report_shows_and_reset_deletes() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("lib.rs"), "pub fn hello() {}\n").unwrap();
    let project_arg = project.path().to_str().unwrap();

    // Nothing recorded yet: the report says so.
    let output = tokensave(home.path(), &["profile", "report"]);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(stdout(&output).contains("No profile data recorded"));

    let output = tokensave(home.path(), &["profile", "enable"]);
    assert!(output.status.success(), "{}", combined(&output));
    let output = tokensave(
        home.path(),
        &["init", "--cpu-usage", "performance", project_arg],
    );
    assert!(output.status.success(), "{}", combined(&output));

    let output = tokensave(home.path(), &["profile", "report", "--indexing"]);
    let text = stdout(&output);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(text.contains("index.total"), "{text}");

    let output = tokensave(home.path(), &["profile", "status"]);
    let text = stdout(&output);
    assert!(!text.contains(" 0 file(s)"), "{text}");
    assert!(text.contains("from 1 process(es)"), "{text}");

    // An unknown engine names the ones that have data instead of reporting
    // an empty profile.
    let output = tokensave(home.path(), &["profile", "report", "--engine", "nope"]);
    assert!(!output.status.success());
    let text = combined(&output);
    assert!(text.contains("no profile data for engine 'nope'"), "{text}");
    assert!(text.contains("available engines: libsql"), "{text}");

    // Without a terminal to confirm on, reset refuses and deletes nothing.
    let output = tokensave(home.path(), &["profile", "reset"]);
    assert!(!output.status.success());
    assert!(
        combined(&output).contains("--force"),
        "{}",
        combined(&output)
    );
    let output = tokensave(home.path(), &["profile", "report", "--indexing"]);
    assert!(stdout(&output).contains("index.total"));

    let output = tokensave(home.path(), &["profile", "reset", "--force"]);
    let text = stdout(&output);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(text.starts_with("Removed "), "{text}");
    assert!(!text.starts_with("Removed 0 "), "{text}");
    let output = tokensave(home.path(), &["profile", "report"]);
    assert!(stdout(&output).contains("No profile data recorded"));
}
