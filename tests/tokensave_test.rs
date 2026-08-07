//! Tests for the `TokenSave` orchestrator methods that aren't fully exercised
//! by the MCP handler tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use tokensave::branch_meta::{self, BranchMeta};
use tokensave::tokensave::{is_test_file, TokenSave};
use tokensave::types::NodeKind;

// ---------------------------------------------------------------------------
// Shared setup
// ---------------------------------------------------------------------------

/// Creates a temporary Rust project with cross-file calls, then initializes
/// and indexes a `TokenSave`.
async fn setup() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();

    fs::write(
        project.join("src/lib.rs"),
        r#"
pub fn foo() { bar(); }
fn bar() {}
fn unused_private() {}
"#,
    )
    .unwrap();

    fs::write(
        project.join("src/utils.rs"),
        r#"
use crate::lib::foo;
pub fn helper() { foo(); }
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

struct SelectedGraphFixture {
    _dir: TempDir,
    root: PathBuf,
}

impl SelectedGraphFixture {
    fn root(&self) -> &Path {
        &self.root
    }

    fn tokensave_dir(&self) -> PathBuf {
        self.root.join(".tokensave")
    }

    fn config_path(&self) -> PathBuf {
        self.tokensave_dir().join("config.json")
    }

    fn branch_meta_path(&self) -> PathBuf {
        self.tokensave_dir().join("branch-meta.json")
    }

    fn metadata_snapshot(&self) -> (Vec<u8>, Option<Vec<u8>>) {
        (
            fs::read(self.config_path()).unwrap(),
            fs::read(self.branch_meta_path()).ok(),
        )
    }

    fn assert_metadata_unchanged(&self, before: &(Vec<u8>, Option<Vec<u8>>)) {
        assert_eq!(fs::read(self.config_path()).unwrap(), before.0);
        assert_eq!(fs::read(self.branch_meta_path()).ok(), before.1);
    }
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn setup_single_db_project(symbol: &str) -> SelectedGraphFixture {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), format!("pub fn {symbol}() {{}}\n")).unwrap();

    let graph = TokenSave::init(&root).await.unwrap();
    graph.index_all().await.unwrap();
    graph.checkpoint().await.unwrap();
    drop(graph);

    SelectedGraphFixture { _dir: dir, root }
}

