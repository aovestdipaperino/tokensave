//! The indexing walk must not follow a directory symlink back onto the project
//! root or one of its ancestors (#327).
//!
//! Every Wine prefix ships `dosdevices/z: -> /`, so a project that contains one
//! (or whose root is `$HOME`) made `init`/`sync` walk the whole filesystem and
//! `serve`'s freshness scan pin a core indefinitely. Links into disjoint trees
//! are a deliberate feature (#34) and must keep working, so these tests pin both
//! halves: root-or-ancestor targets are pruned, everything else is not.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::tokensave::TokenSave;

/// `<tmp>/proj` as the project root, `<tmp>/outside/lib.rs` as content that
/// only an escaping link can reach, and one source file inside the project.
///
/// The layout also carries a walker fingerprint: `ignored/skip.rs` plus a
/// `.gitignore` that hides it. Only the gitignore-aware walker honors that
/// file, so each test's exact expected path set also pins which walker produced
/// it. Without the fingerprint a `git_ignore = true` test could be served by the
/// plain-walkdir fallback (which runs whenever the ignore-aware scan comes back
/// empty, for instance under an enclosing repository that ignores the temp
/// directory) and would then assert nothing about that walker's own guard.
fn ancestor_layout(tmp: &Path) -> std::path::PathBuf {
    let project = tmp.join("proj");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "pub fn inside_symbol() {}\n").unwrap();
    add_walker_fingerprint(&project);
    fs::create_dir_all(tmp.join("outside")).unwrap();
    fs::write(tmp.join("outside/lib.rs"), "pub fn outside_symbol() {}\n").unwrap();
    project
}

fn add_walker_fingerprint(project: &Path) {
    fs::create_dir_all(project.join("ignored")).unwrap();
    fs::write(
        project.join("ignored/skip.rs"),
        "pub fn gitignored_symbol() {}\n",
    )
    .unwrap();
    fs::write(project.join(".gitignore"), "ignored/\n").unwrap();
}

/// Expected indexed paths: the project's own files, plus the fingerprint file
/// when (and only when) the plain walker ran.
fn expect_paths(git_ignore: bool, own: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = own.iter().map(|p| (*p).to_string()).collect();
    if !git_ignore {
        all.push("ignored/skip.rs".to_string());
    }
    all.sort();
    all
}

async fn open_with_git_ignore(project: &Path, git_ignore: bool) -> TokenSave {
    TokenSave::init(project).await.unwrap();
    let mut config = load_config(project).unwrap();
    config.git_ignore = git_ignore;
    save_config(project, &config).unwrap();
    TokenSave::open(project).await.unwrap()
}

async fn indexed_paths(cg: &TokenSave) -> Vec<String> {
    let mut paths: Vec<String> = cg
        .get_all_files()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    paths.sort();
    paths
}

/// A link straight to the root's parent is pruned by the plain walker.
#[tokio::test]
async fn ancestor_link_is_pruned_by_walkdir() {
    let dir = TempDir::new().unwrap();
    let project = ancestor_layout(dir.path());
    symlink(dir.path(), project.join("uplink")).unwrap();

    let cg = open_with_git_ignore(&project, false).await;
    cg.index_all().await.unwrap();

    assert_eq!(
        indexed_paths(&cg).await,
        expect_paths(false, &["src/main.rs"])
    );
    assert!(
        cg.search("outside_symbol", 10).await.unwrap().is_empty(),
        "a link to the root's ancestor must not pull in files outside the project"
    );
}

/// Same for the gitignore-aware walker, which is the default path.
#[tokio::test]
async fn ancestor_link_is_pruned_by_gitignore_walker() {
    let dir = TempDir::new().unwrap();
    let project = ancestor_layout(dir.path());
    symlink(dir.path(), project.join("uplink")).unwrap();

    let cg = open_with_git_ignore(&project, true).await;
    cg.index_all().await.unwrap();

    // `ignored/skip.rs` absent proves the gitignore-aware walker produced this
    // result rather than the plain-walkdir fallback.
    assert_eq!(
        indexed_paths(&cg).await,
        expect_paths(true, &["src/main.rs"])
    );
    assert!(
        cg.search("outside_symbol", 10).await.unwrap().is_empty(),
        "a link to the root's ancestor must not pull in files outside the project"
    );
}

/// A link pointing at the root itself is pruned, so nothing is indexed twice.
#[tokio::test]
async fn self_link_is_pruned() {
    let dir = TempDir::new().unwrap();
    let project = ancestor_layout(dir.path());
    symlink(&project, project.join("selflink")).unwrap();

    let cg = open_with_git_ignore(&project, true).await;
    cg.index_all().await.unwrap();

    assert_eq!(
        indexed_paths(&cg).await,
        expect_paths(true, &["src/main.rs"])
    );
}

