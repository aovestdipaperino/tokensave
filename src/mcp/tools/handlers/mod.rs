//! MCP tool call handlers.
//!
//! Each `handle_*` function implements one MCP tool: it deserializes
//! the JSON arguments, calls the appropriate `TokenSave` method, and
//! formats the result.

pub mod analysis;
pub mod edit;
pub mod git;
pub mod graph;
pub mod health;
pub mod info;
pub mod memory;

use std::collections::HashSet;

use serde_json::Value;

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use super::{ToolResult, MAX_RESPONSE_CHARS};

/// Extracts the `node_id` parameter from tool arguments, accepting `id` as a
/// fallback alias. LLMs occasionally shorten `node_id` to `id`; this avoids a
/// confusing error when that happens.
pub(crate) fn require_node_id(args: &Value) -> Result<&str> {
    args.get("node_id")
        .or_else(|| args.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: node_id".to_string(),
        })
}

/// Returns the user-provided `path` argument, falling back to the scope
/// prefix when the argument is absent. This makes listing tools
/// automatically scoped to the subdirectory the server was launched from.
pub(crate) fn effective_path<'a>(
    args: &'a Value,
    scope_prefix: Option<&'a str>,
) -> Option<&'a str> {
    args.get("path").and_then(|v| v.as_str()).or(scope_prefix)
}

/// Filters a Vec of items by file path prefix when a scope is active.
/// Returns the vec unchanged when `scope_prefix` is `None`.
pub(crate) fn filter_by_scope<T, F>(
    items: Vec<T>,
    scope_prefix: Option<&str>,
    get_path: F,
) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    match scope_prefix {
        Some(prefix) => {
            let with_slash = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            items
                .into_iter()
                .filter(|item| {
                    let p = get_path(item);
                    p.starts_with(&with_slash) || p == prefix
                })
                .collect()
        }
        None => items,
    }
}

/// Deduplicates an iterator of file path strings into a `Vec<String>`.
pub(crate) fn unique_file_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for p in paths {
        if seen.insert(p) {
            result.push(p.to_string());
        }
    }
    result
}

/// Truncates a string to the maximum response character limit, appending
/// a truncation notice if necessary.
pub(crate) fn truncate_response(s: &str) -> String {
    debug_assert!(!s.is_empty(), "truncate_response called with empty string");
    if s.len() <= MAX_RESPONSE_CHARS {
        s.to_string()
    } else {
        // Find a valid UTF-8 character boundary at or before MAX_RESPONSE_CHARS
        let mut end = MAX_RESPONSE_CHARS;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}\n\n[... truncated at {} chars]", &s[..end], end)
    }
}

