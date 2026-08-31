//! One hook failing to write must not stop the other two from installing, and
//! must still be reported.
//!
//! Single test on purpose: it overrides `HOME`, which is process-global, and
//! cargo runs the tests inside one binary on threads.

use tokensave::agents::{offer_git_post_commit_hook, GitHookMode};

#[test]
fn one_unwritable_hook_is_reported_and_the_others_still_install() {
    let home = tempfile::tempdir().expect("tempdir");

    // The hooks directory itself is fine, but `post-commit` is occupied by a
    // directory, so opening it for writing fails on both Unix and Windows.
    let hooks = home.path().join(".config").join("git").join("hooks");
    std::fs::create_dir_all(hooks.join("post-commit")).expect("occupy hook path");

    std::env::set_var("HOME", home.path());
    std::env::set_var("USERPROFILE", home.path());

    let result = offer_git_post_commit_hook("/usr/bin/tokensave", GitHookMode::Yes);

    assert!(
        result.is_err(),
        "the hook that could not be written must be reported, got: {result:?}"
    );
    let message = result.unwrap_err();
    assert!(
        message.contains("post-commit"),
        "the error must name the hook that failed, got: {message}"
    );

    // The whole point of collecting failures instead of returning on the first
    // one: the other two hooks the user asked for still get installed.
    assert!(
        hooks.join("post-checkout").is_file(),
        "post-checkout must still install after post-commit failed"
    );
    assert!(
        hooks.join("post-merge").is_file(),
        "post-merge must still install after post-commit failed"
    );
}
