use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

struct ServeRun {
    status: ExitStatus,
    stdout: String,
    stderr: String,
    project: tempfile::TempDir,
}

fn run_server(
    name: Option<&str>,
    project: tempfile::TempDir,
    initialize_request: bool,
) -> ServeRun {
    let home = tempfile::tempdir().expect("temp home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .args(["serve", "--path"])
        .arg(project.path())
        .current_dir(project.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("APPDATA", home.path())
        .env("LOCALAPPDATA", home.path())
        .env_remove("TOKENSAVE_DISABLE_SERVER")
        .env_remove("DISABLE_TOKENSAVE")
        .stdin(if initialize_request {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(name) = name {
        command.env(name, "true");
    }
    let mut child = command.spawn().expect("run tokensave serve");

    if initialize_request {
        let mut stdin = child.stdin.take().expect("piped tokensave stdin");
        stdin
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
            .expect("write MCP initialize request");
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll tokensave serve") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill hung tokensave serve");
            child.wait().expect("reap hung tokensave serve");
            panic!("tokensave serve did not stop");
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("piped tokensave stdout")
        .read_to_string(&mut stdout)
        .expect("read tokensave stdout");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("piped tokensave stderr")
        .read_to_string(&mut stderr)
        .expect("read tokensave stderr");

    ServeRun {
        status,
        stdout,
        stderr,
        project,
    }
}

fn assert_disables_server(name: &str) {
    let run = run_server(
        Some(name),
        tempfile::tempdir().expect("temp project"),
        false,
    );
    assert!(
        run.status.success(),
        "{name}=true exited with {}\nstderr: {}",
        run.status,
        run.stderr
    );
    assert!(run.stdout.is_empty());
    assert!(!run.project.path().join(".tokensave").exists());
}

#[tokio::test]
async fn server_without_disable_reaches_the_mcp_transport() {
    let project = tempfile::tempdir().expect("temp project");
    let (db, _) =
        tokensave::db::Database::initialize(&project.path().join(".tokensave/tokensave.db"))
            .await
            .expect("initialize project database");
    drop(db);

    let run = run_server(None, project, true);
    #[cfg(windows)]
    if let Some(global_db) = tokensave::global_db::GlobalDb::open().await {
        global_db.delete_project(run.project.path()).await;
    }
    assert!(
        run.status.success(),
        "serve exited with {}\nstderr: {}",
        run.status,
        run.stderr
    );
    assert!(
        run.stdout.contains("\"id\":1"),
        "initialize response missing from stdout: {}",
        run.stdout
    );
}

#[test]
fn canonical_disable_exits_before_project_initialization() {
    assert_disables_server("TOKENSAVE_DISABLE_SERVER");
}

#[test]
fn legacy_disable_exits_before_project_initialization() {
    assert_disables_server("DISABLE_TOKENSAVE");
}
