//! A forwarder that could not be written must be reported too (#488).
//!
//! Claiming a global `core.hooksPath` disables every hook type in each repo's
//! `.git/hooks/`, so tokensave drops pure forwarders for the hooks it does not
//! own. Those writes print the same "Failed to create" line as the owned hooks,
//! so leaving them out of the failure list reproduces the exact bug this issue
//! is about: a printed failure and an exit code of 0.
//!
//! Single test on purpose: it overrides `HOME`, which is process-global.
#![cfg(unix)]

use tokensave::agents::{offer_git_post_commit_hook, GitHookMode};

#[test]
fn an_unwritable_forwarder_is_reported() {
    let home = tempfile::tempdir().expect("tempdir");
    let hooks = home.path().join(".config").join("git").join("hooks");
    std::fs::create_dir_all(&hooks).expect("create hooks dir");

    // A dangling symlink: `exists()` follows it and reports false, so the
    // forwarder is attempted, and the write then fails.
    std::os::unix::fs::symlink(
        home.path().join("gone").join("pre-commit"),
        hooks.join("pre-commit"),
    )
    .expect("dangle pre-commit");

    std::env::set_var("HOME", home.path());
    std::env::set_var("USERPROFILE", home.path());

    let result = offer_git_post_commit_hook("/usr/bin/tokensave", GitHookMode::Yes);

    let message = result.expect_err("a forwarder that could not be written must be reported");
    assert!(
        message.contains("pre-commit"),
        "the error must name the forwarder that failed, got: {message}"
    );

    // The hooks the user actually asked for still install.
    assert!(
        hooks.join("post-commit").is_file(),
        "post-commit must still install"
    );
}
