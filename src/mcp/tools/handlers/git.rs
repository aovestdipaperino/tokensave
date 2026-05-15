//! Git/branch/diff tool handlers: `diff_context`, `commit_context`, `pr_context`,
//! `changelog`, `branch_list`, `branch_search`, `branch_diff`, `affected`, and
//! git helper functions.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use super::super::ToolResult;
use super::{truncate_response, unique_file_paths};
use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

/// Handles `tokensave_affected` tool calls.
pub(super) async fn handle_affected(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect()
        })
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: files (array of strings)".to_string(),
        })?;

    let max_depth = args
        .get("depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(5, |v| v.min(10) as usize);

    let custom_filter = args.get("filter").and_then(|v| v.as_str());
    let custom_glob = custom_filter.and_then(|p| glob::Pattern::new(p).ok());

    // Pre-compute files with inline test modules for test detection.
    let files_with_inline_tests = cg
        .get_files_with_test_annotations()
        .await
        .unwrap_or_default();
    let matches_test = |path: &str| -> bool {
        if let Some(ref g) = custom_glob {
            g.matches(path)
        } else {
            crate::tokensave::is_test_file(path) || files_with_inline_tests.contains(path)
        }
    };

    let mut affected: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();

    for file in &files {
        if matches_test(file) {
            affected.insert(file.clone());
        }
        if visited.insert(file.clone()) {
            queue.push_back((file.clone(), 0));
        }
    }

    while let Some((file, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let dependents = cg.get_file_dependents(&file).await?;
        for dep in dependents {
            if !visited.insert(dep.clone()) {
                continue;
            }
            if matches_test(&dep) {
                affected.insert(dep.clone());
            } else {
                queue.push_back((dep, depth + 1));
            }
        }
    }

    let mut result: Vec<String> = affected.into_iter().collect();
    result.sort();

    let touched_files = result.clone();
    let output = json!({
        "changed_files": files,
        "affected_tests": result,
        "count": result.len(),
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_diff_context` tool calls.
pub(super) async fn handle_diff_context(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(
        args.is_object(),
        "handle_diff_context expects an object argument"
    );
    let files: Vec<String> = args
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect()
        })
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: files (array of strings)".to_string(),
        })?;

    let depth = args
        .get("depth")
        .and_then(serde_json::Value::as_u64)
        .map_or(2, |v| v.min(10) as usize);

    let mut modified_symbols: Vec<Value> = Vec::new();
    let mut modified_seen: HashSet<String> = HashSet::new();
    let mut impacted_symbols: Vec<Value> = Vec::new();
    let mut impacted_seen: HashSet<String> = HashSet::new();
    let mut affected_tests: HashSet<String> = HashSet::new();
    let mut all_touched_files: Vec<String> = Vec::new();
    // Callers can (and in the wild do) pass the same path twice — e.g. when
    // synthesising the list from a directory walk that double-counts symlinked
    // or canonicalised entries. Dedup early so downstream loops don't emit
    // the same node N times for the same path.
    let files: Vec<String> = {
        let mut seen: HashSet<String> = HashSet::new();
        files
            .into_iter()
            .filter(|f| seen.insert(f.clone()))
            .collect()
    };

    // Pre-compute files containing inline test modules.
    let files_with_inline_tests = cg
        .get_files_with_test_annotations()
        .await
        .unwrap_or_default();
    let has_tests =
        |path: &str| crate::tokensave::is_test_file(path) || files_with_inline_tests.contains(path);

    for file in &files {
        let nodes = cg.get_nodes_by_file(file).await?;
        for node in &nodes {
            all_touched_files.push(node.file_path.clone());
            // Dedup by node id: `get_nodes_by_file` can return the same node
            // twice if the index contains duplicates from re-extraction, and
            // even when it doesn't, callers may legitimately want one entry
            // per node — never one entry per (file, node) pair.
            if !modified_seen.insert(node.id.clone()) {
                continue;
            }
            modified_symbols.push(json!({
                "id": node.id,
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
            }));

            // Get impact radius for each modified symbol. The same downstream
            // node can be reached from several modified symbols (diamond
            // dependencies) and from itself — dedupe by node id so callers
            // don't see the same `id` listed N times consecutively.
            let impact = cg.get_impact_radius(&node.id, depth).await?;
            for impacted in &impact.nodes {
                if impacted.id == node.id {
                    continue;
                }
                if !impacted_seen.insert(impacted.id.clone()) {
                    continue;
                }
                impacted_symbols.push(json!({
                    "id": impacted.id,
                    "name": impacted.name,
                    "kind": impacted.kind.as_str(),
                    "file": impacted.file_path,
                    "line": impacted.start_line,
                }));
                if has_tests(&impacted.file_path) {
                    affected_tests.insert(impacted.file_path.clone());
                }
            }
        }
    }

    // Also run affected-tests BFS at file level
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
    for file in &files {
        if has_tests(file) {
            affected_tests.insert(file.clone());
        }
        if visited.insert(file.clone()) {
            queue.push_back((file.clone(), 0));
        }
    }
    while let Some((file, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        let dependents = cg.get_file_dependents(&file).await?;
        for dep in dependents {
            if !visited.insert(dep.clone()) {
                continue;
            }
            if has_tests(&dep) {
                affected_tests.insert(dep.clone());
            } else {
                queue.push_back((dep, d + 1));
            }
        }
    }

    let mut tests_sorted: Vec<String> = affected_tests.into_iter().collect();
    tests_sorted.sort();

    let touched_files = unique_file_paths(
        all_touched_files
            .iter()
            .map(std::string::String::as_str)
            .chain(files.iter().map(std::string::String::as_str)),
    );

    let output = json!({
        "changed_files": files,
        "modified_symbols": modified_symbols,
        "impacted_symbols_count": impacted_symbols.len(),
        "impacted_symbols": impacted_symbols,
        "affected_tests": tests_sorted,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Diff two git refs and return the list of changed file paths.
fn git_diff_files(
    project_root: &std::path::Path,
    from_ref: &str,
    to_ref: &str,
) -> std::result::Result<Vec<String>, String> {
    let repo = gix::open(project_root).map_err(|e| format!("failed to open git repo: {e}"))?;

    let from_tree = repo
        .rev_parse_single(from_ref)
        .map_err(|e| format!("cannot resolve '{from_ref}': {e}"))?
        .object()
        .map_err(|e| format!("cannot read object for '{from_ref}': {e}"))?
        .peel_to_tree()
        .map_err(|e| format!("cannot peel '{from_ref}' to tree: {e}"))?;

    let to_tree = repo
        .rev_parse_single(to_ref)
        .map_err(|e| format!("cannot resolve '{to_ref}': {e}"))?
        .object()
        .map_err(|e| format!("cannot read object for '{to_ref}': {e}"))?
        .peel_to_tree()
        .map_err(|e| format!("cannot peel '{to_ref}' to tree: {e}"))?;

    let mut changed = Vec::new();
    from_tree
        .changes()
        .map_err(|e| format!("diff init failed: {e}"))?
        .for_each_to_obtain_tree(&to_tree, |change| {
            use gix::object::tree::diff::Change;
            // `for_each_to_obtain_tree` walks one level at a time — if an
            // entire subtree was added, deleted, or moved, the entry's
            // `entry_mode` is a tree, not a blob. We only want file paths
            // downstream, so skip tree entries before pushing. The earlier
            // `is_dir()` fallback after-the-fact missed deletions, where the
            // path no longer exists on disk.
            match &change {
                Change::Addition {
                    location,
                    entry_mode,
                    ..
                }
                | Change::Modification {
                    location,
                    entry_mode,
                    ..
                }
                | Change::Deletion {
                    location,
                    entry_mode,
                    ..
                } => {
                    if !entry_mode.is_tree() {
                        changed.push(location.to_string());
                    }
                }
                Change::Rewrite {
                    source_location,
                    location,
                    source_entry_mode,
                    entry_mode,
                    ..
                } => {
                    if !source_entry_mode.is_tree() {
                        changed.push(source_location.to_string());
                    }
                    if !entry_mode.is_tree() {
                        changed.push(location.to_string());
                    }
                }
            }
            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
        })
        .map_err(|e| format!("tree diff failed: {e}"))?;

    // Belt-and-suspenders: even with the entry_mode check above, drop any
    // path that resolves to a directory on disk for additions/modifications.
    // Pure deletions can't be checked this way (the path is gone), which is
    // exactly why entry_mode.is_tree() above is the load-bearing filter.
    changed.retain(|p| !project_root.join(p).is_dir());
    Ok(changed)
}

/// Returns file paths changed in the working tree (unstaged + staged, or staged-only).
fn git_changed_files(
    project_root: &std::path::Path,
    staged_only: bool,
) -> std::result::Result<Vec<String>, String> {
    let repo = gix::open(project_root).map_err(|e| format!("failed to open git repo: {e}"))?;

    let head_tree = repo
        .head()
        .map_err(|e| format!("cannot read HEAD: {e}"))?
        .peel_to_commit()
        .map_err(|e| format!("cannot peel HEAD to commit: {e}"))?
        .tree()
        .map_err(|e| format!("cannot read HEAD tree: {e}"))?;

    // Compare HEAD tree against the index (staged changes)
    let index = repo
        .index()
        .map_err(|e| format!("cannot read index: {e}"))?;

    let mut changed = HashSet::new();

    // Walk the index to find files that differ from HEAD
    for entry in index.entries() {
        let path = entry.path(&index);
        let path_str = String::from_utf8_lossy(path.as_ref()).to_string();
        if path_str.is_empty() {
            continue;
        }

        // Check if file exists in HEAD tree
        let head_entry = head_tree
            .lookup_entry_by_path(std::path::Path::new(&path_str))
            .ok()
            .flatten();

        match head_entry {
            Some(he) => {
                // File exists in both - check if content differs
                if he.object_id() != entry.id {
                    changed.insert(path_str);
                }
            }
            None => {
                // New file (in index but not in HEAD)
                changed.insert(path_str);
            }
        }
    }

    // If not staged_only, also check working-tree modifications via mtime
    if !staged_only {
        for entry in index.entries() {
            let path = entry.path(&index);
            let path_str = String::from_utf8_lossy(path.as_ref()).to_string();
            if path_str.is_empty() {
                continue;
            }
            let full_path = project_root.join(&path_str);
            if let Ok(meta) = std::fs::metadata(&full_path) {
                use std::time::UNIX_EPOCH;
                let mtime = meta
                    .modified()
                    .unwrap_or(UNIX_EPOCH)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as u32;
                // gix index entry stores mtime; if disk mtime is newer, file is modified
                if mtime > entry.stat.mtime.secs {
                    changed.insert(path_str);
                }
            }
        }
    }

    let mut result: Vec<String> = changed.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Returns the last N commit subjects from HEAD.
fn git_recent_commits(
    project_root: &std::path::Path,
    count: usize,
) -> std::result::Result<Vec<String>, String> {
    let repo = gix::open(project_root).map_err(|e| format!("failed to open git repo: {e}"))?;

    let mut commits = Vec::new();
    let head = repo
        .head()
        .map_err(|e| format!("cannot read HEAD: {e}"))?
        .into_peeled_id()
        .map_err(|e| format!("cannot peel HEAD: {e}"))?;

    let mut current_id = head.detach();

    for _ in 0..count {
        let commit = repo
            .find_object(current_id)
            .map_err(|e| format!("cannot find object: {e}"))?
            .try_into_commit()
            .map_err(|e| format!("not a commit: {e}"))?;

        let message = commit
            .message_raw()
            .map_err(|e| format!("cannot read commit message: {e}"))?;
        let subject = String::from_utf8_lossy(message.as_ref())
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        commits.push(subject);

        let parent_id = commit.parent_ids().next().map(gix::Id::detach);
        match parent_id {
            Some(pid) => current_id = pid,
            None => break,
        }
    }

    Ok(commits)
}

/// Returns commit subjects between two refs.
fn git_commit_log(
    project_root: &std::path::Path,
    base_ref: &str,
    head_ref: &str,
) -> std::result::Result<Vec<Value>, String> {
    let repo = gix::open(project_root).map_err(|e| format!("failed to open git repo: {e}"))?;

    let base_id = repo
        .rev_parse_single(base_ref)
        .map_err(|e| format!("cannot resolve '{base_ref}': {e}"))?
        .detach();

    let head_id = repo
        .rev_parse_single(head_ref)
        .map_err(|e| format!("cannot resolve '{head_ref}': {e}"))?
        .detach();

    let mut commits = Vec::new();
    let mut current_id = head_id;

    // Walk back from head until we hit base (max 100 commits)
    for _ in 0..100 {
        if current_id == base_id {
            break;
        }
        let commit = repo
            .find_object(current_id)
            .map_err(|e| format!("cannot find object: {e}"))?
            .try_into_commit()
            .map_err(|e| format!("not a commit: {e}"))?;

        let message = commit
            .message_raw()
            .map_err(|e| format!("cannot read message: {e}"))?;
        let subject = String::from_utf8_lossy(message.as_ref())
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let short_id = format!("{:.7}", commit.id);
        commits.push(json!({"hash": short_id, "subject": subject}));

        let parent_id = commit.parent_ids().next().map(gix::Id::detach);
        match parent_id {
            Some(pid) => current_id = pid,
            None => break,
        }
    }

    Ok(commits)
}

/// Classify a file path into a semantic role.
///
/// Inline tests inside source files don't make the file's role "test" —
/// that bucket is reserved for files that exist purely to host tests
/// (the path-based check). A `src/foo.rs` with a `#[cfg(test)] mod tests`
/// at the bottom still has role "source".
#[allow(clippy::ptr_arg)]
fn classify_file_role(path: &str, _files_with_inline_tests: &HashSet<String>) -> &'static str {
    if crate::tokensave::is_test_file(path) {
        return "test";
    }
    let lower = path.to_lowercase();
    let ext = std::path::Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str());
    // Config files
    if matches!(
        ext,
        Some("toml" | "yaml" | "yml" | "json" | "lock" | "ini" | "cfg")
    ) || lower.contains("config")
    {
        return "config";
    }
    // Documentation
    if matches!(ext, Some("md" | "rst" | "txt"))
        || lower.starts_with("docs/")
        || lower.starts_with("doc/")
    {
        return "docs";
    }
    "source"
}

/// Handles `tokensave_changelog` tool calls.
pub(super) async fn handle_changelog(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    debug_assert!(
        args.is_object(),
        "handle_changelog expects an object argument"
    );
    let from_ref = args
        .get("from_ref")
        .and_then(|v| v.as_str())
        .ok_or_else(|| TokenSaveError::Config {
            message: "missing required parameter: from_ref".to_string(),
        })?;

    let to_ref =
        args.get("to_ref")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: to_ref".to_string(),
            })?;

    // Use gix to diff the two trees
    let changed_files: Vec<String> = match git_diff_files(cg.project_root(), from_ref, to_ref) {
        Ok(files) => files,
        Err(e) => {
            return Ok(ToolResult {
                value: json!({
                    "content": [{ "type": "text", "text": format!("git diff failed: {}", e) }]
                }),
                touched_files: vec![],
            });
        }
    };

    // For each changed file, get current symbols from the graph
    let mut added: Vec<Value> = Vec::new();
    let mut modified: Vec<Value> = Vec::new();
    let mut file_symbols: HashMap<String, Vec<Value>> = HashMap::new();

    for file in &changed_files {
        let nodes = cg.get_nodes_by_file(file).await?;
        let symbols: Vec<Value> = nodes
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

        if symbols.is_empty() {
            // File was likely removed or not indexed
            modified.push(json!({
                "file": file,
                "status": "removed_or_not_indexed",
            }));
        } else {
            for sym in &symbols {
                added.push(sym.clone());
            }
        }
        file_symbols.insert(file.clone(), symbols);
    }

    let touched_files: Vec<String> = changed_files.clone();

    let result = json!({
        "from_ref": from_ref,
        "to_ref": to_ref,
        "changed_file_count": changed_files.len(),
        "changed_files": changed_files,
        "symbols_in_changed_files": added,
        "files_not_indexed": modified,
    });

    let formatted = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&formatted) }]
        }),
        touched_files,
    })
}

