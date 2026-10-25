//! Cross-session memory handlers: `record_decision`, `record_code_area`, `session_recall`.

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use super::super::ToolResult;
use super::truncate_response;

pub(super) async fn handle_record_decision(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: text".to_string(),
        })?;
    let reason = args.get("reason").and_then(|v| v.as_str());
    let files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let tags: Vec<String> = args
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let id = cg.record_decision(text, reason, &files, &tags).await?;
    let out = json!({ "id": id, "status": "recorded" });
    let formatted = serde_json::to_string_pretty(&out).unwrap_or_default();
    Ok(ToolResult {
        value: json!({ "content": [{ "type": "text", "text": truncate_response(&formatted) }] }),
        touched_files: vec![],
    })
}

pub(super) async fn handle_record_code_area(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;
    let description = args.get("description").and_then(|v| v.as_str());

    cg.record_code_area(path, description).await?;
    let out = json!({ "path": path, "status": "recorded" });
    let formatted = serde_json::to_string_pretty(&out).unwrap_or_default();
    Ok(ToolResult {
        value: json!({ "content": [{ "type": "text", "text": truncate_response(&formatted) }] }),
        touched_files: vec![],
    })
}

pub(super) async fn handle_session_recall(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let query = args.get("query").and_then(|v| v.as_str());
    let since = args.get("since").and_then(serde_json::Value::as_i64);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |n| (n as usize).clamp(1, 200));
    let include_areas = args
        .get("include_code_areas")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let decisions = cg.session_recall(query, since, limit).await?;
    let mut out = json!({ "decisions": decisions });
    if include_areas {
        let areas = cg.list_code_areas(limit).await?;
        out["code_areas"] = serde_json::to_value(&areas).unwrap_or(json!([]));
    }
    let formatted = serde_json::to_string_pretty(&out).unwrap_or_default();
    Ok(ToolResult {
        value: json!({ "content": [{ "type": "text", "text": truncate_response(&formatted) }] }),
        touched_files: vec![],
    })
}
