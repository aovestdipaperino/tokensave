//! Structural analysis tool handlers: `dead_code`, `hotspots`, `circular`,
//! `coupling`, `rank`, `largest`, `recursion`, `complexity`, `distribution`,
//! `unused_imports`, `god_class`, `doc_coverage`, `inheritance_depth`, `module_api`.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;
use crate::types::NodeKind;

use super::super::ToolResult;
use super::{
    effective_path, filter_by_path_lists, filter_by_scope, parse_string_array, truncate_response,
    unique_file_paths, with_defaults,
};

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

/// Returns the identifier a Go import brings into scope.
///
/// Go imports are slash-separated paths, not `::`-separated, so the generic
/// [`identifiers_from_use_path`] would treat `net/url` as a single token that
/// never matches the `url.Parse` call site — flagging every import as unused
/// (#148). The in-scope identifier is the alias (`u "net/url"` → `u`, encoded
/// as `"net/url as u"`) or, for a plain import, the package name — which for a
/// semantic-import-versioning path (`github.com/jackc/pgx/v5`) is the segment
/// *before* the `/vN` marker (`pgx`), not the literal last segment (`v5`)
/// (#149 Bug 2). Blank (`_`) and dot (`.`) imports are stored as `Pub` Use
/// nodes and are filtered out before this is ever called.
///
/// Delegates to [`crate::go_import::import_identifier`] so the resolver
/// (#149 Bug 1) derives the same qualifier.
fn go_import_identifier(name: &str) -> Option<String> {
    crate::go_import::import_identifier(name)
}

/// Returns the identifier a Python import statement binds into scope.
///
/// The Python extractor's Use node `name` is a dotted path (`enum.StrEnum`
/// for `from enum import StrEnum`, `os.path` for `import os.path`), not the
/// bound identifier, so the generic `::`-based [`identifiers_from_use_path`]
/// returns the literal dotted string, which never matches the plain
/// `StrEnum` reference in `class Colour(StrEnum)` (#224).
///
/// Python import semantics fix which segment binds the name:
/// - `from a.b import c` → the **last** `.`-segment of `name` (`c`)
/// - `import a.b.c` → the **first** `.`-segment of `name` (`a`)
///
/// An `as` alias always wins over either rule. `signature` (the statement's
/// source text) is needed to detect it, but a single Python `import`/`from`
/// statement can bind several names on one line (`import a, b as c`), all of
/// which share one Use-node-creating statement and therefore one `signature`
/// string — so an alias can't just be read off the tail of `signature`, or
/// `a`'s Use node would pick up `b`'s alias. Instead, the segment that
/// *would* be returned by the two rules above is located as a whole word
/// inside the region of `signature` **after the `import` keyword**, and only
/// an ` as <alias>` immediately following that specific occurrence is
/// honored — `name` itself is always this node's own dotted path (extracted
/// per-import, never shared), so this correctly isolates each name's own
/// alias even when several share one statement. Restricting the search to
/// after `import` matters when the module and the imported name are the
/// same word (`from datetime import datetime as dt`): searching the whole
/// statement would find the *module* occurrence first — its trailing text
/// is `import datetime as dt`, not an `as` clause — and silently drop the
/// alias.
///
/// Wildcard imports (`name` ends in `.*`) return `None` (caller already
/// skips these).
fn python_import_identifier(name: &str, signature: &str) -> Option<String> {
    if name.is_empty() || name.ends_with(".*") || name == "*" {
        return None;
    }
    let sig = signature.trim();
    let is_from = sig.starts_with("from ");
    // For `from a.b import c`, only `c` appears verbatim in the source next
    // to a possible `as` alias — `a.b.c` is never written. For `import
    // a.b.c`, the *whole* dotted path is what a trailing `as x` attaches to
    // (`import a.b.c as x` binds `x`, not `a`), so search on the full name.
    let (search_key, base) = if is_from {
        let last = name.rsplit('.').next().unwrap_or(name);
        (last, last)
    } else {
        let first = name.split('.').next().unwrap_or(name);
        (name, first)
    };
    if base.is_empty() {
        return None;
    }
    // Only the region *after* the `import` keyword can contain a bound name
    // or its alias — the `from MOD` module part (present only in the `from`
    // form) precedes it and can collide with the imported name itself
    // (`from datetime import datetime as dt`: searching the whole signature
    // for `datetime` finds the *module* occurrence first, whose trailing
    // text is `import datetime as dt` — not an `as` clause — silently
    // dropping the alias and reporting the import unused despite `dt.now()`
    // being called). Restricting the search to after `import` skips that
    // collision; every other shape (no `from MOD` prefix to collide with)
    // is unaffected.
    let region_start = find_word_pos(sig, "import").map_or(0, |p| p + "import".len());
    let region = &sig[region_start..];
    if let Some(pos) = find_word_pos(region, search_key) {
        let after = region[pos + search_key.len()..].trim_start();
        if let Some(alias_part) = after.strip_prefix("as ") {
            let alias = alias_part
                .split(|c: char| c == ',' || c.is_whitespace())
                .next()
                .unwrap_or("")
                .trim();
            if !alias.is_empty() {
                return Some(alias.to_string());
            }
        }
    }
    Some(base.to_string())
}

