//! `branch add --if-enabled` honours the `auto_track` knob — #397.
//!
//! `auto_track` was consulted in exactly one place, `TokenSave::open`. The
//! `post-checkout` hook shells out to `tokensave branch add`, which never
//! looked at it, so a branch got tracked on every checkout whether or not the
//! knob was set. The comment at the gate said "(git-hook path is separate)",
//! so this was intentional — but it makes #342 Q1's goal of one runtime knob
//! governing auto-track across every entry point unreachable, and it holds on
//! *fresh* installs, not only on hooks written by an older version.
//!
//! The split is between an automated caller and a human one: a hook passes
//! `--if-enabled` and gets a no-op when the knob is off, while someone typing
//! `tokensave branch add` still means it. An explicit command should never
//! silently do nothing.

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn tokensave(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .current_dir(root)
        // Never let the developer's own global hooks or config leak in.
        .env("GIT_CONFIG_COUNT", "0")
        .env("HOME", root)
        .output()
        .expect("run tokensave")
}

/// A repo on `master`, indexed, with multi-branch bootstrapped and `feature`
/// checked out but untracked. `auto_track` is written as given.
fn project(auto_track: bool) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn base() -> i32 { 1 }\n").unwrap();

    git(root, &["init", "-b", "master"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);

    assert!(
        tokensave(root, &["init", "."]).status.success(),
        "init must succeed"
    );

    // Bootstrap multi-branch mode, then land on an untracked branch.
    git(root, &["checkout", "-b", "bootstrap"]);
    assert!(tokensave(root, &["branch", "add"]).status.success());
    git(root, &["checkout", "-b", "feature"]);

    let config_path = root.join(".tokensave/config.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["auto_track"] = serde_json::Value::Bool(auto_track);
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    tmp
}

fn is_tracked(root: &Path, branch: &str) -> bool {
    let meta = root.join(".tokensave/branch-meta.json");
    let Ok(text) = std::fs::read_to_string(meta) else {
        return false;
    };
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["branches"].get(branch).is_some()
}

/// The #397 fix: the hook's invocation is a no-op when the knob is off.
#[test]
fn if_enabled_does_nothing_when_auto_track_is_off() {
    let tmp = project(false);
    let root = tmp.path();

    let out = tokensave(root, &["branch", "add", "--if-enabled"]);
    assert!(
        out.status.success(),
        "a declined auto-track is a no-op, not a failure: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !is_tracked(root, "feature"),
        "auto_track is off, so the branch must not be tracked"
    );
}

/// With the knob on, the same invocation tracks the branch — otherwise the
/// flag would make the hook permanently inert rather than gated.
#[test]
fn if_enabled_tracks_the_branch_when_auto_track_is_on() {
    let tmp = project(true);
    let root = tmp.path();

    let out = tokensave(root, &["branch", "add", "--if-enabled"]);
    assert!(
        out.status.success(),
        "tracking must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        is_tracked(root, "feature"),
        "auto_track is on, so the branch must be tracked"
    );
}

/// An explicit command means it. Someone typing `branch add` has asked for
/// this, and silently doing nothing because of a config value they may not
/// know about would be worse than the bug being fixed.
#[test]
fn an_explicit_branch_add_ignores_auto_track() {
    let tmp = project(false);
    let root = tmp.path();

    let out = tokensave(root, &["branch", "add"]);
    assert!(
        out.status.success(),
        "explicit add must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        is_tracked(root, "feature"),
        "an explicit `branch add` must track regardless of auto_track"
    );
}

/// `TOKENSAVE_AUTO_TRACK` overrides the config for one run, the convention
/// `TokenSave::open` already follows. Without this the knob would mean two
/// different things depending on which entry point read it.
#[test]
fn if_enabled_honours_the_env_override() {
    let tmp = project(false);
    let root = tmp.path();

    let out = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["branch", "add", "--if-enabled"])
        .current_dir(root)
        .env("GIT_CONFIG_COUNT", "0")
        .env("HOME", root)
        .env("TOKENSAVE_AUTO_TRACK", "1")
        .output()
        .expect("run tokensave");

    assert!(out.status.success());
    assert!(
        is_tracked(root, "feature"),
        "TOKENSAVE_AUTO_TRACK=1 must enable the gated path even with the config off"
    );
}
