use std::process::Command;

use tempfile::TempDir;
use tokensave::{db::Database, sync, verify::verify};

async fn project() -> (TempDir, Database) {
    let dir = TempDir::new().unwrap();
    let index = dir.path().join(".tokensave");
    std::fs::create_dir(&index).unwrap();
    std::fs::write(index.join("config.json"), "{}").unwrap();
    let (db, _) = Database::initialize(&index.join("tokensave.db"))
        .await
        .unwrap();
    add_file(&db, dir.path(), "sample.rs", "fn hello() {}\n").await;
    (dir, db)
}

async fn add_file(db: &Database, root: &std::path::Path, path: &str, source: &str) {
    std::fs::write(root.join(path), source).unwrap();
    let (mtime, size) = sync::file_stat(&root.join(path)).unwrap();
    db.conn().execute("INSERT INTO files(path,content_hash,size,modified_at,indexed_at) VALUES (?1,?2,?3,?4,0)",
        libsql::params![path, sync::content_hash(source), size as i64, mtime]).await.unwrap();
}

#[tokio::test]
async fn healthy_and_stale_sources_are_distinct_from_corruption() {
    let (dir, _db) = project().await;
    for deep in [false, true] {
        let report = verify(dir.path(), deep, false).await;
        assert_eq!(report.exit_code(), 0, "{report:?}");
        assert_eq!(report.checked_files, 1);
    }
    std::fs::write(dir.path().join("sample.rs"), "fn changed() {}\n").unwrap();
    let report = verify(dir.path(), true, false).await;
    assert_eq!(report.exit_code(), 1);
    assert!(report.diagnostics.iter().all(|d| d.severity == "warning"));
    assert_eq!(report.diagnostics[0].code, "SOURCE_STALE");
    std::fs::remove_file(dir.path().join("sample.rs")).unwrap();
    assert_eq!(
        verify(dir.path(), false, false).await.diagnostics[0].code,
        "SOURCE_MISSING"
    );
}

