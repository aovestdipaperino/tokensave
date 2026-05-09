// Rust guideline compliant 2025-10-17
//! JSON-RPC 2.0 transport for an LSP server child process.
//!
//! Each `LspClient` owns a child process and two background tokio tasks: a
//! reader that pulls `Content-Length`-framed messages off the server's stdout
//! and routes them to the right pending request, and a writer that ships
//! request/notification frames to the server's stdin.
//!
//! Requests carry a sequential `i64` id and complete via a `oneshot` channel.
//! Notifications (no id) are fire-and-forget. Server-initiated requests are
//! ignored — tokensave plays the strict-client role and doesn't implement
//! `window/workDoneProgress/create` or similar.
//!
//! The client is async only on the public surface; framing and JSON
//! serialisation happen synchronously in the per-task hot loops.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;

use crate::errors::{Result, TokenSaveError};

/// Default per-request timeout. Tuned for `textDocument/definition` against
/// rust-analyzer / gopls; the resolver layer overrides this for slower
/// requests (initialize, didOpen for very large files).
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Bound on the writer task's input queue. 256 frames in flight per server
/// comfortably covers the bursty `didOpen` + per-ref `definition` pattern.
const WRITER_CHANNEL_CAPACITY: usize = 256;

/// Outbound frame the writer task ships to the server's stdin.
///
/// `id` on `Request` is unused on the wire (it's already in `payload`) but
/// kept so future routing logic — retries, instrumentation, dropping a
/// pending request on cancel — can correlate frames without re-parsing JSON.
enum OutboundFrame {
    /// Ordinary request: id is set, response goes to the oneshot.
    Request {
        #[allow(dead_code)]
        id: i64,
        payload: Value,
    },
    /// Notification: no id, fire-and-forget.
    Notification { payload: Value },
}

/// Async-friendly LSP JSON-RPC client over stdin/stdout.
pub struct LspClient {
    /// Child handle so we can `kill` and `wait` on shutdown.
    child: Mutex<Child>,
    /// Sequential id counter for requests.
    next_id: AtomicI64,
    /// Map of pending request ids to the oneshot that will receive the
    /// response value. Inserted before send, removed on response (or timeout).
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>,
    /// Sender into the writer task. Cloned per call.
    outbound: mpsc::Sender<OutboundFrame>,
    /// Configurable timeout per request.
    request_timeout: Duration,
}

impl LspClient {
    /// Spawn `command` and connect a JSON-RPC client to its stdin/stdout.
    /// Stderr is inherited so server diagnostics surface in the tokensave log.
    pub async fn spawn(mut command: Command) -> Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        let mut child = command.spawn().map_err(|e| TokenSaveError::Config {
            message: format!("failed to spawn LSP server: {e}"),
        })?;

        let stdin = child.stdin.take().ok_or_else(|| TokenSaveError::Config {
            message: "LSP server stdin not piped".to_string(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| TokenSaveError::Config {
            message: "LSP server stdout not piped".to_string(),
        })?;

        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = mpsc::channel::<OutboundFrame>(WRITER_CHANNEL_CAPACITY);

        let pending_for_reader = Arc::clone(&pending);
        tokio::spawn(reader_loop(stdout, pending_for_reader));
        tokio::spawn(writer_loop(stdin, rx));

        Ok(Self {
            child: Mutex::new(child),
            next_id: AtomicI64::new(1),
            pending,
            outbound: tx,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        })
    }

    /// Override the per-request timeout (default: 5s).
    pub fn with_request_timeout(mut self, dur: Duration) -> Self {
        self.request_timeout = dur;
        self
    }

    /// Send a JSON-RPC request and await the typed response.
    ///
    /// Cleanup invariants:
    /// - Param serialisation runs before the pending entry is inserted, so a
    ///   serialise failure can never leak a map entry.
    /// - Send / timeout / oneshot-drop failures all remove the pending entry
    ///   inline before returning, so late responses see a stale id and
    ///   simply get ignored by the reader.
    pub async fn request<P, R>(&self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        // Serialise first — failure here returns before any state changes.
        let params_value = serde_json::to_value(params).map_err(|e| TokenSaveError::Config {
            message: format!("serialise params for {method}: {e}"),
        })?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params_value,
        });