/// Handles `tokensave_commit_context` tool calls.
pub(super) async fn handle_commit_context(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let staged_only = args
        .get("staged_only")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let changed_files = match git_changed_files(cg.project_root(), staged_only) {
        Ok(files) => files,
        Err(e) => {
            return Ok(ToolResult {
                value: json!({"content": [{"type": "text", "text": format!("git error: {}", e)}]}),
                touched_files: vec![],
            });
        }
    };

    if changed_files.is_empty() {
        return Ok(ToolResult {
            value: json!({"content": [{"type": "text", "text": "No changes detected."}]}),
            touched_files: vec![],
        });
    }

    // Pre-compute files with inline test modules.
    let files_with_inline_tests = cg
        .get_files_with_test_annotations()
        .await
        .unwrap_or_default();

    let mut file_roles: Vec<Value> = Vec::new();
    let mut symbols_by_role: HashMap<&str, Vec<Value>> = HashMap::new();

    for file in &changed_files {
        let role = classify_file_role(file, &files_with_inline_tests);
        let nodes = cg.get_nodes_by_file(file).await.unwrap_or_default();
        file_roles.push(json!({"file": file, "role": role, "symbols": nodes.len()}));

        // Config files (Cargo.toml, *.yaml, package.json, ...) explode into
        // one node per key. Surface a single summary entry per file instead
        // — agents only need to know "Cargo.toml changed, N keys touched",
        // not the name of every dependency listed.
        if role == "config" {
            symbols_by_role.entry(role).or_default().push(json!({
                "file": file,
                "kind": "config_summary",
                "config_keys": nodes.len(),
            }));
            continue;
        }
        for node in &nodes {
            symbols_by_role.entry(role).or_default().push(json!({
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
            }));
        }
    }

    let has_tests = file_roles.iter().any(|f| f["role"] == "test");
    let has_source = file_roles.iter().any(|f| f["role"] == "source");
    let category = match (has_source, has_tests) {
        (true, true) => "feature/fix (source + tests)",
        (true, false) => "feature/fix/refactor",
        (false, true) => "test",
        (false, false) => "chore/docs/config",
    };

    let recent_commits = git_recent_commits(cg.project_root(), 5).unwrap_or_default();

    let total_symbols: usize = symbols_by_role.values().map(std::vec::Vec::len).sum();
    let output = json!({
        "changed_files": file_roles,
        "symbols_by_role": symbols_by_role,
        "suggested_category": category,
        "recent_commits": recent_commits,
        "summary": format!("{} file(s) changed, {} symbol(s) affected", changed_files.len(), total_symbols),
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: changed_files,
    })
}