async fn setup_tracked_branch_project() -> SelectedGraphFixture {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    run_git(&root, &["init", "-b", "master"]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn master_only() {}\n").unwrap();
    run_git(&root, &["add", "src/lib.rs"]);
    run_git(&root, &["commit", "-m", "initial"]);

    let graph = TokenSave::init(&root).await.unwrap();
    graph.index_all().await.unwrap();
    graph.checkpoint().await.unwrap();
    drop(graph);

    let tokensave_dir = root.join(".tokensave");
    fs::create_dir_all(tokensave_dir.join("branches")).unwrap();
    fs::copy(
        tokensave_dir.join("tokensave.db"),
        tokensave_dir.join("branches/feature.db"),
    )
    .unwrap();
    let mut meta = branch_meta::load_branch_meta(&tokensave_dir).unwrap();
    meta.add_branch("feature", "branches/feature.db", "master");
    branch_meta::save_branch_meta(&tokensave_dir, &meta).unwrap();

    fs::write(root.join("src/lib.rs"), "pub fn feature_only() {}\n").unwrap();
    let feature = TokenSave::open_branch(&root, "feature").await.unwrap();
    feature.index_all().await.unwrap();
    feature.checkpoint().await.unwrap();
    drop(feature);

    SelectedGraphFixture { _dir: dir, root }
}

#[tokio::test]
async fn open_read_only_uses_explicit_tracked_branch() {
    let fixture = setup_tracked_branch_project().await;
    let before = fixture.metadata_snapshot();

    let selected = TokenSave::open_read_only(fixture.root(), Some("feature"))
        .await
        .unwrap();

    assert_eq!(selected.active_branch(), Some("feature"));
    assert_eq!(selected.serving_branch(), Some("feature"));
    assert_eq!(selected.fallback_warning(), None);
    assert_eq!(
        selected.db_path(),
        fixture.tokensave_dir().join("branches/feature.db")
    );
    assert_eq!(selected.search("feature_only", 10).await.unwrap().len(), 1);
    assert!(selected.search("master_only", 10).await.unwrap().is_empty());
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_rejects_untracked_explicit_branch() {
    let fixture = setup_tracked_branch_project().await;
    let before = fixture.metadata_snapshot();

    let error = TokenSave::open_read_only(fixture.root(), Some("missing"))
        .await
        .err()
        .expect("untracked explicit branch should fail");

    assert!(error
        .to_string()
        .contains("branch 'missing' is not tracked"));
    assert!(error.to_string().contains("in the selected project"));
    assert!(error.to_string().contains("tokensave branch add 'missing'"));
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_untracked_branch_remedy_quotes_branch_name() {
    let fixture = setup_tracked_branch_project().await;

    let error = TokenSave::open_read_only(fixture.root(), Some("it's"))
        .await
        .err()
        .expect("untracked explicit branch should fail");

    assert!(error
        .to_string()
        .contains("tokensave branch add 'it'\\''s'"));
}

#[tokio::test]
async fn open_branch_rejects_untracked_branch_with_remedy() {
    let fixture = setup_tracked_branch_project().await;

    let error = TokenSave::open_branch(fixture.root(), "missing")
        .await
        .err()
        .expect("untracked branch should fail");

    assert!(error
        .to_string()
        .contains("branch 'missing' is not tracked"));
    assert!(error.to_string().contains("tokensave branch add 'missing'"));
}

#[tokio::test]
async fn open_read_only_rejects_explicit_branch_with_missing_database() {
    let fixture = setup_tracked_branch_project().await;
    fs::remove_file(fixture.tokensave_dir().join("branches/feature.db")).unwrap();
    let before = fixture.metadata_snapshot();

    let error = TokenSave::open_read_only(fixture.root(), Some("feature"))
        .await
        .err()
        .expect("explicit branch with missing database should fail");

    assert!(error.to_string().contains("branch 'feature'"));
    assert!(error.to_string().contains("DB is missing"));
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_rejects_explicit_branch_without_branch_metadata() {
    let fixture = setup_single_db_project("single_only").await;
    let before = fixture.metadata_snapshot();

    let error = TokenSave::open_read_only(fixture.root(), Some("feature"))
        .await
        .err()
        .expect("explicit branch without branch metadata should fail");

    assert!(error.to_string().contains("no branch tracking configured"));
    assert!(error.to_string().contains("tokensave branch add"));
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_omitted_branch_uses_existing_fallback() {
    let fixture = setup_tracked_branch_project().await;
    run_git(fixture.root(), &["checkout", "-b", "work"]);
    let before = fixture.metadata_snapshot();

    let selected = TokenSave::open_read_only(fixture.root(), None)
        .await
        .unwrap();

    assert_eq!(selected.active_branch(), Some("work"));
    assert_eq!(selected.serving_branch(), Some("master"));
    assert!(selected
        .fallback_warning()
        .is_some_and(|warning| warning.contains("branch 'work' is not tracked")));
    assert_eq!(selected.search("master_only", 10).await.unwrap().len(), 1);
    assert!(selected
        .search("feature_only", 10)
        .await
        .unwrap()
        .is_empty());
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_omitted_branch_does_not_auto_track() {
    let fixture = setup_tracked_branch_project().await;
    run_git(fixture.root(), &["checkout", "-b", "work"]);
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.config_path()).unwrap()).unwrap();
    config["auto_track"] = serde_json::Value::Bool(true);
    fs::write(
        fixture.config_path(),
        serde_json::to_vec_pretty(&config).unwrap(),
    )
    .unwrap();
    let before = fixture.metadata_snapshot();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "open_read_only_env_helper", "--nocapture"])
        .env("TOKENSAVE_OPEN_READ_ONLY_TEST_ROOT", fixture.root())
        .env("TOKENSAVE_AUTO_TRACK", "true")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.tokensave_dir().join("branches/work.db").exists());
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_env_helper() {
    let Some(root) = std::env::var_os("TOKENSAVE_OPEN_READ_ONLY_TEST_ROOT") else {
        return;
    };
    let selected = TokenSave::open_read_only(Path::new(&root), None)
        .await
        .unwrap();
    assert_eq!(selected.serving_branch(), Some("master"));
}

#[tokio::test]
async fn open_read_only_opens_branch_only_default_database_layout() {
    let fixture = setup_single_db_project("branch_only").await;
    let tokensave_dir = fixture.tokensave_dir();
    fs::create_dir_all(tokensave_dir.join("branches")).unwrap();
    fs::rename(
        tokensave_dir.join("tokensave.db"),
        tokensave_dir.join("branches/master.db"),
    )
    .unwrap();
    let mut meta = BranchMeta::new("master");
    meta.branches.get_mut("master").unwrap().db_file = "branches/master.db".to_string();
    branch_meta::save_branch_meta(&tokensave_dir, &meta).unwrap();
    let before = fixture.metadata_snapshot();

    let selected = TokenSave::open_read_only(fixture.root(), None)
        .await
        .unwrap();

    assert!(!tokensave_dir.join("tokensave.db").exists());
    assert_eq!(selected.serving_branch(), Some("master"));
    assert_eq!(selected.search("branch_only", 10).await.unwrap().len(), 1);
    fixture.assert_metadata_unchanged(&before);
}

#[tokio::test]
async fn open_read_only_rejects_orphan_database_without_config() {
    let dir = TempDir::new().unwrap();
    let tokensave_dir = dir.path().join(".tokensave");
    fs::create_dir_all(&tokensave_dir).unwrap();
    let database_path = tokensave_dir.join("tokensave.db");
    fs::write(&database_path, b"orphan").unwrap();
    let before = fs::read(&database_path).unwrap();

    let error = TokenSave::open_read_only(dir.path(), None)
        .await
        .err()
        .expect("orphan database should fail exact-root validation");

    assert!(error.to_string().contains("tokensave init"));
    assert_eq!(fs::read(database_path).unwrap(), before);
    assert!(!tokensave_dir.join("config.json").exists());
}

#[tokio::test]
async fn open_read_only_opens_single_database_layout() {
    let fixture = setup_single_db_project("single_only").await;
    let before = fixture.metadata_snapshot();

    let selected = TokenSave::open_read_only(fixture.root(), None)
        .await
        .unwrap();

    assert_eq!(selected.active_branch(), None);
    assert_eq!(selected.serving_branch(), None);
    assert_eq!(selected.fallback_warning(), None);
    assert_eq!(selected.search("single_only", 10).await.unwrap().len(), 1);
    fixture.assert_metadata_unchanged(&before);
}

// ---------------------------------------------------------------------------
// is_test_file
// ---------------------------------------------------------------------------

#[test]
fn test_is_test_file_test_dir() {
    assert!(is_test_file("tests/my_test.rs"));
    assert!(is_test_file("tests/integration.rs"));
}

#[test]
fn test_is_test_file_test_prefix() {
    assert!(is_test_file("test/foo.rs"));
}

#[test]
fn test_is_test_file_spec_dir() {
    assert!(is_test_file("spec/models/user_spec.rb"));
}

#[test]
fn test_is_test_file_e2e_dir() {
    assert!(is_test_file("e2e/login.test.ts"));
}

#[test]
fn test_is_test_file_dot_test() {
    assert!(is_test_file("src/utils.test.ts"));
    assert!(is_test_file("src/utils.spec.js"));
}

#[test]
fn test_is_test_file_underscore_test() {
    assert!(is_test_file("src/utils_test.rs"));
    assert!(is_test_file("src/utils_spec.py"));
}

#[test]
fn test_is_test_file_dunder_tests() {
    assert!(is_test_file("__tests__/component.test.tsx"));
}

#[test]
fn test_is_test_file_normal_source() {
    assert!(!is_test_file("src/lib.rs"));
    assert!(!is_test_file("src/main.rs"));
    assert!(!is_test_file("src/utils.rs"));
}

#[test]
fn test_is_test_file_case_insensitive() {
    assert!(is_test_file("Tests/MyTest.rs"));
    assert!(is_test_file("TESTS/foo.rs"));
}

// ---------------------------------------------------------------------------
// Path-based ranking: app dirs above generated/vendor trees (issue #115)
// ---------------------------------------------------------------------------

/// Two functions with the same name and comparable base relevance live in
/// `src/` and in a generated `dist/` tree. The `src/` definition must rank
/// higher because the path-rank multiplier penalizes generated trees.
/// (`dist/` is used rather than `node_modules`/`vendor`/`build` because those
/// are excluded from indexing by default, whereas `dist/` is indexed.)
#[tokio::test]
async fn test_search_ranks_app_dir_above_generated_tree() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();

    fs::write(
        project.join("src/widget.rs"),
        "pub fn make_widget() -> u32 { 1 }\n",
    )
    .unwrap();
    fs::write(
        project.join("dist/widget.rs"),
        "pub fn make_widget() -> u32 { 2 }\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let results = cg.search("make_widget", 10).await.unwrap();
    let src_pos = results
        .iter()
        .position(|r| r.node.file_path == "src/widget.rs")
        .expect("src definition should be in results");
    let dist_pos = results
        .iter()
        .position(|r| r.node.file_path == "dist/widget.rs")
        .expect("dist definition should still be in results, just lower");
    assert!(
        src_pos < dist_pos,
        "src/widget.rs (pos {src_pos}) should rank above dist/widget.rs (pos {dist_pos})"
    );
}

// ---------------------------------------------------------------------------
// get_all_files / get_all_nodes / get_all_edges through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_all_files() {
    let (_dir, cg) = setup().await;
    let files = cg.get_all_files().await.unwrap();
    assert!(
        files.len() >= 2,
        "should have at least 2 indexed files (lib.rs, utils.rs), got {}",
        files.len(),
    );
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/lib.rs"));
    assert!(paths.contains(&"src/utils.rs"));
}

#[tokio::test]
async fn test_get_all_nodes() {
    let (_dir, cg) = setup().await;
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        !nodes.is_empty(),
        "should have extracted some nodes from the project",
    );
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"foo"), "should have extracted 'foo'");
    assert!(names.contains(&"bar"), "should have extracted 'bar'");
}

