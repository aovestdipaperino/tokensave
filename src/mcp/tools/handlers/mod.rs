//! MCP tool call handlers.
//!
//! Each `handle_*` function implements one MCP tool: it deserializes
//! the JSON arguments, calls the appropriate `TokenSave` method, and
//! formats the result.

pub mod analysis;
pub mod blame;
pub mod dependencies;
pub mod edit;
pub mod git;
pub mod graph;
pub mod health;
pub mod info;
pub mod memory;
pub mod receiver_type;
pub mod redundancy;
pub mod workflow;

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use super::{ToolResult, MAX_RESPONSE_CHARS};

/// Converts a stored 0-based line (tree-sitter row, the convention every
/// extractor writes to the DB) into the 1-based editor line used in every
/// user-facing response (#203). Internal span comparisons stay 0-based;
/// apply this only at the presentation edge.
pub(crate) fn display_line(stored: u32) -> u32 {
    stored + 1
}

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

/// Filters a Vec of items by include/exclude path-substring lists.
///
/// Matching is done on the item's path with backslashes normalized to `/`
/// (so callers can pass forward-slash substrings on every platform). The
/// comparison is a case-sensitive substring match.
///
/// Rules:
/// - `exclude` takes precedence: any item whose path contains *any* exclude
///   substring is dropped.
/// - If `include` is non-empty, only items whose path contains *at least one*
///   include substring are kept.
/// - When both lists are empty the vec is returned unchanged.
pub(crate) fn filter_by_path_lists<T, F>(
    items: Vec<T>,
    include: &[String],
    exclude: &[String],
    get_path: F,
) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    if include.is_empty() && exclude.is_empty() {
        return items;
    }
    // Normalize the filter substrings as well as the item paths: a Windows
    // caller passing `apps\admin` must match the canonical forward-slash
    // stored path (#204). Config-level defaults reach this function without
    // going through the dispatcher's arg normalization.
    let include: Vec<String> = include.iter().map(|s| s.replace('\\', "/")).collect();
    let exclude: Vec<String> = exclude.iter().map(|s| s.replace('\\', "/")).collect();
    items
        .into_iter()
        .filter(|item| {
            let normalized = get_path(item).replace('\\', "/");
            if exclude.iter().any(|sub| normalized.contains(sub.as_str())) {
                return false;
            }
            if !include.is_empty() {
                return include.iter().any(|sub| normalized.contains(sub.as_str()));
            }
            true
        })
        .collect()
}

/// Parses an optional JSON array of strings from tool arguments into a
/// `Vec<String>`, returning an empty vec when the key is absent or not an
/// array. Used for the `path_include` / `path_exclude` filter params.
pub(crate) fn parse_string_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Returns `caller` if non-empty, otherwise falls back to `defaults`.
/// Used to merge explicit tool-call args with config-level defaults.
pub(crate) fn with_defaults(caller: Vec<String>, defaults: &[String]) -> Vec<String> {
    if caller.is_empty() {
        defaults.to_vec()
    } else {
        caller
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

/// Advice attached wherever sibling projects are surfaced.
pub(crate) const SIBLING_HINT: &str =
    "Other initialized projects sit beside this one. If a symbol is missing here, \
     retry the same call with graph_root set to one of them.";

/// Returns the initialized projects sitting directly beside `project_root`.
///
/// Best-effort like every other global-DB read: an unavailable global DB yields
/// no siblings rather than an error, since this only ever adds a hint.
pub(crate) async fn sibling_projects(project_root: &std::path::Path) -> Vec<String> {
    match crate::global_db::GlobalDb::open().await {
        Some(gdb) => gdb.sibling_projects(project_root).await,
        None => Vec::new(),
    }
}

/// Builds the empty-result payload naming reachable sibling graphs, if any.
///
/// Returns `None` when the caller has results to return, or when no sibling
/// project exists — in both cases the ordinary response shape is kept.
pub(crate) async fn sibling_note(
    is_empty: bool,
    project_root: &std::path::Path,
) -> Option<serde_json::Value> {
    if !is_empty {
        return None;
    }
    let siblings = sibling_projects(project_root).await;
    if siblings.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "results": [],
        "sibling_projects": siblings,
        "hint": SIBLING_HINT,
    }))
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

