//! Changed-file simplification analysis MCP tool handler.

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::{NodeKind, Visibility};

use super::super::ToolResult;
use super::truncate_response;

/// Handles `tokensave_simplify_scan` tool calls.
pub(super) async fn handle_simplify_scan(
    cg: &TokenSave,
    args: Value,
    _scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let mut files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: files (array of strings)".to_string(),
        })?;
    files.sort();
    files.dedup();

    let mut duplication_targets = Vec::new();
    let mut dead_introductions: Vec<Value> = Vec::new();
    let mut complexity_warnings: Vec<Value> = Vec::new();
    let mut coupling_warnings: Vec<Value> = Vec::new();

    for file in &files {
        let mut nodes = cg.get_nodes_by_file(file).await.unwrap_or_default();
        nodes.sort_by(|left, right| {
            left.start_line
                .cmp(&right.start_line)
                .then_with(|| left.qualified_name.cmp(&right.qualified_name))
                .then_with(|| left.id.cmp(&right.id))
        });

        for node in &nodes {
            // 1. Duplication: collect changed function-like targets. Exact
            // implementation evidence is resolved once after file analysis.
            if matches!(node.kind, NodeKind::Function | NodeKind::Method) {
                duplication_targets.push(node.clone());
            }

            // 2. Dead code: function/method with no incoming edges
            if matches!(node.kind, NodeKind::Function | NodeKind::Method)
                && node.visibility != Visibility::Pub
                && node.name != "main"
                && !node.name.starts_with("test_")
            {
                let incoming = cg.get_incoming_edges(&node.id).await.unwrap_or_default();
                if incoming.is_empty() {
                    dead_introductions.push(json!({
                        "symbol": node.name,
                        "file": node.file_path,
                        "line": super::display_line(node.start_line),
                        "reason": "no incoming edges (unreferenced)",
                    }));
                }
            }

            // 3. Complexity: check if function exceeds threshold
            if matches!(node.kind, NodeKind::Function | NodeKind::Method) {
                let lines = node.end_line.saturating_sub(node.start_line) as usize;
                let fan_out = cg
                    .get_outgoing_edges(&node.id)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .filter(|e| matches!(e.kind, crate::types::EdgeKind::Calls))
                    .count();
                let score = lines + fan_out * 3;
                if score > 100 {
                    complexity_warnings.push(json!({
                        "symbol": node.name,
                        "file": node.file_path,
                        "line": super::display_line(node.start_line),
                        "lines": lines,
                        "fan_out": fan_out,
                        "score": score,
                    }));
                }
            }
        }

        // 4. Coupling: check file fan_in
        let file_deps = cg.get_file_dependents(file).await.unwrap_or_default();
        if file_deps.len() > 15 {
            coupling_warnings.push(json!({
                "file": file,
                "fan_in": file_deps.len(),
                "warning": "high fan-in — changes here affect many dependents",
            }));
        }
    }

    let duplications =
        super::redundancy::find_target_duplications(cg, &duplication_targets).await?;

    let output = json!({
        "duplications": duplications,
        "dead_introductions": dead_introductions,
        "complexity_warnings": complexity_warnings,
        "coupling_warnings": coupling_warnings,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: files,
    })
}