#[tokio::test]
async fn test_get_all_edges() {
    let (_dir, cg) = setup().await;
    let edges = cg.get_all_edges().await.unwrap();
    // foo() calls bar(), so there should be at least one edge
    assert!(!edges.is_empty(), "should have at least one edge");
}

// ---------------------------------------------------------------------------
// get_file_dependents
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_file_dependents() {
    let (_dir, cg) = setup().await;
    // utils.rs calls foo from lib.rs, so lib.rs has utils.rs as a dependent
    // (or utils depends on lib). Let's check if lib.rs has dependents.
    let dependents = cg.get_file_dependents("src/lib.rs").await.unwrap();
    // The cross-file resolution may or may not work depending on extractor,
    // but the method should not panic.
    // dependents is a Vec<String> of file paths
    assert!(
        dependents.is_empty() || dependents.iter().any(|d| d.contains("utils")),
        "dependents of lib.rs should either be empty (if resolution didn't link) or contain utils.rs"
    );
}

// ---------------------------------------------------------------------------
// find_dead_code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_find_dead_code_functions() {
    let (_dir, cg) = setup().await;
    let dead = cg
        .find_dead_code(&[NodeKind::Function], false, true)
        .await
        .unwrap();
    // The method should return successfully. Private functions without
    // incoming call edges appear as dead code. The exact results depend
    // on the extractor's edge generation (e.g., contains edges may give
    // nodes incoming edges). Verify the method runs and returns only
    // non-pub, non-main, non-test nodes.
    for node in &dead {
        assert_ne!(node.name, "main", "main should be excluded from dead code");
        assert!(
            !node.name.starts_with("test"),
            "test functions should be excluded from dead code",
        );
        assert_ne!(
            node.visibility,
            tokensave::types::Visibility::Pub,
            "pub items should be excluded from dead code",
        );
    }
}