/// Serializes a structured payload for a tool response without ever emitting
/// text that has stopped being JSON.
///
/// `truncate_response` slices an already-serialized string, which leaves the
/// caller holding a half-written object — `jq` fails and the CLI still exits 0
/// (#486). Here the payload is bounded *before* serialization by shedding whole
/// elements off the arrays named in `shedable`, so every byte we emit parses.
///
/// `shedable` names the arrays that may lose elements, in the order they should
/// be sacrificed (least useful first); a name may be a dotted path into nested
/// objects, e.g. `"changes.impacted_symbols"`. What was dropped is recorded
/// under a top-level `truncated` object, one entry per shed array:
/// `{"impacted_symbols": {"shown": 40, "total": 900}}`.
pub(crate) fn serialize_bounded_json(value: &Value, shedable: &[&str]) -> String {
    let fits = |v: &Value| -> Option<String> {
        let s = serde_json::to_string_pretty(v).unwrap_or_default();
        (s.len() <= MAX_RESPONSE_CHARS).then_some(s)
    };
    if let Some(s) = fits(value) {
        return s;
    }

    let mut working = value.clone();
    let mut totals: Vec<(&str, usize)> = Vec::new();
    for path in shedable {
        if let Some(len) = array_at(&working, path).map(Vec::len) {
            totals.push((path, len));
        }
    }

    // Halve the longest remaining array each round. Halving (rather than
    // popping) keeps this O(log n) serializations instead of one per dropped
    // element, which matters when a wide impact radius yields tens of
    // thousands of symbols.
    loop {
        let longest = totals
            .iter()
            .filter_map(|(path, _)| {
                let len = array_at(&working, path)?.len();
                (len > 0).then_some((len, *path))
            })
            .max_by_key(|(len, _)| *len);
        let Some((len, path)) = longest else {
            break;
        };
        let keep = len / 2;
        if let Some(arr) = array_at_mut(&mut working, path) {
            arr.truncate(keep);
        }
        set_truncation_note(&mut working, path, keep, &totals);
        if let Some(s) = fits(&working) {
            return s;
        }
    }

    // Nothing left to shed and the remainder still does not fit: the payload's
    // scalar content alone is over budget. Report that as JSON rather than
    // handing back a sliced object.
    let note = json!({
        "truncated": {
            "error": "payload exceeds the response limit even with every list emptied",
            "limit_chars": MAX_RESPONSE_CHARS,
            "totals": totals.iter().map(|(p, n)| json!({"field": p, "total": n})).collect::<Vec<_>>(),
        }
    });
    serde_json::to_string_pretty(&note).unwrap_or_default()
}

/// Records, under a top-level `truncated` object, that `path` was cut down to
/// `keep` of its original element count.
fn set_truncation_note(root: &mut Value, path: &str, keep: usize, totals: &[(&str, usize)]) {
    let total = totals
        .iter()
        .find(|(p, _)| *p == path)
        .map_or(keep, |(_, n)| *n);
    let key = path.rsplit('.').next().unwrap_or(path).to_string();
    let entry = json!({"shown": keep, "total": total});
    match root.get_mut("truncated").and_then(Value::as_object_mut) {
        Some(map) => {
            map.insert(key, entry);
        }
        None => {
            if let Some(map) = root.as_object_mut() {
                map.insert("truncated".to_string(), json!({key: entry}));
            }
        }
    }
}

/// Resolves a dotted path to an array inside `root`.
fn array_at<'a>(root: &'a Value, path: &str) -> Option<&'a Vec<Value>> {
    let mut cur = root;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    cur.as_array()
}

/// Mutable counterpart of `array_at`.
fn array_at_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Vec<Value>> {
    let mut cur = root;
    for seg in path.split('.') {
        cur = cur.get_mut(seg)?;
    }
    cur.as_array_mut()
}

