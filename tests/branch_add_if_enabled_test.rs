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
    std::fs::write(
        root.join("src").join("lib.rs"),
        "pub fn base() -> i32 { 1 }\n",
    )
    .unwrap();

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

    let config_path = tokensave::config::get_tokensave_dir(root).join("config.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["auto_track"] = serde_json::Value::Bool(auto_track);
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    tmp
}

fn is_tracked(root: &Path, branch: &str) -> bool {
    let meta = tokensave::config::get_tokensave_dir(root).join("branch-meta.json");
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

#[test]
fn repeated_branch_add_is_idempotent() {
    let tmp = project(false);
    let root = tmp.path();

    let first = tokensave(root, &["branch", "add"]);
    assert!(
        first.status.success(),
        "first add must succeed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let meta_before =
        std::fs::read(tokensave::config::get_tokensave_dir(root).join("branch-meta.json")).unwrap();

    let second = tokensave(root, &["branch", "add"]);
    assert!(
        second.status.success(),
        "repeated add must remain a no-op: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        std::fs::read(tokensave::config::get_tokensave_dir(root).join("branch-meta.json")).unwrap(),
        meta_before,
        "repeated add must not rewrite branch metadata"
    );
    assert!(is_tracked(root, "feature"));
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

// ---------------------------------------------------------------------------
// #877: an opt-in explanation of what the hook skipped, quiet by default.
// ---------------------------------------------------------------------------

/// Runs `tokensave` with extra environment variables. The temp-dir variables
/// point away from the project: these fixtures live in the system temp dir,
/// which the `post-checkout` hook skips on purpose (#569).
fn tokensave_with_env(root: &Path, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let elsewhere = root.join("elsewhere-tmp");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_COUNT", "0")
        .env("HOME", root)
        .env("TMPDIR", &elsewhere)
        .env("TMP", &elsewhere)
        .env("TEMP", &elsewhere)
        .env_remove("TOKENSAVE_AUTO_TRACK")
        .env_remove("TOKENSAVE_HOOK_DEBUG");
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("run tokensave")
}

fn hook_debug_log(root: &Path) -> Option<String> {
    std::fs::read_to_string(tokensave::config::get_tokensave_dir(root).join("hook-debug.log")).ok()
}

/// git's arguments for an ordinary branch checkout.
const BRANCH_CHECKOUT: &[&str] = &[
    "hook",
    "post-checkout",
    "1111111111111111111111111111111111111111",
    "2222222222222222222222222222222222222222",
    "1",
];

#[test]
fn a_declined_auto_track_stays_quiet_without_hook_debug() {
    let tmp = project(false);
    let root = tmp.path();

    for args in [&["branch", "add", "--if-enabled"][..], BRANCH_CHECKOUT] {
        let out = tokensave_with_env(root, args, &[]);
        assert!(out.status.success());
        assert!(
            out.stdout.is_empty() && out.stderr.is_empty(),
            "`tokensave {}` must not narrate a normal checkout: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(hook_debug_log(root), None, "no debug log unless asked for");
    assert!(!is_tracked(root, "feature"));
}

#[test]
fn hook_debug_explains_a_declined_auto_track() {
    let tmp = project(false);
    let root = tmp.path();

    let out = tokensave_with_env(
        root,
        &["branch", "add", "--if-enabled"],
        &[("TOKENSAVE_HOOK_DEBUG", "1")],
    );
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("did not track branch 'feature'")
            && stderr.contains("automatic branch tracking is disabled")
            && stderr.contains("\"auto_track\": true")
            && stderr.contains("TOKENSAVE_AUTO_TRACK=1"),
        "the explanation must name the setting that enables tracking: {stderr}"
    );
    // The installed hooks discard output, so the log is what a user reads.
    let log = hook_debug_log(root).expect("debug log written");
    assert!(log.contains("did not track branch 'feature'"), "{log}");
    assert!(!is_tracked(root, "feature"));
}

#[test]
fn post_checkout_hook_debug_explains_skipped_tracking() {
    let tmp = project(false);
    let root = tmp.path();

    let out = tokensave_with_env(root, BRANCH_CHECKOUT, &[("TOKENSAVE_HOOK_DEBUG", "1")]);
    assert!(out.status.success(), "a hook must never fail the checkout");
    let log = hook_debug_log(root).expect("debug log written");
    assert!(
        log.contains("post-checkout: did not track branch 'feature'")
            && log.contains("automatic branch tracking is disabled"),
        "{log}"
    );
    assert!(!is_tracked(root, "feature"));
}

#[test]
fn post_checkout_hook_debug_reports_tracking_when_enabled() {
    let tmp = project(true);
    let root = tmp.path();

    let out = tokensave_with_env(root, BRANCH_CHECKOUT, &[("TOKENSAVE_HOOK_DEBUG", "1")]);
    assert!(out.status.success());
    assert!(is_tracked(root, "feature"), "auto_track is on");
    let log = hook_debug_log(root).expect("debug log written");
    assert!(
        log.contains("post-checkout: tracked the current branch"),
        "{log}"
    );
}

#[test]
fn post_checkout_hook_debug_explains_a_file_checkout() {
    let tmp = project(true);
    let root = tmp.path();

    let mut args = BRANCH_CHECKOUT.to_vec();
    *args.last_mut().unwrap() = "0";
    let out = tokensave_with_env(root, &args, &[("TOKENSAVE_HOOK_DEBUG", "1")]);
    assert!(out.status.success());
    assert!(
        !is_tracked(root, "feature"),
        "a file checkout tracks nothing"
    );
    let log = hook_debug_log(root).expect("debug log written");
    assert!(
        log.contains("post-checkout: file checkout, nothing to do"),
        "{log}"
    );
}

/// A debug setting left on must not grow the log without bound: past its
/// limit the log starts afresh.
#[test]
fn hook_debug_log_is_bounded() {
    let tmp = project(false);
    let root = tmp.path();
    let log_path = tokensave::config::get_tokensave_dir(root).join("hook-debug.log");
    std::fs::write(&log_path, vec![b'x'; 300 * 1024]).unwrap();

    let out = tokensave_with_env(root, BRANCH_CHECKOUT, &[("TOKENSAVE_HOOK_DEBUG", "1")]);
    assert!(out.status.success());
    let log = std::fs::read_to_string(&log_path).unwrap();
    assert!(
        log.len() < 4096,
        "the oversized log must restart: {} bytes",
        log.len()
    );
    assert!(log.contains("post-checkout: did not track branch 'feature'"));
}
