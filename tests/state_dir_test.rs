//! Relocatable project state (#919): `TOKENSAVE_STATE_DIR` and `state_dir`.
//!
//! Every test runs the binary with its own home, base directory and git
//! repository, so nothing here touches the developer's state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Sandbox {
    _tmp: TempDir,
    home: PathBuf,
    base: PathBuf,
    work: PathBuf,
}

fn sandbox() -> Sandbox {
    let tmp = TempDir::new().expect("temp dir");
    let home = tmp.path().join("home");
    let base = tmp.path().join("state");
    let work = tmp.path().join("work");
    for dir in [&home, &base, &work] {
        fs::create_dir_all(dir).expect("create sandbox dir");
    }
    // Pricing never refreshes, as in the other CLI tests.
    fs::create_dir_all(home.join(".tokensave")).expect("home .tokensave");
    fs::write(
        home.join(".tokensave").join("config.toml"),
        "last_pricing_fetch_at = 4102444800\n",
    )
    .expect("config.toml");
    Sandbox {
        _tmp: tmp,
        home,
        base,
        work,
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// A committed repository with one Rust file at `<work>/<name>`.
fn repo(sandbox: &Sandbox, name: &str) -> PathBuf {
    let root = sandbox.work.join(name);
    fs::create_dir_all(root.join("src")).expect("repo dir");
    fs::write(root.join("src").join("lib.rs"), "pub fn hello() {}\n").expect("source");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "-q", "-m", "init"]);
    root
}

/// `tokensave` in `sandbox`, with `state_dir` as the variable's value
/// (`None` removes it, so `config.toml` decides).
fn tokensave(sandbox: &Sandbox, state_dir: Option<&Path>, cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .args(args)
        .current_dir(cwd)
        .env("HOME", &sandbox.home)
        .env("USERPROFILE", &sandbox.home)
        .env("TOKENSAVE_UPDATE_CHECK", "off")
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .env("TOKENSAVE_AUTO_INIT", "0")
        .env("TOKENSAVE_DISABLE_SUBPROCESS", "1");
    match state_dir {
        Some(dir) => command.env("TOKENSAVE_STATE_DIR", dir),
        None => command.env_remove("TOKENSAVE_STATE_DIR"),
    };
    command.output().expect("run tokensave")
}

fn ok(output: Output, what: &str) -> String {
    assert!(
        output.status.success(),
        "{what} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn init(sandbox: &Sandbox, state_dir: Option<&Path>, root: &Path) {
    let path = root.to_string_lossy().into_owned();
    ok(
        tokensave(
            sandbox,
            state_dir,
            root,
            &["init", &path, "--no-git-hook", "--cpu-usage", "low"],
        ),
        "init",
    );
}

/// The per-project directories under `<base>/projects`.
fn project_dirs(base: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(base.join("projects"))
        .map(|read| read.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    dirs.sort();
    dirs
}

/// Everything under `root` except `.git` and the committed sources.
fn untracked_entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root)
        .expect("read root")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != ".git" && name != "src")
        .collect();
    names.sort();
    names
}

fn git_exclude(root: &Path) -> String {
    fs::read_to_string(root.join(".git").join("info").join("exclude")).unwrap_or_default()
}

#[test]
fn relocated_state_leaves_nothing_in_the_repository() {
    let sb = sandbox();
    let root = repo(&sb, "app");
    init(&sb, Some(&sb.base), &root);
    let path = root.to_string_lossy().into_owned();
    ok(
        tokensave(&sb, Some(&sb.base), &root, &["sync", &path]),
        "sync",
    );
    git(&root, &["checkout", "-q", "-b", "feature"]);
    ok(
        tokensave(&sb, Some(&sb.base), &root, &["branch", "add", "feature"]),
        "branch add",
    );
    ok(
        tokensave(&sb, Some(&sb.base), &root, &["status", &path]),
        "status",
    );

    assert_eq!(untracked_entries(&root), Vec::<String>::new());
    assert!(
        !git_exclude(&root).contains(".tokensave"),
        "init must not touch .git/info/exclude when state is relocated"
    );

    let dirs = project_dirs(&sb.base);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    let dir = &dirs[0];
    assert!(dir.join("tokensave.db").is_file());
    assert!(dir.join("config.json").is_file());
    assert!(dir.join("branch-meta.json").is_file());
    let origin: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("origin.json")).expect("origin.json"))
            .expect("origin json");
    assert_eq!(
        origin["project_root"].as_str().map(PathBuf::from),
        Some(tokensave::state_dir::canonical_root(&root))
    );

    let bases: Vec<PathBuf> = serde_json::from_str(
        &fs::read_to_string(sb.home.join(".tokensave").join("state-bases.json"))
            .expect("state-bases.json"),
    )
    .expect("bases json");
    assert_eq!(bases, vec![sb.base.clone()]);
}