/// The guard is evaluated at every depth, not only directly under the root: a
/// disjoint link is followed, and the escaping link found *through* it is still
/// pruned.
#[tokio::test]
async fn nested_reentry_link_is_pruned() {
    let dir = TempDir::new().unwrap();
    let project = ancestor_layout(dir.path());
    let sibling = dir.path().join("sibling");
    fs::create_dir_all(&sibling).unwrap();
    fs::write(sibling.join("shared.rs"), "pub fn sibling_symbol() {}\n").unwrap();
    // Followed: disjoint from the root (#34).
    symlink(&sibling, project.join("bridge")).unwrap();
    // Pruned: resolves to the root's parent, reached through the link above.
    symlink(dir.path(), sibling.join("back")).unwrap();

    let cg = open_with_git_ignore(&project, true).await;
    cg.index_all().await.unwrap();

    assert_eq!(
        indexed_paths(&cg).await,
        expect_paths(true, &["bridge/shared.rs", "src/main.rs"]),
    );
    assert!(
        cg.search("outside_symbol", 10).await.unwrap().is_empty(),
        "the nested link back to the root's parent must be pruned"
    );
}

/// #34 stays intact for a tree that lives beside the project under the same
/// parent: the target is disjoint from the root, so it is not an ancestor.
#[tokio::test]
async fn disjoint_sibling_link_is_still_followed() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let external = dir.path().join("external");
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("lib.rs"), "pub fn external_symbol() {}\n").unwrap();
    symlink(&external, project.join("src")).unwrap();
    add_walker_fingerprint(&project);

    for git_ignore in [false, true] {
        let cg = open_with_git_ignore(&project, git_ignore).await;
        cg.index_all().await.unwrap();

        assert_eq!(
            indexed_paths(&cg).await,
            expect_paths(git_ignore, &["src/lib.rs"]),
            "git_ignore = {git_ignore}"
        );
        assert!(
            !cg.search("external_symbol", 10).await.unwrap().is_empty(),
            "git_ignore = {git_ignore}"
        );
    }
}

/// A project whose own root is a symlink must still scan: the walk root is
/// exempt from the guard even though it canonicalizes to the root.
#[tokio::test]
async fn symlinked_project_root_still_scans() {
    let dir = TempDir::new().unwrap();
    let real = dir.path().join("real");
    fs::create_dir_all(real.join("src")).unwrap();
    fs::write(real.join("src/lib.rs"), "pub fn behind_root_link() {}\n").unwrap();
    add_walker_fingerprint(&real);
    let root_link = dir.path().join("proj-link");
    symlink(&real, &root_link).unwrap();

    for git_ignore in [false, true] {
        let cg = open_with_git_ignore(&root_link, git_ignore).await;
        cg.index_all().await.unwrap();

        assert_eq!(
            indexed_paths(&cg).await,
            expect_paths(git_ignore, &["src/lib.rs"]),
            "git_ignore = {git_ignore}"
        );
    }
}

/// An `include` glob cannot re-enable an escaping link: the guard runs before
/// the hidden-directory and include rules.
///
/// The pattern names the hidden directory itself as well as its descendants, so
/// the plain walker's own hidden-entry branch would otherwise let the link
/// through and the guard is the only thing that stops it.
#[tokio::test]
async fn include_glob_cannot_bypass_the_guard() {
    let dir = TempDir::new().unwrap();
    let project = ancestor_layout(dir.path());
    symlink(dir.path(), project.join(".linked")).unwrap();

    for git_ignore in [false, true] {
        TokenSave::init(&project).await.unwrap();
        let mut config = load_config(&project).unwrap();
        config.git_ignore = git_ignore;
        config.include = vec![".linked".to_string(), ".linked/**".to_string()];
        save_config(&project, &config).unwrap();
        let cg = TokenSave::open(&project).await.unwrap();
        cg.index_all().await.unwrap();

        assert_eq!(
            indexed_paths(&cg).await,
            expect_paths(git_ignore, &["src/main.rs"]),
            "git_ignore = {git_ignore}"
        );
    }
}

/// Upgrade path: rows indexed through an ancestor link before the fix are
/// removed by the next full `sync`, and the sync after that is quiet.
#[tokio::test]
async fn full_sync_removes_rows_reached_through_an_ancestor_link() {
    let dir = TempDir::new().unwrap();
    let project = ancestor_layout(dir.path());
    // A real directory first, so the pre-fix path `uplink/outside/lib.rs` is
    // genuinely present in the DB before the link replaces it.
    fs::create_dir_all(project.join("uplink/outside")).unwrap();
    fs::write(
        project.join("uplink/outside/lib.rs"),
        "pub fn outside_symbol() {}\n",
    )
    .unwrap();

    let cg = open_with_git_ignore(&project, true).await;
    cg.index_all().await.unwrap();
    assert_eq!(
        indexed_paths(&cg).await,
        expect_paths(true, &["src/main.rs", "uplink/outside/lib.rs"]),
    );

    fs::remove_dir_all(project.join("uplink")).unwrap();
    symlink(dir.path(), project.join("uplink")).unwrap();

    let result = cg.sync().await.unwrap();
    assert_eq!(result.files_removed, 1);
    assert_eq!(
        indexed_paths(&cg).await,
        expect_paths(true, &["src/main.rs"])
    );
    assert!(cg.search("outside_symbol", 10).await.unwrap().is_empty());

    let second = cg.sync().await.unwrap();
    assert_eq!(second.files_removed, 0);
    assert_eq!(second.files_added, 0);
}
