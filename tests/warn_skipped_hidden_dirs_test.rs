use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tokensave::config::TokenSaveConfig;
use tokensave::tokensave::detect_skipped_hidden_dirs;

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

fn setup_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);

    let scripts = root.join(".github").join("scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(scripts.join("deploy.py"), "print('deploy')\n").unwrap();
    // Untracked hidden file: must not be counted.
    std::fs::write(scripts.join("untracked.py"), "print('untracked')\n").unwrap();
    git(root, &["add", ".github/scripts/deploy.py"]);
    dir
}

#[test]
fn warns_for_tracked_hidden_file_and_ignores_untracked() {
    let dir = setup_repo();
    let config = TokenSaveConfig::default();

    let warn = detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS)
        .expect("expected warning for git-tracked file under .github/");
    assert!(
        warn.contains("skipped 1 tracked file in hidden directories (.github: 1)"),
        "warning should count only the tracked file: {warn}"
    );
    assert!(
        warn.contains("\".github\" and \".github/**\""),
        "warning should suggest both include entries: {warn}"
    );
}

#[test]
fn glob_only_include_does_not_silence_warning() {
    // The walker prunes on the bare directory entry, so `.github/**` alone
    // does not re-enable descent — the warning must persist to flag the trap.
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.include.push(".github/**".to_string());

    assert!(
        detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS).is_some(),
        "include of only \".github/**\" must not suppress the warning"
    );
}

#[test]
fn full_include_suppresses_warning() {
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.include.push(".github".to_string());
    config.include.push(".github/**".to_string());

    assert!(
        detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS).is_none(),
        "warning should be suppressed when the directory is included"
    );
}

#[test]
fn excluded_dir_suppresses_warning() {
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.exclude.push(".github/**".to_string());

    assert!(
        detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS).is_none(),
        "explicitly excluded dirs are a deliberate opt-out, not a trap"
    );
}

#[test]
fn unsupported_extensions_are_not_counted() {
    let dir = setup_repo();
    let root = dir.path();
    let workflows = root.join(".github").join("workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    std::fs::write(workflows.join("ci.yml"), "on: push\n").unwrap();
    git(root, &["add", ".github/workflows/ci.yml"]);

    let config = TokenSaveConfig::default();
    let warn = detect_skipped_hidden_dirs(root, &config, None, EXTS).unwrap();
    assert!(
        warn.contains("(.github: 1)"),
        "the tracked .yml must not be counted alongside the .py: {warn}"
    );
}

#[test]
fn nested_hidden_dir_is_reported_with_full_prefix() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    let hidden = root.join("src").join(".hidden");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(hidden.join("a.py"), "x = 1\n").unwrap();
    git(root, &["add", "src/.hidden/a.py"]);

    let config = TokenSaveConfig::default();
    let warn = detect_skipped_hidden_dirs(root, &config, None, EXTS).unwrap();
    assert!(
        warn.contains("(src/.hidden: 1)") && warn.contains("\"src/.hidden\""),
        "nested hidden dirs should be reported with the exact prefix to include: {warn}"
    );
}

#[test]
fn missing_on_disk_tracked_file_is_not_counted() {
    // Sparse checkouts and staged deletions keep index entries for files the
    // walker never would have seen; they must not trigger the warning.
    let dir = setup_repo();
    std::fs::remove_file(dir.path().join(".github/scripts/deploy.py")).unwrap();
    let config = TokenSaveConfig::default();
    assert!(
        detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS).is_none(),
        "index entries missing from disk must not warn"
    );
}

#[test]
fn file_level_exclude_suppresses_warning() {
    let dir = setup_repo();
    let mut config = TokenSaveConfig::default();
    config.exclude.push("**/deploy.py".to_string());
    assert!(
        detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS).is_none(),
        "file-level excludes are a deliberate opt-out"
    );
}

#[test]
fn non_git_directory_returns_none() {
    let dir = TempDir::new().unwrap();
    let config = TokenSaveConfig::default();
    assert!(detect_skipped_hidden_dirs(dir.path(), &config, None, EXTS).is_none());
}