#[test]
fn ids_are_stable_per_root_and_distinct_per_worktree() {
    let sb = sandbox();
    let root = repo(&sb, "app");
    let linked = sb.work.join("app-linked");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            &linked.to_string_lossy(),
        ],
    );
    init(&sb, Some(&sb.base), &root);
    init(&sb, Some(&sb.base), &linked);
    let first = project_dirs(&sb.base);
    assert_eq!(first.len(), 2, "two worktrees, two ids: {first:?}");

    // A second run for the same root finds the index under the same id.
    let path = root.to_string_lossy().into_owned();
    ok(
        tokensave(&sb, Some(&sb.base), &root, &["sync", &path]),
        "sync",
    );
    assert_eq!(project_dirs(&sb.base), first, "the same root keeps its id");
    assert_eq!(first, {
        let mut expected = vec![
            tokensave::state_dir::relocated_root(&sb.base, &root),
            tokensave::state_dir::relocated_root(&sb.base, &linked),
        ];
        expected.sort();
        expected
    });
}

#[test]
fn unset_keeps_the_in_repository_layout() {
    let sb = sandbox();
    let root = repo(&sb, "app");
    let empty = Path::new("");
    init(&sb, Some(empty), &root);
    assert!(root.join(".tokensave").join("tokensave.db").is_file());
    assert!(git_exclude(&root).contains(".tokensave/"));
    assert!(project_dirs(&sb.base).is_empty());
    assert!(!sb.home.join(".tokensave").join("state-bases.json").exists());
}

#[test]
fn existing_in_repository_state_is_kept_after_a_base_is_configured() {
    let sb = sandbox();
    let root = repo(&sb, "app");
    init(&sb, Some(Path::new("")), &root);
    let path = root.to_string_lossy().into_owned();
    ok(
        tokensave(&sb, Some(&sb.base), &root, &["sync", &path]),
        "sync",
    );
    assert!(root.join(".tokensave").join("tokensave.db").is_file());
    assert!(
        project_dirs(&sb.base).is_empty(),
        "the in-repository index stays in use, nothing is created under the base"
    );
}

#[test]
fn state_under_a_previous_base_stays_reachable() {
    let sb = sandbox();
    let root = repo(&sb, "app");
    init(&sb, Some(&sb.base), &root);
    let path = root.to_string_lossy().into_owned();

    // Another base, then no base at all: the index under the recorded base
    // is still found, and nothing new is created.
    let other = sb.work.join("other-state");
    ok(
        tokensave(&sb, Some(&other), &root, &["sync", &path]),
        "sync with another base",
    );
    assert!(!other.join("projects").exists());
    ok(
        tokensave(&sb, Some(Path::new("")), &root, &["sync", &path]),
        "sync with the default layout",
    );
    assert!(!root.join(".tokensave").exists());
    assert_eq!(project_dirs(&sb.base).len(), 1);
}

#[test]
fn the_variable_overrides_the_user_config_setting() {
    let sb = sandbox();
    let configured = sb.work.join("configured-state");
    fs::write(
        sb.home.join(".tokensave").join("config.toml"),
        format!(
            "last_pricing_fetch_at = 4102444800\nstate_dir = {}\n",
            toml::Value::String(configured.to_string_lossy().into_owned())
        ),
    )
    .expect("config.toml");

    let from_config = repo(&sb, "from-config");
    init(&sb, None, &from_config);
    assert_eq!(project_dirs(&configured).len(), 1);
    assert!(!from_config.join(".tokensave").exists());

    let from_env = repo(&sb, "from-env");
    init(&sb, Some(&sb.base), &from_env);
    assert_eq!(project_dirs(&sb.base).len(), 1);
    assert_eq!(project_dirs(&configured).len(), 1);
}

#[test]
fn a_base_inside_the_home_state_directory_keeps_global_files_apart() {
    let sb = sandbox();
    let base = sb.home.join(".tokensave");
    let root = repo(&sb, "global.db");
    init(&sb, Some(&base), &root);
    assert!(
        base.join("global.db").is_file(),
        "the global database is a file"
    );
    assert!(base.join("config.toml").is_file());
    let dirs = project_dirs(&base);
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].join("tokensave.db").is_file());
}

#[test]
fn prune_deletes_only_state_whose_project_is_gone() {
    let sb = sandbox();
    let kept = repo(&sb, "kept");
    let gone = repo(&sb, "gone");
    init(&sb, Some(&sb.base), &kept);
    init(&sb, Some(&sb.base), &gone);
    let gone_dir = tokensave::state_dir::relocated_root(&sb.base, &gone);
    let kept_dir = tokensave::state_dir::relocated_root(&sb.base, &kept);
    fs::remove_dir_all(&gone).expect("delete checkout");

    let preview = ok(
        tokensave(
            &sb,
            Some(&sb.base),
            &sb.work,
            &["state", "prune", "--dry-run"],
        ),
        "state prune --dry-run",
    );
    assert!(preview.contains("Would delete"), "{preview}");
    assert!(gone_dir.exists(), "--dry-run deletes nothing");

    let listed = ok(
        tokensave(&sb, Some(&sb.base), &sb.work, &["state", "list"]),
        "state list",
    );
    assert!(listed.contains("orphaned"), "{listed}");

    ok(
        tokensave(&sb, Some(&sb.base), &sb.work, &["state", "prune"]),
        "state prune",
    );
    assert!(!gone_dir.exists());
    assert!(kept_dir.join("tokensave.db").is_file());
}
