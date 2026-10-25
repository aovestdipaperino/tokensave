//! #455: git hooks that do not force one hook directory on every repository.
//!
//! The global path works by claiming `core.hooksPath`, which is a single
//! machine-wide setting. It overrides the default of a separate `.git/hooks`
//! per checkout, so someone whose projects need different tooling cannot have
//! different hooks per project — and git stops reading each repository's own
//! hook directory entirely.
//!
//! Per-repository hooks are the git-native answer and need no global config,
//! so nothing here touches git config at all.

use std::path::Path;
use std::process::Command;
use tokensave::agents::{
    install_local_git_hooks, local_git_hooks_present, remove_local_git_hooks, repo_hooks_dir,
};

fn git(repo: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git")
        .status
        .success();
    assert!(ok, "git {args:?} failed");
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("ts455")
        .tempdir()
        .expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    dir
}

#[test]
fn hooks_land_in_the_repositorys_own_directory() {
    let dir = repo();
    let out = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    // Sorted before comparing: which hook is written first is incidental
    // (post-checkout is handled ahead of the others since #342 Q1, because it
    // is the one carrying a versioned fence), and pinning that order would
    // assert an implementation detail rather than the outcome.
    let mut installed = out.installed.clone();
    installed.sort();
    assert_eq!(
        installed,
        vec!["post-checkout", "post-commit", "post-merge"]
    );
    assert_eq!(out.hooks_dir, dir.path().join(".git").join("hooks"));
    assert!(local_git_hooks_present(dir.path()));
}

/// The whole point of the issue: no machine-wide setting is claimed, so other
/// repositories keep whatever hooks they had.
#[test]
fn installing_never_writes_git_config() {
    let dir = repo();
    let before = std::fs::read_to_string(dir.path().join(".git").join("config")).expect("config");
    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    let after = std::fs::read_to_string(dir.path().join(".git").join("config")).expect("config");

    assert_eq!(
        before, after,
        "the repository's git config must be untouched"
    );
    assert!(
        !after.contains("hooksPath"),
        "local hooks must never claim core.hooksPath"
    );
}

/// A repository with husky, pre-commit, or a hand-written hook keeps it.
#[test]
fn an_existing_hook_keeps_its_content_and_gains_a_section() {
    let dir = repo();
    let hook = dir.path().join(".git").join("hooks").join("post-commit");
    std::fs::write(&hook, "#!/bin/sh\necho mine\n").expect("write hook");

    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    let contents = std::fs::read_to_string(&hook).expect("read");

    assert!(contents.contains("echo mine"), "got: {contents}");
    assert!(contents.contains("tokensave"), "got: {contents}");
}

/// Removal is the same conservative rule as the global path: keep anything
/// tokensave did not write, delete a file that is nothing but ours.
#[test]
fn removal_keeps_foreign_content_and_deletes_a_pure_tokensave_hook() {
    let dir = repo();
    let hooks = dir.path().join(".git").join("hooks");
    std::fs::write(hooks.join("post-commit"), "#!/bin/sh\necho mine\n").expect("write hook");
    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    remove_local_git_hooks(dir.path());

    let kept = std::fs::read_to_string(hooks.join("post-commit")).expect("post-commit survives");
    assert!(kept.contains("echo mine"));
    assert!(!kept.contains("tokensave"));
    assert!(
        !hooks.join("post-merge").exists(),
        "a hook holding only tokensave's section is deleted outright"
    );
    assert!(!local_git_hooks_present(dir.path()));
}

#[test]
fn installing_twice_reports_the_second_run_as_already_present() {
    let dir = repo();
    install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    let second = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    assert!(second.installed.is_empty());
    let mut already = second.already_present.clone();
    already.sort();
    assert_eq!(already, vec!["post-checkout", "post-commit", "post-merge"]);
    assert!(
        second.migrated.is_empty(),
        "an unchanged block must be reported as already-present, not rewritten"
    );
}

/// Linked worktrees share one hook directory with the main checkout, so
/// resolving from `--git-dir` would write to a per-worktree directory git
/// never reads.
#[test]
fn a_worktree_resolves_to_the_shared_hook_directory() {
    let dir = repo();
    std::fs::write(dir.path().join("f.txt"), "x").expect("write");
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "init",
        ],
    );
    let wt = dir.path().join("wt");
    git(
        dir.path(),
        &["worktree", "add", "-q", &wt.to_string_lossy(), "-b", "side"],
    );

    let from_main = repo_hooks_dir(dir.path()).expect("main hooks dir");
    let from_worktree = repo_hooks_dir(&wt).expect("worktree hooks dir");
    assert_eq!(
        from_main.canonicalize().ok(),
        from_worktree.canonicalize().ok(),
        "a worktree must resolve to the checkout's shared hook directory"
    );
}

#[test]
fn a_directory_that_is_not_a_repository_is_an_error_not_a_silent_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(install_local_git_hooks(dir.path(), "/usr/bin/tokensave").is_err());
    assert!(!local_git_hooks_present(dir.path()));
}

#[test]
fn a_hook_that_cannot_be_written_is_reported_as_failed() {
    let dir = repo();
    let hooks = repo_hooks_dir(dir.path()).expect("hooks dir");
    // A directory where the hook file belongs: the path exists, so the append
    // branch is taken, and opening a directory for writing fails on both Unix
    // and Windows.
    std::fs::create_dir_all(hooks.join("post-commit")).expect("occupy hook path");

    let out = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");

    assert_eq!(
        out.failed,
        vec!["post-commit"],
        "a hook that could not be written must be reported, not silently dropped"
    );
    assert!(
        !out.installed.contains(&"post-commit".to_string()),
        "a failed hook must not be listed as installed, got: {:?}",
        out.installed
    );
    assert_eq!(
        out.installed,
        vec!["post-checkout", "post-merge"],
        "the other two hooks must still install"
    );
}

/// #342 Q1: an install that already carries tokensave's block gets that block
/// rewritten in place, instead of being skipped forever.
///
/// The append-only writer this replaces meant a change to the hook body never
/// reached anyone who had already installed — so a runtime knob added to the
/// snippet was simply false for them. It also meant `uninstall` could not
/// honestly undo what `install` did to a hook carrying the user's own code.
#[test]
fn a_stale_block_is_rewritten_and_foreign_content_survives() {
    let dir = repo();
    let hooks = dir.path().join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();

    let before = "#!/bin/sh\n\
                  ./scripts/mine.sh \"$@\"\n\
                  # tokensave: auto-init\n\
                  if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
                  \ttokensave init >/dev/null 2>&1 &\n\
                  fi\n\
                  # tokensave: end auto-init\n\
                  ./scripts/after.sh\n";
    std::fs::write(hooks.join("post-checkout"), before).unwrap();

    let out = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    assert_eq!(out.migrated, vec!["post-checkout"]);

    let after = std::fs::read_to_string(hooks.join("post-checkout")).unwrap();
    assert!(after.starts_with("#!/bin/sh\n./scripts/mine.sh \"$@\"\n"));
    assert!(after.ends_with("./scripts/after.sh\n"));
    assert!(after.contains("hook post-checkout"));
    assert!(
        !after.contains("0000000000000000000000000000000000000000"),
        "the old body must be replaced, not accumulated"
    );

    // Running again is a no-op, so a reinstall does not churn the file.
    let second = install_local_git_hooks(dir.path(), "/usr/bin/tokensave").expect("install");
    assert!(second.migrated.is_empty());
    assert_eq!(
        std::fs::read_to_string(hooks.join("post-checkout")).unwrap(),
        after
    );
}
