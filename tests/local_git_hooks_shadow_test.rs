//! #455, the case that has to run alone: a `core.hooksPath` shadowing the
//! repository's own hook directory.
//!
//! Its own test binary because it has to control `core.hooksPath` process-wide.
//! `.cargo/config.toml` forces `core.hooksPath=` into every git child so tests
//! never inherit the developer's real hooks (#381), and that command-scope
//! setting outranks anything written to a repository's config — so the only
//! way to make git report a hooksPath here is to occupy the same indexed-config
//! slot, which the cargo config's own comment anticipates. Doing that from a
//! shared test binary would leak into every other test in it.

use std::path::Path;
use std::process::Command;
use tokensave::agents::{describe_local_git_hooks, install_local_git_hooks};

/// A `core.hooksPath` makes git resolve **every** hook from that one directory
/// with no fallback to the repository's own, so a hook written there would
/// never run. Saying so is the difference between an honest install and a
/// silent no-op — and it is #455's own complaint pointed the other way.
#[test]
fn a_hookspath_that_shadows_the_repository_is_reported() {
    let dir = tempfile::Builder::new()
        .prefix("ts455shadow")
        .tempdir()
        .expect("tempdir");
    let repo: &Path = dir.path();
    let elsewhere = repo.join("other-hooks");
    std::fs::create_dir_all(&elsewhere).expect("create");

    // Occupy slot 0 rather than clearing the isolation: clearing it would let
    // the developer's real global hooksPath decide the outcome, so the test
    // would pass on this machine and fail on a CI runner that has none.
    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var("GIT_CONFIG_KEY_0", "core.hooksPath");
    std::env::set_var("GIT_CONFIG_VALUE_0", &elsewhere);

    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo)
        .output()
        .expect("run git")
        .status
        .success();
    assert!(ok, "git init failed");

    let out = install_local_git_hooks(repo, "/usr/bin/tokensave").expect("install");
    assert_eq!(
        out.shadowed_by.as_deref(),
        Some(elsewhere.as_path()),
        "a shadowed install must name the directory git will actually read"
    );
    assert!(
        !out.installed.is_empty(),
        "the files are still written — the warning is about what git will read, \
         not a reason to refuse"
    );
    assert!(
        describe_local_git_hooks(repo)
            .iter()
            .any(|l| l.contains("core.hooksPath")),
        "the status output must surface it too"
    );
}
