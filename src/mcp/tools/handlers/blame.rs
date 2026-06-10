//! `tokensave_blame` and `tokensave_log` tool handlers.

use serde_json::{json, Value};

use super::super::ToolResult;
use super::truncate_response;
use crate::blame_engine::{self, BlameOptions};
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

/// Shared symbol resolution + working-tree fingerprint computation.
async fn resolve_target(
    cg: &TokenSave,
    args: &Value,
) -> Result<(
    crate::types::Node,
    crate::redundancy::Fingerprint,
    &'static str,
)> {
    let symbol =
        args.get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: symbol".to_string(),
            })?;
    let file_filter = args.get("file").and_then(|v| v.as_str());

    let candidates = cg.get_nodes_by_qualified_name(symbol).await?;
    let candidates: Vec<_> = match file_filter {
        Some(f) => candidates
            .into_iter()
            .filter(|n| n.file_path == f)
            .collect(),
        None => candidates,
    };

    if candidates.is_empty() {
        return Err(TokenSaveError::Config {
            message: format!(
                "symbol '{symbol}' not found in graph; run `tokensave sync` if recently added"
            ),
        });
    }
    if candidates.len() > 1 {
        let listing: Vec<String> = candidates
            .iter()
            .take(5)
            .map(|n| {
                format!(
                    "  - {}:{} ({})",
                    n.file_path,
                    n.start_line + 1,
                    n.kind.as_str()
                )
            })
            .collect();
        return Err(TokenSaveError::Config {
            message: format!(
                "ambiguous symbol '{symbol}'. Pass `--file <path>` to disambiguate. Candidates:\n{}",
                listing.join("\n")
            ),
        });
    }

    let node = candidates
        .into_iter()
        .next()
        .ok_or_else(|| TokenSaveError::Config {
            message: format!("symbol '{symbol}' not found after filtering"),
        })?;
    let lang_key = blame_engine::ts_lang_key_from_path(&node.file_path).ok_or_else(|| {
        TokenSaveError::Config {
            message: format!(
                "no tree-sitter grammar for file extension of '{}'",
                node.file_path
            ),
        }
    })?;

    let abs_path = cg.project_root().join(&node.file_path);
    let source = crate::sync::read_source_file(&abs_path).map_err(|e| TokenSaveError::Config {
        message: format!("cannot read {}: {e}", node.file_path),
    })?;
    let fp =
        blame_engine::compute_target_fingerprint(&source, lang_key, node.start_line, node.end_line)
            .ok_or_else(|| TokenSaveError::Config {
                message: format!(
                    "could not compute fingerprint for '{}' at {}:{}",
                    node.qualified_name,
                    node.file_path,
                    node.start_line + 1
                ),
            })?;

    Ok((node, fp, lang_key))
}

fn opts_from_args(args: &Value) -> BlameOptions {
    let mut opts = BlameOptions::default();
    if let Some(n) = args.get("max_commits").and_then(serde_json::Value::as_u64) {
        opts.max_commits = (n as usize).clamp(1, 50_000);
    }
    opts
}

/// Handles `tokensave_blame`.
pub(super) async fn handle_blame(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let (node, fp, lang) = resolve_target(cg, &args).await?;
    let opts = opts_from_args(&args);
    let result = blame_engine::log(
        cg.project_root(),
        &node.file_path,
        node.start_line,
        node.end_line,
        lang,
        &fp,
        &opts,
    )
    .map_err(|e| TokenSaveError::Config { message: e })?;

    let last = result.events.last();
    let output = json!({
        "symbol": node.qualified_name,
        "file": node.file_path,
        "lines": [node.start_line + 1, node.end_line + 1],
        "last_change": last,
        "boundary_reason": result.boundary_reason,
        "commits_walked": result.commits_walked,
        "parse_failures": result.parse_failures,
        "skipped_large": result.skipped_large,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![node.file_path.clone()],
    })
}

/// Handles `tokensave_log`.
pub(super) async fn handle_log(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let (node, fp, lang) = resolve_target(cg, &args).await?;
    let opts = opts_from_args(&args);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |v| (v as usize).clamp(1, 1_000));

    let result = blame_engine::log(
        cg.project_root(),
        &node.file_path,
        node.start_line,
        node.end_line,
        lang,
        &fp,
        &opts,
    )
    .map_err(|e| TokenSaveError::Config { message: e })?;

    let events: Vec<_> = result.events.iter().take(limit).cloned().collect();
    let output = json!({
        "symbol": node.qualified_name,
        "file": node.file_path,
        "lines": [node.start_line + 1, node.end_line + 1],
        "events": events,
        "boundary_reason": result.boundary_reason,
        "commits_walked": result.commits_walked,
        "parse_failures": result.parse_failures,
        "skipped_large": result.skipped_large,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![node.file_path.clone()],
    })
}
