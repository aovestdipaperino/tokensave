//! Exercise the repository-owned benchmark helper, without indexing a repo.
#[allow(dead_code)]
#[path = "../benches/repos.rs"]
mod repos;

use repos::ScratchDir;

const SCRATCH: &str = ".tokensave-bench-scratch";

#[test]
fn successful_cleanup_preserves_unrelated_files() {
    let temp = tempfile::tempdir().unwrap();
    let unrelated = temp.path().join("user.txt");
    std::fs::write(&unrelated, "keep me").unwrap();
    {
        let _owned = ScratchDir::create(temp.path(), SCRATCH).unwrap();
        std::fs::write(temp.path().join(SCRATCH).join("query.txt"), "scratch").unwrap();
    }
    assert!(!temp.path().join(SCRATCH).exists());
    assert_eq!(std::fs::read_to_string(unrelated).unwrap(), "keep me");
}

#[test]
fn cleanup_runs_on_early_error() {
    let temp = tempfile::tempdir().unwrap();
    let fail = || -> Result<(), String> {
        let _owned = ScratchDir::create(temp.path(), SCRATCH)?;
        std::fs::write(temp.path().join(SCRATCH).join("query.txt"), "scratch").unwrap();
        Err("index failed".into())
    };
    assert!(fail().is_err());
    assert!(!temp.path().join(SCRATCH).exists());
}

#[test]
fn cleanup_runs_on_unwinding() {
    let temp = tempfile::tempdir().unwrap();
    let result = std::panic::catch_unwind(|| {
        let _owned = ScratchDir::create(temp.path(), SCRATCH).unwrap();
        panic!("benchmark failed");
    });
    assert!(result.is_err());
    assert!(!temp.path().join(SCRATCH).exists());
}

#[test]
fn preexisting_scratch_is_never_adopted_or_removed() {
    let temp = tempfile::tempdir().unwrap();
    let existing = temp.path().join(SCRATCH);
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("keep.txt"), "user data").unwrap();
    let error = ScratchDir::create(temp.path(), SCRATCH).err().unwrap();
    assert!(error.contains("cannot reserve scratch"));
    assert!(error.contains("inspect"));
    assert_eq!(
        std::fs::read_to_string(existing.join("keep.txt")).unwrap(),
        "user data"
    );
}

#[test]
fn concurrent_runs_cannot_share_scratch_but_later_runs_can() {
    let temp = tempfile::tempdir().unwrap();
    let owner = ScratchDir::create(temp.path(), SCRATCH).unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| assert!(ScratchDir::create(temp.path(), SCRATCH).is_err()));
    });
    assert!(temp.path().join(SCRATCH).is_dir());
    drop(owner);
    let next = ScratchDir::create(temp.path(), SCRATCH).unwrap();
    drop(next);
    assert!(!temp.path().join(SCRATCH).exists());
}

#[cfg(unix)]
#[test]
fn preexisting_symlink_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    std::fs::write(target.path().join("keep.txt"), "user data").unwrap();
    let link = temp.path().join(SCRATCH);
    std::os::unix::fs::symlink(target.path(), &link).unwrap();
    assert!(ScratchDir::create(temp.path(), SCRATCH).is_err());
    assert!(link.is_symlink());
    assert!(target.path().join("keep.txt").exists());
}
