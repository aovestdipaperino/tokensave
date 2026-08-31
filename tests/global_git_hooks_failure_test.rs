//! `tokensave githooks on` (global, no `--local`) must not report success when
//! it could not install anything.
//!
//! This file holds a single test on purpose: it overrides `HOME`, which is
//! process-global, and cargo runs the tests inside one binary on threads.

use tokensave::agents::{offer_git_post_commit_hook, GitHookMode};

#[test]
fn a_global_hook_install_that_cannot_proceed_is_reported_as_an_error() {
    let home = tempfile::tempdir().expect("tempdir");

    // Occupy `~/.config/git/hooks` with a *file*, so `create_dir_all` for the
    // hooks directory cannot succeed. This is the real-world shape of the bug:
    // the command prints a red ✘ and then exits 0.
    let git_dir = home.path().join(".config").join("git");
    std::fs::create_dir_all(&git_dir).expect("git config dir");
    std::fs::write(git_dir.join("hooks"), "not a directory").expect("occupy hooks path");

    std::env::set_var("HOME", home.path());
    std::env::set_var("USERPROFILE", home.path());

    let result = offer_git_post_commit_hook("/usr/bin/tokensave", GitHookMode::Yes);

    assert!(
        result.is_err(),
        "a global hook install that installed nothing must report an error, got: {result:?}"
    );
}
