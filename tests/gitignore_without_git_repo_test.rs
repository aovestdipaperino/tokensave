//! `.gitignore` filtering applies with or without a `.git` directory (#283).
//!
//! `scan_files_with_gitignore` registers `.gitignore` as a custom ignore
//! filename, so the `ignore` crate reads it directly instead of relying on git
//! repository discovery. The `init` warning for non-repo directories used to
//! claim the opposite; this test pins the behavior the wording now describes,
//! so the two can't drift apart again.

use std::fs;

use tempfile::TempDir;
use tokensave::tokensave::TokenSave;

#[tokio::test]
async fn gitignore_is_honored_without_a_git_repository() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    // Deliberately no `git init` — and directory names that no default
    // `exclude` glob covers, so the .gitignore is the only thing that can
    // account for the difference.
    fs::create_dir(project.join("src")).unwrap();
    fs::create_dir(project.join("zeta")).unwrap();
    fs::create_dir(project.join("omega")).unwrap();
    fs::write(project.join("src/a.rs"), "pub fn keep_me() {}\n").unwrap();
    fs::write(project.join("zeta/b.rs"), "pub fn ignored_zeta() {}\n").unwrap();
    fs::write(project.join("omega/c.rs"), "pub fn kept_omega() {}\n").unwrap();
    fs::write(project.join(".gitignore"), "zeta/\n").unwrap();

    assert!(
        !project.join(".git").exists(),
        "the fixture must not be a git repository"
    );

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let names: Vec<String> = cg
        .get_all_nodes()
        .await
        .unwrap()
        .into_iter()
        .map(|n| n.name)
        .collect();

    assert!(
        names.iter().any(|n| n == "keep_me"),
        "unignored source must be indexed: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "kept_omega"),
        "a directory outside .gitignore must be indexed: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "ignored_zeta"),
        "a .gitignore'd directory must be skipped even with no git repository: {names:?}"
    );
}
