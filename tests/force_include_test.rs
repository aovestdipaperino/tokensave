//! `force_include` indexes a gitignored sub-path without un-ignoring anything
//! else (#571).
//!
//! `include` only re-admits hidden paths, and it is consulted after the
//! `ignore` crate has already dropped every gitignored entry, so a glob naming
//! a gitignored directory matched nothing. `force_include` is the separate,
//! scoped switch: the listed globs beat `.gitignore`, the rest of the ignore
//! rules keep applying, and `exclude` still wins.

use std::fs;

use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::tokensave::TokenSave;

async fn indexed_names(project: &std::path::Path, force_include: &[&str]) -> Vec<String> {
    TokenSave::init(project).await.unwrap();
    let mut config = load_config(project).unwrap();
    config.force_include = force_include.iter().map(ToString::to_string).collect();
    config.exclude.push("docs/private/**".to_string());
    save_config(project, &config).unwrap();
    let cg = TokenSave::open(project).await.unwrap();
    cg.index_all().await.unwrap();
    cg.get_all_nodes()
        .await
        .unwrap()
        .into_iter()
        .map(|n| n.name)
        .collect()
}

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    // A nested repo's .gitignore, as in the reported multi-repo workspace.
    fs::create_dir_all(project.join("repo/generated/api")).unwrap();
    fs::create_dir_all(project.join("repo/scratch")).unwrap();
    fs::create_dir_all(project.join("repo/src")).unwrap();
    fs::create_dir_all(project.join("docs/private")).unwrap();
    fs::write(project.join("repo/.gitignore"), "generated/\nscratch/\n").unwrap();
    fs::write(project.join("repo/src/lib.rs"), "pub fn tracked_fn() {}\n").unwrap();
    fs::write(
        project.join("repo/generated/api/client.rs"),
        "pub fn forced_fn() {}\n",
    )
    .unwrap();
    fs::write(
        project.join("repo/scratch/tmp.rs"),
        "pub fn still_ignored_fn() {}\n",
    )
    .unwrap();
    // Gitignored and force-included, but excluded: exclude wins.
    fs::write(project.join(".gitignore"), "docs/\n").unwrap();
    fs::write(
        project.join("docs/private/secret.rs"),
        "pub fn excluded_fn() {}\n",
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn force_include_indexes_only_the_listed_gitignored_paths() {
    let dir = fixture();
    let names = indexed_names(dir.path(), &["repo/generated/**", "docs/**"]).await;

    assert!(names.iter().any(|n| n == "tracked_fn"), "{names:?}");
    assert!(
        names.iter().any(|n| n == "forced_fn"),
        "a force_include glob must beat .gitignore: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "still_ignored_fn"),
        "a gitignored path no glob names must stay ignored: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "excluded_fn"),
        "exclude must still win over force_include: {names:?}"
    );
}

#[tokio::test]
async fn without_force_include_gitignored_paths_stay_out() {
    let dir = fixture();
    let names = indexed_names(dir.path(), &[]).await;

    assert!(names.iter().any(|n| n == "tracked_fn"), "{names:?}");
    assert!(!names.iter().any(|n| n == "forced_fn"), "{names:?}");
}
