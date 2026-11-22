//! `tokensave purge` (#1055): planning, ownership checks, removal and CLI
//! confirmation behavior.
//!
//! Every test works in throwaway directories. The library tests pass an
//! explicit home to the planner; the CLI tests give the spawned binary its
//! own `HOME`/`USERPROFILE` and only use scopes that never load the real
//! user configuration.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;
use tokensave::purge::{self, Category, ProjectTarget, PurgeRequest};

fn touch(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, contents).expect("write");
}

/// Every path under `root`, sorted, for before/after comparisons.
fn tree(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, acc: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            acc.push(path.clone());
            if fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
                walk(&path, acc);
            }
        }
    }
    let mut acc = Vec::new();
    walk(root, &mut acc);
    acc.sort();
    acc
}

/// A home whose `~/.tokensave` holds every kind of global entry.
fn populated_home() -> TempDir {
    let home = TempDir::new().expect("home");
    let g = home.path().join(".tokensave");
    touch(&g.join("config.toml"), "installed_agents = []\n");
    touch(&g.join("config.toml.lock"), "");
    touch(&g.join("config.tmp.1234.0"), "");
    touch(&g.join("state.toml"), "");
    touch(&g.join("benchmark.toml"), "");
    touch(&g.join("pricing.json"), "{}");
    touch(&g.join("memory.mmap"), "");
    touch(&g.join("servers").join("1.json"), "{}");
    touch(&g.join("profile").join("agg.json"), "{}");
    touch(&g.join("state-bases.json"), "[]");
    touch(&g.join("global.db"), "");
    touch(&g.join("global.db-wal"), "");
    touch(&g.join("beta").join("global.db"), "");
    touch(&g.join("notes-from-user.txt"), "mine");
    home
}

fn global_request(home: &Path) -> PurgeRequest {
    PurgeRequest {
        home: home.to_path_buf(),
        global: true,
        include_history: false,
        projects: Vec::new(),
        all_projects: false,
        agents: Vec::new(),
    }
}