/// Like `truncate_response`, but everything from the last occurrence of
/// `marker` to the end of the string survives truncation as a suffix.
///
/// `tokensave_context` ends with the `### Retrieval` diagnostics footer,
/// followed by `seen_node_ids` and occasional hints — the small, load-bearing
/// tail of the response. Plain prefix truncation removes exactly that tail
/// whenever the Code section is large, which is also when a caller most needs
/// to know whether the retrieval behind it was trustworthy. The truncation
/// notice stays where content was cut, so the seam remains visible.
///
/// Falls back to plain truncation when the marker is absent or the tail is
/// too large to be the footer it is meant for (a marker echoed inside a huge
/// code block must not defeat the response limit).
pub(crate) fn truncate_response_keep_tail(s: &str, marker: &str) -> String {
    const MAX_TAIL_CHARS: usize = 2_000;
    if s.len() <= MAX_RESPONSE_CHARS {
        return s.to_string();
    }
    let Some(idx) = s.rfind(marker) else {
        return truncate_response(s);
    };
    let tail = &s[idx..];
    if tail.len() > MAX_TAIL_CHARS {
        return truncate_response(s);
    }
    let budget = MAX_RESPONSE_CHARS - tail.len();
    let mut end = budget.min(idx);
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!(
        "{}\n\n[... truncated at {} chars]\n\n{}",
        &s[..end],
        end,
        tail
    )
}

/// Edit tools resolve an absolute `path` verbatim (see
/// `TokenSave::resolve_edit_target`) rather than only matching it against
/// the DB's canonical forward-slash paths, so they must keep a
/// drive-letter-absolute Windows path (`C:\...`) exactly as the caller
/// wrote it.
const EDIT_TOOLS_HONORING_ABSOLUTE_PATHS_VERBATIM: &[&str] = &[
    "tokensave_str_replace",
    "tokensave_multi_str_replace",
    "tokensave_insert_at",
    "tokensave_replace_symbol",
    "tokensave_insert_at_symbol",
    "tokensave_ast_grep_rewrite",
];

fn is_drive_absolute(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphabetic) && bytes.get(1) == Some(&b':')
}

