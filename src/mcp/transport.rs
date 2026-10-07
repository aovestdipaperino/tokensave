//! JSON-RPC 2.0 transport types for the MCP server.
//!
//! Provides serialization and deserialization of JSON-RPC 2.0 messages
//! used to communicate between the MCP client and server over stdio.

use serde::{Deserialize, Serialize};

/// A JSON-RPC 2.0 request received from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Protocol version; must be `"2.0"`.
    pub jsonrpc: String,
    /// Request identifier. May be a number, string, or null.
    /// Absent for notifications.
    #[serde(default)]
    pub id: serde_json::Value,
    /// The RPC method name.
    pub method: String,
    /// Optional parameters for the method.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Returns true if this message is a JSON-RPC notification.
    ///
    /// Per JSON-RPC 2.0 §4.1, a notification is a request without an `id` member
    /// (deserialized as `Value::Null`), or an MCP notification method (e.g. prefixed
    /// with `"notifications/"` or `"initialized"`). Servers must never send a response
    /// to notifications.
    #[must_use]
    pub fn is_notification(&self) -> bool {
        self.id.is_null()
            || self.method.starts_with("notifications/")
            || self.method == "initialized"
    }
}

/// A JSON-RPC 2.0 response sent back to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Protocol version; always `"2.0"`.
    pub jsonrpc: String,
    /// The request identifier that this response corresponds to.
    pub id: serde_json::Value,
    /// The result on success; absent on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The error on failure; absent on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Creates a successful JSON-RPC response.
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error JSON-RPC response.
    pub fn error(id: serde_json::Value, code: ErrorCode, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: code.as_i32(),
                message,
                data: None,
            }),
        }
    }
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Standard JSON-RPC 2.0 error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Invalid JSON was received.
    ParseError,
    /// The request is not a valid JSON-RPC request.
    InvalidRequest,
    /// The requested method does not exist.
    MethodNotFound,
    /// Invalid method parameters.
    InvalidParams,
    /// Internal server error.
    InternalError,
}

impl ErrorCode {
    /// Returns the numeric error code as defined by JSON-RPC 2.0.
    pub fn as_i32(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
        }
    }
}

// ---------------------------------------------------------------------------
// Transport abstraction (zero-cost via monomorphization)
// ---------------------------------------------------------------------------

/// Async line-oriented transport for JSON-RPC messages.
///
/// Implementations are monomorphized at each call site — no dyn dispatch.
pub trait McpTransport {
    /// Read the next line from the transport. Returns `None` on EOF.
    fn read_line(
        &mut self,
    ) -> impl std::future::Future<Output = std::io::Result<Option<String>>> + Send;

    /// Write a complete line (including trailing newline) to the transport.
    fn write_line(
        &mut self,
        line: &str,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Flush any buffered output.
    fn flush(&mut self) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

/// Longest request line the stdio transport buffers, in bytes (#636).
///
/// Generous enough for any real request (the largest are `tokensave_*_edit`
/// payloads carrying file content), but bounded so a runaway or hostile
/// host cannot make the server buffer an unterminated line until it runs out
/// of memory.
pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

/// Reads newline-terminated lines with a per-line size cap.
///
/// A line longer than the cap is discarded through its newline and reported
/// as a JSON string literal naming the limit: it is valid JSON but not a
/// request, so the server answers it with a parse error and keeps serving.
///
/// All partial-line state lives in the struct and every await point is
/// `fill_buf`, so [`Self::next_line`] is cancel-safe like
/// [`tokio::io::Lines::next_line`] — the server races it in `select!`.
pub struct BoundedLines<R> {
    reader: R,
    buf: Vec<u8>,
    max: usize,
    discarding: bool,
}

impl<R: tokio::io::AsyncBufRead + Unpin> BoundedLines<R> {
    pub fn new(reader: R, max: usize) -> Self {
        Self {
            reader,
            buf: Vec::new(),
            max,
            discarding: false,
        }
    }

    pub async fn next_line(&mut self) -> std::io::Result<Option<String>> {
        use tokio::io::AsyncBufReadExt;
        loop {
            let chunk = self.reader.fill_buf().await?;
            if chunk.is_empty() {
                // EOF: flush a final unterminated line, as `Lines` does.
                if self.discarding {
                    self.discarding = false;
                    return Ok(Some(self.oversize_line()));
                }
                if self.buf.is_empty() {
                    return Ok(None);
                }
                return self.take_line().map(Some);
            }
            let newline = chunk.iter().position(|&b| b == b'\n');
            let take = newline.map_or(chunk.len(), |i| i + 1);
            if !self.discarding {
                let room = self.max.saturating_sub(self.buf.len());
                let body = newline.unwrap_or(chunk.len());
                if body > room {
                    self.buf.clear();
                    self.discarding = true;
                } else {
                    self.buf.extend_from_slice(&chunk[..take]);
                }
            }
            self.reader.consume(take);
            if newline.is_some() {
                if self.discarding {
                    self.discarding = false;
                    return Ok(Some(self.oversize_line()));
                }
                return self.take_line().map(Some);
            }
        }
    }

    fn take_line(&mut self) -> std::io::Result<String> {
        let mut bytes = std::mem::take(&mut self.buf);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        String::from_utf8(bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn oversize_line(&self) -> String {
        format!("\"request line exceeds the {} byte limit\"", self.max)
    }
}

/// Real stdio transport — reads from stdin, writes to stdout.
pub struct StdioTransport {
    reader: BoundedLines<tokio::io::BufReader<tokio::io::Stdin>>,
    writer: tokio::io::Stdout,
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self {
            reader: BoundedLines::new(
                tokio::io::BufReader::new(tokio::io::stdin()),
                MAX_LINE_BYTES,
            ),
            writer: tokio::io::stdout(),
        }
    }
}

impl StdioTransport {
    pub fn new() -> Self {
        Self::default()
    }
}

impl McpTransport for StdioTransport {
    async fn read_line(&mut self) -> std::io::Result<Option<String>> {
        self.reader.next_line().await
    }

    async fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.writer.write_all(line.as_bytes()).await
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        self.writer.flush().await
    }
}

/// In-memory transport for tests — backed by tokio mpsc channels.
#[cfg(any(test, feature = "test-transport"))]
pub struct ChannelTransport {
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
}

#[cfg(any(test, feature = "test-transport"))]
impl ChannelTransport {
    /// Create a transport and the handles needed by test code.
    ///
    /// Returns `(transport, sender_to_server, receiver_from_server)`.
    pub fn new() -> (
        Self,
        tokio::sync::mpsc::UnboundedSender<String>,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                rx: input_rx,
                tx: output_tx,
            },
            input_tx,
            output_rx,
        )
    }
}