fn names(paths: impl Iterator<Item = PathBuf>) -> Vec<String> {
    let mut v: Vec<String> = paths
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn global_plan_removes_only_recognized_entries_and_keeps_history() {
    let home = populated_home();
    let plan = purge::plan(&global_request(home.path()));

    assert_eq!(
        names(plan.removals.iter().map(|r| r.path.clone())),
        [
            "benchmark.toml",
            "config.tmp.1234.0",
            "config.toml",
            "config.toml.lock",
            "memory.mmap",
            "pricing.json",
            "profile",
            "servers",
            "state.toml",
        ]
    );
    assert_eq!(
        names(plan.kept.iter().map(|n| n.path.clone())),
        ["beta", "global.db", "global.db-wal", "state-bases.json"]
    );
    assert_eq!(
        names(plan.skipped.iter().map(|n| n.path.clone())),
        ["notes-from-user.txt"]
    );
    assert!(plan
        .removals
        .iter()
        .all(|r| r.category != Category::UsageHistory));
    // Deleting config.toml resets the counter opt-out; the plan says so.
    assert!(plan.notes.iter().any(|n| n.contains("TOKENSAVE_TELEMETRY")));
}

#[test]
fn include_history_and_all_projects_extend_the_global_plan() {
    let home = populated_home();
    let mut request = global_request(home.path());
    request.include_history = true;
    request.all_projects = true;
    let plan = purge::plan(&request);
    let removed = names(plan.removals.iter().map(|r| r.path.clone()));
    for name in ["global.db", "global.db-wal", "state-bases.json"] {
        assert!(removed.contains(&name.to_string()), "{name} in {removed:?}");
    }
    // The beta channel's directory is still never part of a stable purge.
    assert!(!removed.contains(&"beta".to_string()));
}

#[test]
fn planning_changes_nothing() {
    let home = populated_home();
    let project = indexed_project();
    let before = (tree(home.path()), tree(project.path()));
    let mut request = global_request(home.path());
    request.include_history = true;
    request.projects.push(ProjectTarget {
        root: project.path().to_path_buf(),
        registered: false,
    });
    let plan = purge::plan(&request);
    assert!(!plan.removals.is_empty());
    assert_eq!(before, (tree(home.path()), tree(project.path())));
}

#[test]
fn execution_deletes_the_plan_and_is_idempotent() {
    let home = populated_home();
    let g = home.path().join(".tokensave");
    let request = global_request(home.path());
    let plan = purge::plan(&request);
    let report = purge::execute_removals(&plan);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(report.removed.len(), plan.removals.len());

    for kept in [
        "global.db",
        "beta",
        "notes-from-user.txt",
        "state-bases.json",
    ] {
        assert!(g.join(kept).exists(), "{kept} must survive");
    }
    assert!(!g.join("config.toml").exists());
    assert!(!g.join("servers").exists());

    // A second run finds nothing left to delete.
    let again = purge::plan(&request);
    assert!(again.removals.is_empty(), "{:?}", again.removals);
}

#[test]
fn an_emptied_global_directory_is_removed() {
    let home = TempDir::new().expect("home");
    touch(&home.path().join(".tokensave").join("config.toml"), "");
    let plan = purge::plan(&global_request(home.path()));
    purge::execute_removals(&plan);
    assert!(!home.path().join(".tokensave").exists());
}

#[test]
fn a_missing_global_directory_is_reported_absent() {
    let home = TempDir::new().expect("home");
    let plan = purge::plan(&global_request(home.path()));
    assert!(plan.removals.is_empty());
    assert_eq!(plan.absent, [home.path().join(".tokensave")]);
}

/// A project with an index, a beta index and source files.
fn indexed_project() -> TempDir {
    let project = TempDir::new().expect("project");
    touch(&project.path().join("src").join("main.rs"), "fn main() {}");
    let state = project.path().join(".tokensave");
    touch(&state.join("tokensave.db"), "db");
    touch(&state.join("okf").join("bundle.json"), "{}");
    touch(&state.join("beta").join("tokensave.db"), "beta");
    project
}

fn project_request(home: &Path, root: &Path) -> PurgeRequest {
    PurgeRequest {
        home: home.to_path_buf(),
        global: false,
        include_history: false,
        projects: vec![ProjectTarget {
            root: root.to_path_buf(),
            registered: false,
        }],
        all_projects: false,
        agents: Vec::new(),
    }
}

#[test]
fn project_purge_removes_the_index_and_keeps_source_and_beta() {
    let home = TempDir::new().expect("home");
    let project = indexed_project();
    let plan = purge::plan(&project_request(home.path(), project.path()));
    assert_eq!(plan.removals.len(), 1);
    let removal = &plan.removals[0];
    assert_eq!(removal.category, Category::ProjectIndex);
    assert_eq!(removal.keep_child, Some("beta"));
    assert_eq!(plan.project_roots.len(), 1);
    // The global scope is untouched.
    assert!(plan.global_agents.is_empty() && !plan.global_hooks);

    let report = purge::execute_removals(&plan);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    let state = project.path().join(".tokensave");
    assert!(!state.join("tokensave.db").exists());
    assert!(!state.join("okf").exists());
    assert!(state.join("beta").join("tokensave.db").exists());
    assert!(project.path().join("src").join("main.rs").exists());
}

#[test]
fn a_state_directory_without_an_index_is_not_deleted() {
    let home = TempDir::new().expect("home");
    let project = TempDir::new().expect("project");
    touch(
        &project.path().join(".tokensave").join("something-else.txt"),
        "",
    );
    let plan = purge::plan(&project_request(home.path(), project.path()));
    assert!(plan.removals.is_empty());
    assert_eq!(plan.skipped.len(), 1);
}

#[test]
fn a_project_without_state_is_absent_and_idempotent() {
    let home = TempDir::new().expect("home");
    let project = TempDir::new().expect("project");
    let plan = purge::plan(&project_request(home.path(), project.path()));
    assert!(plan.removals.is_empty());
    assert_eq!(plan.absent.len(), 1);
}

/// `--project ~` would otherwise name `~/.tokensave`, the global directory.
#[test]
fn the_home_directory_is_never_purged_as_a_project() {
    let home = populated_home();
    let plan = purge::plan(&project_request(home.path(), home.path()));
    assert!(plan.removals.is_empty(), "{:?}", plan.removals);
    assert_eq!(plan.skipped.len(), 1);
    assert!(home.path().join(".tokensave").join("config.toml").exists());
}

#[test]
fn a_registered_root_that_no_longer_exists_is_skipped_and_unregistered() {
    let home = TempDir::new().expect("home");
    let gone = home.path().join("deleted-project");
    let request = PurgeRequest {
        projects: vec![ProjectTarget {
            root: gone.clone(),
            registered: true,
        }],
        ..project_request(home.path(), &gone)
    };
    let plan = purge::plan(&request);
    assert!(plan.removals.is_empty());
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.registry_rows, [gone.to_string_lossy().into_owned()]);
}

/// Creates a directory link, or returns `false` where the platform or the
/// account cannot (unprivileged Windows without developer mode).
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
            || Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
    }
}

