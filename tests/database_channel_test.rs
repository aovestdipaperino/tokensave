use libsql::Builder;
use std::path::Path;
use tempfile::TempDir;
use tokensave::{
    db::{compatibility, migrations, Database},
    global_db::GlobalDb,
    tokensave::TokenSave,
};

async fn header(path: &Path, sql: &str) -> i64 {
    let db = Builder::new_local(path).build().await.unwrap();
    let conn = db.connect().unwrap();
    let mut rows = conn.query(sql, ()).await.unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

async fn set_header(path: &Path, sql: &str) {
    let db = Builder::new_local(path).build().await.unwrap();
    let conn = db.connect().unwrap();
    conn.execute_batch(sql).await.unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .await
        .unwrap();
}

#[tokio::test]
async fn fresh_and_legacy_origins_survive_reinitialization() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let (db, _) = Database::initialize(&path).await.unwrap();
    db.checkpoint().await.unwrap();
    drop(db);
    let expected = if tokensave::cloud::is_beta() {
        0x5453_4254
    } else {
        0x5453_5354
    };
    assert_eq!(header(&path, "PRAGMA application_id").await, expected);
    let (db, migrated) = Database::initialize(&path).await.unwrap();
    assert!(!migrated);
    db.checkpoint().await.unwrap();
    drop(db);
    assert_eq!(header(&path, "PRAGMA application_id").await, expected);
    // Simulate an unmarked existing v17 index. Opening must migrate it while
    // preserving unknown creation origin, including on repeated initialize.
    set_header(
        &path,
        "PRAGMA application_id = 0; PRAGMA user_version = 17;",
    )
    .await;
    let (db, migrated) = Database::initialize(&path).await.unwrap();
    assert!(migrated);
    db.checkpoint().await.unwrap();
    drop(db);
    assert_eq!(header(&path, "PRAGMA application_id").await, 0);
    assert_eq!(
        header(&path, "PRAGMA user_version").await,
        i64::from(migrations::latest_version())
    );
}

#[tokio::test]
async fn incompatible_headers_never_write_or_recover_even_when_dirty() {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    let ts = TokenSave::init(project).await.unwrap();
    ts.db().checkpoint().await.unwrap();
    drop(ts);
    let state = tokensave::config::get_tokensave_dir(project);
    let path = state.join("tokensave.db");
    let mut headers = vec![
        "PRAGMA application_id = 12345; PRAGMA user_version = 18;".to_string(),
        format!(
            "PRAGMA application_id = 0; PRAGMA user_version = {};",
            migrations::latest_version() + 1
        ),
    ];
    if !tokensave::cloud::is_beta() {
        headers.push("PRAGMA application_id = 1414742612; PRAGMA user_version = 18;".to_string());
    }
    for sql in headers {
        set_header(&path, &sql).await;
        let before = std::fs::read(&path).unwrap();
        std::fs::write(state.join("dirty"), "interrupted").unwrap();
        for result in [
            Database::open(&path).await,
            Database::initialize(&path).await,
        ] {
            assert!(compatibility::is_refusal(&result.err().unwrap()));
        }
        assert!(compatibility::is_refusal(
            &Database::open_read_only(&path).await.err().unwrap()
        ));
        assert!(compatibility::is_refusal(
            &TokenSave::open(project).await.err().unwrap()
        ));
        assert!(compatibility::is_refusal(
            &TokenSave::open_rebuilding_failed_migration(project)
                .await
                .err()
                .unwrap()
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(state.join("dirty").exists());
        assert!(!path.with_extension("db-wal").exists());
        assert!(!path.with_extension("db-shm").exists());
    }
}

#[tokio::test]
async fn global_origin_and_legacy_policy() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("global.db");
    let db = GlobalDb::open_at(&path).await.unwrap();
    drop(db);
    assert_ne!(header(&path, "PRAGMA application_id").await, 0);
    set_header(&path, "PRAGMA application_id = 0;").await;
    drop(GlobalDb::open_at(&path).await.unwrap());
    assert_eq!(header(&path, "PRAGMA application_id").await, 0);
    set_header(&path, "PRAGMA application_id = 12345;").await;
    let before = std::fs::read(&path).unwrap();
    assert!(GlobalDb::open_at(&path).await.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!path.with_extension("db-wal").exists());
}

#[tokio::test]
async fn branch_copy_refuses_incompatible_database_before_creating_destination() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("source.db");
    let target = dir.path().join("branch.db");
    let (db, _) = Database::initialize(&path).await.unwrap();
    db.checkpoint().await.unwrap();
    drop(db);
    set_header(&path, "PRAGMA application_id = 12345;").await;
    let before = std::fs::read(&path).unwrap();
    assert!(compatibility::is_refusal(
        &tokensave::branch::copy_branch_db(&path, &target)
            .await
            .err()
            .unwrap()
    ));
    assert!(!target.exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[tokio::test]
async fn wipe_preserves_the_other_channel_state() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let ts = TokenSave::init(&project).await.unwrap();
    ts.db().checkpoint().await.unwrap();
    drop(ts);
    let stable = project.join(".tokensave");
    let other = if tokensave::cloud::is_beta() {
        stable.clone()
    } else {
        stable.join("beta")
    };
    std::fs::create_dir_all(&other).unwrap();
    let marker = other.join("preserve-me");
    std::fs::write(&marker, "other channel").unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("wipe")
        .current_dir(&project)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"go!\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "other channel");
    assert!(!tokensave::config::get_tokensave_dir(&project)
        .join("tokensave.db")
        .exists());
}

#[tokio::test]
async fn beta_initialization_does_not_adopt_shared_legacy_state() {
    if !tokensave::cloud::is_beta() {
        return;
    }
    let dir = TempDir::new().unwrap();
    let stable = dir.path().join(".tokensave");
    std::fs::create_dir_all(&stable).unwrap();
    let shared = stable.join("tokensave.db");
    std::fs::write(&shared, b"legacy beta database must remain untouched").unwrap();
    let ts = TokenSave::init(dir.path()).await.unwrap();
    ts.db().checkpoint().await.unwrap();
    drop(ts);
    assert_eq!(
        std::fs::read(&shared).unwrap(),
        b"legacy beta database must remain untouched"
    );
    assert!(stable.join("beta").join("tokensave.db").exists());
}

#[tokio::test]
async fn opening_an_empty_database_records_origin() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.db");
    let (db, migrated) = Database::open(&path).await.unwrap();
    assert!(migrated);
    db.checkpoint().await.unwrap();
    drop(db);
    let expected = if tokensave::cloud::is_beta() {
        0x5453_4254
    } else {
        0x5453_5354
    };
    assert_eq!(header(&path, "PRAGMA application_id").await, expected);
}
