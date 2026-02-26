//! MCP (Model Context Protocol) server for the code graph.
//!
//! Provides a JSON-RPC 2.0 interface over stdio so that AI assistants can
//! query the code graph interactively. Exposes tools for searching, context
//! building, call graph traversal, impact analysis, and more.

/// MCP server implementation.
pub mod server;

/// Tool definitions and dispatch.
pub mod tools;

/// JSON-RPC 2.0 transport types.
pub mod transport;

pub use server::McpServer;
pub use tools::{get_tool_definitions, handle_tool_call, ToolDefinition};
pub use transport::{ErrorCode, JsonRpcError, JsonRpcRequest, JsonRpcResponse};