/// Handles `tokensave_pr_context` tool calls.
pub(super) async fn handle_pr_context(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let base = args
        .get("base_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("main");
    let head = args
        .get("head_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("HEAD");

    let changed_files = match git_diff_files(cg.project_root(), base, head) {
        Ok(files) => files,
        Err(e) => {
            return Ok(ToolResult {
                value: json!({"content": [{"type": "text", "text": format!("git error: {}", e)}]}),
                touched_files: vec![],
            });
        }
    };

    let commits = git_commit_log(cg.project_root(), base, head).unwrap_or_default();

    let mut symbols_added: Vec<Value> = Vec::new();
    let mut symbols_modified: Vec<Value> = Vec::new();
    let mut test_files_changed: Vec<String> = Vec::new();
    let mut impacted_modules: HashSet<String> = HashSet::new();

    // Pre-compute files with inline test modules.
    let files_with_inline_tests = cg
        .get_files_with_test_annotations()
        .await
        .unwrap_or_default();
    let has_tests =
        |path: &str| crate::tokensave::is_test_file(path) || files_with_inline_tests.contains(path);

    for file in &changed_files {
        if has_tests(file) {
            test_files_changed.push(file.clone());
        }

        let nodes = cg.get_nodes_by_file(file).await.unwrap_or_default();

        // Config files explode into one node per key — Cargo.toml with 50
        // dependencies blows past the response budget. Treat them as a
        // single summary symbol attributed to `symbols_modified` (they're
        // never "added" since the file pre-exists in a typical PR).
        if classify_file_role(file, &files_with_inline_tests) == "config" {
            symbols_modified.push(json!({
                "file": file,
                "kind": "config_summary",
                "config_keys": nodes.len(),
            }));
            continue;
        }

        for node in &nodes {
            let sym = json!({
                "name": node.name,
                "kind": node.kind.as_str(),
                "file": node.file_path,
                "line": node.start_line,
            });

            // Check if this symbol has callers outside changed files — if so, it's
            // a modification to an existing API. Otherwise it's likely new.
            let callers = cg.get_callers(&node.id, 1).await.unwrap_or_default();
            let has_external_callers = callers
                .iter()
                .any(|(c, _)| !changed_files.contains(&c.file_path));

            if has_external_callers {
                symbols_modified.push(sym);
                // Track impacted modules
                for (caller, _) in &callers {
                    if !changed_files.contains(&caller.file_path) {
                        #[allow(clippy::map_unwrap_or)]
                        let dir = caller
                            .file_path
                            .rfind('/')
                            .map(|i| &caller.file_path[..i])
                            .unwrap_or(&caller.file_path);
                        impacted_modules.insert(dir.to_string());
                    }
                }
            } else {
                symbols_added.push(sym);
            }
        }
    }

    // Find transitively affected test files
    let mut affected_tests: HashSet<String> = HashSet::new();
    for file in &changed_files {
        if has_tests(file) {
            continue;
        }
        let nodes = cg.get_nodes_by_file(file).await.unwrap_or_default();
        for node in &nodes {
            let impact = cg.get_impact_radius(&node.id, 2).await.unwrap_or_default();
            for impacted in &impact.nodes {
                if has_tests(&impacted.file_path) {
                    affected_tests.insert(impacted.file_path.clone());
                }
            }
        }
    }

    let mut impacted_sorted: Vec<String> = impacted_modules.into_iter().collect();
    impacted_sorted.sort();
    let mut affected_sorted: Vec<String> = affected_tests.into_iter().collect();
    affected_sorted.sort();

    let output = json!({
        "base": base,
        "head": head,
        "commits": commits,
        "files_changed": changed_files.len(),
        "symbols_added": symbols_added.len(),
        "symbols_modified": symbols_modified.len(),
        "added": symbols_added,
        "modified": symbols_modified,
        "test_files_changed": test_files_changed,
        "affected_tests": affected_sorted,
        "impacted_modules": impacted_sorted,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    Ok(ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: changed_files,
    })
}

// ── Cross-branch tools ─────────────────────────────────────────────────

/// Handles `tokensave_branch_list` tool calls.
pub(super) fn handle_branch_list(cg: &TokenSave) -> ToolResult {
    let tokensave_dir = crate::config::get_tokensave_dir(cg.project_root());
    let current = cg.active_branch();

    let meta = crate::branch_meta::load_branch_meta(&tokensave_dir);
    let branches: Vec<Value> = match meta {
        Some(ref meta) => meta
            .branches
            .iter()
            .map(|(name, entry)| {
                let db_path = tokensave_dir.join(&entry.db_file);
                let size_bytes = db_path.metadata().map_or(0, |m| m.len());
                json!({
                    "name": name,
                    "parent": entry.parent,
                    "size_bytes": size_bytes,
                    "last_synced_at": entry.last_synced_at,
                    "is_current": current == Some(name.as_str()),
                    "is_default": Some(name.as_str()) == meta.default_branch.as_str().into(),
                })
            })
            .collect(),
        None => vec![],
    };

    let result = json!({
        "branch_count": branches.len(),
        "current_branch": current,
        "branches": branches,
    });

    let output = serde_json::to_string_pretty(&result).unwrap_or_default();
    ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files: vec![],
    }
}

/// Handles `tokensave_branch_search` tool calls.
pub(super) async fn handle_branch_search(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let branch =
        args.get("branch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: branch".to_string(),
            })?;
    let query =
        args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TokenSaveError::Config {
                message: "missing required parameter: query".to_string(),
            })?;
    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| v.min(500) as usize);

    let branch_cg = TokenSave::open_branch(cg.project_root(), branch).await?;
    let results = branch_cg.search(query, limit).await?;

    let items: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "id": r.node.id,
                "name": r.node.name,
                "kind": r.node.kind.as_str(),
                "file": r.node.file_path,
                "line": r.node.start_line,
                "signature": r.node.signature,
                "score": r.score,
                "branch": branch,
            })
        })
        .collect();

    let output = serde_json::to_string_pretty(&items).unwrap_or_default();
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files: vec![],
    })
}