/// Locates `key` as a whole token within `text` (boundaries are any
/// non-identifier byte or string ends), mirroring [`has_identifier_match`]
/// but returning the match position instead of a boolean. `key` may itself
/// contain `.` (a dotted import path) — only the two ends are checked
/// against a boundary; the position returned is the first match.
fn find_word_pos(text: &str, key: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let key_bytes = key.as_bytes();
    let key_len = key_bytes.len();
    if key_len == 0 || bytes.len() < key_len {
        return None;
    }
    let mut i = 0;
    while i + key_len <= bytes.len() {
        if &bytes[i..i + key_len] == key_bytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + key_len == bytes.len() || !is_ident_byte(bytes[i + key_len]);
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
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

/// Parses the identifiers bound by a TypeScript/JavaScript import statement
/// from its source text (#207).
///
/// Handles `import Default from 'm'`, `import { a, b as c } from 'm'`,
/// `import * as ns from 'm'`, `import type { T } from 'm'`, mixed
/// default+named forms, and side-effect imports (`import 'm'` → no
/// identifiers, never reported).
fn ts_import_identifiers(signature: &str) -> Vec<String> {
    let text = signature.trim();
    let Some(rest) = text.strip_prefix("import") else {
        return Vec::new();
    };
    // Clause is everything before ` from ` (side-effect imports have none).
    let clause = match rest.find(" from ") {
        Some(pos) => &rest[..pos],
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    // Split the clause into default part and braced part.
    let (outside, braced) = match (clause.find('{'), clause.rfind('}')) {
        (Some(o), Some(c)) if c > o => (
            format!("{}{}", &clause[..o], &clause[c + 1..]),
            Some(&clause[o + 1..c]),
        ),
        _ => (clause.to_string(), None),
    };
    // Outside the braces: `type`, a default import, and/or `* as ns`.
    for seg in outside.split(',') {
        let seg = seg.trim().trim_start_matches("type ").trim();
        // `import type { ... }` leaves a bare `type` keyword outside the braces.
        if seg.is_empty() || seg == "type" {
            continue;
        }
        if let Some(star) = seg.strip_prefix('*') {
            if let Some(ns) = star.trim().strip_prefix("as ") {
                out.push(ns.trim().to_string());
            }
            continue;
        }
        if seg
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        {
            out.push(seg.to_string());
        }
    }
    // Inside the braces: named imports, possibly aliased or `type`-qualified.
    if let Some(braced) = braced {
        for seg in braced.split(',') {
            let seg = seg.trim().trim_start_matches("type ").trim();
            if seg.is_empty() {
                continue;
            }
            let name = match seg.split_whitespace().collect::<Vec<_>>() {
                ref w if w.len() >= 3 && w[1] == "as" => w[2].to_string(),
                ref w if !w.is_empty() => w[0].to_string(),
                _ => continue,
            };
            if !name.is_empty() {
                out.push(name);
            }
        }
    }
    out
}

/// Handles `tokensave_dead_code` tool calls.
pub(super) async fn handle_dead_code(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let kinds: Vec<NodeKind> = args.get("kinds").and_then(|v| v.as_array()).map_or_else(
        || {
            vec![
                NodeKind::Function,
                NodeKind::Method,
                NodeKind::SingletonMethod,
            ]
        },
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
    let include_trait_impls = args
        .get("include_trait_impls")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let cfg = cg.get_config();
    let path_prefix = effective_path(&args, scope_prefix);
    let path_include = with_defaults(
        parse_string_array(&args, "path_include"),
        &cfg.default_path_include,
    );
    let path_exclude = with_defaults(
        parse_string_array(&args, "path_exclude"),
        &cfg.default_path_exclude,
    );

    let dead = cg
        .find_dead_code(&kinds, include_public, include_trait_impls)
        .await?;
    let dead = filter_by_scope(dead, path_prefix, |n| &n.file_path);
    let dead = filter_by_path_lists(dead, &path_include, &path_exclude, |n| &n.file_path);

    let touched_files = unique_file_paths(dead.iter().map(|n| n.file_path.as_str()));

    let items: Vec<Value> = dead
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "name": n.name,
                "kind": n.kind.as_str(),
                "file": n.file_path,
                "line": super::display_line(n.start_line),
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

/// Handles `tokensave_ambiguous_calls` tool calls (#412).
///
/// Surfaces the ties the resolver refused to guess at. Each candidate is
/// returned with the detail needed to choose between them — name, kind, file
/// and line — because the caller is a model with the source in front of it and
/// can read the receiver's type, which the resolver cannot.
pub(super) async fn handle_ambiguous_calls(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let path_prefix = effective_path(&args, scope_prefix);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(25, |v| v.min(200) as usize);

    let ambiguous = cg.db().get_ambiguous_calls(path_prefix, limit).await?;

    // One batched lookup for every candidate across every site, rather than a
    // query per site.
    let candidate_ids: Vec<String> = ambiguous
        .iter()
        .flat_map(|a| a.candidate_node_ids.iter().cloned())
        .collect();
    let candidates = cg.db().get_nodes_by_ids(&candidate_ids).await?;
    let by_id: HashMap<&str, &crate::types::Node> =
        candidates.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut touched_files: Vec<String> = Vec::new();
    let items: Vec<Value> = ambiguous
        .iter()
        .map(|a| {
            touched_files.push(a.file_path.clone());
            let options: Vec<Value> = a
                .candidate_node_ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()))
                .map(|n| {
                    touched_files.push(n.file_path.clone());
                    json!({
                        "id": n.id,
                        "name": n.name,
                        "kind": n.kind.as_str(),
                        "file": n.file_path,
                        "line": super::display_line(n.start_line),
                        "qualified_name": n.qualified_name,
                    })
                })
                .collect();
            json!({
                "call_site": {
                    "from_node_id": a.from_node_id,
                    "file": a.file_path,
                    "line": super::display_line(a.line),
                },
                "reference": a.reference_name,
                "candidates": options,
            })
        })
        .collect();

    touched_files.sort();
    touched_files.dedup();

    let output = json!({
        "ambiguous_call_count": items.len(),
        "note": "These call sites produce no `calls` edge, so they are absent from \
                 callers/callees/impact, and their candidates are excluded from \
                 dead_code. Pick the intended target from the receiver's type.",
        "ambiguous_calls": items,
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

    // Public nodes under `path`, selected in SQL rather than by loading the
    // whole node table and discarding the rest (#410).
    let scoped = cg
        .db()
        .get_nodes_filtered(&crate::db::NodeFilter::new().path_prefix(path).public_only())
        .await?;

    let mut pub_nodes: Vec<&crate::types::Node> = scoped.iter().collect();

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
                "line": super::display_line(n.start_line),
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

/// Handles `tokensave_imports` tool calls.
///
/// Answers the module-level dependency questions that `tokensave_circular`
/// cannot: which packages are mutually reachable, how many import statements
/// hold a given pair together, and whether cutting one dependency would
/// actually break a cycle (#334).
pub(super) async fn handle_imports(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let depth = args
        .get("depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(1, |v| v.clamp(1, 10) as usize);

    let graph = cg.build_module_import_graph(depth).await?;
    let cycles = graph.cycles();

    let mut output = json!({
        "depth": depth,
        "cycle_count": cycles.len(),
        "cycles": cycles,
    });

    // A dependency listing over a whole repo is large and mostly uninteresting;
    // the caller nearly always wants either one module's edges or one specific
    // pair, so an unfiltered dump is not the default.
    let module = args.get("module").and_then(|v| v.as_str());
    let dependencies: Vec<_> = graph
        .dependencies()
        .into_iter()
        .filter(|dep| module.is_none_or(|m| dep.from == m || dep.to == m))
        .collect();
    if module.is_some() {
        output["dependencies"] = serde_json::to_value(&dependencies).unwrap_or(json!([]));
    }

    // Cut simulation: report what survives, not just what was removed. A cut
    // that leaves every module still mutually reachable buys nothing, and only
    // recomputing the components tells the two apart.
    if let (Some(from), Some(to)) = (
        args.get("simulate_removal_from").and_then(|v| v.as_str()),
        args.get("simulate_removal_to").and_then(|v| v.as_str()),
    ) {
        let remaining = graph.cycles_without(from, to);
        let sites = graph
            .dependencies()
            .into_iter()
            .find(|dep| dep.from == from && dep.to == to)
            .map(|dep| dep.sites)
            .unwrap_or_default();
        output["simulated_cut"] = json!({
            "from": from,
            "to": to,
            "import_sites_to_change": sites.len(),
            "sites": sites,
            "cycle_count_after": remaining.len(),
            "cycles_after": remaining,
            "breaks_a_cycle": remaining.len() < cycles.len(),
        });
    }

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

    // Degrees are tallied, sorted and truncated in SQL. Loading all 25,580
    // edges here to emit at most `limit` rows — 10 by default, 100 at most —
    // was #418's clearest case. The path filters below still run afterwards on
    // the same rows they always did, so the result set is unchanged.
    let sorted: Vec<(String, u32, u32)> = cg.db().get_top_degree_nodes(limit).await?;

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
                "line": super::display_line(node.start_line),
                "incoming": incoming,
                "outgoing": outgoing,
                "total": incoming + outgoing,
            }));
        }
    }

    let cfg = cg.get_config();
    let path_prefix = effective_path(&args, scope_prefix);
    let path_include = with_defaults(
        parse_string_array(&args, "path_include"),
        &cfg.default_path_include,
    );
    let path_exclude = with_defaults(
        parse_string_array(&args, "path_exclude"),
        &cfg.default_path_exclude,
    );

    if let Some(prefix) = path_prefix {
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

    if !path_include.is_empty() || !path_exclude.is_empty() {
        items = filter_by_path_lists(items, &path_include, &path_exclude, |item| {
            item["file"].as_str().unwrap_or("")
        });
        touched = filter_by_path_lists(touched, &path_include, &path_exclude, |f| f.as_str());
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
    let cfg = cg.get_config();
    let path_prefix = effective_path(&args, scope_prefix);
    let path_include = with_defaults(
        parse_string_array(&args, "path_include"),
        &cfg.default_path_include,
    );
    let path_exclude = with_defaults(
        parse_string_array(&args, "path_exclude"),
        &cfg.default_path_exclude,
    );

    // `Use` nodes under the requested path, selected in SQL (#410).
    let mut use_filter = crate::db::NodeFilter::new().kinds(&[NodeKind::Use]);
    if let Some(prefix) = path_prefix {
        use_filter = use_filter.path_prefix(prefix);
    }
    let scoped_uses = cg.db().get_nodes_filtered(&use_filter).await?;

    let use_nodes: Vec<&crate::types::Node> = scoped_uses.iter().collect();
    let use_nodes = filter_by_path_lists(use_nodes, &path_include, &path_exclude, |n| &n.file_path);

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
        // Go imports are slash-separated paths; the generic `::`-based parser
        // would never recover the package identifier (#148).
        let ext = std::path::Path::new(&use_node.file_path)
            .extension()
            .map(|e| e.to_ascii_lowercase().to_string_lossy().to_string())
            .unwrap_or_default();
        let is_go = ext == "go";
        // TS/JS Use nodes are named after the *module path* (e.g.
        // `@testing-library/react`), so the `::`-based parser can never
        // recover the bound identifiers — it flagged every used import as
        // unused (#207). Parse the import clause from the stored signature.
        let is_ts_js = matches!(
            ext.as_str(),
            "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs"
        );
        let is_python = matches!(ext.as_str(), "py" | "pyi");
        // `__init__.py` re-exports are a Python-idiomatic public API surface
        // (`from .module import Thing` there is meant to be imported by
        // consumers, not used within the file itself) — never flag them as
        // unused (#224).
        if is_python
            && std::path::Path::new(&use_node.file_path)
                .file_name()
                .is_some_and(|f| f == "__init__.py")
        {
            continue;
        }
        let identifiers: Vec<String> = if is_go {
            go_import_identifier(&use_node.name).into_iter().collect()
        } else if is_ts_js {
            ts_import_identifiers(use_node.signature.as_deref().unwrap_or(""))
        } else if is_python {
            python_import_identifier(&use_node.name, use_node.signature.as_deref().unwrap_or(""))
                .into_iter()
                .collect()
        } else {
            identifiers_from_use_path(&use_node.name)
        };
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
                    "line": super::display_line(use_node.start_line),
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
                "line": super::display_line(node.start_line),
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
                "start_line": super::display_line(node.start_line),
                "end_line": super::display_line(node.end_line),
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
                "line": super::display_line(node.start_line),
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
    let mut lines_cache: HashMap<String, Option<Vec<String>>> = HashMap::new();

    for (src, tgt, line) in &call_edges {
        if src == tgt {
            let Some(node) = cached_node(cg, &mut node_cache, src).await? else {
                continue;
            };
            if !is_direct_self_call(cg, &mut lines_cache, &node, *line) {
                continue;
            }
        }
        adj.entry(src.clone()).or_default().insert(tgt.clone());
        adj.entry(tgt.clone()).or_default();
    }

    // Collect only the cyclic SCCs, then sort smallest-first so we keep
    // shorter / more interesting cycles when the cap kicks in. We still need
    // every cyclic SCC enumerated before sorting (truncating early would bias
    // toward Tarjan emission order), but we cap the per-SCC path search.
    let mut cyclic_sccs: Vec<Vec<String>> = crate::graph::scc::tarjan_scc(&adj)
        .into_iter()
        .filter(|scc| crate::graph::scc::is_cyclic_scc(scc, &adj))
        .collect();
    cyclic_sccs.sort_by_key(Vec::len);

    let mut cycles: Vec<Vec<String>> = Vec::new();
    for mut scc in cyclic_sccs {
        if cycles.len() >= limit {
            break;
        }
        if let Some(path) = cycle_path_for_scc(&mut scc, &adj) {
            cycles.push(path);
        }
    }
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
                    "line": super::display_line(node.start_line),
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

fn cached_lines<'a>(
    cg: &TokenSave,
    cache: &'a mut HashMap<String, Option<Vec<String>>>,
    file_path: &str,
) -> Option<&'a Vec<String>> {
    if !cache.contains_key(file_path) {
        let abs = cg.project_root().join(file_path);
        let lines = std::fs::read_to_string(abs)
            .ok()
            .map(|content| content.lines().map(str::to_string).collect());
        cache.insert(file_path.to_string(), lines);
    }
    cache.get(file_path).and_then(Option::as_ref)
}

fn is_direct_self_call(
    cg: &TokenSave,
    lines_cache: &mut HashMap<String, Option<Vec<String>>>,
    node: &crate::types::Node,
    edge_line: Option<u32>,
) -> bool {
    let Some(lines) = cached_lines(cg, lines_cache, &node.file_path) else {
        return false;
    };
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
    // Fast path: a bare call always needs an opening paren on the same line.
    // For common short names like `new`/`get`/`len` this short-circuits the
    // expensive `match_indices + is_ident_byte` scan on lines that obviously
    // can't contain a call (assignments, comments, docstrings, …).
    if name.is_empty() || !line.contains('(') {
        return false;
    }
    let bytes = line.as_bytes();
    let name_len = name.len();
    line.match_indices(name).any(|(idx, _)| {
        // Reject substring matches inside a larger identifier on either side:
        // `name=new` should not match `newer`, `renew`, etc. Cheap byte
        // checks before the more expensive prefix-trim probe.
        let before_ok = idx == 0 || !is_ident_byte(bytes[idx - 1]);
        if !before_ok {
            return false;
        }
        let after_idx = idx + name_len;
        let after_ok = after_idx == bytes.len() || !is_ident_byte(bytes[after_idx]);
        if !after_ok {
            return false;
        }
        let prefix = line[..idx].trim_end();
        if prefix.ends_with('.') || prefix.ends_with(':') {
            return false;
        }
        call_suffix_starts(&line[after_idx..])
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
        // `path` and `seen` operate on borrowed ids from `scc_set`: the SCC
        // outlives this call, so we never need to allocate `String`s during
        // the DFS itself. The final result has to be `Vec<String>` because
        // it leaves the function, so we materialise once at the end.
        let start_ref: &str = start.as_str();
        let mut path: Vec<&str> = vec![start_ref];
        let mut seen: HashSet<&str> = HashSet::from([start_ref]);
        if dfs_cycle_path(start_ref, start_ref, &scc_set, adj, &mut path, &mut seen) {
            return Some(path.into_iter().map(str::to_string).collect());
        }
    }
    None
}

fn dfs_cycle_path<'a>(
    current: &'a str,
    start: &'a str,
    scc_set: &HashSet<&'a str>,
    adj: &'a HashMap<String, HashSet<String>>,
    path: &mut Vec<&'a str>,
    seen: &mut HashSet<&'a str>,
) -> bool {
    let Some(neighbors) = adj.get(current) else {
        return false;
    };
    let mut neighbors: Vec<&'a str> = neighbors
        .iter()
        .filter_map(|n| scc_set.get(n.as_str()).copied())
        .collect();
    neighbors.sort_unstable();

    for neighbor in neighbors {
        if neighbor == start && path.len() > 1 {
            path.push(start);
            return true;
        }
        if !seen.insert(neighbor) {
            continue;
        }
        path.push(neighbor);
        if dfs_cycle_path(neighbor, start, scc_set, adj, path, seen) {
            return true;
        }
        path.pop();
        seen.remove(neighbor);
    }
    false
}

/// Returns the set of node IDs reached by a test via the call graph: any node
/// targeted by a `Calls` edge whose source is in a test file or is a
/// `#[test]`-annotated function. This is the same "tested" definition used by
/// `tokensave_test_risk`, and is the binary coverage signal behind the `crap`
/// score reported by `tokensave_complexity`.
async fn test_reached_node_ids(cg: &TokenSave) -> Result<HashSet<String>> {
    use crate::types::EdgeKind;
    // `calls` only, selected in SQL: every use below filters on the kind, and
    // `calls` is 19,282 of this repository's 25,580 edges, so the rest was a
    // quarter of the load carried and discarded (#418).
    let all_edges = cg.db().get_edges_by_kind(EdgeKind::Calls).await?;
    // Graph-wide id -> file_path, projected in SQL (#411). Every node is
    // needed, because a `Calls` edge can originate anywhere and the question
    // is whether its *source* lives in a test file — but the other twenty-six
    // columns never are. This was the last `get_all_nodes()` call site in the
    // handlers whose load was pure overhead.
    let node_to_file: HashMap<String, String> = cg.db().get_node_paths().await?;
    let call_source_ids: Vec<String> = all_edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .map(|e| e.source.clone())
        .collect();
    let test_annotated_callers = cg
        .get_test_annotated_node_ids(&call_source_ids)
        .await
        .unwrap_or_default();
    let mut tested: HashSet<String> = HashSet::new();
    for e in &all_edges {
        if e.kind == EdgeKind::Calls {
            let is_test = node_to_file
                .get(&e.source)
                .is_some_and(|f| cg.is_test_file(f))
                || test_annotated_callers.contains(&e.source);
            if is_test {
                tested.insert(e.target.clone());
            }
        }
    }
    Ok(tested)
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

    // CRAP needs a per-unit coverage signal. tokensave has no execution
    // (line/branch) coverage, but it can tell whether a unit is reached by a
    // test via the call graph: a node is "covered" when some `Calls` edge
    // targeting it originates in a test file or a `#[test]`-annotated function
    // (same definition as `tokensave_test_risk`). This is binary, so CRAP's
    // `(1 - coverage)³` term collapses to 0 (tested) or 1 (untested).
    let tested = test_reached_node_ids(cg).await?;

    let items: Vec<Value> = results
        .iter()
        .map(|(node, lines, fan_out, fan_in, score)| {
            use crate::extraction::complexity::{
                crap_score, halstead_difficulty, halstead_effort, halstead_volume,
                maintainability_index,
            };
            let cyclomatic = node.branches + 1;
            let test_covered = tested.contains(&node.id);
            let crap = crap_score(cyclomatic, if test_covered { 1.0 } else { 0.0 });
            let volume = halstead_volume(
                node.distinct_operators,
                node.distinct_operands,
                node.total_operators,
                node.total_operands,
            );
            let difficulty = halstead_difficulty(
                node.distinct_operators,
                node.distinct_operands,
                node.total_operands,
            );
            let effort = halstead_effort(volume, difficulty);
            let mi = maintainability_index(volume, cyclomatic, *lines);
            // Round derived floats to 2 decimals to keep the JSON compact.
            let round2 = |x: f64| (x * 100.0).round() / 100.0;
            json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": super::display_line(node.start_line),
                "lines": lines,
                "cyclomatic_complexity": cyclomatic,
                "cognitive_complexity": node.cognitive_complexity,
                "branches": node.branches,
                "loops": node.loops,
                "returns": node.returns,
                "max_nesting": node.max_nesting,
                "unsafe_blocks": node.unsafe_blocks,
                "unchecked_calls": node.unchecked_calls,
                "assertions": node.assertions,
                "halstead_volume": round2(volume),
                "halstead_difficulty": round2(difficulty),
                "halstead_effort": round2(effort),
                "maintainability_index": round2(mi),
                "test_covered": test_covered,
                "crap": round2(crap),
                "fan_out": fan_out,
                "fan_in": fan_in,
                "score": score,
            })
        })
        .collect();

    let output = json!({
        "formula": "lines + (fan_out × 3) + fan_in",
        "note": "cyclomatic_complexity = branches + 1. cognitive_complexity is SonarSource-style (nesting-weighted). halstead_* derive from operator/operand token counts; maintainability_index uses MI = max(0, (171 - 5.2·ln(V) - 0.23·G - 16.2·ln(LOC)) · 100/171), clamped 0–100 (higher is better). crap = cyclomatic²·(1-coverage)³ + cyclomatic, where coverage is call-graph test reachability (test_covered: a test reaches the unit). Coverage is binary today (no execution coverage), so crap = cyclomatic when test_covered else cyclomatic²+cyclomatic.",
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
                "line": super::display_line(node.start_line),
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
                "line": super::display_line(node.start_line),
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

// ---------------------------------------------------------------------------
// tokensave_unsafe_patterns
// ---------------------------------------------------------------------------

const UNSAFE_KINDS: &[&str] = &[
    "unwrap",
    "expect",
    "panic",
    "todo",
    "unimplemented",
    "unsafe_block",
];

fn line_matches_unsafe_kind(line: &str, kind: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("///") {
        return false;
    }
    match kind {
        "unwrap" => contains_method_call(line, "unwrap", true),
        "expect" => contains_method_call(line, "expect", false),
        "panic" => line.contains("panic!("),
        "todo" => line.contains("todo!("),
        "unimplemented" => line.contains("unimplemented!(") || line.contains("unimplemented!()"),
        "unsafe_block" => contains_unsafe_block_start(line),
        _ => false,
    }
}

fn contains_method_call(line: &str, method: &str, empty_parens: bool) -> bool {
    let needle = format!(".{method}");
    let bytes = line.as_bytes();
    let mut start = 0usize;
    while let Some(pos) = line[start..].find(&needle) {
        let abs = start + pos;
        let after = abs + needle.len();
        let next = bytes.get(after).copied();
        let is_word_boundary = !matches!(next, Some(c) if c.is_ascii_alphanumeric() || c == b'_');
        if is_word_boundary && next == Some(b'(') {
            if empty_parens {
                if line[after + 1..].trim_start().starts_with(')') {
                    return true;
                }
            } else {
                return true;
            }
        }
        start = abs + needle.len();
    }
    false
}

fn contains_unsafe_block_start(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut start = 0usize;
    while let Some(pos) = line[start..].find("unsafe") {
        let abs = start + pos;
        let prev_ok =
            abs == 0 || !matches!(bytes[abs - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_');
        let after = abs + "unsafe".len();
        let next = bytes.get(after).copied();
        let next_ok = matches!(next, Some(b' ' | b'\t' | b'{'));
        if prev_ok && next_ok {
            let rest = line[after..].trim_start();
            if rest.starts_with('{')
                || rest.starts_with("fn ")
                || rest.starts_with("impl ")
                || rest.starts_with("trait ")
            {
                return true;
            }
        }
        start = abs + "unsafe".len();
    }
    false
}

fn path_looks_like_test(path: &str) -> bool {
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
        || path.ends_with("_test.go")
        || path.contains("/__tests__/")
        || path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
        || path.ends_with(".test.js")
        || path.ends_with("_test.py")
        || path.ends_with("Test.java")
}

pub(super) async fn handle_unsafe_patterns(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let kinds: Vec<String> = args
        .get("kinds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| UNSAFE_KINDS.iter().map(|s| (*s).to_string()).collect());

    let path = effective_path(&args, scope_prefix);
    let exclude_tests = args
        .get("exclude_tests")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(200, |v| v.min(2000) as usize);

    let project_root = cg.project_root();
    // Source-text scan: tracked artifacts have no code constructs to find, so
    // reading them is pure cost (#323).
    let files = cg.get_code_files().await?;
    let mut matches: Vec<Value> = Vec::new();
    let mut by_kind: HashMap<String, u64> = HashMap::new();
    let mut touched: Vec<String> = Vec::new();

    'outer: for file in &files {
        if let Some(prefix) = path {
            let with_slash = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            if !file.path.starts_with(&with_slash) && file.path != prefix {
                continue;
            }
        }
        let in_test = path_looks_like_test(&file.path);
        if exclude_tests && in_test {
            continue;
        }
        let abs_path = project_root.join(&file.path);
        let Ok(source) = crate::sync::read_source_file(&abs_path) else {
            continue;
        };
        let nodes = cg.get_nodes_by_file(&file.path).await.unwrap_or_default();

        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx as u32) + 1;
            // Node spans are 0-based; compare against the 0-based index (#203).
            let line0 = idx as u32;
            for kind in &kinds {
                if line_matches_unsafe_kind(line, kind) {
                    let enclosing = nodes
                        .iter()
                        .filter(|n| n.start_line <= line0 && line0 <= n.end_line)
                        .min_by_key(|n| n.end_line.saturating_sub(n.start_line))
                        .map(|n| n.qualified_name.clone());
                    *by_kind.entry(kind.clone()).or_insert(0) += 1;
                    matches.push(json!({
                        "kind": kind,
                        "file": file.path,
                        "line": line_no,
                        "snippet": line.trim(),
                        "enclosing": enclosing,
                        "in_test": in_test,
                    }));
                    if !touched.contains(&file.path) {
                        touched.push(file.path.clone());
                    }
                    if matches.len() >= limit {
                        break 'outer;
                    }
                }
            }
        }
    }

    let counts = serde_json::to_value(&by_kind).unwrap_or(json!({}));
    let payload = json!({
        "match_count": matches.len(),
        "by_kind": counts,
        "matches": matches,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

// ---------------------------------------------------------------------------
// tokensave_diagnostics
// ---------------------------------------------------------------------------

pub(super) async fn handle_diagnostics(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    use crate::diagnostics::{run_all, Scope};

    let scope_str = args
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("workspace");

    let scope = match scope_str {
        "workspace" => Scope::Workspace,
        "package" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| TokenSaveError::Config {
                    message: "scope='package' requires a 'name' argument".to_string(),
                })?
                .to_string();
            Scope::Package { name }
        }
        "file" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| TokenSaveError::Config {
                    message: "scope='file' requires a 'path' argument".to_string(),
                })?
                .to_string();
            Scope::File { path }
        }
        other => {
            return Err(TokenSaveError::Config {
                message: format!("unknown scope '{other}'; expected workspace, package, or file"),
            });
        }
    };

    let project_root = cg.project_root().to_path_buf();
    let mut diagnostics = run_all(&project_root, &scope).await?;

    if let Scope::File { path } = &scope {
        diagnostics.retain(|d| d.file == *path);
    }

    let mut entries: Vec<Value> = Vec::with_capacity(diagnostics.len());
    let mut touched: Vec<String> = Vec::new();
    let mut error_count = 0u64;
    let mut warning_count = 0u64;
    let mut nodes_by_file: HashMap<String, Vec<crate::types::Node>> = HashMap::new();

    for diag in &diagnostics {
        match diag.level.as_str() {
            "error" => error_count += 1,
            "warning" => warning_count += 1,
            _ => {}
        }
        let nodes = if let Some(n) = nodes_by_file.get(&diag.file) {
            n
        } else {
            let fetched = cg.get_nodes_by_file(&diag.file).await.unwrap_or_default();
            nodes_by_file.entry(diag.file.clone()).or_insert(fetched)
        };
        let enclosing = nodes
            .iter()
            .filter(|n| {
                // diag lines are 1-based, node spans 0-based (#203)
                let l0 = diag.line_start.saturating_sub(1);
                n.start_line <= l0 && l0 <= n.end_line
            })
            .min_by_key(|n| n.end_line.saturating_sub(n.start_line))
            .map(|n| n.qualified_name.clone());
        if !touched.contains(&diag.file) {
            touched.push(diag.file.clone());
        }
        entries.push(json!({
            "file": diag.file,
            "line_start": diag.line_start,
            "line_end": diag.line_end,
            "level": diag.level,
            "code": diag.code,
            "message": diag.message,
            "driver": diag.driver,
            "enclosing": enclosing,
        }));
    }

    let payload = json!({
        "scope": scope_str,
        "diagnostic_count": entries.len(),
        "error_count": error_count,
        "warning_count": warning_count,
        "diagnostics": entries,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

// ---------------------------------------------------------------------------
// tokensave_constructors
// ---------------------------------------------------------------------------

/// Does this file's language build values with `Name { ... }` literals?
///
/// Rust and Go do; Python (`Name(...)`), Java (`new Name(...)`), Ruby and the
/// rest do not, and against those the literal scan is guaranteed to find
/// nothing (#458). C#'s `new Name { ... }` is an object *initializer* over an
/// already-constructed value, not a whole-value literal, so a missing-fields
/// list read off it would mean something different from what this tool
/// promises — it is excluded on purpose.
fn literal_syntax_is_supported(file_path: &str) -> bool {
    matches!(
        file_path.rsplit('.').next().unwrap_or_default(),
        "rs" | "go"
    )
}

pub(super) async fn handle_constructors(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let struct_name =
        args.get("struct")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "tokensave_constructors requires a 'struct' argument".to_string(),
            })?;
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(100, |v| v.clamp(1, 1000) as usize);

    let candidates = cg
        .db()
        .search_nodes_by_exact_name(&[struct_name.to_string()], 50)
        .await?;
    let struct_nodes: Vec<&crate::types::Node> = candidates
        .iter()
        .filter(|n| {
            matches!(
                n.kind,
                NodeKind::Struct | NodeKind::Class | NodeKind::CaseClass
            )
        })
        .collect();

    if struct_nodes.is_empty() {
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": format!("No struct, class, or case-class named '{struct_name}' found.") }]
            }),
            touched_files: vec![],
        });
    }

    // The literal scan looks for `Name { ... }`, which is Rust's and Go's
    // construction syntax and nobody else's: Python builds with
    // `Name(...)`, Java with `new Name(...)`. Against those the scan finds
    // nothing and used to report a clean, confident `match_count: 0`, which
    // is indistinguishable from "this type is never constructed" — a false
    // negative dressed as an answer, in a tool whose name promises exactly
    // the question an impact review asks (#458).
    //
    // The type's own declaring file decides. Saying "not supported here" is
    // a refusal the caller can act on; a zero is not.
    if !struct_nodes
        .iter()
        .any(|n| literal_syntax_is_supported(&n.file_path))
    {
        let langs: Vec<String> = struct_nodes.iter().map(|n| n.file_path.clone()).collect();
        let payload = json!({
            "struct": struct_name,
            "language_supported": false,
            "note": format!(
                "'{struct_name}' is declared in {} — this tool scans for `Name {{ ... }}` \
                 literal syntax, which only Rust and Go use, so it cannot answer for this \
                 type. `match_count` and `sites` are omitted deliberately: a zero here \
                 would be indistinguishable from 'never constructed'. Try \
                 tokensave_callers_for on the type's constructor, or tokensave_field_sites \
                 for the individual fields.",
                langs.join(", ")
            ),
        });
        let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": truncate_response(&formatted) }]
            }),
            touched_files: Vec::new(),
        });
    }

    let mut expected_fields: HashSet<String> = HashSet::new();
    for sn in &struct_nodes {
        let children = cg.db().get_children_of(&sn.id).await?;
        for child in children {
            if matches!(
                child.kind,
                NodeKind::Field | NodeKind::ValField | NodeKind::VarField
            ) {
                expected_fields.insert(child.name);
            }
        }
    }

    let project_root = cg.project_root();
    // Source-text scan: tracked artifacts have no code constructs to find, so
    // reading them is pure cost (#323).
    let files = cg.get_code_files().await?;
    let mut sites: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();

    'outer: for file in &files {
        if let Some(prefix) = scope_prefix {
            let with_slash = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            if !file.path.starts_with(&with_slash) && file.path != prefix {
                continue;
            }
        }
        let abs = project_root.join(&file.path);
        let Ok(source) = crate::sync::read_source_file(&abs) else {
            continue;
        };

        for site in find_struct_literals(&source, struct_name) {
            let field_list = parse_literal_fields(&source, site.brace_open_byte);
            let missing: Vec<String> = if expected_fields.is_empty() {
                Vec::new()
            } else {
                expected_fields
                    .iter()
                    .filter(|f| !field_list.contains(f))
                    .cloned()
                    .collect()
            };
            if !touched.contains(&file.path) {
                touched.push(file.path.clone());
            }
            sites.push(json!({
                "file": file.path,
                "line": site.line,
                "fields": field_list,
                "missing_fields": missing,
            }));
            if sites.len() >= limit {
                break 'outer;
            }
        }
    }

    let payload = json!({
        "struct": struct_name,
        "language_supported": true,
        "expected_fields": expected_fields.iter().cloned().collect::<Vec<_>>(),
        "match_count": sites.len(),
        "sites": sites,
    });
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