        if let Err(_send_err) = self
            .outbound
            .send(OutboundFrame::Request { id, payload })
            .await
        {
            // Writer is dead. Reclaim the pending entry so the map doesn't
            // accumulate orphans across the LspClient's lifetime.
            self.pending.lock().await.remove(&id);
            return Err(TokenSaveError::Config {
                message: format!("LSP writer channel closed before send of {method}"),
            });
        }

        let resolved = match timeout(self.request_timeout, rx).await {
            Ok(Ok(value)) => value?,
            Ok(Err(_)) => {
                // The reader dropped the Sender — typically means stdout closed.
                self.pending.lock().await.remove(&id);
                return Err(TokenSaveError::Config {
                    message: format!("LSP response oneshot dropped for '{method}'"),
                });
            }
            Err(_) => {
                // Timeout. Inline cleanup avoids the spawn-vs-reader race.
                self.pending.lock().await.remove(&id);
                return Err(TokenSaveError::Config {
                    message: format!("LSP request '{method}' timed out"),
                });
            }
        };

        serde_json::from_value::<R>(resolved).map_err(|e| TokenSaveError::Config {
            message: format!("LSP response deserialise for '{method}' failed: {e}"),
        })
    }

    /// Send a JSON-RPC notification (no response expected).
    pub async fn notify<P>(&self, method: &str, params: P) -> Result<()>
    where
        P: Serialize,
    {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": serde_json::to_value(params).map_err(|e| TokenSaveError::Config {
                message: format!("serialise notification {method}: {e}"),
            })?,
        });
        self.outbound
            .send(OutboundFrame::Notification { payload })
            .await
            .map_err(|_| TokenSaveError::Config {
                message: format!("LSP writer channel closed before send of {method}"),
            })?;
        Ok(())
    }

    /// Send `shutdown` + `exit` and wait up to `wait_for` for the child.
    /// Falls back to `SIGKILL` (`Child::kill`) on timeout.
    pub async fn shutdown(&self, wait_for: Duration) -> Result<()> {
        // Best-effort — a server that's already crashed will simply error.
        let _ = self.request::<(), Value>("shutdown", ()).await;
        let _ = self.notify("exit", ()).await;

        let mut child = self.child.lock().await;
        match timeout(wait_for, child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(TokenSaveError::Config {
                message: format!("LSP child wait failed: {e}"),
            }),
            Err(_) => {
                let _ = child.kill().await;
                Ok(())
            }
        }
    }
}

/// Reads `Content-Length`-framed messages from the server's stdout and
/// dispatches responses to their pending oneshot senders.
async fn reader_loop(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(Some(f)) => f,
            Ok(None) => break, // server closed stdout — exit cleanly
            Err(_) => break,
        };

        let value: Value = match serde_json::from_slice(&frame) {
            Ok(v) => v,
            Err(_) => continue, // unparseable; drop and keep reading
        };

        // Server-initiated request? Spec requires us to respond with an
        // error, but tokensave's resolver doesn't implement any reverse
        // API. Drop and move on — the server will time out its own request.
        if value.get("method").is_some() && value.get("id").is_some() {
            continue;
        }

        // Server notification? Likewise ignored.
        if value.get("method").is_some() {
            continue;
        }

        // Response. Match on id and route to the pending oneshot.
        let Some(id) = value.get("id").and_then(|v| v.as_i64()) else {
            continue;
        };

        let tx = {
            let mut map = pending.lock().await;
            map.remove(&id)
        };
        let Some(tx) = tx else {
            continue; // already timed out or unknown id
        };

        if let Some(error) = value.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown LSP error")
                .to_string();
            let _ = tx.send(Err(TokenSaveError::Config { message: msg }));
        } else {
            let result = value.get("result").cloned().unwrap_or(Value::Null);
            let _ = tx.send(Ok(result));
        }
    }
}

