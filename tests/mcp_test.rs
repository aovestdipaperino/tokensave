use codegraph::mcp::tools::*;
use codegraph::mcp::transport::*;
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
fn test_tool_definitions() {
    let tools = get_tool_definitions();
    assert!(!tools.is_empty());

    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"codegraph_search"));
    assert!(tool_names.contains(&"codegraph_context"));
    assert!(tool_names.contains(&"codegraph_callers"));
    assert!(tool_names.contains(&"codegraph_callees"));
    assert!(tool_names.contains(&"codegraph_impact"));
    assert!(tool_names.contains(&"codegraph_node"));
    assert!(tool_names.contains(&"codegraph_status"));
}

#[test]
fn test_serialize_jsonrpc_response() {
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(1.into()),
        result: Some(json!({"tools": []})),
        error: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
}

#[test]
fn test_error_response() {
    let response = JsonRpcResponse::error(
        serde_json::Value::Number(1.into()),
        ErrorCode::MethodNotFound,
        "Method not found".to_string(),
    );

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("-32601"));
}

#[test]
fn test_success_response_omits_error() {
    let response = JsonRpcResponse::success(
        serde_json::Value::Number(42.into()),
        json!({"result": "ok"}),
    );

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"result\""));
    assert!(!json.contains("\"error\""));
}

#[test]
fn test_error_response_omits_result() {
    let response = JsonRpcResponse::error(
        serde_json::Value::Number(1.into()),
        ErrorCode::InternalError,
        "something went wrong".to_string(),
    );

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("-32603"));
    assert!(!json.contains("\"result\""));
}

#[test]
fn test_all_error_codes() {
    assert_eq!(ErrorCode::ParseError.as_i32(), -32700);
    assert_eq!(ErrorCode::InvalidRequest.as_i32(), -32600);
    assert_eq!(ErrorCode::MethodNotFound.as_i32(), -32601);
    assert_eq!(ErrorCode::InvalidParams.as_i32(), -32602);
    assert_eq!(ErrorCode::InternalError.as_i32(), -32603);
}

#[test]
fn test_tool_definitions_count() {
    let tools = get_tool_definitions();
    assert_eq!(tools.len(), 7);
}

#[test]
fn test_tool_definitions_have_input_schemas() {
    let tools = get_tool_definitions();
    for tool in &tools {
        assert!(
            tool.input_schema.is_object(),
            "tool '{}' has no input schema",
            tool.name
        );
        assert_eq!(
            tool.input_schema["type"], "object",
            "tool '{}' schema type is not object",
            tool.name
        );
    }
}

#[test]
fn test_tool_definitions_serialization_roundtrip() {
    let tools = get_tool_definitions();
    let json = serde_json::to_string(&tools).unwrap();
    let deserialized: Vec<ToolDefinition> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.len(), tools.len());
    for (orig, deser) in tools.iter().zip(deserialized.iter()) {
        assert_eq!(orig.name, deser.name);
        assert_eq!(orig.description, deser.description);
    }
}

#[test]
fn test_notification_without_id() {
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
fn test_request_with_string_id() {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": "req-42",
        "method": "ping"
    });

    let request: JsonRpcRequest = serde_json::from_value(msg).unwrap();
    assert_eq!(request.id, serde_json::Value::String("req-42".to_string()));
    assert_eq!(request.method, "ping");
}
