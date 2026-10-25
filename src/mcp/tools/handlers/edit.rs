//! File editing tool handlers: `str_replace`, `multi_str_replace`, `insert_at`,
//! `ast_grep_rewrite`.

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use super::super::ToolResult;

/// Extracts the optional `project_root` (alias: `cwd`) parameter shared by
/// every edit tool. When present, it retargets resolution of a *relative*
/// `path`/symbol-file argument to this directory instead of the indexed
/// project root — the fix for callers working in a git worktree, where a
/// bare relative path previously always resolved against the primary
/// checkout regardless of where the caller was actually working.
fn project_root_arg(args: &Value) -> Option<&str> {
    args.get("project_root")
        .or_else(|| args.get("cwd"))
        .and_then(|v| v.as_str())
}

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

    let echo = args
        .get("echo")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let root_override = project_root_arg(&args);
    let result = cg
        .str_replace(path, old_str, new_str, root_override)
        .await?;
    let touched_files = vec![result.file_path.clone()];
    let mut value = json!({
        "ok": result.success,
        "file": path,
    });
    if result.success {
        value["lines"] = json!([result.changed_lines.0, result.changed_lines.1]);
        value["digest"] = json!(result.digest);
        if echo {
            value["matched_str"] = json!(result.matched_str);
            value["new_str"] = json!(result.new_str);
        }
    } else {
        value["message"] = json!(result.message);
        // A "write landed but reindex failed" result carries the post-edit
        // digest so the caller can verify the file instead of retrying (#563).
        if !result.digest.is_empty() {
            value["digest"] = json!(result.digest);
        }
        if let Some(nearest) = result.nearest {
            value["nearest"] = json!(nearest);
        }
    }
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
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

    let root_override = project_root_arg(&args);
    let result = cg
        .multi_str_replace(path, &parsed_replacements, root_override)
        .await?;
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

    let echo = args
        .get("echo")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let root_override = project_root_arg(&args);
    let result = cg
        .insert_at(path, anchor, content, before, root_override)
        .await?;
    let touched_files = vec![result.file_path.clone()];
    let mut value = json!({
        "ok": result.success,
        "file": result.file_path,
    });
    if result.success {
        value["lines"] = json!([result.changed_lines.0, result.changed_lines.1]);
        value["digest"] = json!(result.digest);
        if echo {
            value["content"] = json!(result.content);
        }
    } else {
        value["message"] = json!(result.message);
        if let Some(nearest) = result.nearest {
            value["nearest"] = json!(nearest);
        }
    }
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_delete_symbol(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let symbol =
        args.get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: symbol".to_string(),
            })?;
    let include_doc_comment = args
        .get("include_doc_comment")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let root_override = project_root_arg(&args);
    let result = cg
        .delete_symbol(symbol, include_doc_comment, root_override)
        .await?;
    let touched_files = if result.success {
        vec![result.file_path.clone()]
    } else {
        vec![]
    };
    let mut value = json!({ "ok": result.success, "file": result.file_path });
    if result.success {
        value["lines"] = json!([result.changed_lines.0, result.changed_lines.1]);
        value["digest"] = json!(result.digest);
    } else {
        value["message"] = json!(result.message);
    }
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_replace_lines(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: path".to_string(),
        })?;
    let start = args
        .get("start")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: start".to_string(),
        })? as u32;
    let end = args
        .get("end")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: end".to_string(),
        })? as u32;
    let new_content = args
        .get("new_content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: new_content".to_string(),
        })?;
    let expected_digest = args.get("expected_digest").and_then(|v| v.as_str());
    let root_override = project_root_arg(&args);
    let result = cg
        .replace_lines(
            path,
            start,
            end,
            new_content,
            expected_digest,
            root_override,
        )
        .await?;
    let touched_files = if result.success {
        vec![result.file_path.clone()]
    } else {
        vec![]
    };
    let mut value = json!({ "ok": result.success, "file": path });
    if result.success {
        value["lines"] = json!([result.changed_lines.0, result.changed_lines.1]);
        value["digest"] = json!(result.digest);
    } else {
        value["message"] = json!(result.message);
    }
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_replace_symbol(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let symbol =
        args.get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: symbol".to_string(),
            })?;
    let new_source = args
        .get("new_source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: new_source".to_string(),
        })?;

    let echo = args
        .get("echo")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let root_override = project_root_arg(&args);
    let result = cg.replace_symbol(symbol, new_source, root_override).await?;
    let touched_files = if result.success {
        vec![result.file_path.clone()]
    } else {
        vec![]
    };
    let mut value = json!({
        "ok": result.success,
        "file": result.file_path,
    });
    if result.success {
        value["lines"] = json!([result.changed_lines.0, result.changed_lines.1]);
        value["digest"] = json!(result.digest);
        if echo {
            value["matched_str"] = json!(result.matched_str);
            value["new_str"] = json!(result.new_str);
        }
    } else {
        value["message"] = json!(result.message);
        if let Some(nearest) = result.nearest {
            value["nearest"] = json!(nearest);
        }
    }
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
        }),
        touched_files,
    })
}

