//! Bounded stdio MCP handshake probe for `doctor` (#587).
//!
//! An agent that drops a server which does not finish `initialize` and
//! `tools/list` within its startup timeout starts with no tokensave tools
//! and, often, no visible error. The probe launches the command the agent is
//! configured with, the way the agent would, and measures both requests, so
//! `doctor` can tell a server that fails or answers too slowly from a client
//! that never asked.
//!
//! Bounded on every axis: one child process, a hard deadline, a cap on the
//! bytes read, and a kill on every exit path.

mod constants;

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

pub use constants::MAX_PROBE_DURATION;
use constants::{
    EXIT_POLL_INTERVAL, INITIALIZE_ID, MAX_PROBE_OUTPUT_BYTES, PROBE_CLIENT_NAME,
    PROBE_PROTOCOL_VERSION, SHUTDOWN_GRACE, TOOLS_LIST_ID,
};

/// The request the probe was waiting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStage {
    Initialize,
    ToolsList,
}

impl ProbeStage {
    pub fn method(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::ToolsList => "tools/list",
        }
    }
}

/// A completed handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeTimings {
    /// From spawn to the `initialize` response.
    pub initialize: Duration,
    /// From spawn to the `tools/list` response: what a client's startup
    /// timeout covers.
    pub total: Duration,
    /// Tools the server listed.
    pub tool_count: usize,
}

/// Why the handshake did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    /// The command could not be started.
    Spawn(String),
    /// The server closed its output before answering.
    Exited { stage: ProbeStage },
    /// No answer before the deadline.
    TimedOut { stage: ProbeStage, waited: Duration },
    /// The server answered with an error or an unusable response.
    Protocol { stage: ProbeStage, detail: String },
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "could not start the server: {e}"),
            Self::Exited { stage } => {
                write!(f, "the server exited before answering {}", stage.method())
            }
            Self::TimedOut { stage, waited } => write!(
                f,
                "no answer to {} within {:.1} s",
                stage.method(),
                waited.as_secs_f64()
            ),
            Self::Protocol { stage, detail } => {
                write!(f, "{} failed: {detail}", stage.method())
            }
        }
    }
}

/// Kills and reaps the child on every exit path.
struct ChildGuard(Child);

impl ChildGuard {
    /// Close stdin and give the server [`SHUTDOWN_GRACE`] to exit on its own.
    fn shut_down(&mut self) {
        drop(self.0.stdin.take());
        let started = Instant::now();
        while started.elapsed() < SHUTDOWN_GRACE {
            match self.0.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => std::thread::sleep(EXIT_POLL_INTERVAL),
            }
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

/// Launch `command args` in `cwd` with `envs` added, send `initialize`, the
/// `initialized` notification and `tools/list`, and time the answers.
/// Never runs longer than `deadline` (capped at [`MAX_PROBE_DURATION`]) plus
/// the shutdown grace.
pub fn probe_stdio_server(
    command: &Path,
    args: &[String],
    cwd: &Path,
    envs: &[(String, String)],
    deadline: Duration,
) -> Result<HandshakeTimings, ProbeError> {
    let deadline = deadline.min(MAX_PROBE_DURATION);
    let mut child = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .envs(envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ProbeError::Spawn(e.to_string()))?;
    let stdout = child.stdout.take();
    let mut guard = ChildGuard(child);
    let Some(stdout) = stdout else {
        return Err(ProbeError::Spawn("no stdout pipe".into()));
    };

    let (tx, rx) = mpsc::channel::<String>();
    // Detached on purpose: it ends when the pipe closes or `rx` is dropped.
    // Joining could block on a grandchild that inherited the pipe.
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout.take(MAX_PROBE_OUTPUT_BYTES));
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let result = run_handshake(&mut guard, &rx, deadline);
    guard.shut_down();
    result
}

fn run_handshake(
    guard: &mut ChildGuard,
    rx: &Receiver<String>,
    deadline: Duration,
) -> Result<HandshakeTimings, ProbeError> {
    let start = Instant::now();
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": INITIALIZE_ID,
        "method": "initialize",
        "params": {
            "protocolVersion": PROBE_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": PROBE_CLIENT_NAME, "version": env!("CARGO_PKG_VERSION")},
        },
    });
    send(guard, &initialize, ProbeStage::Initialize)?;
    await_response(rx, INITIALIZE_ID, ProbeStage::Initialize, start, deadline)?;
    let initialize_elapsed = start.elapsed();

    send(
        guard,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        ProbeStage::ToolsList,
    )?;
    send(
        guard,
        &json!({"jsonrpc": "2.0", "id": TOOLS_LIST_ID, "method": "tools/list"}),
        ProbeStage::ToolsList,
    )?;
    let tools = await_response(rx, TOOLS_LIST_ID, ProbeStage::ToolsList, start, deadline)?;
    let tool_count = tools
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    Ok(HandshakeTimings {
        initialize: initialize_elapsed,
        total: start.elapsed(),
        tool_count,
    })
}

fn send(guard: &mut ChildGuard, message: &Value, stage: ProbeStage) -> Result<(), ProbeError> {
    let Some(stdin) = guard.0.stdin.as_mut() else {
        return Err(ProbeError::Exited { stage });
    };
    writeln!(stdin, "{message}")
        .and_then(|()| stdin.flush())
        .map_err(|_| ProbeError::Exited { stage })
}

/// Wait for the response carrying `id`, skipping notifications and any
/// non-JSON output.
fn await_response(
    rx: &Receiver<String>,
    id: u64,
    stage: ProbeStage,
    start: Instant,
    deadline: Duration,
) -> Result<Value, ProbeError> {
    loop {
        let remaining = deadline.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(ProbeError::TimedOut {
                stage,
                waited: start.elapsed(),
            });
        }
        let line = match rx.recv_timeout(remaining) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                return Err(ProbeError::TimedOut {
                    stage,
                    waited: start.elapsed(),
                })
            }
            Err(RecvTimeoutError::Disconnected) => return Err(ProbeError::Exited { stage }),
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error response")
                .to_string();
            return Err(ProbeError::Protocol { stage, detail });
        }
        if message.get("result").is_none() {
            return Err(ProbeError::Protocol {
                stage,
                detail: "response has no result".into(),
            });
        }
        return Ok(message);
    }
}
