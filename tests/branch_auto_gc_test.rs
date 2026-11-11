//! Automatic and explicit branch GC share deletion and safety rules.
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::branch::{collect_if_enabled, collect_stale_indexes};
use tokensave::branch_meta::{load_branch_meta, save_branch_meta};
use tokensave::tokensave::TokenSave;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn project() -> TempDir {
    let dir = tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    std::fs::write(dir.path().join("base.rs"), "fn base() {}\n").unwrap();
    git(dir.path(), &["add", "base.rs"]);
    git(dir.path(), &["commit", "-m", "base"]);
    TokenSave::init(dir.path()).await.unwrap();
    add_index(dir.path(), "gone", "branches/gone.db");
    dir
}

fn add_index(root: &Path, name: &str, db_file: &str) {
    let dir = tokensave::config::get_tokensave_dir(root);
    std::fs::create_dir_all(dir.join("branches")).unwrap();
    let mut meta = load_branch_meta(&dir).unwrap();
    meta.add_branch(name, db_file, "main");
    save_branch_meta(&dir, &meta).unwrap();
    std::fs::write(dir.join(db_file), "unused branch index").unwrap();
}

fn enable(root: &Path, enabled: bool) {
    let mut config = tokensave::config::load_config(root).unwrap();
    config.auto_gc = enabled;
    tokensave::config::save_config(root, &config).unwrap();
}

#[tokio::test]
async fn automatic_gc_is_opt_in_and_runs_on_open() {
    let dir = project().await;
    let root = dir.path();
    assert!(!tokensave::config::load_config(root).unwrap().auto_gc);
    let opened = TokenSave::open(root).await.unwrap();
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    drop(opened);
    enable(root, true);
    TokenSave::open(root).await.unwrap();
    assert!(!tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    let meta = load_branch_meta(&tokensave::config::get_tokensave_dir(root)).unwrap();
    assert!(!meta.is_tracked("gone"));
    assert!(meta.is_tracked("main"));
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("tokensave.db")
        .exists());
}