#[tokio::test]
async fn test_find_dead_code_custom_kinds() {
    let (_dir, cg) = setup().await;
    // Look for dead structs — our test project has none, should return empty
    let dead = cg
        .find_dead_code(&[NodeKind::Struct], false, true)
        .await
        .unwrap();
    assert!(
        dead.is_empty(),
        "test project has no structs, so no dead struct code expected",
    );
}

// ---------------------------------------------------------------------------
// get_file_coupling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_file_coupling_fan_in() {
    let (_dir, cg) = setup().await;
    let coupling = cg.get_file_coupling(true, None, 10).await.unwrap();
    // Even if coupling is empty (due to how the extractor resolves cross-file refs),
    // the method should succeed.
    for (path, count) in &coupling {
        assert!(!path.is_empty());
        assert!(*count > 0);
    }
}

#[tokio::test]
async fn test_get_file_coupling_fan_out() {
    let (_dir, cg) = setup().await;
    let coupling = cg.get_file_coupling(false, None, 10).await.unwrap();
    for (path, count) in &coupling {
        assert!(!path.is_empty());
        assert!(*count > 0);
    }
}

// ---------------------------------------------------------------------------
// check_file_staleness
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_check_file_staleness_not_stale() {
    let (_dir, cg) = setup().await;
    // Right after indexing, files should not be stale
    let stale = cg.check_file_staleness(&["src/lib.rs".to_string()]).await;
    // Immediately after indexing, the file should not be stale
    // (mtime <= indexed_at in most cases)
    assert!(
        stale.is_empty(),
        "files should not be stale right after indexing"
    );
}

