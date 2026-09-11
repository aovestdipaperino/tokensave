//! `check_file_staleness` and paths outside the served project — #528.
//!
//! The MCP edit and read tools accept an absolute `path` so an agent can
//! reach a sibling worktree. The post-call freshness check then fed that
//! path to `check_file_staleness`, which resolved it with
//! `project_root.join(path)` — and `Path::join` discards the base when its
//! argument is absolute. The file existed, the DB had no row for it, so it
//! was classed "new — needs indexing" and the following sync wrote a row
//! under its *absolute* path. Project A's graph then answered with project
//! B's symbols.
//!
//! The edit itself must keep working; only the indexing must not follow.

use tempfile::tempdir;
use tokensave::tokensave::TokenSave;

#[tokio::test]
async fn an_absolute_path_outside_the_root_is_not_stale() {
    let a = tempdir().unwrap();
    let b = tempdir().unwrap();
    std::fs::write(a.path().join("a.py"), "def a():\n    return 1\n").unwrap();
    std::fs::write(b.path().join("b.py"), "def b():\n    return 2\n").unwrap();

    let cg = TokenSave::init(a.path()).await.unwrap();
    cg.sync().await.unwrap();

    let outside = b.path().join("b.py").to_string_lossy().to_string();
    let stale = cg.check_file_staleness(&[outside]).await;

    assert!(
        stale.is_empty(),
        "a file in another project must never be scheduled for indexing: {stale:?}"
    );
}

/// The guard must not swallow the ordinary case: an absolute path that really
/// is inside the root is rewritten to the project-relative form the DB uses,
/// so a genuine edit through an absolute path still refreshes its row.
#[tokio::test]
async fn an_absolute_path_inside_the_root_is_relativized() {
    let tmp = tempdir().unwrap();
    std::fs::write(tmp.path().join("a.py"), "def a():\n    return 1\n").unwrap();

    let cg = TokenSave::init(tmp.path()).await.unwrap();
    cg.sync().await.unwrap();

    // A new file the index has not seen, named by its absolute path.
    std::fs::write(tmp.path().join("new.py"), "def fresh():\n    return 3\n").unwrap();
    let inside = tmp.path().join("new.py").to_string_lossy().to_string();

    let stale = cg.check_file_staleness(&[inside]).await;

    assert_eq!(
        stale,
        vec!["new.py".to_string()],
        "an in-root absolute path must come back as its project-relative form"
    );
}

/// A relative path that climbs out of the root is the same escape with a
/// different spelling, and is dropped rather than joined.
#[tokio::test]
async fn a_relative_path_climbing_out_of_the_root_is_dropped() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.py"), "def a():\n    return 1\n").unwrap();
    std::fs::write(tmp.path().join("outside.py"), "def out():\n    return 9\n").unwrap();

    let cg = TokenSave::init(&root).await.unwrap();
    cg.sync().await.unwrap();

    let stale = cg
        .check_file_staleness(&["../outside.py".to_string()])
        .await;

    assert!(
        stale.is_empty(),
        "a `..` path must not reach outside the project: {stale:?}"
    );
}