#[derive(Debug, Clone, Copy)]
struct LiteralSite {
    line: u32,
    brace_open_byte: usize,
}

fn find_struct_literals(source: &str, struct_name: &str) -> Vec<LiteralSite> {
    let bytes = source.as_bytes();
    let mut pattern_stack: Vec<i32> = Vec::new();
    let mut depth: i32 = 0;
    let mut string_delim: Option<u8> = None;
    let mut prev_was_backslash = false;
    let mut out: Vec<LiteralSite> = Vec::new();
    let mut byte = 0usize;
    let n = bytes.len();
    while byte < n {
        let b = bytes[byte];

        if let Some(delim) = string_delim {
            if !prev_was_backslash && b == delim {
                string_delim = None;
                prev_was_backslash = false;
                byte += 1;
                continue;
            }
            prev_was_backslash = !prev_was_backslash && b == b'\\';
            byte += 1;
            continue;
        }
        if b == b'"' {
            string_delim = Some(b'"');
            prev_was_backslash = false;
            byte += 1;
            continue;
        }
        if b == b'\'' {
            let after = bytes.get(byte + 1).copied();
            if matches!(after, Some(b'a'..=b'z' | b'A'..=b'Z' | b'_')) {
                let mut probe = byte + 1;
                while let Some(c) = bytes.get(probe) {
                    if matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_') {
                        probe += 1;
                    } else {
                        break;
                    }
                }
                if bytes.get(probe).copied() != Some(b'\'') {
                    byte += 1;
                    continue;
                }
            }
            string_delim = Some(b'\'');
            prev_was_backslash = false;
            byte += 1;
            continue;
        }

        if matches_word(bytes, byte, b"match") {
            pattern_stack.push(depth);
            byte += "match".len();
            continue;
        }
        if matches_word(bytes, byte, b"if") && lookahead_let(bytes, byte + 2) {
            pattern_stack.push(depth);
            byte += "if".len();
            continue;
        }
        if matches_word(bytes, byte, b"while") && lookahead_let(bytes, byte + 5) {
            pattern_stack.push(depth);
            byte += "while".len();
            continue;
        }

        if b == b'{' {
            depth += 1;
            byte += 1;
            continue;
        }
        if b == b'}' {
            depth -= 1;
            if let Some(&entered_at) = pattern_stack.last() {
                if depth == entered_at {
                    pattern_stack.pop();
                }
            }
            byte += 1;
            continue;
        }

        if matches_word(bytes, byte, struct_name.as_bytes()) {
            let start = byte;
            let end = start + struct_name.len();

            let mut probe = end;
            while let Some(c) = bytes.get(probe) {
                if c.is_ascii_whitespace() {
                    probe += 1;
                } else {
                    break;
                }
            }
            if bytes.get(probe).copied() != Some(b'{') {
                byte = end;
                continue;
            }
            if has_disqualifying_prefix(source, start) {
                byte = end;
                continue;
            }
            if !pattern_stack.is_empty() {
                byte = end;
                continue;
            }
            let line = source[..start].bytes().filter(|c| *c == b'\n').count() as u32 + 1;
            out.push(LiteralSite {
                line,
                brace_open_byte: probe,
            });
            byte = probe + 1;
            continue;
        }

        byte += 1;
    }
    out
}