/// Writes outbound JSON-RPC frames to the server's stdin with the LSP
/// `Content-Length: <N>\r\n\r\n<body>` framing.
async fn writer_loop(stdin: ChildStdin, mut rx: mpsc::Receiver<OutboundFrame>) {
    let mut stdin = stdin;
    while let Some(frame) = rx.recv().await {
        let payload = match frame {
            OutboundFrame::Request { payload, .. } => payload,
            OutboundFrame::Notification { payload } => payload,
        };
        let body = match serde_json::to_vec(&payload) {
            Ok(b) => b,
            Err(_) => continue, // serialisation failure: skip
        };
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        if stdin.write_all(header.as_bytes()).await.is_err() {
            break;
        }
        if stdin.write_all(&body).await.is_err() {
            break;
        }
        if stdin.flush().await.is_err() {
            break;
        }
    }
}

/// Reads one `Content-Length`-framed message from `reader`. Returns `Ok(None)`
/// on clean EOF (server closed stdout). Other I/O errors propagate.
async fn read_frame<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut content_length: Option<usize> = None;

    // Header loop. Each header line ends with CRLF; an empty line terminates.
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| TokenSaveError::Config {
                message: format!("read LSP header line: {e}"),
            })?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            let v: usize = rest.trim().parse().map_err(|_| TokenSaveError::Config {
                message: format!("invalid Content-Length: {rest}"),
            })?;
            content_length = Some(v);
        }
        // Other headers (Content-Type) are ignored — the spec allows it.
    }

    let len = content_length.ok_or_else(|| TokenSaveError::Config {
        message: "LSP frame missing Content-Length".to_string(),
    })?;
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|e| TokenSaveError::Config {
            message: format!("read LSP frame body: {e}"),
        })?;
    Ok(Some(buf))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Encodes a JSON value into the LSP `Content-Length: N\r\n\r\nBODY`
    /// framing. Used by tests that simulate a server's output.
    fn encode_frame(v: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(v).expect("serialise");
        let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        out.extend_from_slice(&body);
        out
    }

    #[tokio::test]
    async fn read_frame_decodes_single_message() {
        let v = json!({"jsonrpc": "2.0", "id": 1, "result": {"ok": true}});
        let bytes = encode_frame(&v);
        let cursor = std::io::Cursor::new(bytes);
        let mut reader = BufReader::new(cursor);
        let frame = read_frame(&mut reader).await.unwrap().unwrap();
        let decoded: Value = serde_json::from_slice(&frame).unwrap();
        assert_eq!(decoded, v);
    }

    #[tokio::test]
    async fn read_frame_decodes_back_to_back_messages() {
        let a = json!({"jsonrpc": "2.0", "id": 1, "result": "first"});
        let b = json!({"jsonrpc": "2.0", "id": 2, "result": "second"});
        let mut bytes = encode_frame(&a);
        bytes.extend(encode_frame(&b));
        let cursor = std::io::Cursor::new(bytes);
        let mut reader = BufReader::new(cursor);

        let f1 = read_frame(&mut reader).await.unwrap().unwrap();
        let f2 = read_frame(&mut reader).await.unwrap().unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&f1).unwrap(), a);
        assert_eq!(serde_json::from_slice::<Value>(&f2).unwrap(), b);
    }

    #[tokio::test]
    async fn read_frame_clean_eof_returns_none() {
        let cursor = std::io::Cursor::new(Vec::<u8>::new());
        let mut reader = BufReader::new(cursor);
        assert!(read_frame(&mut reader).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn read_frame_ignores_unknown_headers() {
        let v = json!({"jsonrpc": "2.0", "id": 7, "result": null});
        let body = serde_json::to_vec(&v).unwrap();
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             \r\n",
            body.len()
        );
        let mut bytes = raw.into_bytes();
        bytes.extend(body);
        let cursor = std::io::Cursor::new(bytes);
        let mut reader = BufReader::new(cursor);
        let frame = read_frame(&mut reader).await.unwrap().unwrap();
        let decoded: Value = serde_json::from_slice(&frame).unwrap();
        assert_eq!(decoded["id"], 7);
    }

    #[tokio::test]
    async fn read_frame_errors_when_content_length_missing() {
        let body = b"{}";
        let mut bytes = b"Content-Type: application/json\r\n\r\n".to_vec();
        bytes.extend_from_slice(body);
        let cursor = std::io::Cursor::new(bytes);
        let mut reader = BufReader::new(cursor);
        assert!(read_frame(&mut reader).await.is_err());
    }
}