#[test]
fn a_linked_project_state_directory_is_not_followed() {
    let home = TempDir::new().expect("home");
    let outside = TempDir::new().expect("outside");
    touch(&outside.path().join("tokensave.db"), "precious");
    let project = TempDir::new().expect("project");
    if !link_dir(outside.path(), &project.path().join(".tokensave")) {
        eprintln!("skipping: cannot create a directory link here");
        return;
    }
    let plan = purge::plan(&project_request(home.path(), project.path()));
    assert!(plan.removals.is_empty(), "{:?}", plan.removals);
    purge::execute_removals(&plan);
    assert!(outside.path().join("tokensave.db").exists());
}

#[test]
fn a_linked_global_entry_is_not_followed() {
    let home = TempDir::new().expect("home");
    let outside = TempDir::new().expect("outside");
    touch(&outside.path().join("keep.json"), "{}");
    let g = home.path().join(".tokensave");
    fs::create_dir_all(&g).expect("mkdir");
    if !link_dir(outside.path(), &g.join("servers")) {
        eprintln!("skipping: cannot create a directory link here");
        return;
    }
    let plan = purge::plan(&global_request(home.path()));
    assert!(plan.removals.is_empty(), "{:?}", plan.removals);
    assert_eq!(plan.skipped.len(), 1);
    purge::execute_removals(&plan);
    assert!(outside.path().join("keep.json").exists());
}

#[test]
fn remove_path_refuses_a_link_and_reports_absence() {
    let tmp = TempDir::new().expect("tmp");
    let outside = TempDir::new().expect("outside");
    touch(&outside.path().join("x"), "");
    let link = tmp.path().join("link");
    if link_dir(outside.path(), &link) {
        assert!(purge::remove_path(&link, None).is_err());
        assert!(outside.path().join("x").exists());
    }
    assert!(!purge::remove_path(&tmp.path().join("missing"), None).expect("absent"));
}

#[test]
fn agents_to_clean_reads_the_registry_without_writing() {
    let home = TempDir::new().expect("home");
    let config = home.path().join(".tokensave").join("config.toml");
    let contents = "installed_agents = [\"cursor\", \"not-an-agent\", \"cursor\"]\n";
    touch(&config, contents);
    assert_eq!(purge::agents_to_clean(home.path()), ["cursor"]);
    assert_eq!(fs::read_to_string(&config).expect("read"), contents);
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

fn purge_cli(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("purge")
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .env("TOKENSAVE_TELEMETRY", "0")
        .stdin(Stdio::null())
        .output()
        .expect("run tokensave purge")
}

#[test]
fn cli_dry_run_lists_the_plan_and_changes_nothing() {
    let home = populated_home();
    let project = indexed_project();
    let before = (tree(home.path()), tree(project.path()));
    let project_arg = project.path().to_string_lossy().into_owned();
    for args in [
        vec!["--dry-run"],
        vec!["--dry-run", "--include-history"],
        vec!["--dry-run", "--project", project_arg.as_str()],
    ] {
        let out = purge_cli(home.path(), &args);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{args:?}: {out:?}");
        assert!(stdout.contains("Dry run: nothing was changed."), "{stdout}");
        assert!(stdout.contains("still installed"), "{stdout}");
    }
    assert_eq!(before, (tree(home.path()), tree(project.path())));
}

#[test]
fn cli_refuses_without_yes_when_stdin_is_not_a_terminal() {
    let home = populated_home();
    let before = tree(home.path());
    let out = purge_cli(home.path(), &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("--yes"));
    assert_eq!(before, tree(home.path()));
}

#[test]
fn cli_yes_purges_a_project_and_a_rerun_is_a_no_op() {
    let home = TempDir::new().expect("home");
    let project = indexed_project();
    let project_arg = project.path().to_string_lossy().into_owned();
    let out = purge_cli(home.path(), &["--project", &project_arg, "--yes"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{out:?}");
    assert!(stdout.contains("Purge complete."), "{stdout}");
    assert!(!project
        .path()
        .join(".tokensave")
        .join("tokensave.db")
        .exists());
    assert!(project.path().join("src").join("main.rs").exists());

    let again = purge_cli(home.path(), &["--project", &project_arg, "--yes"]);
    assert!(again.status.success(), "{again:?}");
}

#[test]
fn cli_yes_purges_global_state_and_keeps_history() {
    let home = populated_home();
    let out = purge_cli(home.path(), &["--yes"]);
    assert!(out.status.success(), "{out:?}");
    let g = home.path().join(".tokensave");
    assert!(!g.join("config.toml").exists());
    assert!(g.join("global.db").exists());
    assert!(g.join("notes-from-user.txt").exists());
}

#[test]
fn cli_rejects_conflicting_scopes() {
    let home = TempDir::new().expect("home");
    for args in [
        vec!["--local", "--all-projects"],
        vec!["--local", "--include-history"],
        vec!["--project", ".", "--all-projects"],
    ] {
        let out = purge_cli(home.path(), &args);
        assert!(!out.status.success(), "{args:?} should be rejected");
    }
}