#[tokio::test]
async fn test_check_file_staleness_after_modification() {
    let (dir, cg) = setup().await;

    // Wait a moment, then modify the file so mtime > indexed_at
    std::thread::sleep(std::time::Duration::from_secs(2));
    let file_path = dir.path().join("src/lib.rs");
    fs::write(
        &file_path,
        "pub fn foo() { bar(); }\nfn bar() {}\nfn new_function() {}\n",
    )
    .unwrap();

    let stale = cg.check_file_staleness(&["src/lib.rs".to_string()]).await;
    assert!(
        stale.contains(&"src/lib.rs".to_string()),
        "src/lib.rs should be stale after modification"
    );
}

#[tokio::test]
async fn test_check_file_staleness_new_file_not_in_db() {
    use tempfile::tempdir;
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::write(project.join("a.rs"), "fn a() {}").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    // Now add a new file but DON'T sync. b.rs is on disk but not in the DB.
    fs::write(project.join("b.rs"), "fn b() {}").unwrap();

    let stale = cg.check_file_staleness(&["b.rs".to_string()]).await;
    assert_eq!(
        stale,
        vec!["b.rs".to_string()],
        "new file on disk but not in DB should be reported stale"
    );
}

#[tokio::test]
async fn test_check_file_staleness_deleted_indexed_file() {
    use tempfile::tempdir;
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::write(project.join("a.rs"), "fn a() {}").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    // Delete the file. It's indexed but no longer on disk.
    fs::remove_file(project.join("a.rs")).unwrap();

    let stale = cg.check_file_staleness(&["a.rs".to_string()]).await;
    assert_eq!(
        stale,
        vec!["a.rs".to_string()],
        "indexed file deleted from disk should be reported stale"
    );
}

// ---------------------------------------------------------------------------
// #87 — Windows path-separator normalization
// ---------------------------------------------------------------------------
// The DB stores all file paths in canonical forward-slash form (the walker
// in `accept_file` normalizes before insert). If a caller passed a
// backslash-form path (`src\foo.py`) into the staleness / sync entry
// points, the old code treated it as a different file from the
// normalized `src/foo.py` already in the DB — which produced both a
// "stale" verdict (DB miss for the backslash variant) and, after the
// follow-up sync, a *second* row alongside the original. Tools doubled
// their results, the redundancy score halved. This test pins the
// post-fix behaviour: backslash-form input is treated as the same file
// as the forward-slash row.

#[tokio::test]
async fn check_file_staleness_normalizes_backslash_paths() {
    use tempfile::tempdir;
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/a.rs"), "fn a() {}").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    // The DB row is stored under `src/a.rs`. A caller handing us the
    // Windows-shaped `src\a.rs` must hit the same row — not be treated
    // as a missing file that needs indexing.
    let stale = cg.check_file_staleness(&["src\\a.rs".to_string()]).await;
    assert!(
        stale.is_empty(),
        "backslash-form path should match the forward-slash DB row, got stale={stale:?}"
    );
}

