use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::SystemTime;

use tokensave::config::{get_global_tokensave_dir, get_tokensave_dir};
use tokensave::db::Database;
use tokensave::global_db::GlobalDb;

mod constants {
    pub(super) const USER_CONFIG_DIR_ENV: &str = "TOKENSAVE_USER_CONFIG_DIR";
    pub(super) const TEST_HOME_ENV: &str = "HOME";
    pub(super) const PROJECT_DB_FILENAME: &str = "tokensave.db";
    pub(super) const GLOBAL_DB_FILENAME: &str = "global.db";
    pub(super) const VACUUM_CANDIDATE_BYTES: i64 = 128 * 1024;
}

fn run_doctor(home: &Path, project: &Path, config_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["doctor", "--read-only"])
        .current_dir(project)
        .env(constants::TEST_HOME_ENV, home)
        .env(constants::USER_CONFIG_DIR_ENV, config_dir)
        .env(tokensave::state_dir::STATE_DIR_ENV, "")
        .stdin(Stdio::null())
        .output()
        .expect("run read-only doctor")
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Option<(Vec<u8>, SystemTime)>)> {
    walkdir::WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .map(|entry| {
            let entry = entry.expect("inspect fixture");
            let path = entry.path();
            let contents = entry.file_type().is_file().then(|| {
                (
                    fs::read(path).expect("read fixture file"),
                    fs::metadata(path)
                        .expect("read fixture metadata")
                        .modified()
                        .expect("read modification time"),
                )
            });
            (path.to_path_buf(), contents)
        })
        .collect()
}

#[tokio::test]
async fn read_only_doctor_preserves_database_and_settings_files() {
    let temp = tempfile::tempdir().expect("create fixture");
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let config_dir = home.join("config");
    let claude_dir = home.join(".claude");
    fs::create_dir_all(&config_dir).expect("create config directory");
    fs::create_dir_all(&claude_dir).expect("create Claude config directory");
    fs::write(claude_dir.join("settings.json"), "{}").expect("write settings needing repair");
    fs::write(config_dir.join("config.toml"), "unparseable = [").expect("write damaged config");
    fs::write(
        config_dir.join("state.toml"),
        "installed_agents = ['claude']\npending_upload = 10\n",
    )
    .expect("register Claude");

    let project_db = get_tokensave_dir(&project).join(constants::PROJECT_DB_FILENAME);
    let (database, _) = Database::initialize(&project_db)
        .await
        .expect("initialize fixture index");
    database
        .conn()
        .execute_batch("CREATE TABLE doctor_fixture (data BLOB);")
        .await
        .expect("create fixture table");
    database
        .conn()
        .execute(
            "INSERT INTO doctor_fixture VALUES (zeroblob(?1))",
            [constants::VACUUM_CANDIDATE_BYTES],
        )
        .await
        .expect("grow fixture database");
    database
        .conn()
        .execute("DELETE FROM doctor_fixture", ())
        .await
        .expect("leave free pages");
    drop(database);

    let global_db = get_global_tokensave_dir(&home).join(constants::GLOBAL_DB_FILENAME);
    let registry = GlobalDb::open_at(&global_db)
        .await
        .expect("create global registry");
    registry
        .upsert(&temp.path().join("removed-project"), 10)
        .await;
    assert_eq!(registry.list_project_paths().await.len(), 1);
    drop(registry);

    let before = snapshot(temp.path());
    let output = run_doctor(&home, &project, &config_dir);
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{report}");
    assert!(report.contains("Index found:"), "{report}");
    assert!(report.contains("DB size:"), "{report}");
    assert!(
        report.contains("Stale project inspection and cleanup skipped"),
        "{report}"
    );
    assert!(report.contains("Settings:"), "{report}");
    assert!(report.contains("Network probes skipped"), "{report}");
    assert_eq!(snapshot(temp.path()), before);
}

#[test]
fn read_only_doctor_does_not_create_missing_user_state() {
    let temp = tempfile::tempdir().expect("create fixture");
    let home = temp.path();
    let config_dir = home.join("missing-config");
    let before = snapshot(home);
    let output = run_doctor(home, home, &config_dir);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(snapshot(home), before);
}
