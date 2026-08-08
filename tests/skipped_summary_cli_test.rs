//! `tokensave init` and `tokensave sync` must say that tracked source files
//! were skipped because no extractor handles their extension (#345), so an
//! index that omitted every real source file cannot be mistaken for a
//! complete one.
//!
//! Driven through the real binary because the summary lives in the CLI path,
//! and because the point of the issue is what the user actually sees.

use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Runs `tokensave <args> <dir>` with stdin closed (never a TTY under
/// `cargo test`) and returns combined stdout+stderr.
fn run(dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run tokensave");
    assert!(
        output.status.success(),
        "tokensave {args:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A project whose only indexable file is documentation.
///
/// #345 used the Verilog source from its own report here; #344 has since added
/// a Verilog extractor, so this now uses VHDL — still a real hardware language,
/// still with no registered extractor.
fn project_with_unsupported_source() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("README.md"), "# readme\n").unwrap();
    std::fs::write(
        dir.path().join("example.vhd"),
        "entity example is\nend example;\n",
    )
    .unwrap();
    dir
}

/// How many times the compact summary appears, so a second reporting path
/// cannot silently start duplicating it.
fn headline_count(output: &str) -> usize {
    output
        .lines()
        .filter(|l| l.contains("Skipped 1 tracked file (unsupported extension): .vhd (1)"))
        .count()
}

#[test]
fn init_reports_the_unsupported_source_file() {
    let dir = project_with_unsupported_source();
    let output = run(dir.path(), &["init"]);

    assert_eq!(
        headline_count(&output),
        1,
        "init must report the skipped file exactly once: {output}"
    );
}

#[test]
fn sync_reports_the_unsupported_source_file() {
    let dir = project_with_unsupported_source();
    run(dir.path(), &["init"]);
    // A no-op sync is the confusing case from the issue: "0 added, 0 modified,
    // 0 removed" with no hint that example.vhd was never indexed.
    let output = run(dir.path(), &["sync"]);

    assert!(
        output.contains("sync done"),
        "sync must still report its normal summary: {output}"
    );
    assert_eq!(
        headline_count(&output),
        1,
        "sync must report the skipped file exactly once: {output}"
    );
}

#[test]
fn detailed_modes_do_not_duplicate_the_compact_summary() {
    let dir = project_with_unsupported_source();
    run(dir.path(), &["init"]);

    // `--doctor` and `--verbose` each already print the per-extension detail,
    // so the compact headline must not be added on top of it.
    let doctor = run(dir.path(), &["sync", "--doctor"]);
    assert_eq!(headline_count(&doctor), 0, "doctor output: {doctor}");
    assert!(
        doctor.contains("Skipped extensions (no registered extractor):"),
        "doctor must still list the skipped extension: {doctor}"
    );

    let verbose = run(dir.path(), &["sync", "--verbose"]);
    assert_eq!(headline_count(&verbose), 0, "verbose output: {verbose}");
    assert!(
        verbose.contains(".vhd: 1 file(s) skipped (no registered extractor)"),
        "verbose must still list the skipped extension: {verbose}"
    );
}

#[test]
fn nothing_is_reported_when_every_file_is_supported() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

    let output = format!(
        "{}{}",
        run(dir.path(), &["init"]),
        run(dir.path(), &["sync"])
    );

    assert!(
        !output.contains("unsupported extension"),
        "a fully supported project must stay quiet: {output}"
    );
}

/// Regression for #373: a non-interactive `init` (stdin closed, never a TTY)
/// must still write the default local git exclusion, instead of silently
/// dropping it and leaving `.tokensave/` untracked.
#[test]
fn init_without_tty_still_excludes_tokensave_from_git() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

    let init = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("init")
        .arg("-q")
        .status()
        .expect("failed to git init");
    assert!(init.success(), "git init must succeed");

    // `run` drives the real binary with stdin closed, so this is the
    // non-interactive path from #373.
    let output = run(&repo, &["init"]);
    assert!(
        output.contains("Added .tokensave/ to .git/info/exclude (local, untracked)"),
        "non-interactive init must report the local exclusion: {output}"
    );

    let exclude = std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(
        exclude.lines().any(|l| l.trim() == ".tokensave/"),
        "non-interactive init must write .tokensave/ to .git/info/exclude, got: {exclude}"
    );
}