fn lookahead_let(bytes: &[u8], at: usize) -> bool {
    let mut probe = at;
    while let Some(b) = bytes.get(probe) {
        if b.is_ascii_whitespace() {
            probe += 1;
        } else {
            break;
        }
    }
    matches_word(bytes, probe, b"let")
}

fn matches_word(bytes: &[u8], at: usize, needle: &[u8]) -> bool {
    if at + needle.len() > bytes.len() {
        return false;
    }
    if &bytes[at..at + needle.len()] != needle {
        return false;
    }
    let left_ok = at == 0
        || !matches!(
            bytes[at - 1],
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
        );
    let right_ok = match bytes.get(at + needle.len()) {
        None => true,
        Some(b) => !matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'),
    };
    left_ok && right_ok
}

fn has_disqualifying_prefix(source: &str, idx: usize) -> bool {
    let bytes = source.as_bytes();
    let mut probe = idx;
    while probe > 0 && bytes[probe - 1].is_ascii_whitespace() {
        probe -= 1;
    }
    if probe == 0 {
        return false;
    }
    if probe >= 2 && &bytes[probe - 2..probe] == b"->" {
        return true;
    }
    // Go writes a return type where Rust writes `-> T`: `func f() Settings {`
    // and `func (s *S) f() Settings {` both put the type between a `)` and the
    // body's brace. Without this the function's own signature is reported as a
    // construction site, with a `missing_fields` list naming every field —
    // advice to "fix" a declaration that constructs nothing (#458).
    if bytes[probe - 1] == b')' {
        return true;
    }
    let id_end = probe;
    let mut id_start = probe;
    while id_start > 0
        && matches!(
            bytes[id_start - 1],
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
        )
    {
        id_start -= 1;
    }
    if id_start == id_end {
        return false;
    }
    let token = &source[id_start..id_end];
    matches!(
        token,
        "struct" | "enum" | "union" | "impl" | "trait" | "type"
    )
}