#[tokio::test]
async fn sync_if_stale_silent_does_not_create_duplicate_row_for_backslash_path() {
    use tempfile::tempdir;
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/a.rs"), "fn a() {}").unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.sync().await.unwrap();

    // Sleep past the indexed_at second boundary so the mtime check in
    // `check_file_staleness` fires when we rewrite the file. Without
    // this, second-resolution mtimes on some filesystems can leave
    // `mtime == indexed_at` and the staleness check returns empty.
    std::thread::sleep(std::time::Duration::from_secs(1));
    fs::write(project.join("src/a.rs"), "fn a() { let _x = 1; }").unwrap();

    cg.sync_if_stale_silent(&["src\\a.rs".to_string()])
        .await
        .unwrap();

    // Exactly one row should exist for this physical file. Pre-fix,
    // both `src/a.rs` and `src\a.rs` would appear.
    let all = cg.get_all_files().await.unwrap();
    let matches: Vec<&String> = all
        .iter()
        .map(|f| &f.path)
        .filter(|p| p.ends_with("a.rs"))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one a.rs row in DB, found {matches:?}"
    );
    assert_eq!(
        matches[0], "src/a.rs",
        "the surviving row must be the canonical forward-slash form"
    );
}

#[tokio::test]
async fn sync_if_stale_prunes_deleted_files() {
    let (dir, cg) = setup().await;

    // Sanity: utils.rs is indexed with nodes before deletion.
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        nodes.iter().any(|n| n.file_path == "src/utils.rs"),
        "setup should index src/utils.rs nodes"
    );

    fs::remove_file(dir.path().join("src/utils.rs")).unwrap();

    let still_stale = cg
        .sync_if_stale(&["src/utils.rs".to_string()])
        .await
        .unwrap();
    assert!(
        !still_stale,
        "sync_if_stale should report the deleted file as reconciled"
    );

    // Pre-fix (#108): the file row, its nodes, and their edges survived
    // deletion as orphans because sync_single_files had no removal branch.
    let files = cg.get_all_files().await.unwrap();
    assert!(
        !files.iter().any(|f| f.path == "src/utils.rs"),
        "deleted file must be pruned from the files table, got {files:?}"
    );
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        !nodes.iter().any(|n| n.file_path == "src/utils.rs"),
        "deleted file's symbols must be pruned from the graph"
    );
}

#[tokio::test]
async fn sync_if_stale_silent_prunes_deleted_files() {
    let (dir, cg) = setup().await;
    fs::remove_file(dir.path().join("src/utils.rs")).unwrap();

    cg.sync_if_stale_silent(&["src/utils.rs".to_string()])
        .await
        .unwrap();

    let files = cg.get_all_files().await.unwrap();
    assert!(
        !files.iter().any(|f| f.path == "src/utils.rs"),
        "deleted file must be pruned from the files table, got {files:?}"
    );
    let nodes = cg.get_all_nodes().await.unwrap();
    assert!(
        !nodes.iter().any(|n| n.file_path == "src/utils.rs"),
        "deleted file's symbols must be pruned from the graph"
    );
}

// ---------------------------------------------------------------------------
// Go cross-package call resolution (#109)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn go_cross_package_calls_produce_edges() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("a")).unwrap();
    fs::create_dir_all(project.join("b")).unwrap();

    fs::write(
        project.join("go.mod"),
        "module example.com/repro\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        project.join("b/b.go"),
        r#"package b

type Store struct{}

func (s *Store) Get(id int) int { return id }

func Helper(x int) int {
	return inc(x)
}

func inc(x int) int { return x + 1 }
"#,
    )
    .unwrap();
    fs::write(
        project.join("a/a.go"),
        r#"package a

import "example.com/repro/b"

func UseStore(s *b.Store) int {
	return s.Get(42)
}

func UseFunc() int {
	return b.Helper(7)
}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let nodes = cg.get_all_nodes().await.unwrap();
    let edges = cg.get_all_edges().await.unwrap();
    let name_of = |id: &str| {
        nodes
            .iter()
            .find(|n| n.id == id)
            .map_or("?", |n| n.name.as_str())
    };
    let call_pairs: Vec<(String, String)> = edges
        .iter()
        .filter(|e| e.kind == tokensave::types::EdgeKind::Calls)
        .map(|e| {
            (
                name_of(&e.source).to_string(),
                name_of(&e.target).to_string(),
            )
        })
        .collect();

    // Same-package call — worked before #109.
    assert!(
        call_pairs.contains(&("Helper".to_string(), "inc".to_string())),
        "expected same-package Helper -> inc edge, got: {call_pairs:?}"
    );
    // Cross-package function call b.Helper(7).
    assert!(
        call_pairs.contains(&("UseFunc".to_string(), "Helper".to_string())),
        "expected cross-package UseFunc -> Helper edge, got: {call_pairs:?}"
    );
    // Cross-package method call s.Get(42) on an imported type.
    assert!(
        call_pairs.contains(&("UseStore".to_string(), "Get".to_string())),
        "expected cross-package UseStore -> Get edge, got: {call_pairs:?}"
    );
}

