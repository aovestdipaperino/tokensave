//! A linked worktree inside a checkout is not part of that checkout's index
//! (#865).
//!
//! `git worktree add nested -b side`, run inside a repository, puts a second
//! working tree of the same repository below the first. The scan used to
//! enter it, so the index of the main checkout held each file twice, and it
//! held files that exist only on the branch of the worktree. Git does not
//! treat such a directory as content of the main checkout, and the scan must
//! not either. The scan also prunes nested clones with a .git directory.

use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::config::{load_config, save_config};
use tokensave::tokensave::TokenSave;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repository with one commit and a linked worktree at `worktree` below it,
/// on branch `side`, with one file that exists only there.
fn checkout_with_nested_worktree(worktree: &[&str]) -> TempDir {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    std::fs::write(root.join("base.rs"), "pub fn base() {}\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "base"]);
    let mut nested = root.to_path_buf();
    for part in worktree {
        nested.push(part);
    }
    git(
        root,
        &["worktree", "add", "-b", "side", nested.to_str().unwrap()],
    );
    std::fs::write(nested.join("side_only.rs"), "pub fn side_only() {}\n").unwrap();
    tmp
}

async fn indexed_files(cg: &TokenSave) -> Vec<String> {
    let mut files: Vec<String> = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    files.sort();
    files
}

#[tokio::test]
async fn a_linked_worktree_inside_the_checkout_is_not_indexed() {
    let tmp = checkout_with_nested_worktree(&["nested"]);
    let root = tmp.path();
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(indexed_files(&cg).await, vec!["base.rs".to_string()]);

    // A later change in the worktree is not a change of this checkout.
    std::fs::write(root.join("nested").join("later.rs"), "pub fn later() {}\n").unwrap();
    assert_eq!(cg.find_stale_files().await, Vec::<String>::new());
    cg.sync().await.unwrap();
    assert_eq!(indexed_files(&cg).await, vec!["base.rs".to_string()]);
}

/// The scan without gitignore rules is a different walk with the same rule.
#[tokio::test]
async fn the_walk_without_gitignore_rules_skips_the_worktree_too() {
    let tmp = checkout_with_nested_worktree(&["nested"]);
    let root = tmp.path();
    let cg = TokenSave::init(root).await.unwrap();
    drop(cg);
    let mut config = load_config(root).unwrap();
    config.git_ignore = false;
    save_config(root, &config).unwrap();
    let cg = TokenSave::open(root).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(indexed_files(&cg).await, vec!["base.rs".to_string()]);
}

/// An `include` glob admits a hidden directory. It does not admit a worktree
/// below that directory.
#[tokio::test]
async fn an_include_glob_does_not_admit_a_worktree() {
    let tmp = checkout_with_nested_worktree(&[".trees", "side"]);
    let root = tmp.path();
    std::fs::write(root.join(".trees").join("notes.rs"), "pub fn notes() {}\n").unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    drop(cg);
    let mut config = load_config(root).unwrap();
    config.include = vec![".trees".to_string(), ".trees/**".to_string()];
    save_config(root, &config).unwrap();
    let cg = TokenSave::open(root).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(
        indexed_files(&cg).await,
        vec![".trees/notes.rs".to_string(), "base.rs".to_string()]
    );
}

/// `force_include` skips the ignore rules. It does not skip this rule.
#[tokio::test]
async fn a_force_include_glob_does_not_admit_a_worktree() {
    let tmp = checkout_with_nested_worktree(&["nested"]);
    let root = tmp.path();
    let cg = TokenSave::init(root).await.unwrap();
    drop(cg);
    let mut config = load_config(root).unwrap();
    config.force_include = vec!["nested/**".to_string()];
    save_config(root, &config).unwrap();
    let cg = TokenSave::open(root).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(indexed_files(&cg).await, vec!["base.rs".to_string()]);
}

/// An index made by an earlier version holds the files of the worktree. The
/// next sync removes them, and the automatic sync does not put them back.
#[tokio::test]
async fn a_sync_removes_the_rows_of_a_directory_that_is_a_worktree() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    std::fs::write(root.join("base.rs"), "pub fn base() {}\n").unwrap();
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("copy.rs"), "pub fn copy() {}\n").unwrap();
    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(
        indexed_files(&cg).await,
        vec!["base.rs".to_string(), "nested/copy.rs".to_string()],
        "precondition: a plain directory is indexed"
    );

    // The `.git` file that `git worktree add` writes makes it a worktree.
    let gitdir = root.join(".git").join("worktrees").join("nested");
    std::fs::write(
        nested.join(".git"),
        format!("gitdir: {}\n", gitdir.display()),
    )
    .unwrap();

    let stale = cg.find_stale_files().await;
    cg.sync_if_stale_silent(&stale).await.unwrap();
    assert_eq!(
        indexed_files(&cg).await,
        vec!["base.rs".to_string()],
        "the automatic sync must remove the rows of the worktree"
    );
    assert_eq!(cg.find_stale_files().await, Vec::<String>::new());

    cg.sync().await.unwrap();
    assert_eq!(indexed_files(&cg).await, vec!["base.rs".to_string()]);
}

/// Git pointer files for submodules still allow the scan to enter.
#[tokio::test]
async fn a_nested_clone_is_skipped_but_a_submodule_is_indexed() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join(".gitignore"), ".tokensave/\n").unwrap();
    std::fs::write(root.join("base.rs"), "pub fn base() {}\n").unwrap();
    let clone = root.join("vendor_clone");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-b", "master"]);
    std::fs::write(clone.join("cloned.rs"), "pub fn cloned() {}\n").unwrap();
    let submodule = root.join("deps");
    std::fs::create_dir_all(&submodule).unwrap();
    std::fs::write(submodule.join(".git"), "gitdir: ../.git/modules/deps\n").unwrap();
    std::fs::write(submodule.join("module.rs"), "pub fn module() {}\n").unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(
        indexed_files(&cg).await,
        vec!["base.rs".to_string(), "deps/module.rs".to_string()]
    );
}

/// A linked worktree with an index of its own is a project root. The rule
/// applies below the root only.
#[tokio::test]
async fn a_worktree_that_is_the_project_root_is_indexed() {
    let tmp = checkout_with_nested_worktree(&["nested"]);
    let nested = tmp.path().join("nested");
    let cg = TokenSave::init(&nested).await.unwrap();
    cg.index_all().await.unwrap();
    assert_eq!(
        indexed_files(&cg).await,
        vec!["base.rs".to_string(), "side_only.rs".to_string()]
    );
}

#[tokio::test]
async fn syncing_prunes_a_new_nested_clone_and_it_can_be_indexed_separately() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-b", "master"]);
    std::fs::write(root.join("base.rs"), "pub fn base() {}\n").unwrap();
    git(root, &["add", "base.rs"]);
    git(root, &["commit", "-m", "base"]);
    let nested = root.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::copy(root.join("base.rs"), nested.join("base.rs")).unwrap();

    let parent = TokenSave::init(root).await.unwrap();
    parent.index_all().await.unwrap();
    assert_eq!(indexed_files(&parent).await, ["base.rs", "nested/base.rs"]);

    std::fs::remove_file(nested.join("base.rs")).unwrap();
    git(root, &["clone", ".", "nested"]);
    parent.sync().await.unwrap();
    assert_eq!(indexed_files(&parent).await, ["base.rs"]);

    let child = TokenSave::init(&nested).await.unwrap();
    child.index_all().await.unwrap();
    assert_eq!(indexed_files(&child).await, ["base.rs"]);
}