fn parse_literal_fields(source: &str, open_byte: usize) -> Vec<String> {
    let bytes = source.as_bytes();
    if bytes.get(open_byte).copied() != Some(b'{') {
        return Vec::new();
    }
    let mut depth = 0i32;
    let mut close_byte = None;
    for (i, b) in bytes.iter().enumerate().skip(open_byte) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    close_byte = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close_byte else {
        return Vec::new();
    };
    let body = &source[open_byte + 1..close];

    let mut fields: Vec<String> = Vec::new();
    let mut depth_brace = 0i32;
    let mut depth_paren = 0i32;
    let mut current = String::new();
    for c in body.chars() {
        match c {
            '{' | '[' => depth_brace += 1,
            '}' | ']' => depth_brace -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            ',' if depth_brace == 0 && depth_paren == 0 => {
                if let Some(name) = field_name_from_chunk(&current) {
                    fields.push(name);
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if let Some(name) = field_name_from_chunk(&current) {
        fields.push(name);
    }
    fields
}

fn field_name_from_chunk(chunk: &str) -> Option<String> {
    let trimmed = chunk.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("..") || trimmed.starts_with("//") {
        return None;
    }
    let name_end = trimmed
        .find(|c: char| c == ':' || c == ',' || c.is_whitespace())
        .unwrap_or(trimmed.len());
    let name = &trimmed[..name_end];
    if name.is_empty() {
        return None;
    }
    if !name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return None;
    }
    Some(name.to_string())
}

use super::receiver_type;

// ---------------------------------------------------------------------------
// tokensave_field_sites
// ---------------------------------------------------------------------------

/// Declared types of every field the graph holds, keyed `Type::field`, plus
/// whether `want_type` declares `want_field` at all.
///
/// The map is what `receiver_type` follows a chain through; the flag turns a
/// qualified query naming a nonexistent field into a direct answer rather than
/// a broad one (#458). Both come from one pass over the field nodes.
async fn field_type_index(
    cg: &TokenSave,
    want_type: &str,
    want_field: &str,
) -> (receiver_type::TypeIndex, bool) {
    let mut index = receiver_type::TypeIndex::default();
    let mut declared = false;
    // Scala spells its fields `ValField`/`VarField`; everything else uses
    // `Field`.
    let mut nodes = Vec::new();
    for kind in [
        crate::types::NodeKind::Field,
        crate::types::NodeKind::ValField,
        crate::types::NodeKind::VarField,
    ] {
        nodes.extend(cg.db().get_nodes_by_kind(kind).await.unwrap_or_default());
    }
    for node in nodes {
        // `src/types.rs::GraphStats::last_sync_at` -> owner `GraphStats`.
        let Some(owner) = receiver_type::owning_type(&node.qualified_name) else {
            continue;
        };
        if owner == want_type && node.name == want_field {
            declared = true;
        }
        // `pub last_sync_at: u64` -> `u64`. Without a declared type there is
        // nothing to follow, so the entry stays absent and a chain through it
        // reads as unknown rather than as a guess.
        if let Some((_, ty)) = node.signature.as_deref().and_then(|s| s.rsplit_once(':')) {
            let ty = receiver_type::normalize_type(ty);
            if !ty.is_empty() {
                index.fields.insert(format!("{owner}::{}", node.name), ty);
            }
        }
    }

    // Return types, keyed by bare function name and kept only where every
    // function of that name agrees — a text scan cannot tell two same-named
    // methods apart, so a disagreement must leave the name unresolvable
    // rather than attribute a site to the wrong type.
    let mut seen: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut callables = Vec::new();
    for kind in [
        crate::types::NodeKind::Function,
        crate::types::NodeKind::Method,
    ] {
        callables.extend(cg.db().get_nodes_by_kind(kind).await.unwrap_or_default());
    }
    for node in callables {
        let Some(ret) = node.signature.as_deref().and_then(return_type_of) else {
            continue;
        };
        match seen.get(&node.name) {
            Some(Some(existing)) if existing != &ret => {
                seen.insert(node.name.clone(), None);
            }
            Some(_) => {}
            None => {
                seen.insert(node.name.clone(), Some(ret));
            }
        }
    }
    for (name, ret) in seen {
        if let Some(ret) = ret {
            index.returns.insert(name, ret);
        }
    }
    (index, declared)
}

/// The declared return type in a signature: `fn f(&self) -> Result<T>` ->
/// `Result<T>`. Returns `None` for a signature with no return arrow, which is
/// every language whose functions do not write one.
fn return_type_of(signature: &str) -> Option<String> {
    let (_, ret) = signature.split_once("->")?;
    let ret = ret.trim().trim_end_matches(['{', ';']).trim();
    (!ret.is_empty()).then(|| ret.to_string())
}

pub(super) async fn handle_field_sites(
    cg: &TokenSave,
    args: Value,
    scope_prefix: Option<&str>,
) -> Result<ToolResult> {
    let raw = args
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "tokensave_field_sites requires a 'field' argument".to_string(),
        })?;
    let writes_only = args
        .get("writes_only")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(200, |v| v.clamp(1, 2000) as usize);

    let (qualifier, field_name) = match raw.rsplit_once("::") {
        Some((q, f)) => (Some(q.to_string()), f.to_string()),
        None => (None, raw.to_string()),
    };

    let project_root = cg.project_root();

    // Declared types of every field in the graph, keyed `Type::field`. This is
    // what lets a chain like `self.inner.field` be followed one hop at a time
    // (#458).
    let (type_index, declared_on) = match qualifier.as_deref() {
        Some(q) => field_type_index(cg, receiver_type::bare_type_name(q), &field_name).await,
        None => (receiver_type::TypeIndex::default(), true),
    };
    // A qualified query naming a field the type does not declare is answerable
    // outright, and answering it with every other type's sites is exactly the
    // broad-answer-under-a-narrow-heading this tool was reported for.
    if let (Some(q), false) = (qualifier.as_deref(), declared_on) {
        let payload = json!({
            "field": raw,
            "qualifier": q,
            "qualifier_applied": true,
            "write_count": 0,
            "write_returned": 0,
            "write_lines": 0,
            "read_count": 0,
            "read_returned": 0,
            "read_lines": 0,
            "excluded_count": 0,
            "unattributed_count": 0,
            "truncated": false,
            "write_sites": [],
            "read_sites": [],
            "qualifier_note": format!(
                "No field '{field_name}' is declared on '{q}' anywhere in the index, so \
                 there are no sites to narrow to. Check the type name, or query the bare \
                 field name '{field_name}' to see every type that declares it."
            ),
        });
        let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
        return Ok(ToolResult {
            value: json!({
                "content": [{ "type": "text", "text": truncate_response(&formatted) }]
            }),
            touched_files: Vec::new(),
        });
    }

    // Source-text scan: tracked artifacts have no code constructs to find, so
    // reading them is pure cost (#323).
    let files = cg.get_code_files().await?;
    let mut writes: Vec<Value> = Vec::new();
    let mut reads: Vec<Value> = Vec::new();
    let mut touched: Vec<String> = Vec::new();
    // Totals are counted over every matching site, not just the ones that fit
    // under `limit` (#457): a capped page reported as a total understates a
    // blast radius, which is the one question this tool exists to answer.
    let mut write_total = 0usize;
    let mut read_total = 0usize;
    // Two references to the field on one source line are two sites but one
    // line. Sites arrive in byte order within a file, so lines ascend and the
    // previous line is enough to count distinct ones (#457).
    let mut write_lines = 0usize;
    let mut read_lines = 0usize;
    // Sites the qualifier positively ruled out, and sites whose receiver could
    // not be typed at all. Reported separately: the first is the narrowing
    // working, the second is the limit of what a source-text scan can know.
    let mut excluded = 0usize;
    let mut unattributed = 0usize;

    for file in &files {
        if let Some(prefix) = scope_prefix {
            let with_slash = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            if !file.path.starts_with(&with_slash) && file.path != prefix {
                continue;
            }
        }
        let abs = project_root.join(&file.path);
        let Ok(source) = crate::sync::read_source_file(&abs) else {
            continue;
        };
        let nodes = cg.get_nodes_by_file(&file.path).await.unwrap_or_default();
        let mut last_write_line: Option<u32> = None;
        let mut last_read_line: Option<u32> = None;

        for site in find_field_references(&source, &field_name) {
            let line_text = line_at(&source, site.byte).unwrap_or("");
            let enclosing_node = nodes
                .iter()
                .filter(|n| {
                    // field sites are 1-based, node spans 0-based (#203)
                    let l0 = site.line.saturating_sub(1);
                    n.start_line <= l0 && l0 <= n.end_line
                })
                .min_by_key(|n| n.end_line.saturating_sub(n.start_line));
            let enclosing = enclosing_node.map(|n| n.qualified_name.clone());

            // Narrow, when a qualifier was given and the receiver resolves. A
            // site whose receiver cannot be typed is neither kept nor dropped
            // silently: it is counted, and the count is reported, so a
            // narrowed answer never poses as a complete one (#458).
            if let Some(q) = qualifier.as_deref() {
                let enc = enclosing_node.map(|n| receiver_type::Enclosing {
                    qualified_name: n.qualified_name.clone(),
                    signature: n.signature.clone(),
                    start_line: n.start_line,
                    end_line: n.end_line,
                });
                // `site.byte` is just past the field name; the receiver ends
                // at the dot that precedes it.
                let dot = site.byte - field_name.len() - 1;
                match receiver_type::attribute_site(&source, dot, q, enc.as_ref(), &type_index) {
                    receiver_type::Attribution::Matches => {}
                    receiver_type::Attribution::Excludes => {
                        excluded += 1;
                        continue;
                    }
                    receiver_type::Attribution::Unknown => {
                        unattributed += 1;
                        continue;
                    }
                }
            }

            let entry = json!({
                "file": file.path,
                "line": site.line,
                "enclosing": enclosing,
                "snippet": line_text.trim(),
            });
            if !touched.contains(&file.path) {
                touched.push(file.path.clone());
            }
            match site.kind {
                FieldRefKind::Write => {
                    write_total += 1;
                    if last_write_line != Some(site.line) {
                        write_lines += 1;
                        last_write_line = Some(site.line);
                    }
                    if writes.len() < limit {
                        writes.push(entry);
                    }
                }
                FieldRefKind::Read => {
                    if writes_only {
                        continue;
                    }
                    read_total += 1;
                    if last_read_line != Some(site.line) {
                        read_lines += 1;
                        last_read_line = Some(site.line);
                    }
                    if reads.len() < limit {
                        reads.push(entry);
                    }
                }
            }
        }
    }

    // The qualifier now narrows for real (#458): a site is kept only when its
    // receiver resolves to the named type.
    let qualifier_applied = qualifier.is_some();
    let truncated = writes.len() < write_total || reads.len() < read_total;
    let mut payload = if writes_only {
        json!({
            "field": raw,
            "qualifier": qualifier,
            "qualifier_applied": qualifier_applied,
            "write_count": write_total,
            "write_returned": writes.len(),
            "write_lines": write_lines,
            "truncated": truncated,
            "write_sites": writes,
        })
    } else {
        json!({
            "field": raw,
            "qualifier": qualifier,
            "qualifier_applied": qualifier_applied,
            "write_count": write_total,
            "write_returned": writes.len(),
            "write_lines": write_lines,
            "read_count": read_total,
            "read_returned": reads.len(),
            "read_lines": read_lines,
            "truncated": truncated,
            "write_sites": writes,
            "read_sites": reads,
        })
    };
    if let Some(map) = payload.as_object_mut() {
        if let Some(q) = qualifier.as_deref() {
            map.insert("excluded_count".to_string(), json!(excluded));
            map.insert("unattributed_count".to_string(), json!(unattributed));
            map.insert(
                "qualifier_note".to_string(),
                json!(format!(
                    "Narrowed to '{q}': {excluded} site(s) resolved to a different type and \
                     were dropped. {unattributed} site(s) could not be attributed to any \
                     type and are NOT included — a receiver is typed only from an explicit \
                     declaration, a self/receiver binding, or a declared field type, so a \
                     value returned by a call or read out of a container stays unknown. \
                     Treat these counts as a lower bound; query the bare name \
                     '{field_name}' for the unnarrowed answer."
                )),
            );
        }
        if truncated {
            map.insert(
                "truncation_note".to_string(),
                json!(format!(
                    "Site lists were capped at limit={limit}; write_count/read_count \
                     are the true totals over the whole scan, write_returned/\
                     read_returned are how many are listed here."
                )),
            );
        }
    }
    let formatted = serde_json::to_string_pretty(&payload).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files: touched,
    })
}