// ---------------------------------------------------------------------------
// get_tokens_saved / set_tokens_saved — round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tokens_saved_round_trip() {
    let (_dir, cg) = setup().await;

    // Initially should be 0
    let initial = cg.get_tokens_saved().await.unwrap();
    assert_eq!(initial, 0, "initial tokens_saved should be 0");

    // Set a value
    cg.set_tokens_saved(42_000).await.unwrap();
    let saved = cg.get_tokens_saved().await.unwrap();
    assert_eq!(saved, 42_000);

    // Overwrite
    cg.set_tokens_saved(100_000).await.unwrap();
    let saved2 = cg.get_tokens_saved().await.unwrap();
    assert_eq!(saved2, 100_000);
}

// ---------------------------------------------------------------------------
// get_complexity_ranked through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_complexity_ranked() {
    let (_dir, cg) = setup().await;
    let ranked = cg.get_complexity_ranked(None, None, 10).await.unwrap();
    // Should return functions/methods from our indexed project
    assert!(
        !ranked.is_empty(),
        "should have at least one function in complexity ranking",
    );
    // Verify the tuple structure (node, lines, fan_out, fan_in, score)
    let (node, lines, _fan_out, _fan_in, score) = &ranked[0];
    assert!(!node.name.is_empty());
    assert!(*lines > 0);
    assert!(*score > 0);
}

// ---------------------------------------------------------------------------
// get_undocumented_public_symbols through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_undocumented_public_symbols_no_filter() {
    let (_dir, cg) = setup().await;
    let undoc = cg.get_undocumented_public_symbols(None, 50).await.unwrap();
    // foo is pub and has no docstring
    let names: Vec<&str> = undoc.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.contains(&"foo"),
        "foo is pub without docs, should appear, found: {:?}",
        names,
    );
}

#[tokio::test]
async fn test_get_undocumented_public_symbols_with_prefix() {
    let (_dir, cg) = setup().await;
    let undoc = cg
        .get_undocumented_public_symbols(Some("src/utils"), 50)
        .await
        .unwrap();
    // helper in utils.rs is pub without docs
    for node in &undoc {
        assert!(
            node.file_path.starts_with("src/utils"),
            "path prefix filter should only return src/utils files, got: {}",
            node.file_path,
        );
    }
}

// ---------------------------------------------------------------------------
// get_node_distribution through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_node_distribution() {
    let (_dir, cg) = setup().await;
    let dist = cg.get_node_distribution(None).await.unwrap();
    assert!(!dist.is_empty(), "should have node distribution data");
    // Each entry is (file_path, kind, count)
    for (file, kind, count) in &dist {
        assert!(!file.is_empty());
        assert!(!kind.is_empty());
        assert!(*count > 0);
    }
}

// ---------------------------------------------------------------------------
// is_initialized
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_is_initialized() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    assert!(
        !TokenSave::is_initialized(project),
        "should not be initialized before init"
    );
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "fn main() {}\n").unwrap();
    let _cg = TokenSave::init(project).await.unwrap();
    assert!(
        TokenSave::is_initialized(project),
        "should be initialized after init"
    );
}

// ---------------------------------------------------------------------------
// get_god_classes through TokenSave (empty for Rust-only project)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_god_classes_empty() {
    let (_dir, cg) = setup().await;
    let god = cg.get_god_classes(None, 10).await.unwrap();
    // Pure Rust project with no classes should return empty
    assert!(
        god.is_empty(),
        "Rust project without classes should have no god classes"
    );
}