#[tokio::test]
async fn gc_keeps_packed_live_refs_and_removes_deleted_indexes_and_sidecars() {
    let dir = project().await;
    let root = dir.path();
    git(root, &["branch", "live"]);
    git(root, &["pack-refs", "--all", "--prune"]);
    add_index(root, "live", "branches/live.db");
    let db = tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db");
    for suffix in ["db-wal", "db-shm"] {
        std::fs::write(db.with_extension(suffix), "sidecar").unwrap();
    }
    assert_eq!(
        collect_stale_indexes(root, &[]).await.unwrap(),
        vec!["gone"]
    );
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("live.db")
        .exists());
    assert!(!db.exists());
    assert!(!db.with_extension("db-wal").exists());
    assert!(!db.with_extension("db-shm").exists());
    assert!(collect_stale_indexes(root, &[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn gc_protects_worktrees_even_when_their_branch_ref_is_missing() {
    let dir = project().await;
    let root = dir.path();
    let worktree = dir.path().join("linked-worktree");
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            "occupied",
            worktree.to_str().unwrap(),
        ],
    );
    add_index(root, "occupied", "branches/occupied.db");
    git(root, &["update-ref", "-d", "refs/heads/occupied"]);
    assert_eq!(
        collect_stale_indexes(root, &[]).await.unwrap(),
        vec!["gone"]
    );
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("occupied.db")
        .exists());
    // The same safeguards apply when invoked from a linked worktree itself.
    TokenSave::init(&worktree).await.unwrap();
    add_index(&worktree, "occupied", "branches/occupied.db");
    assert!(collect_stale_indexes(&worktree, &[])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn gc_protects_open_and_shared_databases() {
    let dir = project().await;
    let root = dir.path();
    let db = tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db");
    assert!(collect_stale_indexes(root, std::slice::from_ref(&db))
        .await
        .unwrap()
        .is_empty());
    assert!(db.exists());
    let mut meta = load_branch_meta(&tokensave::config::get_tokensave_dir(root)).unwrap();
    meta.add_branch("alias", "branches/gone.db", "main");
    save_branch_meta(&tokensave::config::get_tokensave_dir(root), &meta).unwrap();
    assert!(collect_stale_indexes(root, &[]).await.unwrap().is_empty());
    assert!(db.exists());
}

#[tokio::test]
async fn gc_refuses_concurrent_sync_and_recovers_after_lock_release() {
    let dir = project().await;
    let root = dir.path();
    let lock = tokensave::tokensave::try_acquire_sync_lock(root).unwrap();
    assert!(collect_stale_indexes(root, &[]).await.is_err());
    enable(root, true);
    // Automatic maintenance must not break opening a healthy index.
    TokenSave::open(root).await.unwrap();
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    drop(lock);
    assert_eq!(
        collect_stale_indexes(root, &[]).await.unwrap(),
        vec!["gone"]
    );
}

#[tokio::test]
async fn gc_fails_closed_without_git_and_reloads_configuration() {
    let dir = project().await;
    let root = dir.path();
    enable(root, true);
    std::fs::rename(root.join(".git"), root.join("hidden-git")).unwrap();
    assert!(collect_stale_indexes(root, &[]).await.is_err());
    collect_if_enabled(root, &[]).await;
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    std::fs::rename(root.join("hidden-git"), root.join(".git")).unwrap();
    enable(root, false);
    collect_if_enabled(root, &[]).await;
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    enable(root, true);
    collect_if_enabled(root, &[]).await;
    assert!(!tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
}

#[tokio::test]
async fn gc_does_not_follow_metadata_paths_outside_branch_directory() {
    let dir = project().await;
    let root = dir.path();
    let outside = dir.path().join("keep.db");
    std::fs::write(&outside, "keep me").unwrap();
    let mut meta = load_branch_meta(&tokensave::config::get_tokensave_dir(root)).unwrap();
    meta.add_branch("outside", "../keep.db", "main");
    save_branch_meta(&tokensave::config::get_tokensave_dir(root), &meta).unwrap();
    assert_eq!(
        collect_stale_indexes(root, &[]).await.unwrap(),
        vec!["gone"]
    );
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep me");
    assert!(
        load_branch_meta(&tokensave::config::get_tokensave_dir(root))
            .unwrap()
            .is_tracked("outside")
    );
}

#[tokio::test]
async fn gc_retains_metadata_if_unlink_fails() {
    let dir = project().await;
    let root = dir.path();
    let db = tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db");
    std::fs::remove_file(&db).unwrap();
    std::fs::create_dir(&db).unwrap();
    assert!(collect_stale_indexes(root, &[]).await.is_err());
    assert!(
        load_branch_meta(&tokensave::config::get_tokensave_dir(root))
            .unwrap()
            .is_tracked("gone")
    );
}

#[test]
fn old_project_configs_keep_automatic_gc_disabled() {
    let mut legacy = serde_json::to_value(tokensave::config::TokenSaveConfig::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("auto_gc");
    let config: tokensave::config::TokenSaveConfig = serde_json::from_value(legacy).unwrap();
    assert!(!config.auto_gc);
    let roundtrip = serde_json::to_value(&config).unwrap();
    assert_eq!(roundtrip["auto_gc"], false);
}

#[tokio::test]
async fn environment_override_can_enable_and_disable_automatic_gc() {
    let dir = project().await;
    let root = dir.path();
    let run = |value: &str| {
        let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .args(["status", "--json"])
            .current_dir(root)
            .env("HOME", root)
            .env("USERPROFILE", root)
            .env("TOKENSAVE_AUTO_GC", value)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run("1");
    assert!(!tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    add_index(root, "gone", "branches/gone.db");
    enable(root, true);
    run("0");
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
}

#[tokio::test]
async fn mcp_freshness_checks_reload_the_cleanup_setting() {
    use std::time::Duration;
    let dir = project().await;
    let root = dir.path();
    let cg = TokenSave::open(root).await.unwrap();
    let server = tokensave::mcp::McpServer::new(cg, None).await;
    assert!(
        server
            .wait_for_startup_catch_up(Duration::from_secs(30))
            .await
    );
    assert!(tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    enable(root, true);
    server
        .cg()
        .unwrap()
        .db()
        .set_metadata("last_sync_at", "0")
        .await
        .unwrap();
    server.reset_staleness_cooldown();
    server.maybe_sync_if_stale().await;
    assert!(server.wait_for_lazy_sync(Duration::from_secs(30)).await);
    assert!(!tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db")
        .exists());
    server.shutdown().await;
}

#[tokio::test]
async fn gc_preserves_databases_registered_to_a_live_server() {
    // Use a dedicated process: the registry is keyed by PID, and Windows
    // resolves its location through the profile API, ignoring HOME/USERPROFILE.
    // A child owns its entry without racing other tests in this shared binary.
    if std::env::var_os("TOKENSAVE_TEST_GC_SERVER_CHILD").is_none() {
        let home = tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &crate::common::qualified_test_name(
                    module_path!(),
                    "gc_preserves_databases_registered_to_a_live_server",
                ),
                "--nocapture",
            ])
            .env("TOKENSAVE_TEST_GC_SERVER_CHILD", "1")
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    struct Registration;
    impl Drop for Registration {
        fn drop(&mut self) {
            tokensave::servers::unregister();
        }
    }

    let dir = project().await;
    let root = dir.path();
    let db = tokensave::config::get_tokensave_dir(root)
        .join("branches")
        .join("gone.db");
    tokensave::servers::register(root, &db, None);
    let registration = Registration;
    assert!(tokensave::servers::list()
        .iter()
        .any(|entry| entry.pid == std::process::id()));
    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .args(["branch", "gc"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run();
    assert!(db.exists(), "a live server owns this index");
    drop(registration);
    run();
    assert!(
        !db.exists(),
        "index becomes collectible after the server leaves"
    );
}