#[derive(Debug, Clone, Copy)]
enum FieldRefKind {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy)]
struct FieldSite {
    byte: usize,
    line: u32,
    kind: FieldRefKind,
}
fn find_field_references(source: &str, field: &str) -> Vec<FieldSite> {
    let bytes = source.as_bytes();
    let needle = format!(".{field}");
    let mut out: Vec<FieldSite> = Vec::new();
    let mut byte = 0usize;
    while let Some(rel) = source[byte..].find(&needle) {
        let dot = byte + rel;
        let name_start = dot + 1;
        let name_end = name_start + field.len();
        let right_ok = match bytes.get(name_end) {
            None => true,
            Some(b) => !matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'),
        };
        if !right_ok {
            byte = name_end;
            continue;
        }
        if line_is_comment(source, dot) {
            byte = name_end;
            continue;
        }

        let line = source[..dot].bytes().filter(|c| *c == b'\n').count() as u32 + 1;
        let kind = classify_field_reference(source, name_end);
        out.push(FieldSite {
            byte: name_end,
            line,
            kind,
        });
        byte = name_end;
    }
    out
}

fn classify_field_reference(source: &str, after_name: usize) -> FieldRefKind {
    let bytes = source.as_bytes();
    let mut probe = after_name;
    while let Some(b) = bytes.get(probe) {
        if *b == b' ' || *b == b'\t' {
            probe += 1;
        } else {
            break;
        }
    }

    if let Some(b'\n') = bytes.get(probe).copied() {
        probe += 1;
        while let Some(b) = bytes.get(probe) {
            if *b == b' ' || *b == b'\t' {
                probe += 1;
            } else {
                break;
            }
        }
    }

    let next = bytes.get(probe).copied();
    let next2 = bytes.get(probe + 1).copied();
    match (next, next2) {
        (Some(b'='), Some(b'=' | b'>')) => FieldRefKind::Read,
        (Some(b'='), _) => FieldRefKind::Write,
        (Some(b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^'), Some(b'=')) => {
            FieldRefKind::Write
        }
        (Some(b'<'), Some(b'<')) | (Some(b'>'), Some(b'>')) => {
            if bytes.get(probe + 2).copied() == Some(b'=') {
                FieldRefKind::Write
            } else {
                FieldRefKind::Read
            }
        }
        _ => {
            if has_mut_borrow_prefix(source, after_name.saturating_sub(1)) {
                FieldRefKind::Write
            } else {
                FieldRefKind::Read
            }
        }
    }
}

fn has_mut_borrow_prefix(source: &str, idx: usize) -> bool {
    let bytes = source.as_bytes();
    let mut probe = idx;
    while probe > 0
        && matches!(
            bytes[probe],
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b':' | b'?'
        )
    {
        probe -= 1;
    }
    while probe > 0 && bytes[probe].is_ascii_whitespace() {
        probe -= 1;
    }
    if probe < 4 {
        return false;
    }
    let window = &source[probe.saturating_sub(4)..=probe];
    window.ends_with("&mut")
}

fn line_at(source: &str, byte: usize) -> Option<&str> {
    let line_start = source[..byte].rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[byte..].find('\n').map_or(source.len(), |i| byte + i);
    source.get(line_start..line_end)
}

fn line_is_comment(source: &str, byte: usize) -> bool {
    let line_start = source[..byte].rfind('\n').map_or(0, |i| i + 1);
    let line = &source[line_start..];
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{go_import_identifier, python_import_identifier, ts_import_identifiers};

    #[test]
    fn ts_import_identifiers_named_and_aliased() {
        // #207
        let ids = ts_import_identifiers(
            "import { render, screen as scr } from \"@testing-library/react\";",
        );
        assert_eq!(ids, vec!["render", "scr"]);
    }

    #[test]
    fn ts_import_identifiers_default_namespace_and_type() {
        let ids = ts_import_identifiers("import React, { type FC } from 'react';");
        assert_eq!(ids, vec!["React", "FC"]);
        let ids = ts_import_identifiers("import * as path from 'node:path';");
        assert_eq!(ids, vec!["path"]);
        let ids = ts_import_identifiers("import type { Config } from './config';");
        assert_eq!(ids, vec!["Config"]);
    }

    #[test]
    fn ts_import_identifiers_side_effect_import_is_empty() {
        assert!(ts_import_identifiers("import './styles.css';").is_empty());
    }

    #[test]
    fn go_import_identifier_plain_last_segment() {
        assert_eq!(go_import_identifier("net/url").as_deref(), Some("url"));
    }

    #[test]
    fn go_import_identifier_versioned_uses_preceding_segment() {
        // #149 Bug 2: a bare `/vN` import must derive the package name, not `vN`.
        assert_eq!(
            go_import_identifier("github.com/golang-jwt/jwt/v5").as_deref(),
            Some("jwt")
        );
        assert_eq!(
            go_import_identifier("github.com/jackc/pgx/v5").as_deref(),
            Some("pgx")
        );
    }

    #[test]
    fn go_import_identifier_alias_overrides_version_logic() {
        assert_eq!(
            go_import_identifier("github.com/jackc/pgx/v5 as pgxv5").as_deref(),
            Some("pgxv5")
        );
    }

    #[test]
    fn go_import_identifier_non_version_v_segment() {
        // Only `^v\d+$` triggers; a name merely starting with `v` is unchanged.
        assert_eq!(
            go_import_identifier("example.com/m/internal/foo/revision").as_deref(),
            Some("revision")
        );
    }

    #[test]
    fn python_import_identifier_from_import_last_segment() {
        // #224: `from enum import StrEnum` binds `StrEnum`, not `enum.StrEnum`.
        assert_eq!(
            python_import_identifier("enum.StrEnum", "from enum import StrEnum").as_deref(),
            Some("StrEnum")
        );
    }

    #[test]
    fn python_import_identifier_plain_import_first_segment() {
        // `import os.path` binds `os` into scope, not `os.path`.
        assert_eq!(
            python_import_identifier("os.path", "import os.path").as_deref(),
            Some("os")
        );
    }

    #[test]
    fn python_import_identifier_plain_import_aliased() {
        assert_eq!(
            python_import_identifier("numpy", "import numpy as np").as_deref(),
            Some("np")
        );
    }

    #[test]
    fn python_import_identifier_from_import_aliased() {
        assert_eq!(
            python_import_identifier("a.b", "from a import b as c").as_deref(),
            Some("c")
        );
    }

    #[test]
    fn python_import_identifier_dotted_import_aliased() {
        assert_eq!(
            python_import_identifier("a.b.c", "import a.b.c as x").as_deref(),
            Some("x")
        );
    }

    #[test]
    fn python_import_identifier_multi_name_statement_isolates_own_alias() {
        // A single `import a, b as c` statement creates one Use node per
        // name, all sharing the same `signature` (the whole statement's
        // text) — `a`'s Use node must not pick up `b`'s alias.
        let sig = "import a, b as c";
        assert_eq!(
            python_import_identifier("a", sig).as_deref(),
            Some("a"),
            "a has no alias of its own and must not inherit b's"
        );
        assert_eq!(python_import_identifier("b", sig).as_deref(), Some("c"));
    }

    #[test]
    fn python_import_identifier_wildcard_is_none() {
        assert_eq!(python_import_identifier("pkg.*", "from pkg import *"), None);
    }

    #[test]
    fn python_import_identifier_module_name_alias_collision() {
        // Regression: the module and the imported name are the same word,
        // so searching the whole signature for "datetime" finds the
        // *module* occurrence first (in "from datetime") whose trailing
        // text is "import datetime as dt", not an `as` clause — silently
        // dropping the alias and reporting `datetime.datetime` unused
        // despite `dt.now()` being called.
        assert_eq!(
            python_import_identifier("datetime.datetime", "from datetime import datetime as dt")
                .as_deref(),
            Some("dt")
        );
    }

    #[test]
    fn python_import_identifier_module_name_collision_no_alias() {
        // Same collision, no alias: must still resolve to the imported
        // name, not get confused by the earlier module-side occurrence.
        assert_eq!(
            python_import_identifier("datetime.datetime", "from datetime import datetime")
                .as_deref(),
            Some("datetime")
        );
    }
}