/// Dispatches a tool call to the appropriate handler.
///
/// Returns the tool result and touched file paths, or an error if the tool
/// name is unknown or the handler fails. The optional `server_stats` value
/// is included in `tokensave_status` responses when provided.
pub async fn handle_tool_call(
    cg: &TokenSave,
    tool_name: &str,
    args: Value,
    server_stats: Option<Value>,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    debug_assert!(
        !tool_name.is_empty(),
        "handle_tool_call called with empty tool_name"
    );
    debug_assert!(
        tool_name.starts_with("tokensave_"),
        "tool_name must start with 'tokensave_' prefix"
    );
    match tool_name {
        "tokensave_search" => graph::handle_search(cg, args, scope_prefix).await,
        "tokensave_context" => graph::handle_context(cg, args, scope_prefix).await,
        "tokensave_callers" => graph::handle_callers(cg, args).await,
        "tokensave_callees" => graph::handle_callees(cg, args).await,
        "tokensave_impact" => graph::handle_impact(cg, args).await,
        "tokensave_node" => graph::handle_node(cg, args).await,
        "tokensave_status" => info::handle_status(cg, server_stats, scope_prefix).await,
        "tokensave_files" => info::handle_files(cg, args, scope_prefix).await,
        "tokensave_affected" => git::handle_affected(cg, args).await,
        "tokensave_dead_code" => analysis::handle_dead_code(cg, args, scope_prefix).await,
        "tokensave_diff_context" => git::handle_diff_context(cg, args).await,
        "tokensave_module_api" => analysis::handle_module_api(cg, args, scope_prefix).await,
        "tokensave_circular" => analysis::handle_circular(cg, args).await,
        "tokensave_hotspots" => analysis::handle_hotspots(cg, args, scope_prefix).await,
        "tokensave_similar" => graph::handle_similar(cg, args).await,
        "tokensave_rename_preview" => graph::handle_rename_preview(cg, args).await,
        "tokensave_unused_imports" => analysis::handle_unused_imports(cg, args, scope_prefix).await,
        "tokensave_rank" => analysis::handle_rank(cg, args, scope_prefix).await,
        "tokensave_largest" => analysis::handle_largest(cg, args, scope_prefix).await,
        "tokensave_coupling" => analysis::handle_coupling(cg, args, scope_prefix).await,
        "tokensave_inheritance_depth" => {
            analysis::handle_inheritance_depth(cg, args, scope_prefix).await
        }
        "tokensave_distribution" => analysis::handle_distribution(cg, args, scope_prefix).await,
        "tokensave_recursion" => analysis::handle_recursion(cg, args, scope_prefix).await,
        "tokensave_complexity" => analysis::handle_complexity(cg, args, scope_prefix).await,
        "tokensave_doc_coverage" => analysis::handle_doc_coverage(cg, args, scope_prefix).await,
        "tokensave_god_class" => analysis::handle_god_class(cg, args, scope_prefix).await,
        "tokensave_changelog" => git::handle_changelog(cg, args).await,
        "tokensave_port_status" => info::handle_port_status(cg, args).await,
        "tokensave_port_order" => info::handle_port_order(cg, args).await,
        "tokensave_commit_context" => git::handle_commit_context(cg, args).await,
        "tokensave_pr_context" => git::handle_pr_context(cg, args).await,
        "tokensave_simplify_scan" => info::handle_simplify_scan(cg, args, scope_prefix).await,
        "tokensave_test_map" => health::handle_test_map(cg, args, scope_prefix).await,
        "tokensave_type_hierarchy" => info::handle_type_hierarchy(cg, args).await,
        "tokensave_branch_search" => git::handle_branch_search(cg, args).await,
        "tokensave_branch_diff" => git::handle_branch_diff(cg, args).await,
        "tokensave_branch_list" => Ok(git::handle_branch_list(cg)),
        "tokensave_str_replace" => edit::handle_str_replace(cg, args).await,
        "tokensave_multi_str_replace" => edit::handle_multi_str_replace(cg, args).await,
        "tokensave_insert_at" => edit::handle_insert_at(cg, args).await,
        "tokensave_ast_grep_rewrite" => edit::handle_ast_grep_rewrite(cg, args).await,
        "tokensave_gini" => health::handle_gini(cg, args, scope_prefix).await,
        "tokensave_dependency_depth" => {
            health::handle_dependency_depth(cg, args, scope_prefix).await
        }
        "tokensave_health" => health::handle_health(cg, args, scope_prefix).await,
        "tokensave_dsm" => health::handle_dsm(cg, args, scope_prefix).await,
        "tokensave_test_risk" => health::handle_test_risk(cg, args, scope_prefix).await,
        "tokensave_session_start" => health::handle_session_start(cg, args, scope_prefix).await,
        "tokensave_session_end" => health::handle_session_end(cg, args, scope_prefix).await,
        "tokensave_body" => info::handle_body(cg, args, scope_prefix).await,
        "tokensave_todos" => info::handle_todos(cg, args, scope_prefix).await,
        "tokensave_callers_for" => graph::handle_callers_for(cg, args).await,
        "tokensave_by_qualified_name" => graph::handle_by_qualified_name(cg, args).await,
        "tokensave_signature" => graph::handle_signature(cg, args).await,
        "tokensave_impls" => graph::handle_impls(cg, args).await,
        "tokensave_record_decision" => memory::handle_record_decision(cg, args).await,
        "tokensave_record_code_area" => memory::handle_record_code_area(cg, args).await,
        "tokensave_session_recall" => memory::handle_session_recall(cg, args).await,
        _ => Err(TokenSaveError::Config {
            message: format!("unknown tool: {tool_name}"),
        }),
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::redundant_closure_for_method_calls,
    clippy::uninlined_format_args
)]
mod tests {
    use serde_json::json;

