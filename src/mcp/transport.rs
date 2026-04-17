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

/// Real stdio transport — reads from stdin, writes to stdout.
pub struct StdioTransport {
    reader: tokio::io::Lines<tokio::io::BufReader<tokio::io::Stdin>>,
    writer: tokio::io::Stdout,
}

impl Default for StdioTransport {
    fn default() -> Self {
        use tokio::io::AsyncBufReadExt;
        Self {
            reader: tokio::io::BufReader::new(tokio::io::stdin()).lines(),
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

    async fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.tx
            .send(line.to_string())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()))
    }

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
}
