//! File editing tool handlers: `str_replace`, `multi_str_replace`, `insert_at`,
//! `ast_grep_rewrite`.

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use super::super::ToolResult;

pub(super) async fn handle_str_replace(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;

    let old_str = args
        .get("old_str")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: old_str".to_string(),
        })?;

    let new_str = args
        .get("new_str")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: new_str".to_string(),
        })?;

    let result = cg.str_replace(path, old_str, new_str).await?;
    let touched_files = vec![result.file_path.clone()];
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_multi_str_replace(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;

    let replacements = args
        .get("replacements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: replacements".to_string(),
        })?;

    let parsed_replacements: Vec<(&str, &str)> = replacements
        .iter()
        .filter_map(|pair| {
            let arr = pair.as_array()?;
            if arr.len() != 2 {
                return None;
            }
            let old = arr[0].as_str()?;
            let new = arr[1].as_str()?;
            Some((old, new))
        })
        .collect();

    if parsed_replacements.len() != replacements.len() {
        return Err(TokenSaveError::Config {
            message: "each replacement must be an array of exactly 2 strings".to_string(),
        });
    }

    let result = cg.multi_str_replace(path, &parsed_replacements).await?;
    let touched_files = vec![result.file_path.clone()];
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_insert_at(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;

    let anchor =
        args.get("anchor")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: anchor".to_string(),
            })?;

    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: content".to_string(),
        })?;

    let before = args
        .get("before")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let result = cg.insert_at(path, anchor, content, before).await?;
    let touched_files = vec![result.file_path.clone()];
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_ast_grep_rewrite(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;

    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: pattern".to_string(),
        })?;

    let rewrite = args
        .get("rewrite")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: rewrite".to_string(),
        })?;

    let result = cg.ast_grep_rewrite(path, pattern, rewrite).await?;
    let touched_files = if result.success {
        vec![result.file_path.clone()]
    } else {
        vec![]
    };
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]
        }),
        touched_files,
    })
}