pub(super) async fn handle_insert_at_symbol(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let symbol =
        args.get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: symbol".to_string(),
            })?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: content".to_string(),
        })?;
    let position = args
        .get("position")
        .and_then(|v| v.as_str())
        .unwrap_or("after");

    let echo = args
        .get("echo")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let root_override = project_root_arg(&args);
    let result = cg
        .insert_at_symbol(symbol, content, position, root_override)
        .await?;
    let touched_files = if result.success {
        vec![result.file_path.clone()]
    } else {
        vec![]
    };
    let mut value = json!({
        "ok": result.success,
        "file": result.file_path,
    });
    if result.success {
        value["lines"] = json!([result.changed_lines.0, result.changed_lines.1]);
        value["digest"] = json!(result.digest);
        if echo {
            value["content"] = json!(result.content);
        }
    } else {
        value["message"] = json!(result.message);
        if let Some(nearest) = result.nearest {
            value["nearest"] = json!(nearest);
        }
    }
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
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

    let root_override = project_root_arg(&args);
    let result = cg
        .ast_grep_rewrite(path, pattern, rewrite, root_override)
        .await?;
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

/// Handles `tokensave_rename` (#568): plans a graph-based rename and, when
/// `dry_run` is false, applies it.
pub(super) async fn handle_rename(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let dry_run = args
        .get("dry_run")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    rename(cg, &args, dry_run).await
}

/// Handles the hidden `tokensave_rename_preview` alias: `tokensave_rename`
/// with `dry_run` forced on.
pub(super) async fn handle_rename_preview(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    rename(cg, &args, true).await
}

async fn rename(cg: &TokenSave, args: &Value, dry_run: bool) -> Result<ToolResult> {
    use crate::tokensave::RenameConfidence;

    let node_id = args
        .get("node_id")
        .or_else(|| args.get("id"))
        .and_then(|v| v.as_str());
    let symbol = args.get("symbol").and_then(|v| v.as_str());
    let new_name = args.get("new_name").and_then(|v| v.as_str());
    let allow_heuristic = args
        .get("allow_heuristic")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let root_override = project_root_arg(args);

    let Some(target) = cg.rename_target(node_id, symbol).await? else {
        let id = node_id.unwrap_or_default();
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": format!("Node not found: {id}") }]
            }),
            touched_files: vec![],
        });
    };
    let plan = cg.plan_rename(target, new_name, root_override).await?;

    let mut by_file: Vec<(String, Vec<Value>)> = Vec::new();
    let mut text_only: Vec<Value> = Vec::new();
    for site in &plan.sites {
        let value = serde_json::to_value(site).unwrap_or_default();
        if site.confidence == RenameConfidence::TextOnly {
            text_only.push(value);
            continue;
        }
        match by_file.last_mut() {
            Some((file, sites)) if *file == site.file => sites.push(value),
            _ => by_file.push((site.file.clone(), vec![value])),
        }
    }
    let touched_files: Vec<String> = by_file.iter().map(|(f, _)| f.clone()).collect();
    let files: Vec<Value> = by_file
        .into_iter()
        .map(|(file, sites)| json!({ "file": file, "sites": sites }))
        .collect();

    let mut output = json!({
        "symbol": {
            "id": plan.target.id,
            "name": plan.old_name,
            "kind": plan.target.kind.as_str(),
            "qualified_name": plan.target.qualified_name,
            "file": plan.target.file_path,
            "line": super::display_line(plan.target.start_line),
        },
        "new_name": plan.new_name,
        "dry_run": dry_run,
        "allow_heuristic": allow_heuristic,
        "note": "graph-based, not binding-aware: see the confidence classes in the tool description. line and column are 1-based; column counts bytes, not characters",
        "counts": plan.counts(),
        "files": files,
        "text_only": text_only,
    });
    if !plan.blockers.is_empty() {
        output["blockers"] = json!(plan.blockers);
    }
    if !plan.warnings.is_empty() {
        output["warnings"] = json!(plan.warnings);
    }
    if plan.unlinked_code_omitted > 0 {
        output["unlinked_code_omitted"] = json!(plan.unlinked_code_omitted);
    }
    if !plan.unscanned.is_empty() {
        output["unscanned"] = json!(plan.unscanned);
    }

    if dry_run {
        let non_exact = plan.non_exact_sites().len() + plan.unlinked_code_omitted;
        let unchecked = !plan.unscanned.is_empty();
        output["apply"] = json!(if !plan.blockers.is_empty() {
            "would refuse: see blockers".to_string()
        } else if plan.new_name.is_none() {
            "pass new_name to preview and apply the edit".to_string()
        } else if (non_exact > 0 || unchecked) && !allow_heuristic {
            format!(
                "would refuse: {non_exact} non-exact site(s){}; the diff shows the exact sites \
                 only",
                if unchecked {
                    ", and files that mention the name could not be checked (see unscanned)"
                } else {
                    ""
                }
            )
        } else {
            "would edit the sites in the diff".to_string()
        });
        output["diff"] = json!(plan.file_diffs(allow_heuristic));
    } else {
        let outcome = cg.apply_rename(&plan, allow_heuristic).await?;
        output["applied"] = json!(outcome.applied);
        if let Some(refused) = &outcome.refused {
            output["refused"] = json!(refused);
        }
        if !outcome.blocking_sites.is_empty() {
            output["blocking_sites"] = json!(outcome.blocking_sites);
        }
        output["files_changed"] = json!(outcome
            .files_changed
            .iter()
            .map(|(file, sites)| json!({ "file": file, "sites": sites }))
            .collect::<Vec<_>>());
        if !outcome.skipped.is_empty() {
            output["skipped"] = json!(outcome.skipped);
        }
        if !outcome.warnings.is_empty() {
            output["apply_warnings"] = json!(outcome.warnings);
        }
    }

    let text = super::serialize_bounded_json(&output, &["text_only", "diff", "files"]);
    Ok(ToolResult {
        value: json!({ "content": [{ "type": "text", "text": text }] }),
        touched_files,
    })
}
