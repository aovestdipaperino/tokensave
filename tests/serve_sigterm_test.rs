//! A running MCP server must exit on SIGTERM — #450, and the reapability half
//! of #436.
//!
//! The SIGTERM stream used to be created inside the run loop, so it existed
//! only while `select!` awaited the next line and was dropped before
//! `handle_request` ran. Tokio's registration is process-global and is not
//! undone on drop, so after the first iteration the default disposition
//! (terminate) was permanently replaced while nothing was listening for most
//! of the server's life. A SIGTERM delivered after that first iteration
//! neither killed the process nor reached the loop — the next iteration built
//! a fresh stream, which cannot observe an event delivered before it existed.
//!
//! Reported as: `kill` did nothing, the process was still alive and burning
//! CPU seconds later, and `SIGKILL` was required — which loses the graceful
//! shutdown's WAL checkpoint. A fleet of servers you cannot reap cleanly is
//! the practical cost.
//!
//! The regression is specifically about SIGTERM *after* at least one request
//! has been handled, since the very first iteration did have a live stream.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Wait up to `timeout` for the child to exit, polling rather than blocking so
/// a hung server fails the test instead of hanging the suite.
fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().expect("poll child") {
            Some(status) => return Some(status),
            None if Instant::now() >= deadline => return None,
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Shell out to `kill` rather than take a `libc` dependency for one call.
fn sigterm(child: &Child) {
    let pid = child.id().to_string();
    let status = Command::new("kill")
        .args(["-TERM", &pid])
        .status()
        .expect("run kill");
    assert!(status.success(), "kill -TERM {pid} failed");
}

/// Spawn `tokensave serve` over an initialized project, drive one request
/// through it so the run loop has completed an iteration, then SIGTERM it.
#[test]
fn a_server_that_has_served_a_request_still_exits_on_sigterm() {
    let project = tempfile::tempdir().expect("temp project");
    let home = tempfile::tempdir().expect("temp home");
    std::fs::write(project.path().join("lib.rs"), "fn seed() {}\n").expect("seed source");

    let init = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["init"])
        .current_dir(project.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .output()
        .expect("tokensave init");
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(["serve", "--path"])
        .arg(project.path())
        .current_dir(project.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn tokensave serve");

    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

    // One full request/response, so the loop has been round at least once —
    // this is precisely the state in which the signal used to be swallowed.
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
        .expect("write initialize");
    stdin.flush().expect("flush");

    let mut response = String::new();
    stdout.read_line(&mut response).expect("read response");
    assert!(
        response.contains("\"jsonrpc\""),
        "expected a JSON-RPC response, got {response:?}"
    );

    // stdin stays open, exactly like the live-supervisor case in #436: EOF
    // never arrives, so SIGTERM is the only way out short of SIGKILL.
    sigterm(&child);

    let status = wait_for_exit(&mut child, Duration::from_secs(20));
    let Some(status) = status else {
        let _ = child.kill();
        panic!("server ignored SIGTERM and was still running after 20s");
    };
    assert!(
        status.success() || status.code().is_none(),
        "server should leave through graceful shutdown, got {status:?}"
    );

    // Keep stdin alive until after the assertions so closing it cannot be
    // what ended the process.
    drop(stdin);
}