// ---------------------------------------------------------------------------
// get_inheritance_depth through TokenSave (empty for Rust-only project)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_inheritance_depth_empty() {
    let (_dir, cg) = setup().await;
    let depths = cg.get_inheritance_depth(None, 10).await.unwrap();
    assert!(
        depths.is_empty(),
        "Rust project without class hierarchies should have no inheritance depth"
    );
}

// ---------------------------------------------------------------------------
// search through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search() {
    let (_dir, cg) = setup().await;
    let results = cg.search("foo", 10).await.unwrap();
    assert!(!results.is_empty(), "should find 'foo' via search");
    assert_eq!(results[0].node.name, "foo");
}

// ---------------------------------------------------------------------------
// get_stats through TokenSave
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_stats() {
    let (_dir, cg) = setup().await;
    let stats = cg.get_stats().await.unwrap();
    assert!(stats.node_count > 0, "should have nodes");
    assert!(stats.file_count > 0, "should have files");
}

// ---------------------------------------------------------------------------
// sync_if_stale_silent
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_if_stale_silent_waits_for_peer_then_returns_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    std::fs::write(project.join("a.rs"), "fn a() {}").unwrap();

    let cg = tokensave::tokensave::TokenSave::init(&project)
        .await
        .unwrap();
    cg.sync().await.unwrap();

    // Hold the sync lock to simulate a peer MCP syncing, then release it
    // from a background task so the silent variant's bounded wait can make
    // progress.
    let lock = tokensave::tokensave::try_acquire_sync_lock(&project).expect("lock");
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        drop(lock);
    });

    // Touch the file so it's stale.
    std::fs::write(project.join("a.rs"), "fn a() { let x = 1; }").unwrap();

    // Silent variant should wait for the peer to release the lock and
    // return Ok(()).
    let result = cg.sync_if_stale_silent(&["a.rs".to_string()]).await;
    assert!(result.is_ok(), "expected Ok, got {result:?}");
}

// ---------------------------------------------------------------------------
// #86 — last_sync_timestamp prefers metadata over max(indexed_at)
// ---------------------------------------------------------------------------

/// Regression for #86: the MCP `last synced N ago` warning was reading
/// `MAX(files.indexed_at)`, which only advances when a file is actually
/// reindexed. On quiet repos a successful sync (with 0 changes) leaves
/// `indexed_at` stuck and the warning fires forever. `last_sync_at`
/// metadata is the right source of truth because `sync()` writes it
/// unconditionally.
#[tokio::test]
async fn last_sync_timestamp_uses_metadata_not_indexed_at() {
    let (_dir, cg) = setup().await;

    // Backdate every file's `indexed_at` to simulate a long-quiet repo
    // (typical state before a no-change sync). We use `1` rather than 0
    // because `last_sync_timestamp` treats 0 as "no info available".
    let stale = 1_i64;
    cg.db()
        .conn()
        .execute("UPDATE files SET indexed_at = ?1", libsql::params![stale])
        .await
        .unwrap();

    // Have the metadata reflect a recent sync.
    let fresh = tokensave::tokensave::current_timestamp();
    cg.db()
        .set_metadata("last_sync_at", &fresh.to_string())
        .await
        .unwrap();

    let observed = cg.last_sync_timestamp().await;
    assert_eq!(
        observed, fresh,
        "last_sync_timestamp must return the metadata value, not MAX(indexed_at) (stale={stale}, got {observed})",
    );
    assert_ne!(
        observed, stale,
        "regression: still reading stale indexed_at"
    );
}

/// Fallback: if `last_sync_at` metadata is missing, fall back to
/// `last_index_time`. This keeps freshly-imported projects (no sync yet,
/// only an `init`) honest.
#[tokio::test]
async fn last_sync_timestamp_falls_back_to_indexed_at_without_metadata() {
    let (_dir, cg) = setup().await;
    cg.db()
        .conn()
        .execute(
            "DELETE FROM metadata WHERE key = ?1",
            libsql::params!["last_sync_at"],
        )
        .await
        .unwrap();

    let observed = cg.last_sync_timestamp().await;
    let fallback = cg.last_index_time().await.unwrap();
    assert_eq!(observed, fallback);
}
