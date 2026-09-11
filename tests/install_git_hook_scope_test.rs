//! #506: `install --git-hook` must not claim the machine-wide hooks slot.
//!
//! `core.hooksPath` is a single global setting, and taking it takes it from
//! every other tool that installs hooks. `git lfs install --local` writes its
//! hooks into `core.hooksPath` when one is set, finds tokensave's `pre-push`
//! forwarder already there and refuses with exit 2 — in **every** repository
//! on the machine, including a brand-new empty one with no LFS hook anywhere
//! near it. husky, pre-commit and lefthook collide the same way.
//!
//! Worse, git-lfs's own advice is `git lfs update --force`, which overwrites
//! the hook *in the global directory*, removing tokensave's chain-and-sync for
//! every repository rather than the one the user was standing in.
//!
//! The per-repository mechanism already existed (#455) and `init` already used
//! it. These tests pin the behaviour so `install` cannot drift back.

use std::path::Path;
use std::process::Command;

fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git")
}

/// Run git with `core.hooksPath` claimed, the way the old global install
/// claimed it.
///
/// It has to go through `GIT_CONFIG_*` rather than `git config --local`,
/// because `.cargo/config.toml` sets exactly those variables to force
/// `core.hooksPath` empty for the whole test suite — deliberate isolation from
/// the developer's own global hooks (#381). That env setting **overrides**
/// local config, so a test that sets the path with `git config --local` claims
/// nothing, `git lfs install --local` succeeds, and the test passes while
/// proving the opposite of what it claims.
fn git_with_hooks_path(repo: &Path, hooks: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_0", hooks)
        .output()
        .expect("run git")
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("tokensave_hookscope_")
        .tempdir()
        .expect("temp dir");
    git(dir.path(), &["init", "-q", "."]);
    dir
}

/// The reporter's exact scenario, reproduced against git-lfs itself: with a
/// hooks directory claimed globally and a `pre-push` in it, `git lfs install
/// --local` fails in an unrelated fresh repository. This is the behaviour
/// `install` must no longer create. Skipped where git-lfs is absent.
#[test]
fn git_lfs_install_local_fails_while_a_global_hookspath_is_claimed() {
    if Command::new("git")
        .args(["lfs", "version"])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("git-lfs not installed; skipping the collision reproduction");
        return;
    }

    let fake_hooks = tempfile::Builder::new()
        .prefix("tokensave_fakehooks_")
        .tempdir()
        .expect("temp dir");
    // The forwarder tokensave actually writes, body included. A shebang-plus-
    // comment stub is not enough to reproduce this: git-lfs ignores a hook with
    // no real content, so a minimal fixture passes and proves nothing.
    let pre_push = fake_hooks.path().join("pre-push");
    std::fs::write(
        &pre_push,
        "#!/bin/sh\n\
         # tokensave: chain-repo-hook\n\
         repo_hook=\"$(git rev-parse --git-dir 2>/dev/null)/hooks/pre-push\"\n\
         if [ -x \"$repo_hook\" ] && [ \"$repo_hook\" != \"$0\" ]; then\n\
         \t\"$repo_hook\" \"$@\"\n\
         fi\n",
    )
    .expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&pre_push, std::fs::Permissions::from_mode(0o755))
            .expect("chmod hook");
    }

    // Baseline: in a fresh repository with no hooksPath, git-lfs installs
    // cleanly. A separate repository from the one below, because a successful
    // install writes LFS's own hooks and the second call would then be
    // answering about those rather than about the claimed slot.
    let clean_repo = repo();
    let clean = git(clean_repo.path(), &["lfs", "install", "--local"]);
    assert!(
        clean.status.success(),
        "baseline `git lfs install --local` should succeed: {}",
        String::from_utf8_lossy(&clean.stderr)
    );

    // Now the same thing in an equally fresh repository, with the slot claimed
    // the way the old global install claimed it. Scoped to this repository's
    // own config so the test never touches the user's machine.
    let work = repo();
    let blocked = git_with_hooks_path(
        work.path(),
        fake_hooks.path(),
        &["lfs", "install", "--local"],
    );
    assert!(
        !blocked.status.success(),
        "the collision this issue is about did not reproduce — \
         `git lfs install --local` unexpectedly succeeded with a claimed hooksPath: {}",
        String::from_utf8_lossy(&blocked.stdout)
    );
}

// The mode routing itself — `yes`/`default` declining the global path and
// `global` taking it — is a pure decision and is unit-tested next to the
// function in `src/agents/hooks.rs`. What only an integration test can show is
// the collision above, against the real `git lfs`.