#[cfg(any(test, feature = "test-transport"))]
impl McpTransport for ChannelTransport {
    async fn read_line(&mut self) -> std::io::Result<Option<String>> {
        Ok(self.rx.recv().await)
    }

    // Awaits nothing — a channel send is immediate — but the signature is the
    // trait's, not ours to narrow.
    #[allow(clippy::unused_async_trait_impl)]
    async fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.tx
            .send(line.to_string())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()))
    }

    #[allow(clippy::unused_async_trait_impl)]
    async fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_jsonrpc_request() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });

        let request: JsonRpcRequest = serde_json::from_value(msg).unwrap();
        assert_eq!(request.method, "tools/list");
        assert_eq!(request.id, serde_json::Value::Number(1.into()));
    }

    #[test]
    fn test_parse_notification_without_id() {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let request: JsonRpcRequest = serde_json::from_value(msg).unwrap();
        assert_eq!(request.method, "initialized");
        assert!(request.id.is_null());
        assert!(request.params.is_none());
    }

    #[test]
    fn test_serialize_success_response() {
        let response =
            JsonRpcResponse::success(serde_json::Value::Number(1.into()), json!({"tools": []}));

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"tools\":[]"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_serialize_error_response() {
        let response = JsonRpcResponse::error(
            serde_json::Value::Number(1.into()),
            ErrorCode::MethodNotFound,
            "Method not found".to_string(),
        );

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("-32601"));
        assert!(json.contains("Method not found"));
        assert!(!json.contains("\"result\""));
    }

    async fn read_all(input: &[u8], max: usize) -> Vec<String> {
        let mut lines = BoundedLines::new(tokio::io::BufReader::with_capacity(4, input), max);
        let mut out = Vec::new();
        while let Some(line) = lines.next_line().await.unwrap() {
            out.push(line);
        }
        out
    }

    #[tokio::test]
    async fn bounded_lines_splits_like_lines() {
        let got = read_all(b"{\"a\":1}\r\n\nsecond line\nlast", 64).await;
        assert_eq!(got, vec!["{\"a\":1}", "", "second line", "last"]);
    }

    #[tokio::test]
    async fn bounded_lines_drops_an_oversized_line_and_keeps_reading() {
        let got = read_all(b"ok\n0123456789abcdef\nafter\n0123456789abcdef", 10).await;
        assert_eq!(got.len(), 4);
        assert_eq!(got[0], "ok");
        assert_eq!(got[2], "after");
        for oversized in [&got[1], &got[3]] {
            let v: serde_json::Value = serde_json::from_str(oversized).unwrap();
            assert!(v.as_str().unwrap().contains("10 byte limit"), "{oversized}");
            assert!(serde_json::from_str::<JsonRpcRequest>(oversized).is_err());
        }
    }

    #[tokio::test]
    async fn bounded_lines_accepts_a_line_exactly_at_the_limit() {
        let got = read_all(b"0123456789\n", 10).await;
        assert_eq!(got, vec!["0123456789"]);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(ErrorCode::ParseError.as_i32(), -32700);
        assert_eq!(ErrorCode::InvalidRequest.as_i32(), -32600);
        assert_eq!(ErrorCode::MethodNotFound.as_i32(), -32601);
        assert_eq!(ErrorCode::InvalidParams.as_i32(), -32602);
        assert_eq!(ErrorCode::InternalError.as_i32(), -32603);
    }

    #[test]
    fn test_request_with_string_id() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": "abc-123",
            "method": "ping"
        });

        let request: JsonRpcRequest = serde_json::from_value(msg).unwrap();
        assert_eq!(request.id, serde_json::Value::String("abc-123".to_string()));
    }

    #[test]
    fn test_is_notification() {
        let req_with_num_id = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(1.into()),
            method: "tools/list".to_string(),
            params: None,
        };
        assert!(!req_with_num_id.is_notification());

        let req_with_str_id = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::String("req-1".to_string()),
            method: "tools/list".to_string(),
            params: None,
        };
        assert!(!req_with_str_id.is_notification());

        let notif_null_id = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Null,
            method: "notifications/roots/list_changed".to_string(),
            params: None,
        };
        assert!(notif_null_id.is_notification());

        let notif_custom_method = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Null,
            method: "custom/event".to_string(),
            params: None,
        };
        assert!(notif_custom_method.is_notification());

        let notif_with_id = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(2.into()),
            method: "notifications/roots/list_changed".to_string(),
            params: None,
        };
        assert!(notif_with_id.is_notification());

        let notif_initialized = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Null,
            method: "initialized".to_string(),
            params: None,
        };
        assert!(notif_initialized.is_notification());
    }
}
