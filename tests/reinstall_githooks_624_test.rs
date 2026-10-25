//! #624: `tokensave reinstall`, and the silent resync after an upgrade, must
//! refresh tokensave's section of git hooks that are already installed, and
//! must not install hooks anywhere they are not.
//!
//! The hook migrations (#342 Q1 fenced post-checkout block, the 7.13.0
//! `--git-common-dir` chain preamble) only ran from `githooks on` and `init`,
//! so after an upgrade `reinstall` left the hooks on the old shape even though
//! `doctor` names `reinstall` as the fix. The post-commit and post-merge lines
//! were never rewritten at all, so a moved binary left them pointing at a path
//! that no longer exists.
//!
//! These run the binary against a throwaway home, passed only to the child
//! process, so nothing process-global is touched. They run on Windows too,
//! which is where #624 was reported: hook paths are written to `.gitconfig`
//! as `~/...` so no backslash ever has to survive gitconfig quoting, and the
//! binary path is compared in the forward-slash form the hooks use.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ZERO_SHA: &str = "0000000000000000000000000000000000000000";
const OLD_BIN: &str = "/old/place/tokensave";

/// A post-checkout file as an older release left it: the user's own line, a
/// v1 fenced tokensave block, and more of the user's content after it.
fn stale_post_checkout() -> String {
    format!(
        "#!/bin/sh\n\
         ./scripts/mine.sh \"$@\"\n\
         # tokensave: auto-init\n\
         if [ \"$1\" = \"{ZERO_SHA}\" ]; then\n\
         \ttokensave init >/dev/null 2>&1 &\n\
         fi\n\
         # tokensave: end auto-init\n\
         ./scripts/after.sh\n"
    )
}

/// A post-commit or post-merge file as every release before the fence wrote
/// it, pointing at a binary that has since moved.
fn legacy_sync_hook(marker: &str) -> String {
    format!(
        "#!/bin/sh\n\
         ./scripts/mine.sh \"$@\"\n\
         \n\
         {marker}\n\
         {OLD_BIN} sync >/dev/null 2>&1 &\n"
    )
}

/// The path the hooks should now name: the binary under test, with the
/// forward slashes the snippets write on every platform.
fn current_bin() -> String {
    env!("CARGO_BIN_EXE_tokensave").replace('\\', "/")
}

fn with_home(command: &mut Command, home: &Path) {
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_CONFIG_NOSYSTEM", "1");
}

fn git(dir: &Path, home: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    with_home(&mut command, home);
    let ok = command.output().expect("run git").status.success();
    assert!(ok, "git {args:?} failed");
}

fn run_tokensave(cwd: &Path, home: &Path, args: &[&str], skip_maintenance: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command.args(args).current_dir(cwd);
    with_home(&mut command, home);
    if skip_maintenance {
        command.env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1");
    } else {
        command.env_remove("TOKENSAVE_SKIP_AGENT_MAINTENANCE");
    }
    let output = command.output().expect("run tokensave");
    assert!(
        output.status.success(),
        "tokensave {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn reinstall(cwd: &Path, home: &Path) -> Output {
    run_tokensave(cwd, home, &["reinstall"], true)
}

/// A home whose global hooks tokensave installed with an older release:
/// `core.hooksPath` claimed, the 7.12 chain preamble, a v1 post-checkout
/// block, and post-commit/post-merge lines naming a moved binary.
fn home_with_stale_global_hooks() -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("temp home");
    let hooks = home.path().join(".config").join("git").join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(
        home.path().join(".gitconfig"),
        "[core]\n\thooksPath = ~/.config/git/hooks\n",
    )
    .unwrap();

    // The 7.12 chain preamble resolved the repo hook via `--git-dir` and so
    // missed it from linked worktrees.
    let post_commit = format!(
        "#!/bin/sh\n\
         # tokensave: chain-repo-hook\n\
         repo_hook=\"$(git rev-parse --git-dir 2>/dev/null)/hooks/post-commit\"\n\
         if [ -x \"$repo_hook\" ] && [ \"$repo_hook\" != \"$0\" ]; then\n\
         \t\"$repo_hook\" \"$@\"\n\
         fi\n\
         \n\
         # tokensave: auto-sync\n\
         {OLD_BIN} sync >/dev/null 2>&1 &\n"
    );
    fs::write(hooks.join("post-commit"), post_commit).unwrap();
    fs::write(hooks.join("post-checkout"), stale_post_checkout()).unwrap();
    fs::write(
        hooks.join("post-merge"),
        legacy_sync_hook("# tokensave: auto-sync (post-merge)"),
    )
    .unwrap();
    (home, hooks)
}

fn assert_checkout_refreshed(path: &Path) {
    let after = fs::read_to_string(path).expect("read post-checkout");
    assert!(
        after.contains("hook post-checkout"),
        "the stale tokensave block in {} must be rewritten:\n{after}",
        path.display()
    );
    assert!(
        !after.contains(ZERO_SHA),
        "the old body must be replaced, not kept beside the new one:\n{after}"
    );
    assert!(
        after.starts_with("#!/bin/sh\n./scripts/mine.sh \"$@\"\n")
            && after.ends_with("./scripts/after.sh\n"),
        "content outside tokensave's markers must survive:\n{after}"
    );
}