#[tokio::test]
async fn reports_dangling_dependencies_references_and_fingerprints() {
    let (dir, db) = project().await;
    db.conn().execute_batch("PRAGMA foreign_keys=OFF;
        INSERT INTO edges(source,target,kind) VALUES ('absent','missing','calls');
        INSERT INTO unresolved_refs(from_node_id,reference_name,reference_kind,line,col,file_path) VALUES ('absent','f','calls',1,1,'sample.rs');
        INSERT INTO trait_dispatch_callers VALUES ('absent','missing','caller',1);
        INSERT INTO nodes(id,kind,name,qualified_name,file_path,start_line,end_line,start_column,end_column,updated_at,parent_id) VALUES ('orphan','function','f','f','absent.rs',1,1,0,1,0,'missing-parent');
        INSERT INTO vectors(node_id,embedding,model,created_at) VALUES ('absent',X'00','test',0);
        INSERT INTO ambiguous_calls VALUES ('absent','f','sample.rs',1,'[]');
        UPDATE files SET content_hash='invalid';").await.unwrap();
    let report = verify(dir.path(), false, false).await;
    for code in [
        "DANGLING_EDGE",
        "DANGLING_REFERENCE",
        "INVALID_DISPATCH_RELATION",
        "INVALID_FINGERPRINT",
        "ORPHAN_NODE",
        "DANGLING_PARENT",
        "ORPHAN_VECTOR",
        "ORPHAN_AMBIGUOUS_CALL",
    ] {
        assert!(
            report.diagnostics.iter().any(|d| d.code == code),
            "{report:?}"
        );
    }
    assert_eq!(report.exit_code(), 1);
}

#[tokio::test]
async fn invalid_branch_metadata_and_missing_branch_database() {
    let (dir, _db) = project().await;
    let meta = dir.path().join(".tokensave").join("branch-meta.json");
    std::fs::write(&meta, "{").unwrap();
    let report = verify(dir.path(), false, false).await;
    assert_eq!(report.exit_code(), 2);
    assert_eq!(report.diagnostics[0].code, "BRANCH_METADATA_INVALID");
    let mut metadata = tokensave::branch_meta::BranchMeta::new("main");
    metadata.branches.get_mut("main").unwrap().db_file = "missing.db".into();
    std::fs::write(&meta, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let report = verify(dir.path(), false, false).await;
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "BRANCH_DATABASE_MISSING"));
    metadata.branches.get_mut("main").unwrap().db_file = "../../outside.db".into();
    std::fs::write(&meta, serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert_eq!(
        verify(dir.path(), false, false).await.diagnostics[0].code,
        "BRANCH_DATABASE_PATH_INVALID"
    );
}

#[tokio::test]
async fn verification_and_unsupported_repair_do_not_write() {
    let (dir, db) = project().await;
    db.checkpoint().await.unwrap();
    drop(db);
    let path = dir.path().join(".tokensave").join("tokensave.db");
    let before = std::fs::read(&path).unwrap();
    for _ in 0..3 {
        assert_eq!(verify(dir.path(), true, false).await.exit_code(), 0);
        let report = verify(dir.path(), true, true).await;
        assert_eq!(report.exit_code(), 2);
        assert_eq!(report.diagnostics[0].code, "REPAIR_UNSUPPORTED");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[tokio::test]
async fn active_writer_and_rolled_back_interruption_are_not_corruption() {
    let (dir, db) = project().await;
    db.conn().execute_batch("PRAGMA foreign_keys=OFF; BEGIN IMMEDIATE; INSERT INTO edges(source,target,kind) VALUES ('absent','absent','calls');").await.unwrap();
    let report = verify(dir.path(), true, false).await;
    assert_eq!(report.exit_code(), 0, "{report:?}");
    db.conn().execute_batch("ROLLBACK").await.unwrap();
    assert_eq!(verify(dir.path(), true, false).await.exit_code(), 0);
}

#[tokio::test]
async fn schema_mismatch_and_corrupt_database_are_not_migrated() {
    let (dir, db) = project().await;
    db.conn()
        .execute_batch("PRAGMA user_version=999")
        .await
        .unwrap();
    assert_eq!(
        verify(dir.path(), false, false).await.diagnostics[0].code,
        "SCHEMA_INCOMPATIBLE"
    );
    db.checkpoint().await.unwrap();
    drop(db);
    std::fs::write(
        dir.path().join(".tokensave").join("tokensave.db"),
        "not sqlite",
    )
    .unwrap();
    assert_eq!(verify(dir.path(), false, false).await.exit_code(), 2);
}

#[tokio::test]
async fn diagnostics_are_capped_and_repeated_runs_release_handles() {
    let (dir, db) = project().await;
    db.conn().execute_batch("WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<1100) INSERT INTO files(path,content_hash,size,modified_at,indexed_at) SELECT 'missing'||x,printf('%064d',0),0,0,0 FROM seq;").await.unwrap();
    for _ in 0..5 {
        let report = verify(dir.path(), false, false).await;
        assert_eq!(report.diagnostics.len(), 1000);
        assert_eq!(report.findings, 1100);
        assert_eq!(report.omitted, 100);
        assert_eq!(report.checked_files, 1101);
    }
}

#[tokio::test]
async fn deep_hash_matches_indexer_decoding_and_limit_is_explicit() {
    let (dir, db) = project().await;
    for (name, bytes) in [
        ("utf16.rs", vec![0xff, 0xfe, 97, 0, 98]),
        ("legacy.rs", vec![97, 255]),
        ("bom.rs", vec![0xef, 0xbb, 0xbf, 97]),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, &bytes).unwrap();
        let source = sync::read_source_file(&path).unwrap();
        let (mtime, size) = sync::file_stat(&path).unwrap();
        db.conn().execute("INSERT INTO files(path,content_hash,size,modified_at,indexed_at) VALUES (?1,?2,?3,?4,0)", libsql::params![name,sync::content_hash(&source),size as i64,mtime]).await.unwrap();
    }
    assert_eq!(verify(dir.path(), true, false).await.exit_code(), 0);
    add_file(&db, dir.path(), "large.rs", "").await;
    std::fs::File::options()
        .write(true)
        .open(dir.path().join("large.rs"))
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(verify(dir.path(), true, false)
        .await
        .diagnostics
        .iter()
        .any(|d| d.code == "SOURCE_CHECK_SKIPPED"));
}

#[tokio::test]
async fn cli_json_exit_codes_and_startup_are_read_only() {
    let (dir, db) = project().await;
    db.checkpoint().await.unwrap();
    let home = TempDir::new().unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .arg("verify")
            .arg(dir.path())
            .args(args)
            .env("HOME", home.path())
            .output()
            .unwrap()
    };
    let output = run(&["--json", "--deep"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["completed"], true);
    assert_eq!(value["findings"], 0);
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
    let output = run(&["--json", "--repair"]);
    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["diagnostics"][0]["code"], "REPAIR_UNSUPPORTED");
}

#[cfg(unix)]
#[tokio::test]
async fn deep_check_does_not_open_a_named_pipe() {
    let (dir, db) = project().await;
    add_file(&db, dir.path(), "pipe.rs", "").await;
    let pipe = dir.path().join("pipe.rs");
    std::fs::remove_file(&pipe).unwrap();
    assert!(Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .unwrap()
        .success());
    let report = verify(dir.path(), true, false).await;
    assert_eq!(report.exit_code(), 1);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.code == "SOURCE_CHECK_SKIPPED"));
}
