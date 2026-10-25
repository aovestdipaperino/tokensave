//! `serve` starts with no default project when none resolves (#606).
//!
//! With several projects registered and a working directory inside none of
//! them, or with `--path` naming a folder that has no index, `serve` used to
//! print the ambiguity and exit before answering `initialize`. The MCP host
//! then showed the server as failed and the agent lost `graph_root` access to
//! every registered project, which would still have worked.
//!
//! These tests spawn the binary with a throwaway `HOME`, so the global
//! database they register projects in is their own.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};
use tempfile::TempDir;

/// Applies the throwaway home to a child command.
fn with_home<'a>(command: &'a mut Command, home: &Path) -> &'a mut Command {
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .env_remove("TOKENSAVE_DISABLE_SERVER")
        .env_remove("DISABLE_TOKENSAVE")
}

/// Creates a small project and indexes it with `tokensave init`, which also
/// registers it in the global database under `home`.
fn init_project(home: &Path, symbol: &str) -> TempDir {
    let project = TempDir::new().expect("temp project");
    std::fs::create_dir_all(project.path().join("src")).expect("create src");
    std::fs::write(
        project.path().join("src").join("lib.rs"),
        format!("pub fn {symbol}() -> u32 {{ 1 }}\n"),
    )
    .expect("write source");
    let output = with_home(
        Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .args(["init", "--no-git-hook"])
            .arg(project.path())
            .current_dir(project.path())
            .stdin(Stdio::null()),
        home,
    )
    .output()
    .expect("run tokensave init");
    assert!(
        output.status.success(),
        "tokensave init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    project
}

/// A running `tokensave serve` driven one request at a time.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
}

impl Server {
    fn spawn(home: &Path, cwd: &Path, explicit_path: Option<&Path>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
        command.arg("serve");
        if let Some(path) = explicit_path {
            command.arg("--path").arg(path);
        }
        let mut child = with_home(
            command
                .current_dir(cwd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
            home,
        )
        .spawn()
        .expect("spawn tokensave serve");
        let stdout: ChildStdout = child.stdout.take().expect("piped stdout");
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            lines,
        }
    }

    /// Sends one request and returns the response that carries its id,
    /// skipping notifications.
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let stdin = self.stdin.as_mut().expect("stdin still open");
        writeln!(stdin, "{request}").expect("write request");
        stdin.flush().expect("flush request");
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|_| panic!("no response to {method}: server exited or hung"));
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if value.get("id") == Some(&json!(id)) {
                return value;
            }
        }
    }

    fn initialize(&mut self) -> Value {
        self.request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "t", "version": "1"}
            }),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn path_str(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn error_message(response: &Value) -> String {
    response["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a JSON-RPC error, got {response}"))
        .to_string()
}

