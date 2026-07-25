//! `tokensave init` must not offer — or act on — the git-exclusion prompt when
//! there is no git repository to exclude from, and must never read stdin when
//! it isn't a terminal (#288).
//!
//! Driven through the real binary because the prompt lives in the CLI path.

use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Runs `tokensave init <dir>` with `stdin_data` piped in (never a TTY under
/// `cargo test`) and returns combined stdout+stderr.
fn run_init(dir: &std::path::Path, stdin_data: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("init")
        .arg(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn tokensave");
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(stdin_data.as_bytes())
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to wait for init");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn sample_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    dir
}

#[test]
fn init_outside_a_git_repo_does_not_claim_to_have_excluded_anything() {
    let dir = sample_project();
    // "g" would previously have been consumed as an answer and reported as a
    // successful .gitignore write.
    let output = run_init(dir.path(), "g\n");

    assert!(
        !output.contains("Added .tokensave"),
        "no exclusion is possible without a git repository, so none may be reported: {output}"
    );
    assert!(
        !output.contains("Exclude .tokensave from git?"),
        "the prompt must not be offered where neither answer has any effect: {output}"
    );
    assert!(
        !dir.path().join(".gitignore").exists(),
        "a piped answer must not create a .gitignore"
    );
    assert!(
        dir.path().join(".tokensave").exists(),
        "init itself must still succeed: {output}"
    );
}

#[test]
fn init_does_not_prompt_when_stdin_is_not_a_terminal() {
    let dir = sample_project();
    let status = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .arg("init")
        .arg("-q")
        .status()
        .unwrap();
    assert!(status.success());

    // Inside a real repository the prompt is meaningful, but stdin is a pipe:
    // it must neither be shown nor consume the caller's input.
    let output = run_init(dir.path(), "g\n");

    assert!(
        !output.contains("Exclude .tokensave from git?"),
        "a non-interactive init must not prompt: {output}"
    );
    assert!(
        !dir.path().join(".gitignore").exists(),
        "the piped 'g' must not be taken for an answer"
    );
}