/// Handles `tokensave_branch_diff` tool calls.
///
/// Compares code graphs between two branches. For each symbol present in
/// either branch, reports whether it was added, removed, or changed
/// (signature differs).
pub(super) async fn handle_branch_diff(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let project_root = cg.project_root();
    let tokensave_dir = crate::config::get_tokensave_dir(project_root);

    // Resolve base and head branches
    let meta = crate::branch_meta::load_branch_meta(&tokensave_dir).ok_or_else(|| {
        TokenSaveError::Config {
            message: "no branch tracking configured — run `tokensave branch add` first".to_string(),
        }
    })?;

    let base_name = args
        .get("base")
        .and_then(|v| v.as_str())
        .unwrap_or(&meta.default_branch);
    let head_name = args
        .get("head")
        .and_then(|v| v.as_str())
        .or_else(|| cg.active_branch())
        .ok_or_else(|| TokenSaveError::Config {
            message: "cannot determine head branch — specify it explicitly".to_string(),
        })?;

    if base_name == head_name {
        return Err(TokenSaveError::Config {
            message: format!("base and head are the same branch: '{base_name}'"),
        });
    }

    let file_filter = args.get("file").and_then(|v| v.as_str());
    let kind_filter = args.get("kind").and_then(|v| v.as_str());

    let base_cg = TokenSave::open_branch(project_root, base_name).await?;
    let head_cg = if cg.active_branch() == Some(head_name) && !cg.is_fallback() {
        None // use the already-open cg
    } else {
        Some(TokenSave::open_branch(project_root, head_name).await?)
    };
    let head_ref = head_cg.as_ref().unwrap_or(cg);

    // Collect nodes from both branches
    let base_files = base_cg.get_all_files().await?;
    let head_files = head_ref.get_all_files().await?;

    // Build file sets for filtering — only compare files present in either branch
    let base_file_set: HashSet<&str> = base_files.iter().map(|f| f.path.as_str()).collect();
    let head_file_set: HashSet<&str> = head_files.iter().map(|f| f.path.as_str()).collect();
    let all_files: HashSet<&str> = base_file_set.union(&head_file_set).copied().collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut touched = Vec::new();

    for file_path in &all_files {
        if let Some(filter) = file_filter {
            if !file_path.starts_with(filter) && *file_path != filter {
                continue;
            }
        }

        let base_nodes = base_cg
            .get_nodes_by_file(file_path)
            .await
            .unwrap_or_default();
        let head_nodes = head_ref
            .get_nodes_by_file(file_path)
            .await
            .unwrap_or_default();

        // Index by qualified_name for matching
        let base_map: HashMap<&str, &crate::types::Node> = base_nodes
            .iter()
            .map(|n| (n.qualified_name.as_str(), n))
            .collect();
        let head_map: HashMap<&str, &crate::types::Node> = head_nodes
            .iter()
            .map(|n| (n.qualified_name.as_str(), n))
            .collect();

        // Added: in head but not in base
        for (qn, node) in &head_map {
            if let Some(filter) = kind_filter {
                if node.kind.as_str() != filter {
                    continue;
                }
            }
            if !base_map.contains_key(qn) {
                added.push(json!({
                    "name": node.name,
                    "qualified_name": node.qualified_name,
                    "kind": node.kind.as_str(),
                    "file": node.file_path,
                    "line": node.start_line,
                    "signature": node.signature,
                }));
                touched.push(node.file_path.clone());
            }
        }

        // Removed: in base but not in head
        for (qn, node) in &base_map {
            if let Some(filter) = kind_filter {
                if node.kind.as_str() != filter {
                    continue;
                }
            }
            if !head_map.contains_key(qn) {
                removed.push(json!({
                    "name": node.name,
                    "qualified_name": node.qualified_name,
                    "kind": node.kind.as_str(),
                    "file": node.file_path,
                    "line": node.start_line,
                    "signature": node.signature,
                }));
                touched.push(node.file_path.clone());
            }
        }

        // Changed: in both but signature differs
        for (qn, head_node) in &head_map {
            if let Some(filter) = kind_filter {
                if head_node.kind.as_str() != filter {
                    continue;
                }
            }
            if let Some(base_node) = base_map.get(qn) {
                if base_node.signature != head_node.signature {
                    changed.push(json!({
                        "name": head_node.name,
                        "qualified_name": head_node.qualified_name,
                        "kind": head_node.kind.as_str(),
                        "file": head_node.file_path,
                        "line": head_node.start_line,
                        "base_signature": base_node.signature,
                        "head_signature": head_node.signature,
                    }));
                    touched.push(head_node.file_path.clone());
                }
            }
        }
    }

    let result = json!({
        "base": base_name,
        "head": head_name,
        "summary": {
            "added": added.len(),
            "removed": removed.len(),
            "changed": changed.len(),
        },
        "added": added,
        "removed": removed,
        "changed": changed,
    });

    let output = serde_json::to_string_pretty(&result).unwrap_or_default();
    let touched_files = unique_file_paths(touched.iter().map(std::string::String::as_str));
    Ok(ToolResult {
        value: json!({
            "content": [{ "type": "text", "text": truncate_response(&output) }]
        }),
        touched_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_files_classified_as_config_not_source() {
        let empty: HashSet<String> = HashSet::new();
        assert_eq!(classify_file_role("Cargo.toml", &empty), "config");
        assert_eq!(classify_file_role("package.json", &empty), "config");
        assert_eq!(classify_file_role("foo.yaml", &empty), "config");
        assert_eq!(classify_file_role("config.ini", &empty), "config");
    }

    /// Regression for bug #3 follow-up: a source file with `#[cfg(test)] mod
    /// tests` at the bottom is still a source file — its role must not flip
    /// to "test" just because it contains inline tests. Only the path-based
    /// `is_test_file` check governs role classification.
    #[test]
    fn source_file_with_inline_tests_keeps_source_role() {
        let mut with_inline: HashSet<String> = HashSet::new();
        with_inline.insert("src/lib.rs".to_string());
        assert_eq!(classify_file_role("src/lib.rs", &with_inline), "source");
    }

    #[test]
    fn path_based_test_files_classify_as_test() {
        let empty: HashSet<String> = HashSet::new();
        assert_eq!(classify_file_role("tests/integration.rs", &empty), "test");
        assert_eq!(classify_file_role("src/foo_test.rs", &empty), "test");
    }
}