fn result_text(response: &Value) -> String {
    response["result"]["content"]
        .as_array()
        .unwrap_or_else(|| panic!("expected a tool result, got {response}"))
        .iter()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Registered project names as the error and instructions may spell them:
/// the global DB stores a normalized key, so compare on the final component.
fn names_project(text: &str, project: &Path) -> bool {
    let name = project
        .file_name()
        .expect("temp dir has a name")
        .to_string_lossy();
    text.contains(name.as_ref())
}

fn assert_serves_without_project(explicit_path: bool) {
    let home = TempDir::new().expect("temp home");
    let alpha = init_project(home.path(), "alpha_symbol");
    let beta = init_project(home.path(), "beta_symbol");
    let outside = TempDir::new().expect("folder inside no project");

    let mut server = Server::spawn(
        home.path(),
        outside.path(),
        explicit_path.then(|| outside.path()),
    );

    let init = server.initialize();
    assert!(
        init.get("result").is_some(),
        "initialize must succeed with no default project: {init}"
    );
    let instructions = init["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        names_project(instructions, alpha.path()) && names_project(instructions, beta.path()),
        "instructions should name the registered projects: {instructions}"
    );

    let local = server.request(
        2,
        "tools/call",
        json!({"name": "tokensave_search", "arguments": {"query": "alpha_symbol"}}),
    );
    let message = error_message(&local);
    assert!(
        message.contains("graph_root"),
        "error should point at graph_root: {message}"
    );
    assert!(
        names_project(&message, alpha.path()) && names_project(&message, beta.path()),
        "error should list the registered projects: {message}"
    );

    let selected = server.request(
        3,
        "tools/call",
        json!({
            "name": "tokensave_search",
            "arguments": {"query": "alpha_symbol", "graph_root": path_str(alpha.path())}
        }),
    );
    assert!(
        selected.get("error").is_none(),
        "graph_root call must work with no default project: {selected}"
    );
    assert!(
        result_text(&selected).contains("alpha_symbol"),
        "graph_root call should answer from the selected project: {selected}"
    );

    let federated = server.request(
        4,
        "tools/call",
        json!({
            "name": "tokensave_search",
            "arguments": {
                "query": "beta_symbol",
                "graph_root": [path_str(alpha.path()), path_str(beta.path())]
            }
        }),
    );
    assert!(
        result_text(&federated).contains("beta_symbol"),
        "federated call should work with no default project: {federated}"
    );

    let resource = server.request(5, "resources/read", json!({"uri": "tokensave://status"}));
    assert!(
        error_message(&resource).contains("graph_root"),
        "a resource of the default project should explain there is none: {resource}"
    );

    let tools = server.request(6, "tools/list", json!({}));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()),
        "tools/list must still list tools: {tools}"
    );

    // `tokensave_status` is the diagnostic tool, so it answers rather than
    // refusing: it says there is no default project and what to do instead.
    let status = server.request(
        7,
        "tools/call",
        json!({"name": "tokensave_status", "arguments": {}}),
    );
    assert!(
        status.get("error").is_none(),
        "tokensave_status must answer with no default project: {status}"
    );
    let text = result_text(&status);
    assert!(
        text.contains("no default project") && text.contains("graph_root"),
        "status should say there is no default project and point at graph_root: {text}"
    );
    assert!(
        names_project(&text, alpha.path()) && names_project(&text, beta.path()),
        "status should list the registered projects: {text}"
    );
}

/// Whether a call without `graph_root` is answered, i.e. a default project
/// is served.
fn serves_a_default_project(server: &mut Server) -> bool {
    let init = server.initialize();
    assert!(init.get("result").is_some(), "initialize failed: {init}");
    let response = server.request(
        2,
        "tools/call",
        json!({"name": "tokensave_search", "arguments": {"query": "alpha_symbol"}}),
    );
    response.get("error").is_none()
}

#[test]
fn an_explicit_path_without_an_index_does_not_fall_back_to_the_only_registered_project() {
    let home = TempDir::new().expect("temp home");
    let _alpha = init_project(home.path(), "alpha_symbol");
    let unindexed = TempDir::new().expect("folder without an index");

    let mut server = Server::spawn(home.path(), unindexed.path(), Some(unindexed.path()));
    assert!(
        !serves_a_default_project(&mut server),
        "`--path` names the project to serve; a folder without an index must not \
         silently serve another project"
    );
}

#[test]
fn discovery_without_a_path_still_serves_the_only_registered_project() {
    let home = TempDir::new().expect("temp home");
    let _alpha = init_project(home.path(), "alpha_symbol");
    let outside = TempDir::new().expect("folder inside no project");

    let mut server = Server::spawn(home.path(), outside.path(), None);
    assert!(
        serves_a_default_project(&mut server),
        "with no --path and one registered project, serve keeps serving it"
    );
}

#[test]
fn serve_outside_every_project_starts_with_no_default_project() {
    assert_serves_without_project(false);
}

#[test]
fn serve_with_a_path_that_has_no_index_starts_with_no_default_project() {
    assert_serves_without_project(true);
}