fn assert_sync_repointed(path: &Path, end_marker: &str) {
    let after = fs::read_to_string(path).expect("read sync hook");
    assert!(
        !after.contains(OLD_BIN),
        "the moved binary's path must be gone from {}:\n{after}",
        path.display()
    );
    assert!(
        after.contains(&format!("'{}' sync >/dev/null 2>&1 &", current_bin())),
        "{} must name the current binary:\n{after}",
        path.display()
    );
    assert!(
        after.contains(end_marker),
        "{} must now carry tokensave's end marker:\n{after}",
        path.display()
    );
    assert_eq!(
        after.matches(" sync >/dev/null").count(),
        1,
        "the line must be replaced, not duplicated:\n{after}"
    );
}

fn assert_global_hooks_refreshed(hooks: &Path) {
    let commit_after = fs::read_to_string(hooks.join("post-commit")).unwrap();
    assert!(
        commit_after.contains("git rev-parse --git-common-dir")
            && !commit_after.contains("git rev-parse --git-dir"),
        "the chain preamble must be migrated:\n{commit_after}"
    );
    assert_sync_repointed(&hooks.join("post-commit"), "# tokensave: end auto-sync");
    assert_sync_repointed(
        &hooks.join("post-merge"),
        "# tokensave: end auto-sync (post-merge)",
    );
    let merge_after = fs::read_to_string(hooks.join("post-merge")).unwrap();
    assert!(
        merge_after.starts_with("#!/bin/sh\n./scripts/mine.sh \"$@\"\n"),
        "content outside tokensave's markers must survive:\n{merge_after}"
    );
    assert_checkout_refreshed(&hooks.join("post-checkout"));
}

#[test]
fn reinstall_refreshes_existing_global_hooks() {
    let (home, hooks) = home_with_stale_global_hooks();
    let cwd = tempfile::tempdir().expect("temp cwd");

    reinstall(cwd.path(), home.path());

    assert_global_hooks_refreshed(&hooks);
}

#[test]
fn reinstall_refreshes_the_current_repositorys_hooks() {
    let home = tempfile::tempdir().expect("temp home");
    let repo = tempfile::tempdir().expect("temp repo");
    git(repo.path(), home.path(), &["init", "-q"]);
    let hooks = repo.path().join(".git").join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(hooks.join("post-checkout"), stale_post_checkout()).unwrap();
    fs::write(
        hooks.join("post-commit"),
        legacy_sync_hook("# tokensave: auto-sync"),
    )
    .unwrap();
    fs::write(
        hooks.join("post-merge"),
        legacy_sync_hook("# tokensave: auto-sync (post-merge)"),
    )
    .unwrap();

    reinstall(repo.path(), home.path());

    assert_checkout_refreshed(&hooks.join("post-checkout"));
    assert_sync_repointed(&hooks.join("post-commit"), "# tokensave: end auto-sync");
    assert_sync_repointed(
        &hooks.join("post-merge"),
        "# tokensave: end auto-sync (post-merge)",
    );
}

#[test]
fn reinstall_does_not_install_hooks_nobody_opted_into() {
    let home = tempfile::tempdir().expect("temp home");
    let repo = tempfile::tempdir().expect("temp repo");
    git(repo.path(), home.path(), &["init", "-q"]);
    let local_hooks = repo.path().join(".git").join("hooks");
    let global_hooks = home.path().join(".config").join("git").join("hooks");

    reinstall(repo.path(), home.path());

    for name in ["post-commit", "post-checkout", "post-merge"] {
        assert!(
            !local_hooks.join(name).exists(),
            "reinstall must not install a local {name} hook"
        );
        assert!(
            !global_hooks.join(name).exists(),
            "reinstall must not install a global {name} hook"
        );
    }
    let gitconfig = fs::read_to_string(home.path().join(".gitconfig")).unwrap_or_default();
    assert!(
        !gitconfig.contains("hooksPath"),
        "reinstall must not claim core.hooksPath:\n{gitconfig}"
    );
}

/// The silent resync after an upgrade refreshes hooks too, so a user who
/// never runs `reinstall` still gets the new shape.
///
/// Unix only, for the reason given in `agent_maintenance_env_test`: on
/// Windows the version marker that gates the resync goes through
/// `dirs::home_dir`, which ignores `HOME` and `USERPROFILE`, so the resync
/// would be judged against the real profile and could no-op.
#[cfg(unix)]
#[test]
fn the_upgrade_resync_refreshes_existing_hooks() {
    let (home, global_hooks) = home_with_stale_global_hooks();
    let repo = tempfile::tempdir().expect("temp repo");
    git(repo.path(), home.path(), &["init", "-q"]);
    let local_hooks = repo.path().join(".git").join("hooks");
    fs::create_dir_all(&local_hooks).unwrap();
    fs::write(local_hooks.join("post-checkout"), stale_post_checkout()).unwrap();

    // A fresh home has no version marker, which the resync reads as an
    // upgrade. `githooks` with no action only reports, so any write here is
    // the resync's.
    run_tokensave(repo.path(), home.path(), &["githooks"], false);

    assert_global_hooks_refreshed(&global_hooks);
    assert_checkout_refreshed(&local_hooks.join("post-checkout"));
}

/// The resync stays a refresh: a machine with no tokensave hooks gets none.
#[cfg(unix)]
#[test]
fn the_upgrade_resync_does_not_install_hooks() {
    let home = tempfile::tempdir().expect("temp home");
    let repo = tempfile::tempdir().expect("temp repo");
    git(repo.path(), home.path(), &["init", "-q"]);

    run_tokensave(repo.path(), home.path(), &["githooks"], false);

    for dir in [
        repo.path().join(".git").join("hooks"),
        home.path().join(".config").join("git").join("hooks"),
    ] {
        for name in ["post-commit", "post-checkout", "post-merge"] {
            assert!(
                !dir.join(name).exists(),
                "the resync must not install {}",
                dir.join(name).display()
            );
        }
    }
}
