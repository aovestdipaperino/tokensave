//! Structural analysis tool handlers: `dead_code`, `hotspots`, `circular`,
//! `coupling`, `rank`, `largest`, `recursion`, `complexity`, `distribution`,
//! `unused_imports`, `god_class`, `doc_coverage`, `inheritance_depth`, `module_api`.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::{NodeKind, Visibility};

use super::super::ToolResult;
use super::{effective_path, filter_by_scope, truncate_response, unique_file_paths};

/// True if `line` contains `identifier` as a whole token (boundaries are
/// any non-`[A-Za-z0-9_]` char or string ends). Avoids false positives
/// from substring matches like `Map` inside `HashMap`.
fn has_identifier_match(line: &str, identifier: &str) -> bool {
    debug_assert!(!identifier.is_empty(), "identifier must be non-empty");
    let bytes = line.as_bytes();
    let id_bytes = identifier.as_bytes();
    let id_len = id_bytes.len();
    if bytes.len() < id_len {
        return false;
    }
    let mut i = 0;
    while i + id_len <= bytes.len() {
        if &bytes[i..i + id_len] == id_bytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + id_len == bytes.len() || !is_ident_byte(bytes[i + id_len]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Returns the identifiers a `use` statement brings into scope, parsing
/// grouped and aliased forms. Examples:
///   `foo::bar`             → bar
///   `foo::bar as baz`      → baz
///   `foo::{a, b}`          → a, b
///   `foo::{a, b as c}`     → a, c
///   `foo::{a, nested::b}`  → a, b
///   `foo::{self, bar}`     → foo, bar   (self brings the module in)
///   `foo::*`               → (empty, glob — handled separately)
fn identifiers_from_use_path(path: &str) -> Vec<String> {
    let trimmed = path.trim().trim_end_matches(';').trim();
    if trimmed.ends_with('*') {
        return Vec::new();
    }
    if let (Some(open), Some(close)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if close <= open {
            return Vec::new();
        }
        let prefix = trimmed[..open].trim().trim_end_matches("::").trim();
        let parent = prefix
            .rsplit("::")
            .next()
            .unwrap_or(prefix)
            .trim()
            .to_string();
        let inside = &trimmed[open + 1..close];
        let mut out: Vec<String> = Vec::new();
        let mut depth = 0i32;
        let mut start = 0usize;
        let bytes = inside.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                b',' if depth == 0 => {
                    let item = &inside[start..i];
                    push_identifier(&mut out, item, &parent);
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        push_identifier(&mut out, &inside[start..], &parent);
        return out;
    }
    let last_seg = trimmed.rsplit("::").next().unwrap_or(trimmed).trim();
    let id = identifier_from_segment(last_seg);
    if id.is_empty() || id == "*" {
        Vec::new()
    } else {
        vec![id]
    }
}

fn push_identifier(out: &mut Vec<String>, item: &str, parent: &str) {
    let item = item.trim();
    if item.is_empty() {
        return;
    }
    // Nested group: `foo::{a, sub::{x, y}}` — recurse on the nested part.
    if item.contains('{') {
        for id in identifiers_from_use_path(item) {
            out.push(id);
        }
        return;
    }
    let last_seg = item.rsplit("::").next().unwrap_or(item).trim();
    let id = identifier_from_segment(last_seg);
    if id.is_empty() {
        return;
    }
    if id == "self" {
        // `use foo::{self, bar}` brings `foo` itself into scope.
        if !parent.is_empty() {
            out.push(parent.to_string());
        }
        return;
    }
    if id == "*" {
        return;
    }
    out.push(id);
}

/// Resolves a single use-tree segment (no `::`) into the identifier it
/// brings into scope, accounting for `as` aliases.
fn identifier_from_segment(seg: &str) -> String {
    let seg = seg.trim().trim_end_matches(';').trim();
    if seg.is_empty() {
        return String::new();
    }
    // `foo as bar` → keep `bar`.
    let after_as = seg.split_whitespace().collect::<Vec<_>>();
    if let Some(pos) = after_as.iter().position(|w| *w == "as") {
        if let Some(alias) = after_as.get(pos + 1) {
            return (*alias).to_string();
        }
    }
    seg.split_whitespace()
        .next()
        .unwrap_or(seg)
        .trim()
        .to_string()
}

/// Handles `tokensave_dead_code` tool calls.
pub(super) async fn handle_dead_code(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let kinds: Vec<NodeKind> = args.get("kinds").and_then(|v| v.as_array()).map_or_else(
        || vec![NodeKind::Function, NodeKind::Method],
        |arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(NodeKind::from_str))
                .collect()
        },
    );

    let include_public = args
        .get("include_public")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let dead = cg.find_dead_code(&kinds, include_public).await?;
    let dead = filter_by_scope(dead, scope_prefix, |n| &n.file_path);

    let touched_files = unique_file_paths(dead.iter().map(|n| n.file_path.as_str()));

    let items: Vec<Value> = dead
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
                "signature": n.signature,
            })
        })
        .collect();

    let output = json!({
        "dead_code_count": items.len(),
        "symbols": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_module_api` tool calls.
pub(super) async fn handle_module_api(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path = effective_path(&args, scope_prefix).ok_or_else(|| TokenSaveError::Config {
        message: "missing required parameter: path".to_string(),
    })?;

    let all_nodes = cg.get_all_nodes().await?;

    // Filter to nodes in matching files (exact path or directory prefix)
    let prefix = if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    };

    let mut pub_nodes: Vec<&crate::types::Node> = all_nodes
        .iter()
        .filter(|n| {
            n.visibility == Visibility::Pub
                && (n.file_path == path || n.file_path.starts_with(&prefix))
        })
        .collect();

    pub_nodes.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.start_line.cmp(&b.start_line))
    });

    let touched_files = unique_file_paths(pub_nodes.iter().map(|n| n.file_path.as_str()));

    let items: Vec<Value> = pub_nodes
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": n.start_line,
                "signature": n.signature,
            })
        })
        .collect();

    let output = json!({
        "path": path,
        "public_symbol_count": items.len(),
        "symbols": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_circular` tool calls.
pub(super) async fn handle_circular(cg: &TokenSave, _args: Value) -> Result<ToolResult> {
    let cycles = cg.find_circular_dependencies().await?;

    let items: Vec<Value> = cycles.iter().map(|cycle| json!(cycle)).collect();

    let output = json!({
        "cycle_count": cycles.len(),
        "cycles": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_hotspots` tool calls.
pub(super) async fn handle_hotspots(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);
    debug_assert!(limit > 0, "handle_hotspots limit must be positive");

    let all_edges = cg.get_all_edges().await?;

    // Count incoming + outgoing edges per node
    let mut connectivity: HashMap<String, (usize, usize)> = HashMap::new();
    for edge in &all_edges {
        connectivity.entry(edge.source.clone()).or_insert((0, 0)).1 += 1; // outgoing
        connectivity.entry(edge.target.clone()).or_insert((0, 0)).0 += 1; // incoming
    }

    // Sort by total connectivity descending
    let mut sorted: Vec<(String, usize, usize)> = connectivity
        .into_iter()
        .map(|(id, (inc, out))| (id, inc, out))
        .collect();
    sorted.sort_by_key(|x| std::cmp::Reverse(x.1 + x.2));
    sorted.truncate(limit);

    // Resolve node details
    let mut items: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    for (node_id, incoming, outgoing) in &sorted {
        if let Some(node) = cg.get_node(node_id).await? {
            touched.push(node.file_path.clone());
            items.push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "incoming": incoming,
                "outgoing": outgoing,
                "total": incoming + outgoing,
            }));
        }
    }

    if let Some(prefix) = scope_prefix {
        let with_slash = if prefix.ends_with('/') {
            prefix.to_string()
        } else {
            format!("{prefix}/")
        };
        items.retain(|item| {
            item["file"]
                .as_str()
                .is_some_and(|f| f.starts_with(&with_slash) || f == prefix)
        });
        touched.retain(|f| f.starts_with(&with_slash) || f == prefix);
    }

    let touched_files = unique_file_paths(touched.iter().map(std::string::String::as_str));

    let output = json!({
        "hotspot_count": items.len(),
        "hotspots": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_unused_imports` tool calls.
pub(super) async fn handle_unused_imports(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let _ = args; // currently unused beyond scope filtering
    let all_nodes = cg.get_all_nodes().await?;

    // Find all Use nodes
    let use_nodes: Vec<&crate::types::Node> = all_nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .filter(|n| {
            scope_prefix.is_none_or(|prefix| {
                let with_slash = if prefix.ends_with('/') {
                    prefix.to_string()
                } else {
                    format!("{prefix}/")
                };
                n.file_path.starts_with(&with_slash) || n.file_path == prefix
            })
        })
        .collect();

    let mut unused: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();

    // Source-text fallback (cheap + cached per file): every Use node is
    // potentially unused if the imported identifier appears nowhere else in
    // the file body. The previous graph-only check was unreliable because
    // the Rust resolver doesn't create `Uses` edges for std/foreign-crate
    // imports — every `use std::collections::HashSet` had no outgoing edge
    // regardless of whether it was actually referenced.
    //
    // `pub use` re-exports are intentional public aliases; we never report
    // them as unused.
    let project_root = cg.project_root();
    let mut file_cache: HashMap<String, Option<String>> = HashMap::new();
    for use_node in &use_nodes {
        if use_node.visibility == crate::types::Visibility::Pub {
            continue;
        }
        // The Use node's `name` is the full import path as written. Three
        // shapes show up in real Rust code:
        //   - `foo::bar`           → single identifier `bar`
        //   - `foo::bar as baz`    → single identifier `baz`
        //   - `foo::{a, b as c}`   → grouped: identifiers `a`, `c`
        // The previous version only handled the first two: it took the last
        // `::` segment and treated the literal string `{a, b as c}` as one
        // identifier, which never matched anything and therefore either
        // flagged every grouped import (false positive) or missed unused
        // members inside a partially-used group (false negative). Real
        // codebases lean heavily on grouped imports.
        let identifiers = identifiers_from_use_path(&use_node.name);
        if identifiers.is_empty() {
            continue;
        }

        let source = file_cache
            .entry(use_node.file_path.clone())
            .or_insert_with(|| {
                let abs = project_root.join(&use_node.file_path);
                std::fs::read_to_string(&abs).ok()
            })
            .clone();
        let Some(source) = source else {
            continue;
        };

        for identifier in &identifiers {
            // Count word-boundary occurrences of the identifier outside the
            // use statement's own line range. If zero non-use references
            // appear, this particular identifier is unused.
            let mut found = false;
            for (line_idx, line) in source.lines().enumerate() {
                let line_idx = line_idx as u32;
                if line_idx >= use_node.start_line && line_idx <= use_node.end_line {
                    continue;
                }
                if has_identifier_match(line, identifier) {
                    found = true;
                    break;
                }
            }
            if !found {
                touched.push(use_node.file_path.clone());
                unused.push(json!({
                    "id": use_node.id,
                    "name": use_node.name,
                    "unused": identifier,
                    "file": use_node.file_path,
                    "line": use_node.start_line,
                }));
            }
        }
    }

    let touched_files = unique_file_paths(touched.iter().map(std::string::String::as_str));

    let output = json!({
        "unused_import_count": unused.len(),
        "imports": unused,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_rank` tool calls.
pub(super) async fn handle_rank(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    use crate::types::EdgeKind;
    debug_assert!(args.is_object(), "handle_rank expects an object argument");

    let edge_kind_str = args
        .get("edge_kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: edge_kind".to_string(),
        })?;

    let edge_kind = EdgeKind::from_str(edge_kind_str).ok_or_else(|| TokenSaveError::Config {
        message: format!(
            "invalid edge_kind '{edge_kind_str}'. Valid values: implements, extends, calls, uses, contains, annotates, derives_macro"
        ),
    })?;

    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("incoming");

    let incoming = match direction {
        "incoming" => true,
        "outgoing" => false,
        _ => {
            return Err(TokenSaveError::Config {
                message: format!(
                    "invalid direction '{direction}'. Valid values: incoming, outgoing"
                ),
            });
        }
    };

    let node_kind = args
        .get("node_kind")
        .and_then(|v| v.as_str())
        .and_then(NodeKind::from_str);

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    let path_prefix = effective_path(&args, scope_prefix);

    let results = cg
        .get_ranked_nodes_by_edge_kind(&edge_kind, node_kind.as_ref(), incoming, path_prefix, limit)
        .await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, count)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "count": count,
            })
        })
        .collect();

    let output = json!({
        "edge_kind": edge_kind_str,
        "direction": direction,
        "node_kind_filter": args.get("node_kind").and_then(|v| v.as_str()),
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_largest` tool calls.
pub(super) async fn handle_largest(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let node_kind = args
        .get("node_kind")
        .and_then(|v| v.as_str())
        .and_then(NodeKind::from_str);

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    let path_prefix = effective_path(&args, scope_prefix);

    let results = cg
        .get_largest_nodes(node_kind.as_ref(), path_prefix, limit)
        .await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, lines)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "start_line": node.start_line,
                "end_line": node.end_line,
                "lines": lines,
            })
        })
        .collect();

    let output = json!({
        "node_kind_filter": args.get("node_kind").and_then(|v| v.as_str()),
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_coupling` tool calls.
pub(super) async fn handle_coupling(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("fan_in");

    let fan_in = match direction {
        "fan_in" => true,
        "fan_out" => false,
        _ => {
            return Err(TokenSaveError::Config {
                message: format!("invalid direction '{direction}'. Valid values: fan_in, fan_out"),
            });
        }
    };

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    let path_prefix = effective_path(&args, scope_prefix);

    let results = cg.get_file_coupling(fan_in, path_prefix, limit).await?;

    let items: Vec<Value> = results
        .iter()
        .map(|(file, count)| {
            json!({
                "file": file,
                "coupled_files": count,
            })
        })
        .collect();

    let output = json!({
        "direction": direction,
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_inheritance_depth` tool calls.
pub(super) async fn handle_inheritance_depth(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    let path_prefix = effective_path(&args, scope_prefix);

    let results = cg.get_inheritance_depth(path_prefix, limit).await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, depth)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "depth": depth,
            })
        })
        .collect();

    let output = json!({
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_distribution` tool calls.
pub(super) async fn handle_distribution(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    debug_assert!(
        args.is_object(),
        "handle_distribution expects an object argument"
    );
    let path_prefix = effective_path(&args, scope_prefix);
    let summary = args
        .get("summary")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let results = cg.get_node_distribution(path_prefix).await?;

    let output = if summary {
        // Aggregate counts across all files
        let mut totals: HashMap<String, u64> = HashMap::new();
        for (_file, kind, count) in &results {
            *totals.entry(kind.clone()).or_insert(0) += count;
        }
        let mut sorted: Vec<(String, u64)> = totals.into_iter().collect();
        sorted.sort_by_key(|x| std::cmp::Reverse(x.1));

        let items: Vec<Value> = sorted
            .iter()
            .map(|(kind, count)| json!({ "kind": kind, "count": count }))
            .collect();

        json!({
            "path_filter": path_prefix,
            "mode": "summary",
            "total_kinds": items.len(),
            "distribution": items,
        })
    } else {
        // Per-file breakdown, grouped by file
        let mut by_file: Vec<(String, Vec<Value>)> = Vec::new();
        let mut current_file = String::new();
        for (file, kind, count) in &results {
            if *file != current_file {
                current_file.clone_from(file);
                by_file.push((file.clone(), Vec::new()));
            }
            if let Some(last) = by_file.last_mut() {
                last.1.push(json!({ "kind": kind, "count": count }));
            }
        }

        let items: Vec<Value> = by_file
            .iter()
            .map(|(file, kinds)| json!({ "file": file, "kinds": kinds }))
            .collect();

        json!({
            "path_filter": path_prefix,
            "mode": "per_file",
            "file_count": items.len(),
            "files": items,
        })
    };

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_recursion` tool calls.
///
/// Detects cycles in the call graph using iterative DFS on the calls-only
/// edge subgraph. Each cycle is a vec of node IDs forming the loop.
pub(super) async fn handle_recursion(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);
    let path_prefix = effective_path(&args, scope_prefix);

    debug_assert!(limit > 0, "handle_recursion limit must be positive");

    let call_edges = cg.get_call_edges_with_lines(path_prefix).await?;

    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    let mut node_cache: HashMap<String, Option<crate::types::Node>> = HashMap::new();
    let mut source_cache: HashMap<String, Option<String>> = HashMap::new();

    for (src, tgt, line) in &call_edges {
        if src == tgt {
            let Some(node) = cached_node(cg, &mut node_cache, src).await? else {
                continue;
            };
            if !is_direct_self_call(cg, &mut source_cache, &node, *line) {
                continue;
            }
        }
        adj.entry(src.clone()).or_default().insert(tgt.clone());
        adj.entry(tgt.clone()).or_default();
    }

    let mut cycles: Vec<Vec<String>> = crate::graph::scc::tarjan_scc(&adj)
        .into_iter()
        .filter(|scc| crate::graph::scc::is_cyclic_scc(scc, &adj))
        .filter_map(|mut scc| cycle_path_for_scc(&mut scc, &adj))
        .collect();

    cycles.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    cycles.truncate(limit);

    // Resolve node details for each cycle
    let mut cycle_items: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    for cycle in &cycles {
        let mut chain: Vec<Value> = Vec::new();
        for node_id in cycle {
            if let Some(node) = cg.get_node(node_id).await? {
                touched.push(node.file_path.clone());
                chain.push(json!({
                    "id": node.id,
                    "name": node.name,
                    "kind": node.kind.as_str(),
                    "file": node.file_path,
                    "line": node.start_line,
                }));
            } else {
                chain.push(json!({ "id": node_id }));
            }
        }
        cycle_items.push(json!({
            "length": cycle.len() - 1,
            "chain": chain,
        }));
    }

    let touched_files = unique_file_paths(touched.iter().map(std::string::String::as_str));

    let output = json!({
        "cycle_count": cycle_items.len(),
        "cycles": cycle_items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

async fn cached_node(
    cg: &TokenSave,
    cache: &mut HashMap<String, Option<crate::types::Node>>,
    id: &str,
) -> Result<Option<crate::types::Node>> {
    if let Some(node) = cache.get(id) {
        return Ok(node.clone());
    }
    let node = cg.get_node(id).await?;
    cache.insert(id.to_string(), node.clone());
    Ok(node)
}

fn cached_source(
    cg: &TokenSave,
    cache: &mut HashMap<String, Option<String>>,
    file_path: &str,
) -> Option<String> {
    if let Some(source) = cache.get(file_path) {
        return source.clone();
    }
    let abs = cg.project_root().join(file_path);
    let source = std::fs::read_to_string(abs).ok();
    cache.insert(file_path.to_string(), source.clone());
    source
}

fn is_direct_self_call(
    cg: &TokenSave,
    source_cache: &mut HashMap<String, Option<String>>,
    node: &crate::types::Node,
    edge_line: Option<u32>,
) -> bool {
    let Some(source) = cached_source(cg, source_cache, &node.file_path) else {
        return false;
    };
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return false;
    }

    let mut candidate_lines: Vec<u32> = edge_line.into_iter().collect();
    if let Some(line) = edge_line {
        candidate_lines.push(line.saturating_sub(1));
        candidate_lines.push(line.saturating_add(1));
    }
    candidate_lines.sort_unstable();
    candidate_lines.dedup();

    for line in candidate_lines {
        let Some(text) = lines.get(line as usize) else {
            continue;
        };
        if looks_like_function_declaration(text, &node.name) {
            continue;
        }
        if has_qualified_call(text, node) || has_bare_call(text, &node.name) {
            return true;
        }
    }

    false
}

fn looks_like_function_declaration(line: &str, name: &str) -> bool {
    let Some(pos) = line.find(name) else {
        return false;
    };
    let prefix = &line[..pos];
    (prefix.contains("fn ")
        || prefix.contains("function ")
        || prefix.contains("def ")
        || prefix.contains("sub "))
        && call_suffix_starts(&line[pos + name.len()..])
}

fn parent_type_name(node: &crate::types::Node) -> Option<&str> {
    let needle = format!("::{}", node.name);
    node.qualified_name
        .strip_suffix(&needle)
        .and_then(|parent| parent.rsplit("::").next())
        .filter(|parent| !parent.is_empty())
}

fn has_qualified_call(line: &str, node: &crate::types::Node) -> bool {
    let Some(parent) = parent_type_name(node) else {
        return false;
    };
    let type_call = format!("{parent}::{}", node.name);
    if line
        .match_indices(&type_call)
        .any(|(idx, _)| call_suffix_starts(&line[idx + type_call.len()..]))
    {
        return true;
    }

    let self_call = format!("Self::{}", node.name);
    if line
        .match_indices(&self_call)
        .any(|(idx, _)| call_suffix_starts(&line[idx + self_call.len()..]))
    {
        return true;
    }

    let self_method_call = format!("self.{}", node.name);
    line.match_indices(&self_method_call)
        .any(|(idx, _)| call_suffix_starts(&line[idx + self_method_call.len()..]))
}

fn has_bare_call(line: &str, name: &str) -> bool {
    line.match_indices(name).any(|(idx, _)| {
        let before_ok = idx == 0 || !is_ident_byte(line.as_bytes()[idx - 1]);
        if before_ok {
            let prefix = line[..idx].trim_end();
            if prefix.ends_with('.') || prefix.ends_with(':') {
                return false;
            }
        }
        let after_idx = idx + name.len();
        before_ok && call_suffix_starts(&line[after_idx..])
    })
}

fn call_suffix_starts(suffix: &str) -> bool {
    suffix.trim_start().starts_with('(')
}

fn cycle_path_for_scc(
    scc: &mut [String],
    adj: &HashMap<String, HashSet<String>>,
) -> Option<Vec<String>> {
    scc.sort();
    let scc_set: HashSet<&str> = scc.iter().map(std::string::String::as_str).collect();
    if scc.len() == 1 {
        let id = scc[0].clone();
        if adj
            .get(&id)
            .is_some_and(|neighbors| neighbors.contains(&id))
        {
            return Some(vec![id.clone(), id]);
        }
        return None;
    }

    for start in scc.iter() {
        let mut path = vec![start.clone()];
        let mut seen: HashSet<String> = HashSet::from([start.clone()]);
        if dfs_cycle_path(start, start, &scc_set, adj, &mut path, &mut seen) {
            return Some(path);
        }
    }
    None
}

fn dfs_cycle_path(
    current: &str,
    start: &str,
    scc_set: &HashSet<&str>,
    adj: &HashMap<String, HashSet<String>>,
    path: &mut Vec<String>,
    seen: &mut HashSet<String>,
) -> bool {
    let Some(neighbors) = adj.get(current) else {
        return false;
    };
    let mut neighbors: Vec<&str> = neighbors
        .iter()
        .map(std::string::String::as_str)
        .filter(|n| scc_set.contains(n))
        .collect();
    neighbors.sort_unstable();

    for neighbor in neighbors {
        if neighbor == start && path.len() > 1 {
            path.push(start.to_string());
            return true;
        }
        if !seen.insert(neighbor.to_string()) {
            continue;
        }
        path.push(neighbor.to_string());
        if dfs_cycle_path(neighbor, start, scc_set, adj, path, seen) {
            return true;
        }
        path.pop();
        seen.remove(neighbor);
    }
    false
}

/// Handles `tokensave_complexity` tool calls.
pub(super) async fn handle_complexity(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let node_kind = args
        .get("node_kind")
        .and_then(|v| v.as_str())
        .and_then(NodeKind::from_str);

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    let path_prefix = effective_path(&args, scope_prefix);

    let results = cg
        .get_complexity_ranked(node_kind.as_ref(), path_prefix, limit)
        .await?;

    let touched_files =
        unique_file_paths(results.iter().map(|(n, _, _, _, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, lines, fan_out, fan_in, score)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "lines": lines,
                "cyclomatic_complexity": node.branches + 1,
                "branches": node.branches,
                "loops": node.loops,
                "returns": node.returns,
                "max_nesting": node.max_nesting,
                "unsafe_blocks": node.unsafe_blocks,
                "unchecked_calls": node.unchecked_calls,
                "assertions": node.assertions,
                "fan_out": fan_out,
                "fan_in": fan_in,
                "score": score,
            })
        })
        .collect();

    let output = json!({
        "formula": "lines + (fan_out × 3) + fan_in",
        "note": "cyclomatic_complexity = branches + 1 (computed from AST during extraction)",
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_doc_coverage` tool calls.
pub(super) async fn handle_doc_coverage(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path_prefix = effective_path(&args, scope_prefix);

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(50, |v| v.min(500) as usize);

    let results = cg
        .get_undocumented_public_symbols(path_prefix, limit)
        .await?;

    let touched_files = unique_file_paths(results.iter().map(|n| n.file_path.as_str()));

    // Group by file for readability
    let mut by_file: HashMap<String, Vec<Value>> = HashMap::new();
    for node in &results {
        by_file
            .entry(node.file_path.clone())
            .or_default()
            .push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "line": node.start_line,
                "signature": node.signature,
            }));
    }

    let mut file_items: Vec<Value> = by_file
        .into_iter()
        .map(|(file, symbols)| {
            json!({
                "file": file,
                "count": symbols.len(),
                "symbols": symbols,
            })
        })
        .collect();
    file_items.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));

    let output = json!({
        "path_filter": path_prefix,
        "total_undocumented": results.len(),
        "file_count": file_items.len(),
        "files": file_items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_god_class` tool calls.
pub(super) async fn handle_god_class(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(100) as usize);

    let path_prefix = effective_path(&args, scope_prefix);

    let results = cg.get_god_classes(path_prefix, limit).await?;

    let touched_files = unique_file_paths(results.iter().map(|(n, _, _, _)| n.file_path.as_str()));

    let items: Vec<Value> = results
        .iter()
        .map(|(node, methods, fields, total)| {
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
                "methods": methods,
                "fields": fields,
                "total_members": total,
            })
        })
        .collect();

    let output = json!({
        "result_count": items.len(),
        "ranking": items,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}
