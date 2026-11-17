//! Tracked source dropped by an ambiguous default exclude glob (#949).

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tokensave::config::{TokenSaveConfig, AMBIGUOUS_DEFAULT_EXCLUDES};
use tokensave::tokensave::detect_tracked_ambiguous_excludes;

const EXTS: &[&str] = &["py"];

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("git {args:?} failed to spawn: {e}"));
    assert!(status.success(), "git {args:?} failed");
}

/// Writes `rel` (forward-slashed) under `root`, creating its parents.
fn write(root: &Path, rel: &str, contents: &str) {
    let path = rel
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    let parent = path
        .parent()
        .expect("a written file has a parent directory");
    std::fs::create_dir_all(parent).expect("create parent directories");
    std::fs::write(&path, contents).expect("write file");
}

/// The issue's reproduction: a tracked Python package named `build/`, plus an
/// untracked file beside it that must not be counted.
fn setup_repo() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, "build/__init__.py", "");
    write(root, "build/probe.py", "def probe_fn():\n    return 1\n");
    write(root, "build/untracked.py", "x = 1\n");
    git(root, &["add", "build/__init__.py", "build/probe.py"]);
    dir
}

fn detect(root: &Path, config: &TokenSaveConfig) -> Option<String> {
    detect_tracked_ambiguous_excludes(root, config, EXTS)
}

#[test]
fn ambiguous_globs_are_default_excludes() {
    let defaults = TokenSaveConfig::default().exclude;
    for pattern in AMBIGUOUS_DEFAULT_EXCLUDES {
        assert!(
            defaults.iter().any(|d| d == pattern),
            "{pattern} is listed as ambiguous but is not a default exclude"
        );
    }
}

#[test]
fn warns_for_tracked_source_under_build() {
    let dir = setup_repo();
    let warn = detect(dir.path(), &TokenSaveConfig::default())
        .expect("expected a warning for tracked files under build/");
    assert!(
        warn.contains("excluded 2 tracked files by default build-output excludes (build: 2)"),
        "warning should count only the tracked files: {warn}"
    );
    assert!(
        warn.contains("remove \"**/build/**\" from exclude[]"),
        "warning should name the pattern to remove: {warn}"
    );
    assert!(
        warn.contains("add \"build/**\" to exclude[]"),
        "warning should name an explicit glob that silences it: {warn}"
    );
}

#[test]
fn nested_dirs_are_reported_at_the_pruned_directory() {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, "tools/out/gen.py", "x = 1\n");
    write(root, "tools/out/deeper/more.py", "x = 2\n");
    git(root, &["add", "-A"]);

    let warn = detect(root, &TokenSaveConfig::default()).expect("expected a warning");
    assert!(
        warn.contains("(tools/out: 2)") && warn.contains("\"**/out/**\""),
        "files are grouped at the directory the walker prunes: {warn}"
    );
}

#[test]
fn bin_suggests_a_silencing_glob_that_is_not_itself_ambiguous() {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, "bin/cli.py", "x = 1\n");
    git(root, &["add", "-A"]);

    let warn = detect(root, &TokenSaveConfig::default()).expect("expected a warning");
    assert!(warn.contains("add \"bin/**/*\" to exclude[]"), "{warn}");

    let mut config = TokenSaveConfig::default();
    config.exclude.push("bin/**/*".to_string());
    assert!(
        detect(root, &config).is_none(),
        "the suggested glob must silence the warning"
    );
}

#[test]
fn explicit_exclude_is_a_deliberate_opt_out() {
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.exclude.push("build/**".to_string());
    assert!(detect(dir.path(), &config).is_none());
}

#[test]
fn file_level_exclude_is_a_deliberate_opt_out() {
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.exclude.push("**/*.py".to_string());
    assert!(detect(dir.path(), &config).is_none());
}

#[test]
fn removing_the_default_silences_the_warning() {
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.exclude.retain(|pattern| pattern != "**/build/**");
    assert!(detect(dir.path(), &config).is_none());
}

#[test]
fn other_defaults_are_not_reported() {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, "vendor/lib/mod.py", "x = 1\n");
    write(root, "node_modules/pkg/index.py", "x = 1\n");
    write(root, "vendor/build/gen.py", "x = 1\n");
    git(root, &["add", "-A"]);
    assert!(
        detect(root, &TokenSaveConfig::default()).is_none(),
        "vendor/ and node_modules/ are deliberate defaults, and a build/ \
         inside them is pruned before the ambiguous glob applies"
    );
}

#[test]
fn hidden_dirs_are_left_to_the_hidden_dir_warning() {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    write(root, ".tools/build/gen.py", "x = 1\n");
    git(root, &["add", "-A"]);
    assert!(detect(root, &TokenSaveConfig::default()).is_none());
}

#[test]
fn unsupported_extensions_are_not_counted() {
    let dir = setup_repo();
    let root = dir.path();
    write(root, "build/notes.txt", "notes\n");
    git(root, &["add", "build/notes.txt"]);
    let warn = detect(root, &TokenSaveConfig::default()).expect("expected a warning");
    assert!(warn.contains("(build: 2)"), "{warn}");
}

#[test]
fn missing_on_disk_tracked_file_is_not_counted() {
    let dir = setup_repo();
    let build = dir.path().join("build");
    std::fs::remove_file(build.join("probe.py")).expect("remove probe.py");
    std::fs::remove_file(build.join("__init__.py")).expect("remove __init__.py");
    assert!(detect(dir.path(), &TokenSaveConfig::default()).is_none());
}

#[test]
fn non_git_directory_returns_none() {
    let dir = TempDir::new().expect("create temp dir");
    write(dir.path(), "build/probe.py", "x = 1\n");
    assert!(detect(dir.path(), &TokenSaveConfig::default()).is_none());
}
