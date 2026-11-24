//! Commit provenance reported by `tokensave --version --verbose` (#1062).
//!
//! Exercises `build/provenance.rs`, which `build.rs` includes, against Git
//! checkouts and packaged (crates.io) source trees.

use std::fs;
use std::path::Path;
use std::process::Command;

#[allow(
    dead_code,
    reason = "the module is shared with build.rs, which uses the items not exercised here"
)]
#[path = "../build/constants.rs"]
mod constants;

#[path = "../build/provenance.rs"]
mod provenance;

use constants::{CARGO_VCS_INFO_FILE_NAME, COMMIT_ABBREV_LEN, DIRTY_SUFFIX};
use provenance::{commit_from_cargo_vcs_info, resolve};

const SAMPLE_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// `.cargo_vcs_info.json` as written by `cargo package` from a clean tree.
fn clean_vcs_info(sha: &str) -> String {
    format!("{{\n  \"git\": {{\n    \"sha1\": \"{sha}\"\n  }},\n  \"path_in_vcs\": \"\"\n}}")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=tokensave-test",
            "-c",
            "user.email=tokensave-test@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout)
        .expect("UTF-8 git output")
        .trim()
        .to_owned()
}

fn init_repo_with_commit(dir: &Path) -> String {
    git(dir, &["init", "-q"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "unrelated"]);
    git(dir, &["rev-parse", "HEAD"])
}

#[test]
fn cargo_vcs_info_reports_short_commit() {
    assert_eq!(
        commit_from_cargo_vcs_info(&clean_vcs_info(SAMPLE_SHA)).as_deref(),
        Some(&SAMPLE_SHA[..COMMIT_ABBREV_LEN])
    );
    let compact = format!(
        r#"{{"git":{{"sha1":"{}"}},"path_in_vcs":""}}"#,
        SAMPLE_SHA.to_uppercase()
    );
    assert_eq!(
        commit_from_cargo_vcs_info(&compact).as_deref(),
        Some(&SAMPLE_SHA[..COMMIT_ABBREV_LEN])
    );
}

#[test]
fn dirty_package_is_marked() {
    let json =
        format!(r#"{{"git": {{"sha1": "{SAMPLE_SHA}", "dirty": true}}, "path_in_vcs": ""}}"#);
    assert_eq!(
        commit_from_cargo_vcs_info(&json),
        Some(format!(
            "{}{DIRTY_SUFFIX}",
            &SAMPLE_SHA[..COMMIT_ABBREV_LEN]
        ))
    );
    let not_dirty = json.replace("true", "false");
    assert_eq!(
        commit_from_cargo_vcs_info(&not_dirty).as_deref(),
        Some(&SAMPLE_SHA[..COMMIT_ABBREV_LEN])
    );
}

#[test]
fn malformed_vcs_info_reports_no_commit() {
    for json in [
        String::new(),
        "{}".to_owned(),
        "not json".to_owned(),
        r#"{"path_in_vcs": ""}"#.to_owned(),
        clean_vcs_info(&SAMPLE_SHA[..COMMIT_ABBREV_LEN]),
        clean_vcs_info(&format!("{SAMPLE_SHA}00")),
        clean_vcs_info(&SAMPLE_SHA.replace('a', "g")),
        r#"{"git": {"sha1": 12345}}"#.to_owned(),
        format!(r#"{{"sha1":"{SAMPLE_SHA}","git":{{}}}}"#),
        format!(r#"{{"other":{{"sha1":"{SAMPLE_SHA}"}},"git":{{}}}}"#),
    ] {
        assert_eq!(commit_from_cargo_vcs_info(&json), None, "input: {json:?}");
    }
}

#[test]
fn invalid_git_metadata_falls_back_to_cargo_vcs_info() {
    let dir = tempfile::tempdir().expect("create package directory");
    fs::create_dir(dir.path().join(constants::git::DIR)).expect("create invalid git metadata");
    fs::write(
        dir.path().join(CARGO_VCS_INFO_FILE_NAME),
        clean_vcs_info(SAMPLE_SHA),
    )
    .expect("write vcs info");

    assert_eq!(
        resolve(dir.path()).commit.as_deref(),
        Some(&SAMPLE_SHA[..COMMIT_ABBREV_LEN])
    );
}

#[test]
fn packaged_crate_uses_cargo_vcs_info() {
    let dir = tempfile::tempdir().expect("create package directory");
    let vcs_info = dir.path().join(CARGO_VCS_INFO_FILE_NAME);
    fs::write(&vcs_info, clean_vcs_info(SAMPLE_SHA)).expect("write vcs info");

    let resolved = resolve(dir.path());
    assert_eq!(
        resolved.commit.as_deref(),
        Some(&SAMPLE_SHA[..COMMIT_ABBREV_LEN])
    );
    assert_eq!(
        resolved.rerun_if_changed,
        vec![vcs_info.display().to_string()]
    );
}

#[test]
fn source_without_provenance_reports_no_commit() {
    let dir = tempfile::tempdir().expect("create package directory");
    let resolved = resolve(dir.path());
    assert_eq!(resolved.commit, None);
    assert!(resolved.rerun_if_changed.is_empty());
}

#[test]
fn package_inside_unrelated_repository_ignores_that_repository() {
    let dir = tempfile::tempdir().expect("create repository directory");
    let unrelated = init_repo_with_commit(dir.path());

    let without_info = dir.path().join("unpacked-without-info");
    fs::create_dir(&without_info).expect("create package directory");
    assert_eq!(resolve(&without_info).commit, None);

    let with_info = dir.path().join("unpacked-with-info");
    fs::create_dir(&with_info).expect("create package directory");
    fs::write(
        with_info.join(CARGO_VCS_INFO_FILE_NAME),
        clean_vcs_info(SAMPLE_SHA),
    )
    .expect("write vcs info");
    let commit = resolve(&with_info).commit.expect("packaged commit");
    assert_eq!(commit, SAMPLE_SHA[..COMMIT_ABBREV_LEN]);
    assert!(!unrelated.starts_with(&commit));
}

#[test]
fn git_checkout_reports_head_and_prefers_git_over_vcs_info() {
    let dir = tempfile::tempdir().expect("create repository directory");
    let head = init_repo_with_commit(dir.path());
    fs::write(
        dir.path().join(CARGO_VCS_INFO_FILE_NAME),
        clean_vcs_info(SAMPLE_SHA),
    )
    .expect("write vcs info");

    let resolved = resolve(dir.path());
    assert_eq!(resolved.commit.as_deref(), Some(&head[..COMMIT_ABBREV_LEN]));
    assert!(!resolved.rerun_if_changed.is_empty());
}