    use super::super::get_tool_definitions;
    use super::*;

    #[test]
    fn test_tool_definitions_complete() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 57);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"tokensave_search"));
        assert!(tool_names.contains(&"tokensave_context"));
        assert!(tool_names.contains(&"tokensave_callers"));
        assert!(tool_names.contains(&"tokensave_callees"));
        assert!(tool_names.contains(&"tokensave_callers_for"));
        assert!(tool_names.contains(&"tokensave_by_qualified_name"));
        assert!(tool_names.contains(&"tokensave_signature"));
        assert!(tool_names.contains(&"tokensave_impls"));
        assert!(tool_names.contains(&"tokensave_impact"));
        assert!(tool_names.contains(&"tokensave_node"));
        assert!(tool_names.contains(&"tokensave_status"));
        assert!(tool_names.contains(&"tokensave_files"));
        assert!(tool_names.contains(&"tokensave_affected"));
        assert!(tool_names.contains(&"tokensave_dead_code"));
        assert!(tool_names.contains(&"tokensave_diff_context"));
        assert!(tool_names.contains(&"tokensave_module_api"));
        assert!(tool_names.contains(&"tokensave_circular"));
        assert!(tool_names.contains(&"tokensave_hotspots"));
        assert!(tool_names.contains(&"tokensave_similar"));
        assert!(tool_names.contains(&"tokensave_rename_preview"));
        assert!(tool_names.contains(&"tokensave_unused_imports"));
        assert!(tool_names.contains(&"tokensave_changelog"));
        assert!(tool_names.contains(&"tokensave_rank"));
        assert!(tool_names.contains(&"tokensave_largest"));
        assert!(tool_names.contains(&"tokensave_coupling"));
        assert!(tool_names.contains(&"tokensave_inheritance_depth"));
        assert!(tool_names.contains(&"tokensave_distribution"));
        assert!(tool_names.contains(&"tokensave_recursion"));
        assert!(tool_names.contains(&"tokensave_complexity"));
        assert!(tool_names.contains(&"tokensave_doc_coverage"));
        assert!(tool_names.contains(&"tokensave_god_class"));
        assert!(tool_names.contains(&"tokensave_port_status"));
        assert!(tool_names.contains(&"tokensave_port_order"));
        assert!(tool_names.contains(&"tokensave_commit_context"));
        assert!(tool_names.contains(&"tokensave_pr_context"));
        assert!(tool_names.contains(&"tokensave_simplify_scan"));
        assert!(tool_names.contains(&"tokensave_test_map"));
        assert!(tool_names.contains(&"tokensave_type_hierarchy"));
        assert!(tool_names.contains(&"tokensave_branch_search"));
        assert!(tool_names.contains(&"tokensave_branch_diff"));
        assert!(tool_names.contains(&"tokensave_branch_list"));
        assert!(tool_names.contains(&"tokensave_str_replace"));
        assert!(tool_names.contains(&"tokensave_multi_str_replace"));
        assert!(tool_names.contains(&"tokensave_insert_at"));
        assert!(tool_names.contains(&"tokensave_ast_grep_rewrite"));
        assert!(tool_names.contains(&"tokensave_gini"));
        assert!(tool_names.contains(&"tokensave_dependency_depth"));
        assert!(tool_names.contains(&"tokensave_health"));
        assert!(tool_names.contains(&"tokensave_dsm"));
        assert!(tool_names.contains(&"tokensave_test_risk"));
        assert!(tool_names.contains(&"tokensave_session_start"));
        assert!(tool_names.contains(&"tokensave_session_end"));
        assert!(tool_names.contains(&"tokensave_body"));
        assert!(tool_names.contains(&"tokensave_todos"));
        assert!(tool_names.contains(&"tokensave_record_decision"));
        assert!(tool_names.contains(&"tokensave_record_code_area"));
        assert!(tool_names.contains(&"tokensave_session_recall"));
    }

    #[test]
    fn test_tool_definitions_have_schemas() {
        let tools = get_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(tool.input_schema.is_object());
            assert_eq!(tool.input_schema["type"], "object");
        }
    }

    #[test]
    fn test_tool_definitions_have_annotations() {
        let tools = get_tool_definitions();
        let write_tools = [
            "tokensave_str_replace",
            "tokensave_multi_str_replace",
            "tokensave_insert_at",
            "tokensave_ast_grep_rewrite",
            "tokensave_session_start",
            "tokensave_record_decision",
            "tokensave_record_code_area",
        ];
        for tool in &tools {
            let ann = tool
                .annotations
                .as_ref()
                .unwrap_or_else(|| panic!("{} missing annotations", tool.name));
            if write_tools.contains(&tool.name.as_str()) {
                assert_eq!(
                    ann["readOnlyHint"], false,
                    "{} should have readOnlyHint=false",
                    tool.name
                );
            } else {
                assert_eq!(
                    ann["readOnlyHint"], true,
                    "{} missing readOnlyHint",
                    tool.name
                );
            }
            assert!(
                ann["title"].is_string(),
                "{} missing title annotation",
                tool.name
            );
        }
    }

    #[test]
    fn test_always_load_tools() {
        let tools = get_tool_definitions();
        let always_load: Vec<&str> = tools
            .iter()
            .filter(|t| {
                t.meta
                    .as_ref()
                    .and_then(|m| m.get("anthropic/alwaysLoad"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .map(|t| t.name.as_str())
            .collect();
        assert!(
            always_load.contains(&"tokensave_context"),
            "tokensave_context must be alwaysLoad"
        );
        assert!(
            always_load.contains(&"tokensave_search"),
            "tokensave_search must be alwaysLoad"
        );
        assert!(
            always_load.contains(&"tokensave_status"),
            "tokensave_status must be alwaysLoad"
        );
        assert_eq!(
            always_load.len(),
            3,
            "exactly 3 tools should be alwaysLoad, got {:?}",
            always_load
        );
    }

    #[test]
    fn test_truncate_short_response() {
        let short = "hello world";
        assert_eq!(truncate_response(short), short);
    }

    #[test]
    fn test_truncate_long_response() {
        let long = "x".repeat(20_000);
        let result = truncate_response(&long);
        assert!(result.len() < 20_000);
        assert!(result.contains("[... truncated at 15000 chars]"));
    }

    #[test]
    fn test_tool_definitions_serializable() {
        let tools = get_tool_definitions();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("tokensave_search"));
        assert!(json.contains("tokensave_status"));
    }

    #[test]
    fn test_require_node_id_canonical() {
        let args = json!({"node_id": "fn:abc123"});
        assert_eq!(require_node_id(&args).unwrap(), "fn:abc123");
    }

    #[test]
    fn test_require_node_id_alias() {
        let args = json!({"id": "trait:def456"});
        assert_eq!(require_node_id(&args).unwrap(), "trait:def456");
    }

    #[test]
    fn test_require_node_id_prefers_canonical() {
        let args = json!({"node_id": "fn:canonical", "id": "fn:alias"});
        assert_eq!(require_node_id(&args).unwrap(), "fn:canonical");
    }

    #[test]
    fn test_require_node_id_missing() {
        let args = json!({"query": "something"});
        assert!(require_node_id(&args).is_err());
    }
}