/// Normalizes Windows backslash separators to `/` in the path-shaped tool
/// arguments (`file`, `path`, `file_path`, and the `path_include` /
/// `path_exclude` arrays) so they match the DB's canonical forward-slash
/// stored paths (#204). Verbatim/UNC paths (leading `\\`) are left alone —
/// rewriting their prefix would break them on Windows. For the edit tools
/// (`preserve_drive_absolute`), a drive-letter-absolute path is also left
/// alone, since those tools honor an absolute `path` verbatim.
fn normalize_path_args(args: &mut Value, preserve_drive_absolute: bool) {
    fn normalize(s: &str, preserve_drive_absolute: bool) -> Option<String> {
        if s.contains('\\')
            && !s.starts_with("\\\\")
            && !(preserve_drive_absolute && is_drive_absolute(s))
        {
            Some(s.replace('\\', "/"))
        } else {
            None
        }
    }
    let Some(map) = args.as_object_mut() else {
        return;
    };
    for key in ["file", "path", "file_path"] {
        if let Some(v) = map.get_mut(key) {
            if let Some(fixed) = v
                .as_str()
                .and_then(|s| normalize(s, preserve_drive_absolute))
            {
                *v = Value::String(fixed);
            }
        }
    }
    for key in ["path_include", "path_exclude"] {
        if let Some(arr) = map.get_mut(key).and_then(|v| v.as_array_mut()) {
            for v in arr {
                if let Some(fixed) = v
                    .as_str()
                    .and_then(|s| normalize(s, preserve_drive_absolute))
                {
                    *v = Value::String(fixed);
                }
            }
        }
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
    mut args: Value,
    server_stats: Option<Value>,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    normalize_path_args(
        &mut args,
        EDIT_TOOLS_HONORING_ABSOLUTE_PATHS_VERBATIM.contains(&tool_name),
    );
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
        "tokensave_ambiguous_calls" => {
            analysis::handle_ambiguous_calls(cg, args, scope_prefix).await
        }
        "tokensave_diff" => git::handle_diff(cg, args).await,
        "tokensave_diff_context" => git::handle_diff_context(cg, args).await,
        "tokensave_module_api" => analysis::handle_module_api(cg, args, scope_prefix).await,
        "tokensave_circular" => analysis::handle_circular(cg, args).await,
        "tokensave_imports" => analysis::handle_imports(cg, args).await,
        "tokensave_hotspots" => analysis::handle_hotspots(cg, args, scope_prefix).await,
        "tokensave_similar" => graph::handle_similar(cg, args).await,
        "tokensave_rename_preview" => graph::handle_rename_preview(cg, args).await,
        "tokensave_unused_imports" => analysis::handle_unused_imports(cg, args, scope_prefix).await,
        "tokensave_rank" => analysis::handle_rank(cg, args, scope_prefix).await,
        "tokensave_largest" => analysis::handle_largest(cg, args, scope_prefix).await,
        "tokensave_log" => blame::handle_log(cg, args).await,
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
        "tokensave_redundancy" => redundancy::handle_redundancy(cg, args, scope_prefix).await,
        "tokensave_runtime" => health::handle_runtime(cg, args).await,
        "tokensave_dsm" => health::handle_dsm(cg, args, scope_prefix).await,
        "tokensave_test_risk" => health::handle_test_risk(cg, args, scope_prefix).await,
        "tokensave_test_coverage" => health::handle_test_coverage(cg, args).await,
        "tokensave_dependencies" => dependencies::handle_dependencies(cg, args).await,
        "tokensave_session_start" => health::handle_session_start(cg, args, scope_prefix).await,
        "tokensave_session_end" => health::handle_session_end(cg, args, scope_prefix).await,
        "tokensave_blame" => blame::handle_blame(cg, args).await,
        "tokensave_body" => info::handle_body(cg, args, scope_prefix).await,
        "tokensave_doc" => info::handle_doc(cg, args, scope_prefix).await,
        "tokensave_todos" => info::handle_todos(cg, args, scope_prefix).await,
        "tokensave_read" => info::handle_read(cg, args).await,
        "tokensave_entities" => info::handle_outline(cg, args).await,
        "tokensave_config" => info::handle_config(cg, &args),
        "tokensave_signature_search" => info::handle_signature_search(cg, args, scope_prefix).await,
        "tokensave_implementations" => graph::handle_implementations(cg, args, scope_prefix).await,
        "tokensave_unsafe_patterns" => {
            analysis::handle_unsafe_patterns(cg, args, scope_prefix).await
        }
        "tokensave_diagnostics" => analysis::handle_diagnostics(cg, args).await,
        "tokensave_constructors" => analysis::handle_constructors(cg, args, scope_prefix).await,
        "tokensave_field_sites" => analysis::handle_field_sites(cg, args, scope_prefix).await,
        "tokensave_callers_for" => graph::handle_callers_for(cg, args).await,
        "tokensave_call_chain" => graph::handle_call_chain(cg, args).await,
        "tokensave_file_dependents" => graph::handle_file_dependents(cg, args).await,
        "tokensave_replace_symbol" => edit::handle_replace_symbol(cg, args).await,
        "tokensave_insert_at_symbol" => edit::handle_insert_at_symbol(cg, args).await,
        "tokensave_find_exact_symbol" => {
            graph::handle_find_exact_symbol(cg, args, scope_prefix).await
        }
        "tokensave_by_qualified_name" => graph::handle_by_qualified_name(cg, args).await,
        "tokensave_signature" => graph::handle_signature(cg, args).await,
        "tokensave_impls" => graph::handle_impls(cg, args).await,
        "tokensave_diagnose" => workflow::handle_diagnose(cg, args).await,
        "tokensave_run_affected_tests" => workflow::handle_run_affected_tests(cg, args).await,
        "tokensave_derives" => graph::handle_derives(cg, args).await,
        "tokensave_annotations" => graph::handle_annotations(cg, args).await,
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

    /// #486: a payload over the limit must still serialize to parseable JSON.
    #[test]
    fn bounded_json_stays_parseable_when_oversized() {
        let items: Vec<Value> = (0..5_000)
            .map(|i| json!({"id": format!("node-{i}"), "name": "some_function_name"}))
            .collect();
        let payload = json!({
            "changed_files": ["src/foo.rs"],
            "impacted_symbols_count": items.len(),
            "impacted_symbols": items,
        });
        let out = serialize_bounded_json(&payload, &["impacted_symbols"]);
        assert!(out.len() <= MAX_RESPONSE_CHARS, "len {}", out.len());
        let parsed: Value = serde_json::from_str(&out).expect("output must be valid JSON");
        assert_eq!(parsed["impacted_symbols_count"], 5_000);
        let shown = parsed["impacted_symbols"].as_array().unwrap().len();
        assert!(shown > 0 && shown < 5_000, "shown {shown}");
        assert_eq!(parsed["truncated"]["impacted_symbols"]["shown"], shown);
        assert_eq!(parsed["truncated"]["impacted_symbols"]["total"], 5_000);
        assert!(!out.contains("[... truncated at"));
    }

    #[test]
    fn bounded_json_leaves_small_payload_untouched() {
        let payload = json!({"impacted_symbols": [{"id": "a"}]});
        let out = serialize_bounded_json(&payload, &["impacted_symbols"]);
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed, payload);
        assert!(parsed.get("truncated").is_none());
    }

    /// Sheds in the order given: the first list is sacrificed before the ones
    /// after it, so a caller's ranking of what matters is respected.
    #[test]
    fn bounded_json_sheds_in_declared_order() {
        let big: Vec<Value> = (0..4_000)
            .map(|i| json!({"i": i, "pad": "xxxxxxxxxx"}))
            .collect();
        let payload = json!({"first": big.clone(), "second": ["keep-me"]});
        let out = serialize_bounded_json(&payload, &["first", "second"]);
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["second"], json!(["keep-me"]));
        assert!(parsed["first"].as_array().unwrap().len() < 4_000);
    }

    /// Nested (dotted) paths are what `tokensave_diff` uses for its envelope.
    #[test]
    fn bounded_json_sheds_through_dotted_path() {
        let items: Vec<Value> = (0..5_000)
            .map(|i| json!({"id": i, "pad": "yyyyyyyyyy"}))
            .collect();
        let payload =
            json!({"delegated_to": "diff_context", "changes": {"impacted_symbols": items}});
        let out = serialize_bounded_json(&payload, &["changes.impacted_symbols"]);
        assert!(out.len() <= MAX_RESPONSE_CHARS);
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["delegated_to"], "diff_context");
        assert_eq!(parsed["truncated"]["impacted_symbols"]["total"], 5_000);
    }

    /// Unshedable bulk (one giant scalar) still yields JSON, not a sliced object.
    #[test]
    fn bounded_json_reports_when_nothing_can_be_shed() {
        let payload = json!({"blob": "z".repeat(MAX_RESPONSE_CHARS + 100)});
        let out = serialize_bounded_json(&payload, &["items"]);
        let parsed: Value = serde_json::from_str(&out).expect("output must be valid JSON");
        assert!(parsed["truncated"]["error"].is_string());
    }

    #[test]
    fn test_truncate_keep_tail_preserves_footer_and_ids() {
        let body = "x".repeat(MAX_RESPONSE_CHARS + 5_000);
        let tail =
            "### Retrieval\n- match: strong (best score 12.00)\n\nseen_node_ids: [\"a\",\"b\"]\n";
        let s = format!("{body}\n{tail}");
        let out = truncate_response_keep_tail(&s, "### Retrieval");
        assert!(out.contains("[... truncated at"), "{}", &out[..200]);
        assert!(out.ends_with(tail), "tail must survive truncation");
        assert!(out.len() <= MAX_RESPONSE_CHARS + 100, "len {}", out.len());
    }

    #[test]
    fn test_truncate_keep_tail_short_input_unchanged() {
        let s = "short\n### Retrieval\n- match: exact\n";
        assert_eq!(truncate_response_keep_tail(s, "### Retrieval"), s);
    }

    #[test]
    fn test_truncate_keep_tail_falls_back_without_marker() {
        let s = "y".repeat(MAX_RESPONSE_CHARS + 100);
        let out = truncate_response_keep_tail(&s, "### Retrieval");
        assert_eq!(out, truncate_response(&s));
    }

    #[test]
    fn test_truncate_keep_tail_rejects_oversized_tail() {
        // A marker echoed inside a huge code block must not defeat the limit.
        let s = format!(
            "{}### Retrieval\n{}",
            "b".repeat(100),
            "z".repeat(MAX_RESPONSE_CHARS)
        );
        let out = truncate_response_keep_tail(&s, "### Retrieval");
        assert_eq!(out, truncate_response(&s));
    }

    #[test]
    fn test_tool_definitions_complete() {
        let tools = get_tool_definitions();
        // ast_grep_rewrite is conditionally registered based on whether the
        // external `ast-grep` binary is on PATH — agents should never see a
        // tool that will instantly fail. The count and the per-tool checks
        // below adapt to the host's capability set.
        let expected_total = if super::super::definitions::ast_grep_available() {
            85
        } else {
            84
        };
        assert_eq!(tools.len(), expected_total);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"tokensave_doc"));
        assert!(tool_names.contains(&"tokensave_search"));
        assert!(tool_names.contains(&"tokensave_context"));
        assert!(tool_names.contains(&"tokensave_callers"));
        assert!(tool_names.contains(&"tokensave_callees"));
        assert!(tool_names.contains(&"tokensave_callers_for"));
        assert!(tool_names.contains(&"tokensave_by_qualified_name"));
        assert!(tool_names.contains(&"tokensave_signature"));
        assert!(tool_names.contains(&"tokensave_impls"));
        assert!(tool_names.contains(&"tokensave_diagnose"));
        assert!(tool_names.contains(&"tokensave_run_affected_tests"));
        assert!(tool_names.contains(&"tokensave_derives"));
        assert!(tool_names.contains(&"tokensave_annotations"));
        assert!(tool_names.contains(&"tokensave_impact"));
        assert!(tool_names.contains(&"tokensave_node"));
        assert!(tool_names.contains(&"tokensave_status"));
        assert!(tool_names.contains(&"tokensave_imports"));
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
        if super::super::definitions::ast_grep_available() {
            assert!(tool_names.contains(&"tokensave_ast_grep_rewrite"));
        } else {
            assert!(!tool_names.contains(&"tokensave_ast_grep_rewrite"));
        }
        assert!(tool_names.contains(&"tokensave_gini"));
        assert!(tool_names.contains(&"tokensave_dependency_depth"));
        assert!(tool_names.contains(&"tokensave_health"));
        assert!(tool_names.contains(&"tokensave_redundancy"));
        assert!(tool_names.contains(&"tokensave_runtime"));
        assert!(tool_names.contains(&"tokensave_dsm"));
        assert!(tool_names.contains(&"tokensave_test_risk"));
        assert!(tool_names.contains(&"tokensave_test_coverage"));
        assert!(tool_names.contains(&"tokensave_dependencies"));
        assert!(tool_names.contains(&"tokensave_session_start"));
        assert!(tool_names.contains(&"tokensave_session_end"));
        assert!(tool_names.contains(&"tokensave_body"));
        assert!(tool_names.contains(&"tokensave_todos"));
        assert!(tool_names.contains(&"tokensave_record_decision"));
        assert!(tool_names.contains(&"tokensave_record_code_area"));
        assert!(tool_names.contains(&"tokensave_session_recall"));
        assert!(tool_names.contains(&"tokensave_read"));
        assert!(tool_names.contains(&"tokensave_entities"));
        assert!(!tool_names.contains(&"tokensave_outline"));
        assert!(tool_names.contains(&"tokensave_implementations"));
        assert!(tool_names.contains(&"tokensave_unsafe_patterns"));
        assert!(tool_names.contains(&"tokensave_diagnostics"));
        assert!(tool_names.contains(&"tokensave_config"));
        assert!(tool_names.contains(&"tokensave_signature_search"));
        assert!(tool_names.contains(&"tokensave_constructors"));
        assert!(tool_names.contains(&"tokensave_field_sites"));
        assert!(tool_names.contains(&"tokensave_call_chain"));
        assert!(tool_names.contains(&"tokensave_file_dependents"));
        assert!(tool_names.contains(&"tokensave_replace_symbol"));
        assert!(tool_names.contains(&"tokensave_insert_at_symbol"));
        assert!(tool_names.contains(&"tokensave_find_exact_symbol"));
        assert!(tool_names.contains(&"tokensave_blame"));
        assert!(tool_names.contains(&"tokensave_log"));
        assert!(tool_names.contains(&"tokensave_diff"));
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
            "tokensave_session_end",
            "tokensave_record_decision",
            "tokensave_record_code_area",
            // Tools defined via `def_rw` (mutate files / run subprocesses).
            "tokensave_replace_symbol",
            "tokensave_insert_at_symbol",
            "tokensave_run_affected_tests",
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
        // Structural call-graph tools promoted to alwaysLoad in #333.
        assert!(
            always_load.contains(&"tokensave_impact"),
            "tokensave_impact must be alwaysLoad"
        );
        assert!(
            always_load.contains(&"tokensave_callees"),
            "tokensave_callees must be alwaysLoad"
        );
        assert_eq!(
            always_load.len(),
            5,
            "exactly 5 tools should be alwaysLoad, got {:?}",
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

    #[test]
    fn diff_tool_is_registered() {
        let tools = get_tool_definitions();
        assert!(tools.iter().any(|t| t.name == "tokensave_diff"));
    }

    #[test]
    fn filter_by_path_lists_empty_lists_unchanged() {
        let items = vec!["src/a.rs", "vendor/b.rs"];
        let out = filter_by_path_lists(items.clone(), &[], &[], |s| s);
        assert_eq!(out, items);
    }

    #[test]
    fn filter_by_path_lists_exclude_drops_match() {
        let items = vec!["src/a.rs", "vendor/b.rs"];
        let out = filter_by_path_lists(items, &[], &["vendor".to_string()], |s| s);
        assert_eq!(out, vec!["src/a.rs"]);
    }

    #[test]
    fn filter_by_path_lists_include_keeps_only_match() {
        let items = vec!["src/a.rs", "vendor/b.rs"];
        let out = filter_by_path_lists(items, &["vendor".to_string()], &[], |s| s);
        assert_eq!(out, vec!["vendor/b.rs"]);
    }

    #[test]
    fn filter_by_path_lists_exclude_takes_precedence() {
        let items = vec!["src/a.rs", "vendor/b.rs"];
        // "b" matches include for vendor, but vendor is also excluded → dropped.
        let out = filter_by_path_lists(
            items,
            &["b.rs".to_string(), "a.rs".to_string()],
            &["vendor".to_string()],
            |s| s,
        );
        assert_eq!(out, vec!["src/a.rs"]);
    }

    #[test]
    fn filter_by_path_lists_normalizes_backslashes() {
        let items = vec!["src\\a.rs", "vendor\\b.rs"];
        let out = filter_by_path_lists(items, &["src/".to_string()], &[], |s| s);
        assert_eq!(out, vec!["src\\a.rs"]);
    }

    #[test]
    fn filter_by_path_lists_normalizes_backslash_substrings() {
        // Windows caller passes backslash filters against canonical
        // forward-slash stored paths (#204).
        let items = vec!["apps/admin/src/x.tsx", "apps/web/src/y.tsx"];
        let out = filter_by_path_lists(items, &["apps\\admin\\src".to_string()], &[], |s| s);
        assert_eq!(out, vec!["apps/admin/src/x.tsx"]);
        let items = vec!["apps/admin/src/x.tsx", "apps/web/src/y.tsx"];
        let out = filter_by_path_lists(items, &[], &["apps\\web".to_string()], |s| s);
        assert_eq!(out, vec!["apps/admin/src/x.tsx"]);
    }

    #[test]
    fn normalize_path_args_rewrites_path_shaped_keys() {
        let mut args = serde_json::json!({
            "file": "apps\\admin\\src\\StatCard.tsx",
            "path": "apps\\admin",
            "path_include": ["apps\\admin\\src", "already/fine"],
            "path_exclude": ["node_modules\\x"],
            "query": "leave\\alone",
        });
        normalize_path_args(&mut args, false);
        assert_eq!(args["file"], "apps/admin/src/StatCard.tsx");
        assert_eq!(args["path"], "apps/admin");
        assert_eq!(args["path_include"][0], "apps/admin/src");
        assert_eq!(args["path_include"][1], "already/fine");
        assert_eq!(args["path_exclude"][0], "node_modules/x");
        // Non-path keys are untouched.
        assert_eq!(args["query"], "leave\\alone");
    }

    #[test]
    fn normalize_path_args_leaves_verbatim_unc_paths_alone() {
        let mut args = serde_json::json!({
            "path": "\\\\?\\C:\\repo\\src",
            "file": "C:\\repo\\src\\a.rs",
        });
        normalize_path_args(&mut args, false);
        // Verbatim prefix must not be rewritten; drive-letter paths are,
        // since preserve_drive_absolute is false here.
        assert_eq!(args["path"], "\\\\?\\C:\\repo\\src");
        assert_eq!(args["file"], "C:/repo/src/a.rs");
    }

    #[test]
    fn normalize_path_args_preserves_drive_absolute_for_edit_tools() {
        let mut args = serde_json::json!({
            "path": "C:\\Users\\dev\\notes.txt",
        });
        normalize_path_args(&mut args, true);
        assert_eq!(args["path"], "C:\\Users\\dev\\notes.txt");
    }
}
