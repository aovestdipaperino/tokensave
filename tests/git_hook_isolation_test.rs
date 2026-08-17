use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn run_git(root: &Path, global_config: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        // The fixture must exercise a controlled global Git configuration,
        // independently of the developer's real home directory.
        .env("GIT_CONFIG_GLOBAL", global_config)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit(
    project: &Path,
    global_config: &Path,
    marker: &Path,
    message: &str,
    restore_global_hooks: bool,
) {
    let mut command = Command::new("git");
    command
        .args(["commit", "--allow-empty", "-m", message])
        .current_dir(project)
        .env("GIT_CONFIG_GLOBAL", global_config)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .env("TOKENSAVE_TEST_HOOK_MARKER", marker);
    if restore_global_hooks {
        // Cancel the command-scope core.hooksPath supplied by Cargo so the
        // controlled global hooks path applies for the positive control.
        command.env("GIT_CONFIG_COUNT", "0");
    }
    let output = command.output().expect("commit fixture");
    assert!(
        output.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cargo_test_disables_global_git_hooks_in_fixtures() {
    let sandbox = tempfile::tempdir().expect("tempdir");
    let project = sandbox.path().join("project");
    let hooks = sandbox.path().join("hooks");
    let global_config = sandbox.path().join("gitconfig");
    let marker = sandbox.path().join("hook-fired");
    fs::create_dir_all(&project).expect("create project");
    fs::create_dir_all(&hooks).expect("create hooks");

    let hook = hooks.join("post-commit");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf fired > \"$TOKENSAVE_TEST_HOOK_MARKER\"\n",
    )
    .expect("write hook");
    #[cfg(unix)]
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("chmod hook");

    let config_output = Command::new("git")
        .args(["config", "--file"])
        .arg(&global_config)
        .arg("core.hooksPath")
        .arg(&hooks)
        .output()
        .expect("write controlled global config");
    assert!(config_output.status.success());

    run_git(&project, &global_config, &["init", "-b", "master"]);

    commit(&project, &global_config, &marker, "positive control", true);
    assert!(
        marker.exists(),
        "controlled post-commit hook did not run; this test cannot verify Cargo hook isolation"
    );
    fs::remove_file(&marker).expect("remove positive-control marker");

    commit(&project, &global_config, &marker, "isolated fixture", false);

    assert!(
        !marker.exists(),
        "developer-level post-commit hook ran inside a test fixture"
    );
}
